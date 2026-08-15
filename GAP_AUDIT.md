# Аудит: чего не хватает до уровня Яндекс Мессенджера

> ⚠️ **Исторический снимок от 2026-08-08 (~2.165.x).** Часть P0/P1 из этого аудита закрыта в **2.173.0** (notify, tray, session, outbox, SQLite, DnD, ticks, night-тема). Актуальный статус: [README.ru.md](README.ru.md). English: [GAP_AUDIT.en.md](GAP_AUDIT.en.md).

**Дата снимка:** 2026-08-08  
**Проект:** Yandex Messenger Native (неофициальный Linux-клиент, Rust + GTK4/Libadwaita)  
**Версия кода на момент аудита:** ~2.165.x (по `dist/` / changelog)  
**Объём:** ~20.6k LOC, 51 `.rs` файл  
**Метод:** code review исходников, сверка с ROADMAP/README, визуальный снимок UI (`screenshot.png`)

> Цель документа — честно ответить: **почему приложением пока «нет желания пользоваться»** и что именно нужно, чтобы дотянуться до повседневного уровня официального Яндекс Мессенджера (web/desktop), а не «спринтового чеклиста».

---

## 1. Короткий вердикт

| Область | Оценка | Комментарий |
|--------|--------|-------------|
| Архитектура / каркас | ⭐⭐⭐⭐ | Слои API/Core/UI, OAuth, HTTP, WS, модели — заложены серьёзно |
| Документация / roadmap | ⭐⭐⭐⭐ | Много, местами **опережает реальность** |
| Базовый messaging (текст) | ⭐⭐⭐ | Отправка/история в целом есть, но хрупко и с обходами |
| Медиа (фото/файл/видео/голос) | ⭐⭐ | Превью и UI-оболочки; воспроизведение и часть пайплайнов — stub |
| Звонки (Телемост) | ⭐ | Окно с URL-лейблом, не звонок |
| Desktop UX (tray, notify, offline) | ⭐ | Заглушки вместо реального desktop-поведения |
| Полировка UI / доверие | ⭐⭐ | Выглядит «почти мессенджером», но сыро и ненадёжно |
| **Готовность «жить в этом каждый день»** | **~25–35%** | Не rival официальному клиенту |

**Главный разрыв:** ROADMAP и README описывают продукт «почти готовый» (спринты 1–12 «завершены»), а код по критичным пользовательским сценариям часто **логирует действие вместо выполнения** или рисует UI поверх пустой реализации. Для ежедневного использования это хуже, чем «мало фич, но они работают».

---

## 2. Почему «нет желания пользоваться» (user trust)

Пользовательский отказ почти никогда не про «нет папок стикеров». Он про:

1. **Сообщения приходят/уходят непредсказуемо** — WS зависит от session cookies (`session.json` / login script), OAuth-only путь хрупкий.
2. **Уведомлений фактически нет** — `notifications.rs` пишет в stderr, не в desktop.
3. **Голос и видео «есть», но не играют** — player stub, video player TODO.
4. **Звонок не звонок** — Telemost UI не открывает WebRTC/WebView звонка.
5. **Список чатов врёт** — `No messages`, `0 участников`, «Вложение или системное сообщение» вместо превью.
6. **Контекстные действия — декорация** — mute/pin/archive/mark_read только `log::info`.
7. **Трей / «свернуть в трей» не работают** — `TrayHandle` пустой placeholder.
8. **Документация обещает notify-rust и tray**, TESTPLAN проверяет их — код не соответствует.

Пока P0-сценарии не зелёные, любые «enterprise» и «accessibility» фичи не повышают желание открывать приложение.

---

## 3. Матрица паритета с официальным Яндекс Мессенджером

Легенда: ✅ работает на уровне «можно пользоваться» · 🟡 частично / UI без backend · 🔴 нет / stub · ⬜ не целевой scope

