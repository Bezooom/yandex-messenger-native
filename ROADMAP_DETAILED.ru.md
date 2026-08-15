# Детальная дорожная карта доработки

**Проект:** Yandex Messenger Native (Linux, Rust + GTK4/Libadwaita)  
**Базовая версия плана:** 2.165.0  
**Текущий релиз:** 2.173.0 (2026-08-15)  
**Дата:** 2026-08-15  
**Основание:** [`GAP_AUDIT.md`](GAP_AUDIT.md), текущий код `src/`, существующий [`ROADMAP.ru.md`](ROADMAP.ru.md)  
**English:** [`ROADMAP_DETAILED.md`](ROADMAP_DETAILED.md)  
**Принцип:** *stub ban* — фича либо работает end-to-end, либо скрыта из UI; статус в docs = статус в коде

### Статус на 2.173.0

Закрыто относительно Gate A / начала Gate B: notify, tray, mark-read, mute/pin/archive UI, session-in-login, outbox, drafts, pagination, SQLite cache, Download/Open, DnD/paste, delivery ticks, skeleton/empty states, night-тема.  
Ещё открыто: голос/видеоплеер, настоящий Telemost WebRTC, надёжные chat-action RPC, полноценный офлайн и паритет групп.

---

## 0. Как пользоваться этим документом

| Термин | Значение |
|--------|----------|
| **Фаза** | Крупный этап с продуктовым результатом |
| **Спринт** | 1–2 недели focused work (1 full-time dev) |
| **S / M / L / XL** | Effort: ~0.5–1д / 2–3д / 5–8д / 2+ недели |
| **DoD** | Definition of Done — без этого задача не «✅» |
| **Gate** | Критерий выхода из фазы (dogfood) |
| **Depends** | Блокирующие задачи |

**Статусы задач:** `TODO` · `IN PROGRESS` · `BLOCKED` · `DONE` · `WONTFIX`

**Правило прогресса:**
1. Сначала P0 (trust), потом P1 (daily), потом polish.
2. Не открывать новые «панели» (bots/scheduled UI), пока не закрыт Gate фазы A.
3. Каждая задача: **файл(ы) → API → UI → manual test → честный статус в README**.

**Оценки:** 1 разработчик, знакомый с кодовой базой. При 2 dev можно параллелить UI/API внутри спринта, но Gate фазы общий.

```
Фаза 0 ──► Фаза A ──► Фаза B ──► Фаза C ──► Фаза D ──► Фаза E
 Prep      Trust      Daily      Media      Parity     Polish
 ~1 нед    ~3–4 нед   ~3–4 нед   ~3–5 нед   ~4–6 нед   continuous
            MVP-0      MVP-1      MVP-2      «почти YM»  release quality
```

| Вехи | Версия (предложение) | Пользовательский смысл |
|------|----------------------|------------------------|
| Gate A | **2.170** | Хочется держать открытым: notify + tray + честные действия |
| Gate B | **2.180** | Можно писать текст/файлы каждый день вместо web |
| Gate C | **2.190** | Голос/видео/превью медиа не стыдно |
| Gate D | **2.200** | Группы, поиск, звонки, офлайн — паритет «основного» |
| Gate E | **2.210+** | Полировка, a11y, i18n, автообновления |

---

## 1. Продуктовые цели и метрики

### 1.1 Цели по фазам

| Фаза | Цель одной фразой |
|------|-------------------|
| **0** | Правда в docs + тестовый каркас; можно мерять прогресс |
| **A** | Desktop-клиент, которому доверяешь базовые действия |
| **B** | Daily driver для текста и файлов |
| **C** | Медиа как в нормальном мессенджере |
| **D** | Паритет ключевых сценариев YM (кроме «всего enterprise») |
| **E** | Приятно жить: a11y, i18n, updates, polish |

### 1.2 KPI (измерять dogfood-неделей)

| Метрика | Gate A | Gate B | Gate C | Gate D |
|---------|--------|--------|--------|--------|
| Cold start → список чатов | < 3s (cache) | < 2s | < 2s | < 1.5s |
| Send text perceived latency | < 2s | < 1s | < 1s | < 0.8s |
| Silent fail на mute/read/notify | 0 | 0 | 0 | 0 |
| Crash-free session | 2h | 8h | 8h | 24h |
| % типов msg с нормальным preview | ≥ 80% | ≥ 95% | ≥ 98% | ≥ 99% |
| Голос play works | — | — | 100% happy path | + edge |
| Dogfood дней/нед в клиенте | 3 | 5 | 5 | 5 |

