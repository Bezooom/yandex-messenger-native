# Функциональный аудит (проверка «что реально работает»)

> ⚠️ **Исторический снимок от 2026-08-08 (дерево ~2.165).** Актуальный статус функций — в [README.ru.md](README.ru.md) и [CHANGELOG.ru.md](CHANGELOG.ru.md) для **2.173.0**. English: [FUNCTIONAL_AUDIT.en.md](FUNCTIONAL_AUDIT.en.md).

**Дата снимка:** 2026-08-08  
**Версия дерева на момент аудита:** 2.165.0 + uncommitted changes  
**Метод:** code review, `cargo check` / `cargo test`, сверка README/CHANGELOG с `src/`  
**Связано:** [`GAP_AUDIT.md`](GAP_AUDIT.md), [`ROADMAP_DETAILED.ru.md`](ROADMAP_DETAILED.ru.md)

---

## 0. Красный флаг (блокирует всё)

| Проверка | Результат |
|----------|-----------|
| `cargo check` (до фикса в этой сессии) | **FAIL** — `telemost_window.rs` (GTK3 API) |
| `cargo check` (после минимального фикса) | **OK** (warnings remain) |
| `cargo test` | **13 passed**, 0 failed, 1 ignored (`test_api_send`) |

### Ошибки, которые ломали сборку

```
error[E0432]: unresolved import `gtk::Container`
error[E0599]: no method named `children` found for struct `gtk4::Box`
```

**Причина:** WIP `telemost_window.rs` писал API как GTK3.  
**Сделано в аудите:** убран `Container`, children clear через `first_child`/`remove`, `hide()` → `set_visible(false)`.

⚠️ Telemost по-прежнему **не** настоящие звонки (placeholder UI, без WebRTC). Фикс только разблокировал build.

CHANGELOG.ru.md («0 warnings, 13 tests») на текущем дереве **не совпадает** с реальностью (есть warnings; число тестов уточнять `cargo test`).

---

## 1. Что изменилось относительно прошлого аудита

| Область | Было | Стало (код) | Оценка |
|---------|------|-------------|--------|
| Честность docs | ROADMAP врал | README matrix + STUB + feature flags | ✅ полезно |
| Voice/Telemost UI | Всегда видны stub | Скрыты без `YM_ENABLE_*` | ✅ |
| File attach | Callback только log | `send_file_message` → upload + session RPC | 🟡 wiring есть, E2E не проверен; **нужен session** |
| Telemost | URL-label окно | Новый client + window | 🔴 **ломает build**; WebRTC всё равно нет |
| Preview tests | не было | unit tests для preview/settings | ✅ но preview всё ещё EN |
| Notifications | stub | stub + `// STUB` | 🔴 без изменений |
| Tray | stub | stub + `// STUB` | 🔴 без изменений |
| Chat context menu | log-only | log-only + STUB comment | 🔴 без изменений |
| SQLite | no-op | no-op | 🔴 без изменений |

**Вывод:** основная работа была **честность + каркас Telemost/files**, а не закрытие P0 trust (notify/tray/actions). Сборка откатилась назад.

---

## 2. Матрица функций (as-is, после проверки кода)

Легенда:

| Символ | Значение |
|--------|----------|
| ✅ | E2E-путь в коде выглядит рабочим (при session/token) |
| 🟡 | Частично / UI есть / backend сомнительный / не проверено live |
| 🔴 | Stub / broken / не wired |
| 💥 | Ломает сборку |

### 2.1 Инфраструктура

| Функция | Статус | Доказательство | Комментарий |
|---------|--------|----------------|-------------|
| Сборка release/debug | 💥 | `cargo check` FAIL | telemost_window |
| Unit tests | 💥 | не запускаются | |
| OAuth login UI | 🟡 | `auth_dialog.rs` | WebView + callback; dogfood зависит от client_id |
| Token на диске | ✅ | `~/.config/.../token.json` есть | На этой машине |
| Session cookies | 🟡 | `session.json` есть | **Обязателен** для history/send/files; без script — деградация |
| Auth proxy | 🟡 | код есть | Не гонялся в этом аудите |
| Multi-account | 🟡 | model + dropdown | Switch есть; изоляция кэша ? |