| Функция (официальный YM) | У нас | Доказательство в коде | Что нужно для паритета |
|--------------------------|-------|------------------------|-------------------------|
| OAuth / вход | 🟡 | `auth.rs`, dialog, proxy | Стабильный login без ручных токенов/session scripts; корректный refresh всегда |
| Список чатов | 🟡 | `chat_list.rs`, API get_chat_list | Превью, аватар, mute/pin/archive **с API**, сортировка, empty-states на RU |
| История сообщений | 🟡 | session RPC + search fallback | Пагинация вверх, full history, стабильный parse всех типов |
| Отправка текста | 🟡 | WS binary push + optimistic UI | Подтверждение delivery/read, retry, offline queue |
| Редактирование / удаление | 🟡 | UI + меню | Надёжный server round-trip + sync в другие клиенты |
| Reply / forward / pin | 🟡 | UI actions | Проверить API end-to-end, pin bar sync |
| Реакции | 🟡 | Reaction panel + WS hooks | Полный set emoji, realtime без refetch |
| Вложения: файлы | 🟡 | upload/download API | Прогресс, типы, open-with, drag-drop, paste из буфера |
| Фото | 🟡 | inline + ImageViewer | Качество, EXIF, multi-image bubble, compress |
| Видео | 🔴 | UI overlay, `// TODO: Open video player` | Реальный player (GStreamer), scrub, fullscreen |
| Голосовые | 🔴 | recorder cfg-gated; **playback stub** | Реальная запись→upload→play с waveform |
| Стикеры | 🟡 | catalog + mock fallback | Актуальный каталог, animated, recent/favorites |
| Опросы | 🟡 | PollCreator/Renderer + API | Live results, quiz, multi-vote |
| Треды | 🟡 | ThreadView + API | Навигация, счётчик, deep-link |
| Папки | 🟡 | FolderSidebar + get/update | CRUD, drag chats between folders |
| Глобальный поиск | 🟡 | Ctrl+K UI | Server search quality, jump-to-message |
| Поиск в чате | 🟡 | highlight | Jump between matches, count |
| Saved / Избранное | 🟡 | panel + store | Server-side sync (не только локально, если API есть) |
| Отложенные | 🟡 | scheduled panel | Надёжная отправка по времени |
| Боты | 🟡 | bot panel, inline | Callbacks, keyboards end-to-end |
| Группы / каналы | 🟡 | create + group panel | Участники (не «0»), роли, invite links |
| Профиль / контакты | 🟡 | contact candidates | Карточка профиля, last seen, start DM |
| Typing / online | 🟡 | WS + status text | Надёжные presence events |
| Mark as read / unread badge | 🟡 | unread_update handler | **Явный mark_read API** при открытии чата |
| Mute / archive / pin chat | 🔴 | context menu → только log | Реальные API + UI state |
| Desktop notifications | 🔴 | `eprintln!("[notification]…")` | notify-rust / portal, actions, mute respect |
| System tray | 🔴 | `TrayHandle` empty | StatusNotifierItem / ayatana |
| Черновики (drafts) | 🔴 | нет | Persist draft per chat |
| Link preview | 🔴 | entities → markdown text, не rich | OG-карточки, кликабельные ссылки |
| Mentions / @user | 🔴 | нет UI autocomplete | Parse + picker |
| Rich text (bold/italic/code) | 🔴 | entities → `**text**` string | Pango markup / TextTag |
| Локация / контакт-карточка | 🔴 | типы в модели, нет UI | Render + share |
| GIF / animated emoji | 🔴 | модели частично | Playback |
| Телемост / звонки | 🔴 | label + Mute/Video/End без эффекта | WebView/WebRTC или deep-link + return |
| Screen share | ⬜/🔴 | MessageType есть | Через Телемост |
| Multi-account | 🟡 | Account model + dropdown | Seamless switch, isolated cache |
| Offline / SQLite | 🔴 | `db.rs` schema ok, methods stub (`Ok("[]")`) | Реальный L2 SQLite + sync |
| Локализация ru/en | 🔴 | hardcoded RU + EN mixed | gettext/fluent |
| Accessibility / keyboard | 🟡 | roles частично | Full keyboard nav, AT-SPI |
| RTL | 🔴 | — | GTK direction |
| Автообновления клиента | 🔴 | — | Flatpak/deb update channel |
| E2E encryption | ⬜ | у YM обычно нет классического E2E | Не блокер паритета |
| Мобильные платформы | ⬜ | Linux-only by design | Ок |

---

## 4. P0 — блокеры ежедневного использования

Без закрытия этого блока приложение **не может** конкурировать даже с web.yandex.ru/chat в браузере.

### P0.1 Надёжная доставка сообщений end-to-end

**Сейчас:**
- Отправка через кастомный binary WS frame; optimistic `Message` без server ack.
- Hardcoded fallback `yuid` в `send_text_message` (`api/mod.rs`) — smell нестабильности auth/session.
- История: session cookies **или** search fallback; без `session.json`/login script история «может быть incomplete».