### 1.3 Out of scope (явно)

- Windows / macOS / mobile
- Классический E2E encryption (у YM нет как у Signal)
- Собственный push-сервер (только Yandex push/WS)
- Полный паритет Yandex 360 admin / corporate policies (кроме auth-proxy)

---

## 2. Фаза 0 — Подготовка и честность (Sprint 0)

**Длительность:** 3–5 дней  
**Результат:** команда и пользователи понимают, что реально работает

### Sprint 0.1 — Docs & inventory

| ID | Задача | Effort | Файлы | DoD |
|----|--------|--------|-------|-----|
| S0-01 | Матрица статусов shipped/beta/stub в README | S | `README.ru.md`, `README.md` | Таблица из GAP_AUDIT §3, статусы правдивые |
| S0-02 | Пометить stub-UI (`// STUB:`) в notifications, tray, telemost, voice play, chat actions | S | `src/ui/*` | grep `STUB` находит все ложные кнопки |
| S0-03 | Скрыть из UI действия без backend (или disabled + tooltip «скоро») | S | `chat_list.rs`, `telemost.rs` | Нет кликабельного «фейка» без предупреждения |
| S0-04 | Обновить TESTPLAN под реальность + smoke checklist | S | `TESTPLAN.md` | Checklist = то, что можно прогнать руками |
| S0-05 | CHANGELOG: секция «Known limitations» | S | `CHANGELOG.ru.md` | Пользователь видит ограничения v2.165 |

### Sprint 0.2 — Engineering hygiene

| ID | Задача | Effort | DoD |
|----|--------|--------|-----|
| S0-10 | Свести warnings (`cargo fix` + убрать dead code где осознанно) | S–M | `cargo build` ≤ 10 warnings |
| S0-11 | Модульные smoke-тесты: parse message preview, settings load/save | S | `cargo test` зелёный |
| S0-12 | Manual smoke script в `docs/` или `TESTPLAN`: login → chats → send | S | 1 страница, 10 шагов |
| S0-13 | Feature flags (cfg/env): `YM_ENABLE_VOICE`, `YM_ENABLE_TELEMOST_UI` для скрытия сырого | S | Default: hide stubs |

**Gate 0:** README честный; stub-кнопки не обманывают; smoke checklist существует.

---

## 3. Фаза A — Trust & Desktop (MVP-0) → **v2.170**

**Длительность:** 3–4 недели  
**Цель:** «хочется держать в трее, а не стыдно открыть»

```
                    ┌─────────────────┐
                    │  A1 Notifications│
                    └────────┬────────┘
                             │
              ┌──────────────┼──────────────┐
              ▼              ▼              ▼
        ┌──────────┐  ┌──────────┐  ┌──────────────┐
        │ A2 Tray  │  │ A3 Chat  │  │ A4 Previews  │
        │          │  │ actions  │  │ & i18n strings│
        └────┬─────┘  └────┬─────┘  └──────┬───────┘
             │             │               │
             └─────────────┼───────────────┘
                           ▼
                 ┌──────────────────┐
                 │ A5 Settings shell│
                 └────────┬─────────┘
                          ▼
                 ┌──────────────────┐
                 │ Gate A dogfood   │
                 └──────────────────┘
```

### Sprint A1 — Desktop notifications (1 нед)

| ID | Задача | Effort | Depends | Файлы / подход | DoD |
|----|--------|--------|---------|----------------|-----|
| A1-01 | Подключить `notify-rust` (или xdg portal) | S | — | `Cargo.toml`, `notifications.rs` | Toast виден в GNOME/KDE |
| A1-02 | API: `send_notification(summary, body, chat_id?)` | S | A1-01 | | ID уведомления, replace same chat |
| A1-03 | Уважать `AppSettings.notifications_enabled` | S | A1-02 | `settings.rs`, `main.rs` | Выкл = тишина |
| A1-04 | Уважать `chat.muted` | S | A1-02 | | Mute chat → нет toast |
| A1-05 | Не уведомлять, если окно focused + тот же chat open | S | A1-02 | | Нет спама |
| A1-06 | Click action → present window + select chat | M | A1-02 | `main.rs`, gio actions | Клик открывает чат |
| A1-07 | Wire WS `new_message` → notification | S | A1-02 | `main.rs` | Реальный incoming → toast |