### 2.2 Desktop

| Функция | Статус | Доказательство |
|---------|--------|----------------|
| System notifications | 🔴 | `notifications.rs` → `eprintln!` only |
| System tray | 🔴 | `TrayHandle::init` empty |
| minimize_to_tray setting | 🔴 | setting exists, tray no-op |
| Settings **window** | 🔴 | только `SettingsStore` JSON, UI Preferences нет |
| Dark theme CSS | 🟡 | `theme.css` большой, libadwaita | Визуально «ок», не YM-polish |

### 2.3 Чаты и список

| Функция | Статус | Доказательство |
|---------|--------|----------------|
| Load chat list | ✅/🟡 | `get_chat_list` + UI | При валидном токене |
| Sort pinned / time | 🟡 | chat_list | Код есть |
| Avatars cache | 🟡 | AVATAR_CACHE | |
| Search chats local | 🟡 | search entry | |
| Preview text | 🟡 | `Message::preview` | EN: `No messages`, `[Image]`…; chat bubble fallback всё ещё «Вложение или системное…» |
| Unread badge | 🟡 | WS `unread_update` | mark_read API **нет** |
| Mute / pin / archive / delete chat | 🔴 | context menu → `log::info` + STUB | |
| Mark as read on open | 🔴 | не найдено API call | |

### 2.4 Messaging

| Функция | Статус | Доказательство |
|---------|--------|----------------|
| Send text (WS binary) | 🟡 | `ws.send_text_message` | Hardcoded yuid fallback; optimistic msg без server ack |
| Send text (session RPC alt) | 🟡 | `http.send_message` session | Используется для files; text path через WS |
| Receive realtime | 🟡 | WS push + `new_message` | Depends session cookies for connect |
| History load | 🟡 | session → search → cache | Без session — incomplete |
| L2 JSON cache | 🟡 | `save_cache_l2` / load | Работает как файлы |
| SQLite cache | 🔴 | `db.rs` returns `[]` / no-op upsert | |
| Reply UI | 🟡 | reply bar + id stored | **id не передаётся в on_send** → reply **не уходит на сервер** |
| Edit UI | 🟡 | edit id stored | **то же**: только log, callback без edit API |
| Delete + undo UI | 🟡 | undo bar | Нужна проверка server delete |
| Forward | 🟡 | menu action | picker/API ? |
| Pin message | 🟡 | pin bar UI | API E2E ? |
| Delivery/read ticks | 🟡 | UI ✓✓ | Не подтверждено server-driven update |
| Outbox / retry | 🔴 | нет | |
| Drafts | 🔴 | нет | |
| Pagination older msgs | 🔴 | limit ~50, scroll-up load не найден | |
| Typing indicator | 🟡 | WS `typing_enhanced` | |
| Online status | 🟡 | set_online API в UI | |

**Критичный баг UX/API (reply/edit):**

```885:908:src/ui/chat_view.rs
    fn handle_send(&self) {
        ...
            let reply_id = self.reply_to_msg_id.lock().unwrap().clone();
            let edit_id = self.edit_msg_id.lock().unwrap().clone();
            // только log...
            if let Some(cb) = self.on_send.lock().unwrap().as_ref() {
                cb(chat.id.clone(), text.clone()); // ← reply/edit id НЕ передаются
            }
```

`bind_callbacks` = `Fn(String, String)` — нет канала для reply/edit.  
`core.send_text_message` всегда `reply_to: None`.

### 2.5 Медиа

| Функция | Статус | Доказательство |
|---------|--------|----------------|
| Inline image + viewer | 🟡 | load async + ImageViewer | |
| File upload+send | 🟡 | `send_message_with_file` | Session RPC; README устарел («не реализовано») |
| File download | 🟡 | `download_file` API | open-with / DnD / paste — нет |
| Video play | 🔴 | `// TODO: Open video player` | |
| Voice record | 🟡 | GStreamer optional + flag | default hidden |
| Voice play | 🔴 | stub progress, no audio | |
| Voice send protocol | 🔴 | upload + **text** `"Voice message (N s)"` | не voice-type |
| Stickers | 🟡 | catalog + mock fallback | |
| Polls UI | 🟡 | creator/renderer + API | live E2E ? |

