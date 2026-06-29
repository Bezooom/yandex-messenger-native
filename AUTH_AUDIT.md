# Аудит авторизации — Yandex Messenger (Rust/GTK4)

**Дата:** 2026-05-17

---

## КРИТИЧЕСКИЕ ПРОБЛЕМЫ (1-6)

---

### C1. OAuth implicit flow: token в URL fragment не попадает на callback-сервер

**Severity:** HIGH

**Файл:** `src/ui/auth_dialog.rs:577-624`

**Суть:** OAuth implicit flow (`response_type=token`) возвращает token в URL fragment (после `#`), который никогда не отправляется на сервер. Callback-сервер в `spawn_callback_listener()` может захватить только query-params (после `?`).

**Как это работает сейчас:**
1. OAuth возвращает URL вида: `http://127.0.0.1:PORT/callback#access_token=XXX&expires_in=3600`
2. Браузер НЕ отправляет fragment на сервер
3. Callback-сервер получает GET запрос на корень — fragment пуст
4. HTML-страница `capture_page_html()` читает `location.hash` и POST-ит его на `/token`

**Проблема:** Fragment-POST работает, но:
- Если Yandex OAuth не разрешает redirect URI `127.0.0.1` — redirect вообще не произойдёт
- Если redirect URI не совпадает с зарегистрированным — OAuth откажет
- `redirect_uri` в `config.rs:31` пустой — implicit flow без redirect_uri может быть отклонён

**Воспроизведение:**
- Зарегистрированный redirect URI: `http://127.0.0.1:8080/callback`
- Приложение запрашивает без redirect_uri: `?client_id=XXX&response_type=token`
- Yandex возвращает ошибку: `redirect_uri_mismatch`

---

### C2. Callback listener не валидирует OAuth state параметр

**Severity:** HIGH

**Файл:** `src/ui/auth_dialog.rs:577-624`, `src/ui/auth_dialog.rs:630-663`

**Суть:** При implicit flow state генерируется в `auth_code_url()` (строка 321), но нигде не проверяется при получении callback. Это позволяет CSRF-атаку: злоумышленник может отправить жертву на URL, который перенаправит на локальный callback-сервер с чужим token.

```rust
// auth_dialog.rs:321 — state генерируется
let state = uuid::Uuid::new_v4().to_string();

// auth_dialog.rs:328 — state передаётся в OAuth URL
format!("state={}", urlencoding::encode(&state)),

// Но нигде state НЕ проверяется при callback!
```

**Последствие:** CSRF-атака через OAuth redirect — злоумышленник может получить access token жертвы, если тот нажмёт на ссылку, ведущую на callback-сервер.

---

### C3. Конфликт между auth.rs и api/auth.rs — dead code и путаница

**Severity:** HIGH

**Файлы:** `src/auth.rs` ( AuthService, dead code), `src/api/auth.rs` ( AuthManager, active code)

**Суть:** В проекте два разных модуля авторизации:
1. `src/auth.rs` — старый `AuthService` с login/refresh через `/auth/login`, `/auth/refresh` — **НИГДЕ не используется**
2. `src/api/auth.rs` — новый `AuthManager` с OAuth flow — **активный код**

**Проблемы:**
1. `AuthService` — dead code, путает разработчиков
2. Разные форматы хранения токенов:
   - `AuthService`: `cache_dir()/yandex-messenger/auth_tokens.json` (dirs::cache_dir)
   - `AuthManager`: `config_dir()/yandex-messenger-native/token.json` (dirs::config_dir)
3. Разные структуры токенов:
   - `AuthService`: `TokenStore` с `expires_at` (unix timestamp)
   - `AuthManager`: `OAuthToken` с `received_at + expires_in` (relative expiry)
4. `AuthService` хранит refresh token как обязательный, но OAuth implicit flow может не возвращать refresh_token

---

### C4. is_expired() — инвертированная логика для legacy tokens

**Severity:** HIGH

**Файл:** `src/api/auth.rs:63-76`

```rust
pub fn is_expired(&self) -> bool {
    if self.received_at == 0 {
        // Legacy token without received_at — use simple heuristic
        self.expires_in <= 300  // <-- ОШИБКА: проверяет expires_in <= 300
    } else {
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let expiry = self.received_at + self.expires_in;
        (now as u64).saturating_sub(expiry) >= 300  // <-- ТАКЖЕ ОШИБКА
    }
}
```