**DoD спринта:** входящее сообщение от другого клиента → desktop notification → клик → чат.

### Sprint A2 — System tray (1 нед)

| ID | Задача | Effort | Depends | Подход | DoD |
|----|--------|--------|---------|--------|-----|
| A2-01 | Выбрать backend: `ksni` / `libappindicator` / StatusNotifier | S | — | Исследование DE | Работает GNOME+KDE |
| A2-02 | Реализовать `TrayHandle` (icon, menu) | M | A2-01 | `tray.rs` | Иконка в трее |
| A2-03 | Меню: Показать / Настройки / Выход | S | A2-02 | | Пункты работают |
| A2-04 | Close window → hide (если `minimize_to_tray`) | M | A2-02, settings | `main.rs` | X не убивает процесс |
| A2-05 | Unread badge / tooltip count (best-effort) | S–M | A2-02 | | Tooltip «N непрочитанных» |
| A2-06 | Single-instance (optional, dbus/lockfile) | M | — | | Второй запуск фокусирует первый |

### Sprint A3 — Chat actions (реальные) (1–1.5 нед)

| ID | Задача | Effort | Depends | DoD |
|----|--------|--------|---------|-----|
| A3-01 | RE reverse/sniff: mute, pin, archive, mark_read, delete_chat endpoints | M | — | Зафиксировать в `API.ru.md` |
| A3-02 | HTTP/WS методы: `mute_chat`, `pin_chat`, `archive_chat`, `mark_read` | M | A3-01 | Unit/integration call returns Ok |
| A3-03 | Wire context menu → API + optimistic UI | M | A3-02 | `chat_list.rs` | Клик меняет UI и сервер |
| A3-04 | Mark as read при открытии чата / просмотре | M | A3-02 | `main.rs` select_chat | Badge падает |
| A3-05 | Persist muted/pinned/archived в state + sort | S | A3-03 | | После reload порядок верный |
| A3-06 | Delete chat: confirm dialog + API | S | A3-01 | | Chat исчезает из списка |

**Риск:** API может отличаться; заложить 2–3 дня reverse. Если endpoint недоступен — UI disabled + issue, не log-only.

### Sprint A4 — Previews & honesty strings (3–5 дн)

| ID | Задача | Effort | DoD |
|----|--------|--------|-----|
| A4-01 | `Message::preview()` / `Chat::preview_text()` — типы media/system/call/file/sticker/voice/video/poll | M | Нет «No messages» / generic attachment на 80%+ типов |
| A4-02 | Все user-facing строки RU (временно hardcoded ok) | S | Нет EN в sidebar/preview |
| A4-03 | Participants count: парсинг + отображение | S–M | Не «0 участников» если API отдаёт данные |
| A4-04 | Empty states: «Нет сообщений», «Выберите чат» | S | Аккуратные placeholders |
| A4-05 | Unread badge styling + muted icon in list | S | Визуально ясно mute/unread |

### Sprint A5 — Settings product shell (3–4 дн)

| ID | Задача | Effort | DoD |
|----|--------|--------|-----|
| A5-01 | Окно настроек (Adw PreferencesWindow) | M | `settings.rs` UI |
| A5-02 | Секции: Внешний вид / Уведомления / Поведение / Аккаунт | S | 3+ существующих флага + logout |
| A5-03 | Logout / remove account | S | Чистит token, показывает auth |
| A5-04 | About: version, disclaimer unofficial | S | adw::AboutDialog |

### Gate A (v2.170) — dogfood checklist

- [ ] Входящее → desktop notification
- [ ] Клик по notification → чат
- [ ] Трей есть; закрытие окна не убивает (настройка)
- [ ] Mute / pin / archive / mark read работают против сервера
- [ ] Превью чатов на русском и осмысленные
- [ ] Нет STUB-кнопок, которые «делают вид»
- [ ] 3 дня личного dogfood без «сейчас открою web»

---

## 4. Фаза B — Daily messaging (MVP-1) → **v2.180**

**Длительность:** 3–4 недели  
**Цель:** писать и получать текст/файлы надёжнее, чем «иногда»