### 2.6 Звонки / Telemost

| Функция | Статус | Доказательство |
|---------|--------|----------------|
| Feature flag hide | ✅ | `ym_enable_telemost_ui()` |
| Cloud API client | 🟡 | `api/telemost.rs` create/get | `session_cookies() → None` always |
| Call window UI | 💥 | не компилируется | placeholder video, нет WebRTC |
| start_call from main | 🟡 | wired | URL computed but **unused** (`call_url` warning) |
| Real A/V | 🔴 | нет | |

### 2.7 Прочее

| Функция | Статус |
|---------|--------|
| Global search Ctrl+K | 🟡 UI |
| Folders sidebar | 🟡 |
| Saved / scheduled / bots panels | 🟡 UI-heavy, E2E unclear |
| Groups create | 🟡 contacts load |
| Participants count | 🟡 часто 0 — parse/API gap |
| Reactions | 🟡 UI + WS hooks; README says «не реализовано» (docs drift) |
| Translation button | 🟡 |
| i18n | 🔴 mixed EN/RU |

---

## 3. Зависимость от `session.json` (важно)

На машине аудита: **HAS_SESSION + HAS_TOKEN**.

| Операция | OAuth token | Session cookies |
|----------|-------------|-----------------|
| Chat list | часто ok | — |
| Full history | partial (search) | **нужны** |
| WS connect | — | **нужны** (`get_session_cookies_and_uid`) |
| send_message RPC / files | — | **нужны** |
| Text send WS | yuid from session (else **hardcoded fallback**) | желательны |

Hardcoded:

```870:871:src/api/mod.rs
        let yuid =
            Self::get_yuid_from_session().unwrap_or_else(|| "1057346851777820885".to_string());
```

Без актуальной session отправка/realtime **непредсказуемы**. Login script (`scripts/login_browser.py`) — отдельный шаг, не встроен в happy-path OAuth.

---

## 4. Расхождения docs ↔ code

| Утверждение | Реальность |
|-------------|------------|
| CHANGELOG: 0 warnings, 13 tests pass | Build broken; telemost warnings/errors |
| README: загрузка файлов «не реализовано» | `send_file_message` + attach callback wired |
| README: реакции «не реализовано» | reaction panel + WS partial code |
| ROADMAP historical ✅ sprints | Много UI shells (см. GAP_AUDIT) |
| TESTPLAN: notify-rust, tray | stubs |

---

## 5. Приоритетный backlog «починить функциональность» (после green build)

### P0 — разблокировать

| # | Задача | Effort | DoD |
|---|--------|--------|-----|
| F0 | Починить / feature-gate Telemost compile | S | `cargo check` green |
| F1 | `cargo test` green (settings + preview tests) | S | CI green |
| F2 | Синхронизировать README matrix с фактом (files, reactions) | S | no docs lie |

### P0 — trust (всё ещё не сделано)

| # | Задача | Effort |
|---|--------|--------|
| F3 | notify-rust + settings respect + mute | M |
| F4 | tray + close-to-tray | M |
| F5 | mute/pin/archive/mark_read **API + wire** | L |
| F6 | mark_read on chat open | M |
| F7 | Preview RU + media types + bubble fallback | S |

### P0/P1 — messaging correctness

| # | Задача | Effort | Почему больно |
|---|--------|--------|---------------|
| F8 | Reply: передать `reply_to` в callback → WS/HTTP | M | UI врёт |
| F9 | Edit: реальный edit API, не send new | M | UI врёт |
| F10 | Убрать hardcoded yuid; fail loud | S | wrong identity risk |
| F11 | Outbox + delivery ack | L | trust send |
| F12 | History pagination | M | long chats |

### P1 — files/media

| # | Задача |
|---|--------|
| F13 | Live E2E file send with session (manual checklist) |
| F14 | Download open / save as |
| F15 | Video player or hide UI |
| F16 | Voice play real or keep hidden |

