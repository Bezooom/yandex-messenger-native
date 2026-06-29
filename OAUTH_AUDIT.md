# OAuth Flow Audit — Yandex Messenger (Rust GTK4)

## Critical Severity (Критические)

---

### CRIT-1: Yandex OAuth не поддерживает localhost/127.0.0.1 для OAuth-redirect

**Файл:** `src/ui/auth_dialog.rs`, строки 325–348, 563–636

**Проблема:** OAuth URL формируется с `response_type=token` (implicit grant), но Yandex **не поддерживает localhost в redirect_uri** для OAuth-приложений. Yandex требует, чтобы `redirect_uri` был HTTPS-адресом, зарегистрированным в OAuth-приложении.

```rust
// auth_dialog.rs:331-338
let mut params = vec![
    format!("response_type=token"),  // ← implicit flow
    format!("client_id={}", urlencoding::encode(&client_id)),
    format!("state={}", urlencoding::encode(&state)),
    ...
];
```

```rust
// auth_dialog.rs:563-636 — spawn_callback_listener
// Пытается запустить HTTP-сервер на 127.0.0.1:0 — это НЕ сработает
// для implicit flow, т.к. Yandex не отправит callback на localhost
```

**Почему это критично:** Implicit flow не будет получать callback вообще. Пользователь **всегда** будет вынужден вручную вставлять access_token. Функция `spawn_callback_listener` — мёртвый код, который никогда не сработает для implicit flow.

**Рекомендация:** Использовать `response_type=code` (authorization code flow with PKCE) для desktop-приложений. Код PKCE (`code_challenge` + `code_verifier`) компенсирует отсутствие клиентского секрета.

---

### CRIT-2: Race condition между sync и async путями при работе с токенами

**Файл:** `src/api/auth.rs`, строки 86–96 (AuthManager), 275–288 (token_file), 414–470 (save_token/load_token)

**Проблема:** `AuthManager` использует **два разных механизма** блокировки:
- Async: `tokio::sync::Mutex` для `token`, `accounts`, `current_account_id`
- Sync: `std::sync::Mutex` (alias `StdMutex`) для `current_account_id_sync`

```rust
// auth.rs:86-96
token: Arc<tokio::sync::Mutex<Option<OAuthToken>>>,           // async
current_account_id_sync: Arc<StdMutex<Option<String>>>,       // sync ← источник бага
```

```rust
// auth.rs:275-288 — token_file() использует только sync-версию!
fn token_file(&self) -> PathBuf {
    let id = self.current_account_id_sync.lock().ok().and_then(|g| g.clone());
    // ...
}
```

**Сценарий бага:**
1. `switch_account()` (async) устанавливает `current_account_id` через `tokio::sync::Mutex`
2. `token_file()` (может вызываться из sync-контекста, например `get_user_info()`) читает из `current_account_id_sync` — **получает старое значение**
3. Токен сохраняется в файл **не того аккаунта**

**Почему это критично:** При переключении аккаунтов токены **записываются в неверный файл**, что приводит к потере данных и авторизации под чужим аккаунтом.

**Рекомендация:** Либо использовать `tokio::sync::Mutex` везде, либо использовать `futures::executor::block_on` для блокировки async-мьютекса в sync-контексте.

---

### CRIT-3: AuthManager::new() вызывается без Arc в ChatView — изолированное состояние

**Файл:** `src/ui/chat_view.rs`, строки 830, 831, 915

**Проблема:** В `ChatView::render_messages()` и `ChatView::handle_bot_chat()` создаётся **новая** `AuthManager` без `Arc`, которая не имеет доступа к общему состоянию токенов:

```rust
// chat_view.rs:915
let current_user_id = AuthManager::new().ok().and_then(|a| a.user_id());
```

```rust
// chat_view.rs:830-831
let auth = AuthManager::new().ok().map(Arc::new);
let bot_p = BotPanel::new(auth.unwrap_or_else(|| Arc::new(AuthManager::new().unwrap())));
```

**Почему это критично:**
- `AuthManager::new()` загружает токены из `~/.config/yandex-messenger-native/` — но при переключении аккаунтов `current_account_id_sync` в этом новом экземпляре **всегда указывает на первый аккаунт**
- `user_id()` в строке 915 всегда возвращает user_id **первого** аккаунта, а не текущего
- `BotPanel` получает AuthManager с **отдельным** состоянием токенов — операции с ботом используют wrong token