**Нужно:**
- [ ] Единый auth path: OAuth + cookies/session, которые WS/history реально принимают.
- [ ] Подтверждение `delivered` / `read` из push; статусы галочек не декоративные.
- [ ] Retry + outbox при offline; не терять текст при реконнекте.
- [ ] Убрать hardcoded yuid / fake timestamps; брать identity только из сессии.

### P0.2 Реальные desktop-уведомления

**Сейчас:** `src/ui/notifications.rs` — только `eprintln!`.  
TESTPLAN и SECURITY/ROADMAP ссылаются на `notify-rust` — **расхождение с кодом**.

**Нужно:**
- [ ] `notify-rust` или xdg-desktop-portal Notifications.
- [ ] Клик → фокус окна + открыть чат.
- [ ] Уважать mute чата и `notifications_enabled` из settings.
- [ ] Не спамить при активном открытом чате.

### P0.3 System tray и поведение «закрыть = свернуть»

**Сейчас:** `TrayHandle::init()` — пустой placeholder. Settings `minimize_to_tray` нечем реализовать.

**Нужно:**
- [ ] StatusNotifierItem / Ayatana / `ksni` (или portal).
- [ ] Меню: открыть / mute all / выход.
- [ ] Badge непрочитанных на иконке (если DE позволяет).

### P0.4 Корректные превью и типы сообщений

**Сейчас (видно на screenshot):**
- `[Вложение или системное сообщение]` вместо файла/системы.
- `No messages` (EN) в превью.
- `Группа · 0 участников` — participants не заполняются.

**Нужно:**
- [ ] Парсинг media/system/call/telemost/file → человекочитаемые превью («📷 Фото», «📎 report.pdf», «📞 Звонок»).
- [ ] Все UI-строки на RU (или i18n).
- [ ] Реальный count участников / last_message preview.

### P0.5 Контекстные действия чата — не log-only

`chat_list.rs` → `show_context_menu`: mute / pin / archive / mark_read / delete — **только log**.

**Нужно:**
- [ ] API + optimistic UI + persist state.
- [ ] Без этого правый клик = «фейковая функциональность» → потеря доверия.

### P0.6 Голос: play/record как в нормальном мессенджере

**Сейчас:**
- `VoiceMessagePlayer::toggle_play` — stub-анимация прогресса **без аудио**.
- Callback play → `log::info`.
- `send_voice_message` в core заливает файл, но шлёт **текст** `"Voice message (N.Ns)"` через WS — сомнительный протокол.

**Нужно:**
- [ ] GStreamer playback URL/file.
- [ ] Корректный voice message type в протоколе (не text-заглушка).
- [ ] Запись → upload → bubble с waveform → play.

### P0.7 Видео

**Сейчас:** `// TODO: Open video player` при клике.

**Нужно:**
- [ ] Inline/fullscreen player (GStreamer `gtk4` sink или внешний).
- [ ] Скачивание, длительность, постер.

---

## 5. P1 — паритет «хочу перейти с web/desktop»

После P0 — то, без чего всё ещё «игрушка», но уже можно жить.

### Messaging UX
- [ ] Пагинация истории (scroll up → load older); сейчас limit ~50.
- [ ] Черновики на чат (drafts).
- [ ] Undo send / delete — уже UI, проверить server consistency.
- [ ] Forward picker (выбор чата), не только callback.
- [ ] Link preview + кликабельные URL (Pango links / open in browser).
- [ ] Mentions autocomplete.
- [ ] Rich text rendering (не `**bold**` как plain).
- [ ] Copy message link / copy selection.
- [ ] Drag-drop файлов в окно чата; paste image из clipboard.

### Presence & sync
- [ ] Mark as read при открытии/просмотре (явный API).
- [ ] Unread badge на чатах и глобальный счётчик в header/tray.
- [ ] Online / last seen точность.
- [ ] Multi-device: не дублировать, не терять при switch account.

### Media quality
- [ ] Прогресс upload/download.
- [ ] Документы: иконка типа, размер, open / save as.
- [ ] Multi-image galleries в одном bubble.
- [ ] Сжатие/лимиты как у YM.

### Calls
- [ ] Минимум: «Открыть в браузере» с `xdg-open` на telemost URL **и** возврат фокуса.
- [ ] Лучше: WebKitGTK WebView с полноценным Telemost.
- [ ] Mute/Video/End должны управлять реальным call state (сейчас кнопки mute/video ничего не делают, End только closes window).