```
B1 Auth/Session unify ──► B2 Delivery/outbox ──► B3 History pagination
         │                        │
         └────────► B4 Files polish ◄───────────┘
                           │
                    B5 Drafts + UX
```

### Sprint B1 — Auth & realtime path (1–1.5 нед) ⚠️ критичный R&D

| ID | Задача | Effort | DoD |
|----|--------|--------|-----|
| B1-01 | Документировать текущие 2 пути: OAuth token vs session cookies | S | Диаграмма в ARCHITECTURE |
| B1-02 | Встроить получение session cookies в login flow (без внешнего python, или встроить script) | L | После OAuth WS connect без ручного `login_browser.py` |
| B1-03 | Убрать hardcoded `yuid` fallback; fail loud if missing | S | `api/mod.rs` |
| B1-04 | Health: UI indicator Connected / Reconnecting / Offline | S | Status в header |
| B1-05 | Token refresh + rebind HTTP+WS on account switch | M | Multi-account не ломает session |
| B1-06 | Интеграционный тест: connect → subscribe → send (manual + log harness) | M | Воспроизводимый сценарий |

**Gate sprint:** новый пользователь: install → login → receive realtime **без** terminal scripts.

### Sprint B2 — Delivery, read ticks, outbox (1–1.5 нед)

| ID | Задача | Effort | DoD |
|----|--------|--------|-----|
| B2-01 | Outbox queue (pending messages) in memory + disk | M | Перезапуск не теряет unsent |
| B2-02 | Retry with backoff on send failure | M | UI: ◷ → ✓ → ✓✓ |
| B2-03 | Parse server ack / delivery / read from WS | M | Галочки отражают сервер |
| B2-04 | Dedup by payload_id / message id | S | Нет двойных bubbles |
| B2-05 | Edit/delete: verify server round-trip + remote update | M | Другой клиент видит change |
| B2-06 | Reply/forward end-to-end verification checklist | S | QA matrix filled |

### Sprint B3 — History & cache (1 нед)

| ID | Задача | Effort | DoD |
|----|--------|--------|-----|
| B3-01 | Scroll-up pagination (load older) | M | 500+ msg chat usable |
| B3-02 | SQLite: implement `cache_chats` / `get_cached_chats` / messages upsert | L | `db.rs` не stub |
| B3-03 | Startup: SQLite → UI, then network refresh | M | Instant open |
| B3-04 | Invalidate cache on account switch | S | Нет чужих чатов |
| B3-05 | Удалить или пометить JSON L2 vs SQLite single source of truth | S | Один канонический кэш |

### Sprint B4 — Files daily-grade (1 нед)

| ID | Задача | Effort | DoD |
|----|--------|--------|-----|
| B4-01 | Upload progress bar | M | % или indeterminate |
| B4-02 | Download + open with default app / save as | M | Файл открывается |
| B4-03 | Drag-and-drop files into chat | M | DnD works |
| B4-04 | Paste image from clipboard | M | Ctrl+V image |
| B4-05 | Document bubble: name, size, icon by mime | S | Не generic attachment |
| B4-06 | Multi-file send queue | S–M | 3 файла подряд |

### Sprint B5 — Drafts & messaging UX (3–5 дн)

| ID | Задача | Effort | DoD |
|----|--------|--------|-----|
| B5-01 | Draft per chat (persist settings/sqlite) | M | Смена чата сохраняет текст |
| B5-02 | Forward: chat picker dialog | M | Не только callback |
| B5-03 | Jump to replied message | S | Click reply preview |
| B5-04 | Ctrl+F search matches: next/prev | S | N matches UX |
| B5-05 | Linkify URLs (clickable) | S | open browser |

### Gate B (v2.180)

- [ ] Login-only path (no manual session script) for happy path
- [ ] Send/receive stable 1 working day
- [ ] Outbox survives restart
- [ ] Files: send photo + pdf, open received
- [ ] History scroll-up works
- [ ] Drafts work
- [ ] 5 дней dogfood как primary for text work chats

---

## 5. Фаза C — Media (MVP-2) → **v2.190**

**Длительность:** 3–5 недель  
**Цель:** голос и видео не стыдно; медиа-бабблы полные

**Зависимость:** feature `gstreamer` в Cargo; default builds могут оставаться без, но package `-full` с gstreamer.