**Проблема в ветке `else`:** `(now as u64).saturating_sub(expiry) >= 300` — если `now < expiry`, результат `saturating_sub` равен 0, и токен НЕ считается истёкшим, даже если он истёк.

**Пример:**
- `received_at = 100`, `expires_in = 3600`
- `expiry = 3700`
- Сейчас `now = 3800` (токен истёк 100 секунд назад)
- `saturating_sub(3800, 3700) = 100` → `100 >= 300` = `false` → токен НЕ считается истёкшим!

**Правильная логика:**
```rust
let now = ...;
let expiry = self.received_at + self.expires_in;
if now >= expiry {
    return true;  // expired
}
if now >= expiry - 300 {
    return true;  // about to expire (5 min buffer)
}
false
```

---

### C5. get_access_token() — блокировка mutex при проверке expired

**Severity:** HIGH

**Файл:** `src/api/auth.rs:836-845`

```rust
pub fn get_access_token(&self) -> Result<String, AuthError> {
    let token = self.token.try_lock().ok().ok_or(AuthError::NotFound)?;
    let t = token.as_ref().ok_or(AuthError::NotFound)?;

    if t.is_expired() {
        return Err(AuthError::Expired);  // lock held!
    }

    Ok(t.access_token.clone())
}
```

**Проблема:** Метод блокирует mutex и проверяет expired, но не пытается auto-refresh. При получении `AuthError::Expired` вызывающий код должен сначала вызвать `refresh_if_needed()`, а затем `get_access_token()` — это двухшаговый процесс.

**Последствие:** Все места, где вызывается `get_access_token()` и получает `AuthError::Expired`, должны явно вызывать `refresh_if_needed()`. Если этого не сделано — запрос провалится.

---

### C6. Token rotation: refresh_token может не обновляться

**Severity:** HIGH

**Файл:** `src/api/auth.rs:450-530`

**Суть:** При refresh OAuth токена, Yandex может вернуть НОВЫЙ refresh_token вместо старого. В коде refresh_token передаётся в ответ как тот же самый:

```rust
Ok(OAuthToken {
    access_token,
    refresh_token: Some(refresh_token.to_string()),  // <-- старый refresh_token
    ...
})
```

**Правильно:** Нужно проверить, пришёл ли новый refresh_token в ответе:
```rust
let new_refresh_token = json.get("refresh_token").and_then(|v| v.as_str()).map(|v| v.to_string());
let refresh_token = new_refresh_token.or(Some(refresh_token.to_string()));
```

Если старый refresh_token аннулирован (например, после single-session policy), новый не будет получен.

---

## СРЕДНИЕ ПРОБЛЕМЫ (7-14)

---

### M1. Xdg-open может открыть не тот браузер

**Severity:** MEDIUM

**Файл:** `src/ui/auth_dialog.rs:453-456`

```rust
let open_result = std::process::Command::new("xdg-open")
    .arg(&auth_url_owned)
    .spawn();
```

**Проблемы:**
1. `xdg-open` может не быть установлен (headless Linux, minimal environments)
2. `xdg-open` может открыть браузер в фоне, но не дождаться завершения
3. Нет fallback на `gio launch` или `env BROWSER=...`

**Рекомендация:** Добавить fallback:
```rust
let open_result = std::process::Command::new("xdg-open")
    .arg(&auth_url_owned)
    .spawn();

if open_result.is_err() {
    // Fallback: gio
    let _ = gio::AppInfo::launch_default_for_uri(&auth_url_owned, gio::AppLaunchContext::NONE);
}
```

---

### M2. Callback listener может захватить чужой callback

**Severity:** MEDIUM

**Файл:** `src/ui/auth_dialog.rs:577-624`

**Суть:** Callback listener захватывает любой `code`/`access_token` на любом пути. Нет валидации state:

```rust
if let Some((key, val)) = pair.split_once('=') {
    if (key == "code" || key == "access_token") && !val.is_empty() {
        captured = Some(val.to_string());
        break;
    }
}
```