### Groups / channels
- [ ] Список участников с аватарами.
- [ ] Invite link, kick, roles, permissions.
- [ ] Channel post mode (автор, comments/threads).

### Search
- [ ] Global search → jump to message in chat.
- [ ] Filters: people / files / media / links.
- [ ] Recent searches.

### Settings (реально есть 3 boolean + store)
**Сейчас:** `dark_theme`, `notifications_enabled`, `minimize_to_tray`. UI окна настроек по сути отсутствует как продукт.

**Нужно:**
- [ ] Окно настроек: уведомления, звук, тема, поведение закрытия, аккаунты, кэш, о прокси, о приложении.
- [ ] Hotkeys list.
- [ ] «Выйти из аккаунта» / «удалить локальные данные».

---

## 6. P2 — polish до «приятно, как у Яндекса»

- [ ] Визуальный язык ближе к YM (bubbles, denseness, sidebar, empty states, skeleton already partial).
- [ ] Анимации: new message, send, reaction pop.
- [ ] Emoji picker: recent + skin tones + search (сейчас большой static set — ок как база).
- [ ] Sticker animated (Lottie/WebP) + recent.
- [ ] High-DPI polish (классы есть, нужна проверка на 2x/3x).
- [ ] Full keyboard: ↑ edit last, Esc cancel reply, Ctrl+F in-chat, j/k navigation optional.
- [ ] Accessibility: screen reader labels на всех actions.
- [ ] i18n ru/en (fluent/gettext).
- [ ] Onboarding first-run.
- [ ] Crash reports / user-visible errors (не только log).
- [ ] Auto-update / канал релизов (deb + optional Flatpak).

---

## 7. P3 — «вау» и enterprise (не мешает старту)

- [ ] Multi-account polish (avatars, reorder).
- [ ] Corporate auth-proxy polish (уже есть задел).
- [ ] Translation button reliability.
- [ ] Bots advanced (webhooks UI — no; inline games — later).
- [ ] Scheduled with calendar UI.
- [ ] Export chat / backup.
- [ ] Plugin/scripting — out of scope.

---

## 8. Расхождение документации и кода (credibility debt)

| Утверждение | Реальность |
|-------------|------------|
| Desktop notifications (notify-rust) | `eprintln` |
| System tray | empty struct |
| Video playback support (Sprint 8 checklist) | TODO в chat_view |
| Voice playback GStreamer | stub progress animation |
| SQLite caching «complete» (commit message vibe) | `cache_chats`/`get_cached_chats` no-op |
| Chat context mute/pin/archive | log only |
| Telemost call window | URL label + non-functional Mute/Video |
| TESTPLAN §5–6 | Не соответствуют реализации |
| ROADMAP «Sprint 7–12 ✅» | Много UI-shells без production-grade backend |

**Рекомендация:** в README/ROADMAP помечать фичи честно:

- `shipped` — E2E, пользовались сами неделю  
- `beta` — работает на happy path  
- `ui-only` / `stub` — не вводить в заблуждение

Иначе contributors и пользователи теряют доверие быстрее, чем растёт feature list.

---

## 9. Технический долг, влияющий на UX

| Проблема | Где | Риск |
|----------|-----|------|
| Dual path history (session vs OAuth search) | `api/mod.rs` | Неполная история, разное качество |
| WS cookies из `session.json` | WS connect | «Вошёл через OAuth, но realtime мёртв» |
| Optimistic send без ack | WS send | Дубликаты / потерянные |
| Entity → markdown string | `apply_entities` | Пользователь видит `**жирный**`, не **жирный** |
| L2 cache = JSON files | core.rs | OK как MVP; SQLite stub вводит в заблуждение |
| `chat_view.rs` ~3k LOC | UI god-object | Сложно стабилизировать UX |
| `api/mod.rs` ~3.8k LOC | God-module | Сложно чинить протоколы |
| Hardcoded UA / region / version strings | send payload | Fingerprint drift vs official client |
| Смешение EN/RU строк | models + UI | «No messages» в RU UI |
| Warnings ~35 | cargo | Запах заброшенности |
| Мало/хрупкие automated tests | TESTPLAN manual | Регрессии в messaging |

---

## 10. Минимальный «хочу пользоваться» (MVP Daily Driver)

Цель: **заменить web-клиент на Linux для текста + файлов + уведомлений**. Не полный паритет.

### Definition of Done (пользовательский)