### Sprint C1 — Voice real (1.5–2 нед)

| ID | Задача | Effort | DoD |
|----|--------|--------|-----|
| C1-01 | GStreamer playback pipeline in VoiceMessagePlayer | L | Слышен звук |
| C1-02 | Seek / pause / progress real | M | Progress = position |
| C1-03 | Record pipeline verify (opus/ogg as server expects) | M | Server accepts |
| C1-04 | Send voice as **voice message type**, not text stub | M | Bubble voice, not text |
| C1-05 | Waveform from real samples or server | S–M | Красиво |
| C1-06 | Transcription UI only if API works; else hide | S | No fake spinner forever |
| C1-07 | Feature-gate UI mic if no gstreamer | S | Clear error / hide |

### Sprint C2 — Video (1–1.5 нед)

| ID | Задача | Effort | DoD |
|----|--------|--------|-----|
| C2-01 | Video player window/overlay (GStreamer gtk4paintablesink or similar) | L | Play received video |
| C2-02 | Poster + duration + fullscreen | M | UX ok |
| C2-03 | Download video file | S | Save works |
| C2-04 | Upload video with progress | M | Send works |

### Sprint C3 — Images & stickers quality (1 нед)

| ID | Задача | Effort | DoD |
|----|--------|--------|-----|
| C3-01 | Multi-image bubble (grid) | M | 2–4 photos |
| C3-02 | Compress before upload (size limits) | M | Large photo ok |
| C3-03 | Sticker catalog reliability (less mock) | M | Real packs when online |
| C3-04 | Recent stickers + favorites local | S | Persist |
| C3-05 | Animated stickers/WebP if feasible | L | Optional stretch |

### Sprint C4 — Rich content rendering (1 нед)

| ID | Задача | Effort | DoD |
|----|--------|--------|-----|
| C4-01 | Entities → Pango markup (bold/italic/code/link) | M | Не `**text**` plain |
| C4-02 | Link preview card (optional OG fetch) | L | Preview box |
| C4-03 | System messages dedicated style | S | Join/leave/pin |
| C4-04 | Location / contact cards minimal render | M | If API provides |
| C4-05 | Call/telemost history messages | S | «Звонок · 5 мин» |

### Gate C (v2.190)

- [ ] Record voice → peer hears in official client
- [ ] Play voice received
- [ ] Play video received
- [ ] Photos multi + compress
- [ ] Rich text looks rich
- [ ] Dogfood media-heavy chat 3 days

---

## 6. Фаза D — Parity core → **v2.200**

**Длительность:** 4–6 недель  
**Цель:** группы, звонки, поиск, боты — на уровне «можно не открывать web»

### Sprint D1 — Telemost (1–2 нед)

| ID | Задача | Effort | DoD |
|----|--------|--------|-----|
| D1-01 | MVP: `xdg-open` telemost URL + copy link | S | Звонок стартует в browser |
| D1-02 | Better: WebKitGTK embed telemost (feature `in_app_webview`) | L | Call in-app |
| D1-03 | Mute/Video/End bound to real controls **or** remove fake buttons | M | No lying UI |
| D1-04 | Incoming call notification + join | M | Depends on push events |
| D1-05 | In-chat call history messages | S | Linked to D C4-05 |

### Sprint D2 — Groups & channels (1.5 нед)

| ID | Задача | Effort | DoD |
|----|--------|--------|-----|
| D2-01 | Members list real (avatars, roles) | M | Not empty |
| D2-02 | Add/remove member E2E | M | Works |
| D2-03 | Invite link create/copy | M | Works |
| D2-04 | Channel create + post mode | M | Works |
| D2-05 | Group settings panel (title, avatar, notifications) | M | Works |
| D2-06 | Permissions basic (who can post) if API allows | M | Best-effort |

### Sprint D3 — Search & navigation (1 нед)

| ID | Задача | Effort | DoD |
|----|--------|--------|-----|
| D3-01 | Global search → open chat at message | M | Jump works |
| D3-02 | Filters: people / media / files | M | Tabs |
| D3-03 | Recent searches | S | Persist |
| D3-04 | Mentions autocomplete @ | M | Suggest members |
| D3-05 | Thread navigation polish | M | Breadcrumb + counter |

### Sprint D4 — Folders, saved, scheduled, bots (1.5–2 нед)