**Последствие:** Если два OAuth flow запущены параллельно, второй может захватить callback первого.

---

### M3. Session cookies expire без автоматического refresh

**Severity:** MEDIUM

**Файл:** `src/api/mod.rs:479-487`

```rust
// Check if session is too old (older than 30 days)
if data.saved_at > 0 && now - data.saved_at > 30 * 24 * 3600 {
    log::warn!("Session cookies expired (older than 30 days), please re-login");
    return (None, None);
}
```

**Проблема:** При истечении session cookies — все методы, требующие session-based auth (например, `messages`), перестанут работать. Нет механизма автоматического refresh session cookies.

**Последствие:** Пользователь должен снова запустить `python3 scripts/login_browser.py` для получения новых cookies.

---

### M4. get_access_token() не auto-refresh

**Severity:** MEDIUM

**Файл:** `src/api/auth.rs:836-845`

Метод возвращает `AuthError::Expired` без попытки auto-refresh. В отличие от `refresh_if_needed()`, который делает auto-refresh.

**Последствие:** Все вызовы `get_access_token()` должны обрабатывать `AuthError::Expired` и вызывать `refresh_if_needed()` вручную.

---

### M5. Token file path inconsistency

**Severity:** MEDIUM

**Файл:** `src/api/auth.rs:97-117`, `src/api/auth.rs:269-282`

**Суть:** Есть несколько путей для токенов:
1. `token.json` — legacy, для single-account
2. `accounts/<id>/token.json` — для multi-account
3. `accounts.json` — список аккаунтов

**Проблема:** `user_id()` и `set_user_id()` всегда работают с `token.json`, а не с `token_file()`:
```rust
pub fn user_id(&self) -> Option<String> {
    let path = self.data_dir.join("token.json");  // <-- hardcoded
    ...
}
```

**Последствие:** После переключения на другой аккаунт, `user_id()` может вернуть user_id от старого токена.

---

### M6. auth_code_url() — response_type=token (implicit grant), не auth code

**Severity:** MEDIUM

**Файл:** `src/api/auth.rs:316-342`

```rust
pub fn auth_code_url(&self) -> String {
    // ...
    let mut params = vec![
        format!("response_type=token"),  // <-- implicit grant!
        ...
    ];
}
```

**Проблема:** Метод называется `auth_code_url()`, но использует implicit grant (`response_type=token`), а не authorization code flow. Это создаёт путаницу.

**Для implicit grant:**
- Redirect URI не обязателен (token в URL fragment)
- Но если redirect URI указан, он должен совпадать с зарегистрированным
- Token возвращается в fragment (не POST, не body)

**Для auth code flow:**
- redirect_uri обязателен
- `response_type=code`
- Token обменивается через `/token` endpoint

---

### M7. No token refresh on 401 responses

**Severity:** MEDIUM

**Файл:** `src/api/mod.rs` (все HTTP-методы)

**Суть:** Когда API возвращает 401 (token expired), нет автоматического retry с обновлённым токеном. Код должен:
1. Поймать 401
2. Вызвать `refresh_if_needed()`
3. Повторить запрос

**Последствие:** Запросы с истёкшим токеном провалятся без auto-retry.

---

## НИЗКИЕ ПРОБЛЕМЫ (15-21)

---

### L1. get_device_id() дублируется

**Severity:** LOW

**Файлы:** `src/ui/auth_dialog.rs:666-678`, `src/api/auth.rs:291-300`

```rust
// auth_dialog.rs — дубликат
fn get_device_id() -> String {
    let data_dir = dirs::config_dir()
        .map(|d| d.join("yandex-messenger-native"))
        .unwrap_or_else(|| dirs::config_dir().unwrap_or_default().join("yandex-messenger-native"));
    // ...
}

// api/auth.rs — то же самое
pub fn get_device_id(&self) -> String {
    let path = self.data_dir.join("device_id.txt");
    // ...
}
```

---

### L2. Session cookies — plaintext storage

**Severity:** LOW

**Файл:** `src/api/mod.rs:493-499`

Session cookies хранятся в plaintext в `~/.config/yandex-messenger-native/session.json`. Это можно улучшить шифрованием, но это low-priority.