### P1 — auth UX

| # | Задача |
|---|--------|
| F17 | Session capture inside app login (no separate python) |

---

## 6. Ручной smoke (когда build снова зелёный)

Выполнять с `HAS_SESSION` + `HAS_TOKEN`.

### Smoke A — Core chat (15 мин)

1. [ ] Запуск без crash  
2. [ ] Список чатов не пустой  
3. [ ] Открыть чат → история > 0  
4. [ ] Отправить текст → bubble local  
5. [ ] Сообщение видно в официальном web/mobile  
6. [ ] Ответ с web → появляется в native (или после reselect)  
7. [ ] Notification: **ожидаемо FAIL** (stderr only)  

### Smoke B — Files (10 мин)

1. [ ] Attach pdf/png  
2. [ ] У peer файл открывается  
3. [ ] Получить файл → скачать/открыть  

### Smoke C — Actions honesty (5 мин)

1. [ ] Reply: peer видит reply quote — **сейчас вероятно FAIL**  
2. [ ] Edit last: peer видит edit — **вероятно FAIL**  
3. [ ] Mute chat: тишина notify — **FAIL** (нет API + нет notify)  

### Smoke D — Flags (2 мин)

1. [ ] Default: нет mic, нет call button  
2. [ ] `YM_ENABLE_VOICE=1`: mic visible (play may stub)  
3. [ ] `YM_ENABLE_TELEMOST_UI=1`: call visible (call may stub)  

---

## 7. Оценка готовности (обновлено)

| Область | Было (GAP) | Сейчас |
|---------|------------|--------|
| Docs honesty | ⭐⭐ | ⭐⭐⭐⭐ |
| Build health | ⭐⭐⭐⭐ | ⭐⭐⭐⭐ (починен в аудите; 8 warnings) |
| Trust desktop | ⭐ | ⭐ |
| Text messaging | ⭐⭐⭐ | ⭐⭐⭐ (reply/edit дыра) |
| Files | ⭐⭐ | ⭐⭐⭐ wiring (нужен session + live smoke) |
| Media play | ⭐ | ⭐ |
| Calls | ⭐ | ⭐ (UI shell, не WebRTC) |
| **Daily driver** | ~25–35% | **~30–35%** (docs honesty + files wire; P0 trust ещё нет) |

---

## 8. Рекомендация на ближайшие 48 часов

```
1. СРОЧНО: green build (fix or gate telemost_window)
2. Не трогать дизайн, пока cargo check + test не зелёные
3. F8/F9 reply+edit wire (высокий user trust / мало кода)
4. F7 RU previews (быстрый win)
5. F3 notifications (максимальный dogfood impact)
6. Live Smoke A/B с session
7. Потом — дизайн-система (отдельный DESIGN.md)
```

**Дизайн имеет смысл после green build + 2–3 дня dogfood:** иначе полируем UI, который не компилируется / врёт в reply.

---

## 9. Чеклист «функция done»

Функция считается рабочей только если:

1. `cargo check` проходит  
2. Есть путь UI → Core → API (не только log)  
3. Manual smoke с peer-клиентом (web YM)  
4. README status = shipped/beta, не stub  
5. Feature flag только для opt-in beta, не для скрытия вечной заглушки без плана  

---

## 10. Якоря в коде

| Файл | Проблема |
|------|----------|
| `src/ui/telemost_window.rs` | **build break** |
| `src/ui/notifications.rs` | stub notify |
| `src/ui/tray.rs` | stub tray |
| `src/ui/chat_list.rs` ~1024 | STUB chat actions |
| `src/ui/chat_view.rs` handle_send | reply/edit not sent |
| `src/core.rs` send_text / send_voice | no reply; voice as text |
| `src/core/db.rs` | SQLite no-op |
| `src/api/mod.rs` yuid fallback | hardcoded identity |
| `src/models/mod.rs` preview | EN strings |
| `src/ui/settings.rs` | store only, no window |

---

*Перезапускать этот аудит после F0: достаточно `cargo check && cargo test` + Smoke A.*