---

### CRIT-4: CSRF-защита в implicit flow — state генерируется, но не валидируется при захвате токена

**Файл:** `src/ui/auth_dialog.rs`, строки 327, 334, 538–551, 639–672

**Проблема:** `state` параметр генерируется в OAuth URL (строка 327), но при захвате токена через `spawn_callback_listener` и `extract_auth_code` он **никогда не валидируется**:

```rust
// auth_dialog.rs:327 — генерация state
let state = uuid::Uuid::new_v4().to_string();
```

```rust
// auth_dialog.rs:538-551 — захват из callback — state НЕ проверяется
match rx.try_recv() {
    Ok(code) => {
        // ← state не извлекается из callback URL
        // ← state не сравнивается с сохранённым
        *outcome.borrow_mut() = Some(Ok(extract_auth_code(&code)));
    }
}
```

```rust
// auth_dialog.rs:639-672 — extract_auth_code — вообще не знает про state
fn extract_auth_code(input: &str) -> String {
    // ← не проверяет state параметр
}
```

**Почему это критично:** Любая веб-страница может перенаправить пользователя на URL с `access_token=...` и перехватить токен без проверки state. Это **классический CSRF-уязвимость OAuth-flow**.

**Рекомендация:** Сохранять `state` в локальную переменную и валидировать его при перехвате токена (либо в callback, либо в `extract_auth_code`).

---

## High Severity (Высокий)

---

### HIGH-1: Deadlock-риск через glib::MainContext::iteration(true)

**Файл:** `src/ui/auth_dialog.rs`, строки 219–221, 309–311, 531–556

**Проблема:** Блокирующий цикл через `glib::MainContext::default().iteration(true)` может привести к deadlock при определённых условиях:

```rust
// auth_dialog.rs:309-311 — select_account
loop {
    glib::MainContext::default().iteration(true);
    if let Some(idx) = *selected_idx.borrow() { ... }
}
```

```rust
// auth_dialog.rs:531-556 — obtain_auth_code
loop {
    ctx.iteration(true);
    // ... poll callback_rx ...
    if outcome.borrow().is_some() { break; }
}
```

**Почему это высокий риск:**
- Если GTK событие (например, `close_request`) обрабатывается в том же MainContext и блокируется (например, через `RefCell` borrow), цикл может зависнуть
- При закрытии окна через `close_request` handler, который также пытается взять `RefCell`, может возникнуть borrow deadlock
- `glib::MainContext::default().iteration(true)` — это **blocking iteration**: если MainContext заблокирован, поток зависает

**Рекомендация:** Использовать `glib::MainContext::with_default` + `glib::timeout_add` для неблокирующего ожидания, или `glib::MainLoop` для ожидания событий.

---

### HIGH-2: Токен без refresh_token — приложение не может восстановить сессию

**Файл:** `src/api/auth.rs`, строки 78–92 (finalize_token), 63–78 (is_expired)

**Проблема:** При ручном вставке токена (строки 78–92) `refresh_token` устанавливается в `None`:

```rust
// auth_dialog.rs:78-92
let token = if looks_like_access_token {
    OAuthToken {
        access_token: code,
        refresh_token: None,  // ← ТОКЕН БЕЗ REFRESH!
        expires_in: 31_536_000,  // ← жёстко закодированный 1 год
        ...
    }
}
```

```rust
// auth.rs:876-900 — refresh_if_needed требует refresh_token
let refresh = token.refresh_token.ok_or_else(|| {
    "No refresh_token available; user must re-authenticate".to_string()
})?;
```

**Почему это высокий риск:** При истечении токена пользователь **обязан** заново проходить полную OAuth-аутентификацию. Нет механизма автоматического обновления — это ухудшает UX и делает приложение неработоспособным после истечения токена.

---

### HIGH-3: Непоследовательная обработка expires_in в implicit flow

**Файл:** `src/api/auth.rs`, строки 383–387 (parse_token_from_url)

**Проблема:** `expires_in` из OAuth-ответа implicit flow парсится как `u64`, но Yandex может вернуть его как строку или отсутствующим:

```rust
// auth.rs:383-387
let expires_in = params
    .iter()
    .find(|(k, _)| *k == "expires_in")
    .and_then(|(_, v)| v.parse().ok())
    .unwrap_or(3600);  // ← 1 час по умолчанию
```