| ID | Задача | Effort | DoD |
|----|--------|--------|-----|
| D4-01 | Folders CRUD + assign chats | M | Real API |
| D4-02 | Saved messages server sync verify | M | Cross-device |
| D4-03 | Scheduled: create, list, cancel E2E | M | Fires on time |
| D4-04 | Bot inline keyboard callbacks E2E | M | Buttons work |
| D4-05 | Polls live updates via WS | M | Votes sync |
| D4-06 | Reactions full set + realtime | S–M | No full refetch |

### Sprint D5 — Presence, multi-account, offline (1 нед)

| ID | Задача | Effort | DoD |
|----|--------|--------|-----|
| D5-01 | Online/last seen accuracy | M | Matches official-ish |
| D5-02 | Typing debounce + multi-user «A, B печатают» | S | UX |
| D5-03 | Multi-account: isolated DB + tokens | M | Clean switch |
| D5-04 | Offline banner + queue drain | M | Clear UX |
| D5-05 | Contact profile card + start DM | M | From search/members |

### Gate D (v2.200)

- [ ] Group with 5+ members manageable without web
- [ ] Call can be started (browser or embed)
- [ ] Global search jump works
- [ ] Folders usable
- [ ] Bots basic work
- [ ] Offline → online without data loss
- [ ] 2 weeks primary client for non-media work + media ok

---

## 7. Фаза E — Release quality (continuous) → **v2.210+**

### E1 Accessibility & keyboard

| ID | Задача | Effort | DoD |
|----|--------|--------|-----|
| E1-01 | Full keyboard nav (chat list, messages, input) | L | No mouse needed core paths |
| E1-02 | AT-SPI labels on all actions | M | Orca basic |
| E1-03 | Focus rings / high contrast | S | Visible |
| E1-04 | Reduced motion option | S | Settings |

### E2 Localization

| ID | Задача | Effort | DoD |
|----|--------|--------|-----|
| E2-01 | Extract strings (fluent or gettext) | L | All UI |
| E2-02 | ru + en complete | M | Switch in settings |
| E2-03 | Date/time locale | S | chrono locale |

### E3 Packaging & updates

| ID | Задача | Effort | DoD |
|----|--------|--------|-----|
| E3-01 | Flatpak manifest (optional) | L | Flathub-ready structure |
| E3-02 | Deb CI artifact stable | M | Every tag |
| E3-03 | In-app update check (GitHub releases) | M | Notify new version |
| E3-04 | Crash log export button | S | User can send logs |

### E4 Performance & refactor

| ID | Задача | Effort | DoD |
|----|--------|--------|-----|
| E4-01 | Split `chat_view.rs` (~3k) into modules | L | Compile + tests |
| E4-02 | Split `api/mod.rs` (~3.8k) by domain | L | Same |
| E4-03 | Message list virtualization if needed | L | 10k msgs smooth |
| E4-04 | Avatar disk cache + memory LRU | M | No flicker |
| E4-05 | Clippy pedantic CI gate | S | Clean |

### E5 Security & privacy

| ID | Задача | Effort | DoD |
|----|--------|--------|-----|
| E5-01 | Token file 0600 + optional secret service | M | SECURITY.md update |
| E5-02 | Clear cache action | S | Works |
| E5-03 | Audit logs for no secrets in log | S | grep CI |
| E5-04 | Dependency audit (cargo deny / audit) | S | CI job |

### Gate E (ongoing)

- [ ] a11y core paths
- [ ] i18n ru/en
- [ ] Release pipeline
- [ ] Refactors without regressions

---

## 8. Сводный timeline (1 FTE)

| Недели | Фаза | Версия | Результат |
|--------|------|--------|-----------|
| 1 | 0 | 2.166 | Honesty + hygiene |
| 2–5 | A | **2.170** | Trust desktop MVP |
| 6–9 | B | **2.180** | Daily text+files |
| 10–14 | C | **2.190** | Media |
| 15–20 | D | **2.200** | Parity core |
| 21+ | E | 2.210+ | Polish continuous |

**Ускорение (2 FTE):** фазы A–C ~12–14 недель вместо 14; D/E параллелятся лучше.  
**R&D buffer:** +20–30% на reverse API (особенно B1, A3, D1).