1. Запустил → вошёл → увидел **все** чаты с нормальными превью.  
2. Пишу текст → собеседник в официальном клиенте видит **сразу**.  
3. Собеседник пишет → у меня **тост-уведомление** и сообщение в открытом чате.  
4. Фото/файл: отправить и открыть/скачать.  
5. Mute и mark as read **работают**.  
6. Закрыл окно → остался в трее, не потерял сессию.  
7. Перезапуск → мгновенный UI из кэша + background refresh.  
8. Нет «0 участников», «No messages», «[Вложение…]» на основных типах.

### Инженерный backlog (порядок)

| # | Задача | Effort (грубо) | Impact |
|---|--------|----------------|--------|
| 1 | Notifications real | S | Trust |
| 2 | Tray + close behavior | S–M | Desktop feel |
| 3 | Chat actions API (mute/pin/archive/read) | M | Trust |
| 4 | Message previews + RU strings | S | First impression |
| 5 | Session/auth unify for WS+history | L | Reliability |
| 6 | Delivery/read ticks from server | M | Messaging confidence |
| 7 | Outbox + retry | M | Reliability |
| 8 | File send/open polish + DnD | M | Daily use |
| 9 | Voice real play/record protocol | L | Parity |
| 10 | Video player | M–L | Parity |
| 11 | Telemost deep-link or WebView | M | Calls |
| 12 | SQLite real cache (or delete stub) | M | Offline/startup |
| 13 | Settings window productized | S–M | Control |
| 14 | History pagination | M | Large chats |
| 15 | Docs honesty pass | S | Contributors |

**Оценка до Daily Driver:** 4–8 недель focused work (1 сильный dev), если API reverse-engineering не упрётся в стену.  
**Оценка до «почти как Яндекс»:** 4–9 месяцев + continuous protocol maintenance.

---

## 11. Что уже хорошо (не выкидывать)

Имеет смысл **не** переписывать с нуля:

- Слоистая архитектура Auth / HTTP / WS / UI.
- OAuth + multi-account foundation.
- Libadwaita shell, dark theme CSS, Paned layout.
- Virtualized chat list (ListView).
- Image viewer (zoom/swipe/download) — ближе к shipped.
- Emoji picker объём.
- Reverse-engineered send path (binary frames) — ценный R&D.
- Packaging (deb), CI, man page, auth-proxy.
- Модели данных богатые (polls, bots, threads…) — хорошая основа, когда UI перестанет врать.

---

## 12. Рекомендуемая политика продукта

1. **Stop feature sprawl.** Новые панели (bots/scheduled/saved) не трогать, пока P0 зелёный.  
2. **Dogfood weekly.** Если сами не сидите в клиенте 5 дней — feature не shipped.  
3. **Stub ban.** Любой `log::info!("action")` вместо API — либо API, либо скрыть из UI.  
4. **One happy path auth.** Либо session login встроен, либо OAuth полностью достаточен — не «run python script separately».  
5. **Honest README status.** Заменить «много ✅» на матрицу из §3.  
6. **Metrics of desire:**  
   - cold start → first message visible < 2s (cache)  
   - send latency p95 < 1s perceived  
   - 0 silent failures на mute/read/notify  
   - crash-free session > 8h

---

## 13. Итог одной фразой

> **До «уровня Яндекса» не хватает не ещё 10 панелей — не хватает надёжного realtime-messaging, desktop-интеграции (tray/notify), честных действий над чатами и работающих медиа/звонков; сейчас продукт выглядит feature-complete на бумаге и half-stub в руках, поэтому желания пользоваться нет.**

---

## 14. Связанные файлы (якоря аудита)

| Файл | Замечание |
|------|-----------|
| `src/ui/notifications.rs` | stub notifications |
| `src/ui/tray.rs` | empty tray |
| `src/ui/telemost.rs` | non-call UI |
| `src/ui/voice_message_player.rs` | stub playback |
| `src/ui/chat_view.rs` | video TODO; attachment fallback text |
| `src/ui/chat_list.rs` | context menu log-only |
| `src/core/db.rs` | SQLite no-op cache |
| `src/ui/settings.rs` | 3 settings, no product UI |
| `src/api/mod.rs` | WS/history/send complexity; hardcoded yuid |
| `src/models/mod.rs` | `"No messages"` EN preview |
| `ROADMAP.ru.md` | overstated completion |
| `TESTPLAN.md` | tests behaviors that don't exist |
| `screenshot.png` | visual evidence of preview/participants issues |

---

*Документ живой: обновлять статусы в §3 по мере закрытия P0/P1. Не раздувать новыми «спринтами ради спринтов».*