**Почему это высокий риск:**
- Если `expires_in` отсутствует — используется 3600 секунд (1 час), но Yandex может вернуть токен на 1 год (31536000 сек)
- Если `expires_in` — строка (например `"3600"` с пробелами), парсинг `u64` может не сработать
- Токен может считаться истёкшим до того, как это реально произойдёт

---

### HIGH-4: Нет механизма обновления session cookies

**Файл:** `src/api/mod.rs`, строки 453–500 (load_session_cookies), 457 (session.json)

**Проблема:** Session cookies загружаются из `session.json` **один раз** при создании `HttpClient` (строка 433) и **никогда не обновляются**:

```rust
// mod.rs:433 — загружаются один раз при создании HttpClient
let (session_cookies, csrf_token) = Self::load_session_cookies();

// mod.rs:479-486 — проверка срока годности, но НЕ обновление
if data.saved_at > 0 && now - data.saved_at > 30 * 24 * 3600 {
    log::warn!("Session cookies expired (older than 30 days)");
    return (None, None);  // ← просто возвращаем None, ничего не делаем
}
```

**Почему это высокий риск:** При истечении session cookies (30 дней) все API-запросы, требующие сессии (`session_rpc_request`), **начинают падать с ошибкой**. Приложение не может восстановить сессию автоматически — пользователь не может даже отправить сообщение.

**Рекомендация:** Реализовать механизм обновления session cookies через:
1. Отслеживание HTTP-ответов на `Set-Cookie` и обновление файла
2. Или вызов API-метода для получения новых cookies (если Yandex поддерживает)
3. Или запрос на повторную авторизацию при истечении cookies

---

### HIGH-5: Асинхронный set_token vs sync-чтение — race condition

**Файл:** `src/api/auth.rs`, строки 464–470 (set_token), 427–441 (load_token), 444–461 (get_token)

**Проблема:** `set_token` — async (использует `tokio::sync::Mutex`), но `token_file()` — sync. Между сохранением токена и его чтением может произойти race:

```rust
// auth.rs:464-470
pub async fn set_token(&self, token: OAuthToken) -> Result<(), String> {
    let mut t = self.token.lock().await;  // ← async lock
    *t = Some(token.clone());
    drop(t);
    self.save_token(&token).await  // ← async write
}

// auth.rs:275-288 — sync-метод
fn token_file(&self) -> PathBuf {
    let id = self.current_account_id_sync.lock().ok().and_then(|g| g.clone());
    // ...
}
```

**Почему это высокий риск:** При переключении аккаунтов:
1. `set_token(new_token)` начинает запись токена
2. `switch_account()` читает `current_account_id_sync` — получает старое значение
3. Токен нового аккаунта записывается **в файл старого аккаунта**

---

## Medium Severity (Средний)

---

### MED-1: Небезопасная генерация device_id

**Файл:** `src/ui/auth_dialog.rs`, строки 675–687 (get_device_id)

**Проблема:** `get_device_id()` использует `uuid::Uuid::new_v4()` — это не криптографически безопасный генератор UUID:

```rust
// auth_dialog.rs:684-685
let new_id = uuid::Uuid::new_v4().to_string();
let _ = std::fs::write(&path, &new_id);
```

**Почему это средний риск:** UUID v4 генерируется из случайных байтов, но `uuid::Uuid::new_v4()` использует `rand::random()`, который может быть предсказуем на некоторых платформах. Если `rand` использует слабый PRNG, злоумышленник может предсказать device_id.

**Рекомендация:** Использовать `uuid::Uuid::new_v4()` с `rand::thread_rng()`, или `getrandom` crate для криптографически безопасной генерации.

---

### MED-2: Потенциальная утечка токена в логах

**Файл:** `src/ui/auth_dialog.rs`, строки 496–502, 539–543

**Проблема:** Токен логируется с его длиной в eprintln:

```rust
// auth_dialog.rs:496
eprintln!("[AUTH] Confirm clicked, text length={}", txt.len());
// ← txt может содержать токен, но логируется только длина — OK

// auth_dialog.rs:539-543
Ok(code) => {
    eprintln!("[AUTH] Got code from callback: length={}", code.len());
    // ← code.len() — OK, но eprintln может быть в продакшен-логах
}
```