```
Месяц 1:  [==== Phase 0+A ==================]  → v2.170
Месяц 2:  [==== Phase B ====================]  → v2.180
Месяц 3:  [==== Phase C ========|== D start ]  → v2.190
Месяц 4–5:[======== Phase D ================]  → v2.200
Месяц 6+: [==== Phase E continuous =========]
```

---

## 9. Зависимости и риски

| Риск | Вероятность | Impact | Митигация |
|------|-------------|--------|-----------|
| Неофициальный API ломается | Высокая | Высокий | Версионировать protocol notes; fallback HTTP; feature detect |
| Session cookies недоступны из OAuth-only | Средняя | Критичный для B1 | WebView login capture; document dual-mode |
| Telemost WebRTC в WebKitGTK | Средняя | Средний | Fallback xdg-open |
| GStreamer на всех DE | Средняя | Средний | Optional feature + clear UX |
| Нет времени reverse mute/archive | Средняя | Средний | UI hide until known |
| Scope creep (ещё панели) | Высокая | Высокий | Gate A hard freeze features |
| Docs again overstate | Высокая | Средний | PR checklist: status matrix |

### Критический путь

```
B1 Auth/Session ──┬──► B2 Outbox/Delivery ──► Gate B
                  │
A3 Chat actions ◄─┘ (частично независим, но mark_read связан)
A1 Notify ──► A2 Tray ──► Gate A
C1 Voice ──► C2 Video ──► Gate C
D1 Telemost ──► Gate D (можно после C)
```

---

## 10. Организация работы

### 10.1 Definition of Done (общий)

Задача `DONE` только если:
1. Код в `main` / PR merged  
2. Manual test из DoD пройден (записан в PR)  
3. Нет `log::info!("action")` вместо эффекта  
4. README/GAP matrix status обновлён  
5. Нет новых clippy deny-level  
6. Changelog entry (если user-facing)

### 10.2 PR checklist

```markdown
- [ ] User-facing? CHANGELOG updated
- [ ] Stub removed or still marked STUB + hidden
- [ ] Manual test steps in PR body
- [ ] Screenshots if UI
- [ ] API notes if protocol change
```

### 10.3 Dogfood protocol

- **Gate A/B/C/D:** минимум N дней primary client (см. KPI)
- Каждую пятницу: 15-min note «что бесило» → backlog
- Regression: smoke TESTPLAN перед каждым tag

### 10.4 Версионирование

| Изменение | Semver-ish (2.x) |
|-----------|------------------|
| Stub→real desktop trust | 2.170 |
| Daily messaging | 2.180 |
| Media | 2.190 |
| Parity | 2.200 |
| Patch fixes | 2.170.1 / 2.x.y |

---

## 11. Бэклог задач (плоский, для issue tracker)

Скопировать в GitHub Issues / Linear. Приоритет = порядок.

### P0 — Trust (Phase A)

1. [ ] A1-01…A1-07 Notifications  
2. [ ] A2-01…A2-06 Tray  
3. [ ] A3-01…A3-06 Chat actions  
4. [ ] A4-01…A4-05 Previews  
5. [ ] A5-01…A5-04 Settings  

### P0/P1 — Reliability (Phase B)

6. [ ] B1-01…B1-06 Auth/session  
7. [ ] B2-01…B2-06 Outbox/delivery  
8. [ ] B3-01…B3-05 History/SQLite  
9. [ ] B4-01…B4-06 Files  
10. [ ] B5-01…B5-05 Drafts/UX  

### P1 — Media (Phase C)

11. [ ] C1 Voice  
12. [ ] C2 Video  
13. [ ] C3 Images/stickers  
14. [ ] C4 Rich content  

### P1/P2 — Parity (Phase D)

15. [ ] D1 Telemost  
16. [ ] D2 Groups  
17. [ ] D3 Search  
18. [ ] D4 Folders/bots/scheduled  
19. [ ] D5 Presence/offline  

### P2 — Quality (Phase E)

20. [ ] E1 a11y  
21. [ ] E2 i18n  
22. [ ] E3 packaging  
23. [ ] E4 refactor/perf  
24. [ ] E5 security  

### P3 — Later / nice

- RTL  
- Animated stickers full  
- Export chat  
- Plugin hooks  
- Flatpak Flathub publication  

---

## 12. Первые 10 рабочих дней (конкретный старт)