---

### L3. No token validation on app startup

**Severity:** LOW

**Файл:** `src/api/auth.rs:775-794`

`is_authenticated()` проверяет только expiry, но не валидирует токен у сервера. Можно использовать `validate_session()`, но он не вызывается при старте.

---

### L4. Multi-account switch — race condition в sync/async mirror

**Severity:** LOW

**Файл:** `src/api/auth.rs:1028-1051`

```rust
pub async fn switch_account(&self, account_id: &str) -> Result<(), String> {
    let accounts = self.accounts.lock().await;  // async lock
    if let Some(account) = accounts.iter().find(|a| a.id == account_id).cloned() {
        drop(accounts);
        // sync mirror set
        self.set_current_account_sync(Some(account_id.to_string()));
        // ...
        *self.current_account_id.lock().await = Some(account_id.to_string());
        // ...
    }
}
```

**Проблема:** Между `drop(accounts)` и установкой sync mirror — window, где sync и async mirrors рассинхронизированы.

---

### L5. Legacy token.json при миграции — не обновляется

**Severity:** LOW

**Файл:** `src/api/auth.rs:213-239`

При миграции legacy `token.json` в `accounts/<id>/token.json` — старый файл не удаляется. Это может привести к путанице при чтении.

---

### L6. YANDEX_FORCE_AUTH — inconsistent string comparison

**Severity:** LOW

**Файл:** `src/api/auth.rs:776-782`

```rust
if std::env::var("YANDEX_FORCE_AUTH")
    .ok()
    .map(|v| v == "1" || v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("yes"))
    .unwrap_or(false)
{
    return false;
}
```

**Проблема:** `v == "1"` — case-sensitive, `v == "True"` не сработает. Но `v.eq_ignore_ascii_case("true")` — case-insensitive. Непоследовательно.

---

### L7. No error handling for callback listener thread death

**Severity:** LOW

**Файл:** `src/ui/auth_dialog.rs:573-624`

Если callback listener падает (например, port уже занят), `mpsc::Receiver` закроется. В main loop проверяется `TryRecvError::Disconnected`, но нет логики для restart listener.

---

## ИТОГОВАЯ ТАБЛИЦА

| # | Severity | Описание | Файл |
|---|----------|----------|------|
| C1 | HIGH | OAuth implicit flow: token fragment не попадает на callback | auth_dialog.rs:577-624 |
| C2 | HIGH | Нет валидации OAuth state в callback | auth_dialog.rs:577-624 |
| C3 | HIGH | Два разных auth-модуля (dead code + active code) | auth.rs vs api/auth.rs |
| C4 | HIGH | is_expired() — инвертированная логика | api/auth.rs:63-76 |
| C5 | HIGH | get_access_token() — блокировка mutex при expired check | api/auth.rs:836-845 |
| C6 | HIGH | Refresh token не обновляется при rotation | api/auth.rs:450-530 |
| M1 | MEDIUM | Xdg-open fallback на gio | auth_dialog.rs:453-456 |
| M2 | MEDIUM | Callback listener без state validation | auth_dialog.rs:577-624 |
| M3 | MEDIUM | Session cookies expire без refresh | api/mod.rs:479-487 |
| M4 | MEDIUM | get_access_token() не auto-refresh | api/auth.rs:836-845 |
| M5 | MEDIUM | Token file path inconsistency | api/auth.rs:97-117, 269-282 |
| M6 | MEDIUM | auth_code_url() — implicit grant вместо code flow | api/auth.rs:316-342 |
| M7 | MEDIUM | Нет retry на 401 | api/mod.rs |
| L1 | LOW | Дублирование get_device_id | auth_dialog.rs vs api/auth.rs |
| L2 | LOW | Session cookies plaintext | api/mod.rs |
| L3 | LOW | Нет валидации токена при старте | api/auth.rs:775-794 |
| L4 | LOW | Race condition sync/async mirror | api/auth.rs:1028-1051 |
| L5 | LOW | Legacy token.json не удаляется | api/auth.rs:213-239 |
| L6 | LOW | Inconsistent string comparison | api/auth.rs:776-782 |
| L7 | LOW | Нет restart callback listener | auth_dialog.rs:573-624 |