**Почему это средний риск:** Хотя токены логируются только по длине, `eprintln` может попасть в:
- Логи-системы (systemd journal, syslog)
- Злоупотребление: если злоумышленник получает доступ к журналу, он может коррелировать длину токена с конкретным пользователем

**Рекомендация:** Использовать `log::debug!` вместо `eprintln!` и убедиться, что уровень логирования не позволяет видеть sensitive данные в продакшене.

---

### MED-3: Непоследовательная обработка error-ответов OAuth

**Файл:** `src/api/auth.rs`, строки 589–663 (exchange_code)

**Проблема:** При обмене кода на токен, ошибка `unauthorized_client` обрабатывается только в Basic auth-пути, но не в fallback-пути:

```rust
// auth.rs:619-623 — проверка error только для Basic auth
let has_auth_error = first_json
    .get("error")
    .and_then(|e| e.as_str())
    .map(|e| e == "invalid_client" || e == "unauthorized_client")
    .unwrap_or(false);
```

```rust
// auth.rs:642-662 — fallback path: если Basic auth failed,
// берётся ошибка из fallback-ответа — но это может быть ошибка Basic auth,
// а не ошибка OAuth обмена
```

**Почему это средний риск:** Если Basic auth fails с `unauthorized_client`, пользователь видит ошибку OAuth-обмена, а не ошибку авторизации клиента — это вводит в заблуждение.

---

### MED-4: Отсутствие проверки redirect_uri при обмене кода

**Файл:** `src/api/auth.rs`, строки 560–712 (exchange_code)

**Проблема:** При authorization code flow, `redirect_uri` передаётся в exchange-запрос, но если он не совпадает с тем, что был при запросе авторизации — Yandex вернёт `invalid_grant`. Код не проверяет это:

```rust
// auth.rs:575-577
if let Some(redirect_uri) = self.effective_redirect_uri() {
    params.push(("redirect_uri", redirect_uri));  // ← не проверяет совпадение
}
```

**Почему это средний риск:** Если `redirect_uri` изменился между авторизацией и обменом кода, пользователь получает непонятную ошибку.

---

### MED-5: Мёртвый код — session_auth_flow

**Файл:** `src/ui/auth_dialog.rs`, строки 126–234

**Проблема:** Метод `session_auth_flow` помечен как `#[allow(dead_code)]` — это означает, что он **никогда не вызывается** в коде:

```rust
// auth_dialog.rs:126-127
#[allow(dead_code)]
fn session_auth_flow(&self) -> Result<OAuthToken, String> {
```

**Почему это средний риск:** Мёртвый код — это технический долг. Если он будет вызван в будущем (например, из `AuthDialog`), он может содержать баги, которые никто не заметит.

---

### MED-6: Непоследовательная обработка expires_in для legacy-токенов

**Файл:** `src/api/auth.rs`, строки 63–78 (is_expired), 383–387 (parse_token_from_url)

**Проблема:** Legacy-токены (без `received_at`) обрабатываются через эвристику `expires_in <= 300`, но implicit flow может вернуть токен с `expires_in=31536000` (1 год), который будет считаться валидным даже после истечения:

```rust
// auth.rs:63-78
pub fn is_expired(&self) -> bool {
    if self.received_at == 0 {
        self.expires_in <= 300  // ← legacy-эвристика
    } else {
        let now = ...;
        let expiry = self.received_at + self.expires_in;
        now >= expiry - 300
    }
}
```

**Почему это средний риск:** Legacy-токены с `expires_in > 300` всегда считаются валидными. Если такой токен истёк на сервере, приложение будет продолжать его использовать.

---

### MED-7: Потенциальная утечка токена через URL-fragment в browser

**Файл:** `src/ui/auth_dialog.rs`, строки 692–725 (capture_page_html)

**Проблема:** HTML-страница для захвата implicit flow использует `fetch` для POST токена на `http://127.0.0.1:PORT/token` — это **незащищённый HTTP** запрос:

```html
<!-- auth_dialog.rs:714-724 -->
<script>
  (function(){
    var h = location.hash.replace(/^#/, '');
    var q = location.search.replace(/^\?/, '');
    var payload = h || q;
    if (!payload) return;
    fetch('/token', {method:'POST',  // ← HTTP POST на localhost
      headers:{'Content-Type':'application/x-www-form-urlencoded'},
      body: payload});  // ← access_token отправляется в теле
  })();
</script>
```