| День | Фокус | Выход |
|------|-------|-------|
| 1 | S0 honesty: README matrix, STUB marks, hide fake actions | PR docs+ui |
| 2 | A1 notifications crate + basic toast | Toast on test call |
| 3 | A1 wire WS + mute/settings respect | Incoming works |
| 4 | A1 click → open chat | Complete A1 |
| 5 | A2 tray research + minimal icon | Icon visible |
| 6 | A2 close-to-tray + menu | Tray usable |
| 7 | A4 previews + RU strings | List looks honest |
| 8 | A3 reverse mute/mark_read | API notes |
| 9 | A3 implement mark_read + mute | 2 actions live |
| 10 | A5 settings shell + release 2.170-beta | Tag beta |

После дня 10 — добить A3 pin/archive/delete и Gate A checklist.

---

## 13. Критерии «мы догнали Яндекс» (реалистично)

Полный паритет с closed-source multi-platform YM **нереален** для community client. Целевая планка:

### «Достаточно, чтобы не хотеть web» (Gate D+)

| Обязательно | Желательно | Не обязательно |
|-------------|------------|----------------|
| Text realtime | Threads polish | Mobile |
| Notify + tray | Embed Telemost | E2E crypto |
| Files/photos | Link previews | 100% bots |
| Voice | Animated stickers | Admin 360 |
| Mute/read/pin/archive | i18n en | Screen share native |
| Groups members | a11y full | |
| Search jump | Flatpak | |
| Offline cache | Auto-update | |

**Метрика успеха:** 4 недели подряд primary client ≥ 80% messaging time.

---

## 14. Связь с другими документами

| Документ | Роль |
|----------|------|
| [`GAP_AUDIT.md`](GAP_AUDIT.md) | Почему и что сломано (as-is) |
| **Этот файл** | Как чинить и в каком порядке (to-be) |
| [`ROADMAP.ru.md`](ROADMAP.ru.md) | Исторический спринт-лог; **не** source of truth статусов |
| [`API.ru.md`](API.ru.md) | Протокол; обновлять на A3/B1/D* |
| [`TESTPLAN.md`](TESTPLAN.md) | Smoke; синхронизировать с Gate checklist |
| [`ARCHITECTURE.ru.md`](ARCHITECTURE.ru.md) | Обновлять после B1/B3/E4 |
| [`CHANGELOG.ru.md`](CHANGELOG.ru.md) | User-facing changes per version |

**Рекомендация:** в шапке `ROADMAP.ru.md` добавить:

> ⚠️ Актуальная дорожная карта: [`ROADMAP_DETAILED.ru.md`](ROADMAP_DETAILED.ru.md). Этот файл — архив спринтов 1–12.

---

## 15. Чеклист релиза по вехам

### v2.170 (Gate A)

- [ ] notify-rust работает  
- [ ] tray + minimize  
- [ ] mute/pin/archive/read real  
- [ ] previews RU  
- [ ] settings window  
- [ ] CHANGELOG + known issues  
- [ ] deb package  

### v2.180 (Gate B)

- [ ] session-in-login  
- [ ] outbox  
- [ ] sqlite cache  
- [ ] files DnD/paste  
- [ ] drafts  
- [ ] pagination  

### v2.190 (Gate C)

- [ ] voice E2E  
- [ ] video play  
- [ ] rich entities  
- [ ] multi-image  

### v2.200 (Gate D)

- [ ] telemost open path  
- [ ] groups members  
- [ ] search jump  
- [ ] folders  
- [ ] bots callbacks  

---

## 16. Итог

Дорожная карта сознательно **сужает** scope: сначала trust (notify, tray, честные действия, превью), затем reliability messaging, затем media, затем parity features, которые уже нарисованы, но не доведены.

| Если делать только… | Получите |
|---------------------|----------|
| Фазу A | Клиент, который не стыдно держать в трее |
| A+B | Реальная замена web для текста/файлов |
| A+B+C | Почти полный личный мессенджер |
| A–D | «Достаточно, чтобы не хотеть Яндекс web» |
| +E | Product, а не pet project |

**Старт завтра:** Sprint 0 (день 1) + A1 notifications — максимальный impact на «желание пользоваться» за минимальный код.

---

*Живой документ. При закрытии Gate обновлять таблицу вех и статусы в GAP_AUDIT §3. Не добавлять спринты «ради галочек».*