**Почему это средний риск:** Хотя localhost не подвержен сетевым атакам, злоумышленник с локальным доступом может перехватить токен через:
- Браузерный history (если open_redirect)
- Логи-файлы локального HTTP-сервера
- DevTools или browser extensions

---

## Low Severity (Низкий)

---

### LOW-1: Неинициализированный `is_valid` в Account

**Файл:** `src/models/account.rs`, строки 19, 31

**Проблема:** Поле `is_valid` в `Account` всегда `true` и никогда не обновляется:

```rust
// account.rs:19
pub is_valid: bool,

// account.rs:31
is_valid: true,
```

**Почему это низкий риск:** Поле `is_valid` не используется нигде в коде (проверено grep) — это просто мёртвое поле.

---

### LOW-2: Непоследовательная обработка user_id в implicit flow

**Файл:** `src/api/auth.rs`, строки 389–392 (parse_token_from_url)

**Проблема:** `user_id` из OAuth-ответа implicit flow берётся из фрагмента URL, но Yandex может вернуть его в другом параметре:

```rust
// auth.rs:389-392
let user_id = params
    .iter()
    .find(|(k, _)| *k == "user_id")  // ← только user_id
    .map(|(_, v)| v.to_string());
```

**Почему это низкий риск:** Yandex OAuth implicit flow обычно возвращает `user_id` в фрагменте, но если Yandex изменит формат ответа — поле будет пустым.

---

### LOW-3: Отсутствие rate-limit обработки при refresh

**Файл:** `src/api/auth.rs`, строки 473–557 (refresh_token)

**Проблема:** При refresh токена нет обработки rate-limit ответов (429 Too Many Requests):

```rust
// auth.rs:490-494
if let Some(err) = json.get("error").and_then(|v| v.as_str()) {
    let desc = json
        .get("error_description")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown error");
    return Err(format!("Refresh failed: {} ({})", err, desc));
}
```

**Почему это низкий риск:** Rate-limit для OAuth-refresh — редкий случай, но при частых запросах refresh-токен может быть заблокирован.

---

### LOW-4: Нет проверки HTTP-ответов от localhost-сервера

**Файл:** `src/ui/auth_dialog.rs`, строки 587–632 (callback listener)

**Проблема:** Локальный HTTP-сервер не обрабатывает ошибки:

```rust
// auth_dialog.rs:587-632
loop {
    let Ok(mut request) = server.recv() else { break };
    // ... обработка запроса
    // ← нет обработки ошибок чтения тела запроса
    // ← нет обработки ошибок записи ответа
}
```

**Почему это низкий риск:** Локальный сервер на `127.0.0.1:PORT` защищён от внешних угроз. Ошибки чтения/записи обрабатываются через `let Ok(mut request) = ...` — если `recv()` падает, цикл прерывается.

---

### LOW-5: Файлы токенов не шифруются

**Файл:** `src/api/auth.rs`, строки 415–441 (save_token, load_token)

**Проблема:** Токены хранятся в plaintext на диске:

```rust
// auth.rs:415-424
pub async fn save_token(&self, token: &OAuthToken) -> Result<(), String> {
    let token_json = serde_json::to_string_pretty(token)
        .map_err(|e| format!("Token serialize failed: {}", e))?;
    fs::write(&self.token_file(), token_json)  // ← plaintext
        .map_err(|e| format!("Token write failed: {}", e))?;
}
```

**Почему это низкий риск:** Токены хранятся в `~/.config/yandex-messenger-native/` — это стандартное поведение для десктоп-приложений (аналогично Electron-приложениям). Но при компрометации файла токены могут быть использованы.

---

## Summary

| Severity | Count |
|----------|-------|
| Critical | 4 |
| High | 5 |
| Medium | 7 |
| Low | 4 |
| **Total** | **20** |

### Top priorities for fixing:
1. **CRIT-1**: Мигрировать на authorization code flow с PKCE — implicit flow не работает с localhost
2. **CRIT-2**: Синхронизировать пути async/sync для `current_account_id`
3. **CRIT-3**: Передавать `Arc<AuthManager>` в `ChatView` вместо создания новых экземпляров
4. **CRIT-4**: Добавить валидацию `state` параметра при захвате токена
