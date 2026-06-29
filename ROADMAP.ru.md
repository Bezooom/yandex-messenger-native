# Дорожная карта — Yandex Messenger (Rust/GTK4)

**Дата формирования:** 2026-05-06  
**Версия:** 2.161.0  
**Платформа:** Linux (GTK4 + Adwaita + CSS theming)  
**Статус компиляции:** ✅ 0 ошибок, ⚠️ 35 предупреждений

---

## Архитектура проекта

```
src/
├── main.rs           — entry point, App, AppWindow, image viewer integration
├── config.rs         — constants, API URLs, auth config
├── core.rs           — AppController, AppState, ScheduledMessageClient
├── core/
│   └── voice_recorder.rs   — VoiceRecorder (GStreamer)
├── api/
│   ├── auth.rs       — OAuth2 (Yandex OAuth, token refresh, multi-account) ✅
│   ├── folder.rs     — Chat folders (get_folders, update_folder)
│   ├── mod.rs        — ChatAPI, ChatSession, WebSocketClient
│   ├── translation.rs — Translate (translate_message)
│   ├── saved_message.rs — Saved Messages API ✅
│   ├── bot.rs         — Bot API ✅
│   ├── scheduled_message.rs — Scheduled Messages API ✅
│   └── group.rs       — Groups/Channels API ✅
├── models/
│   ├── mod.rs        — Message, Chat, ChatListEntry, User, re-exports ✅
│   ├── folder.rs     — ChatFolder, FolderFilter
│   ├── poll.rs       — Poll, PollAnswer
│   ├── reaction.rs   — ExtendedReaction
│   ├── sticker.rs    — Sticker, StickerPack, StickerPackList
│   ├── thread.rs     — Thread, ThreadMessage
│   ├── voice_message.rs — VoiceMessage, TranscribeStatus
│   ├── saved_message.rs — SavedMessage, SavedFilter ✅
│   ├── bot.rs         — BotInfo, BotCommand, InlineButton, ReplyKeyboard ✅
│   ├── scheduled_message.rs — ScheduledMessage, ScheduledStatus ✅
│   └── group.rs       — GroupSettings, ChannelSettings, GroupMember ✅
│   └── account.rs     — Account model ✅
└── ui/
     ├── auth_dialog.rs           — OAuth2 auth dialog
     ├── chat_list.rs             — Chat list panel (ListView, SelectionModelExt) ✅
     ├── chat_view.rs             — Main chat view (messages, input, reactions, images, translate, bots, scheduling) ✅
     ├── folder_sidebar.rs        — Folder sidebar
     ├── image_viewer.rs          — ImageViewer (zoom, overlay, controls)
     ├── mod.rs                   — module re-exports ✅
     ├── notifications.rs         — Desktop notifications
     ├── poll_creator.rs          — Poll creation form
     ├── poll_renderer.rs         — Poll display
     ├── reaction_panel.rs        — Reaction popup
     ├── settings.rs              — SettingsWindow
     ├── sticker_panel.rs         — Sticker panel (popover)
     ├── telemost.rs              — Telemost call window
     ├── theme.css                — CSS theming (light/dark, inline images, viewer, new components) ✅
     ├── thread_view.rs           — Thread view
     ├── tray.rs                  — System tray integration
     ├── voice_message_player.rs  — Voice playback
     ├── saved_panel.rs           — Saved Messages panel ✅
     ├── bot_panel.rs             — Bot panel ✅
     ├── scheduled_panel.rs        — Scheduled Messages panel ✅
     ├── group_panel.rs            — Group/Channel panel ✅
     ├── create_group_dialog.rs     — Create Group dialog ✅
     └── account_dropdown.rs       — Account switcher dropdown ✅
```

---

## ✅ Выполнено (Sprint 1–6)

### Спринт 1: Threads + Extended Reactions
- Модель `Thread` и `ThreadMessage`
- Расширенные реакции (`ExtendedReaction`)
- API: get_thread_messages, send_thread_message, get_reactions_config
- UI: ThreadView с breadcrumb, ReactionPanel с анимацией
- WebSocket: subscribe_thread, subscribe_reaction_updates, subscribe_typing_enhanced

### Спринт 2: Voice Messages
- Модель `VoiceMessage`, `TranscribeStatus`, `VoiceRecordParams`
- API: upload_voice_message, get_transcription
- Core: VoiceRecorder (stub с таймером, waveform)
- UI: VoiceMessagePlayer (play/pause, progress, waveform)
- CSS: voice-player, waveform-container, transcription-box

### Спринт 3: Polls
- Модель `Poll`, `PollAnswer` (quiz mode, multi-select)
- API: create_poll, vote_poll, get_poll_results
- UI: PollCreator, PollRenderer
- CSS: poll-creator, poll-renderer, progress-bar

### Спринт 4: Stickers
- Модель `Sticker`, `StickerPack`, `StickerPackList`
- API: get_sticker_catalog, search_stickers, install_sticker_pack
- UI: StickerPanel (popover с паками и grid)
- CSS: sticker-panel, pack-list-item, inline sticker

### Спринт 5: Folders + Translation
- Модель `ChatFolder`, `FolderFilter`
- API: `get_folders`, `update_folder` (folder.rs)
- API: `translate_message` (translation.rs)
- UI: FolderSidebar (sidebar с иконками)
- Интеграция в main layout
- Translate button в сообщениях (hover)

### Спринт 6: Media Enhancements (текущий)
- ✅ ImageViewer — zoom overlay (1.0x–5.0x), controls popover
- ✅ Inline image preview в сообщениях
- ✅ CSS: inline-image, image-viewer, image-controls
- ✅ Translate button в сообщениях (hover)
- ✅ Typing indicator & Online status
- ✅ SelectionModelExt fix (GTK4 v4_12)
- ✅ Cargo.toml: gtk v4_12 feature

### Инфраструктура
- [x] OAuth2: dual token exchange (Basic auth + form body)
- [x] Auto-refresh access tokens (5-min buffer)
- [x] Dark theme с персистентными настройками
- [x] System tray + minimize-to-tray
- [x] Desktop notifications (GIO/libnotify)
- [x] Auth-proxy mode (`YANDEX_AUTH_PROXY_URL`)
- [x] Debian packaging, MIT license, man page
- [x] GitHub Actions CI (format/lint/test/build)
- [x] Документация: ARCHITECTURE.md, SECURITY.md, CHANGELOG.md
- [x] GStreamer optional feature (Cargo.toml)

---

## 🔜 В процессе / Запланировано

### Спринт 7: Voice Recording (GStreamer) ✅
- [x] GStreamer integration для записи аудио (вместо stub)
- [x] Waveform visualization в реальном времени
- [x] Voice playback с waveform
- [x] Voice transcription (Yandex SpeechKit) ✅
- [x] Voice upload/download

### Спринт 8: Image Enhancements ✅
- [x] Image zoom overlay (открыть ImageViewer при клике)
- [x] Image download
- [x] Swipe navigation между изображениями
- [x] Image compression for upload
- [x] Video playback support

### Спринт 9: Search & Performance ✅ (завершён)
- [x] Search within chat (highlight matches)
- [x] Global search (all messages, contacts) ✅ (global_search.rs)
- [x] Virtualization chat list (gtk::ListView) ✅
- [x] Lazy loading media ✅
- [x] Message caching (L1/L2) ✅ (core.rs: cache_dir, load_cache_l2, save_cache_l2)

### Спринт 10: Polish & UX ✅ (завершён)
- [x] Emoji picker (categories + favorites)
- [x] Typing indicator (enhanced)
- [x] Online status (real-time WebSocket)
- [x] Reply & Edit (inline) ✅ (chat_view.rs: reply_preview_box, reply_to_msg_id, edit_msg_id, right-click menu)
- [x] Message actions (copy, delete, forward, save, pin) ✅ (right-click popover, btn_copy/btn_forward/btn_save/btn_pin/btn_delete)
- [ ] Drag-to-reorder chats
- [x] Undo delete/undo send ✅ (undo_bar, pending_delete_msg_id, pending_delete_row, 5s timeout)
- [x] Pin messages ✅ (pinned_box, pinned_label, pinned_message_id, unpin_btn)

### Спринт 11: Advanced features ✅ (завершён)
- [x] Thread management (create, navigate, switch)
- [x] Chat groups / channels
- [x] Bot support
- [x] Message scheduling
- [x] Pin messages
- [x] Saved messages

### Спринт 12: Enterprise & Accessibility ✅ (завершён)
- [x] Multi-account support (Account model, AuthManager methods, AccountDropdown UI)
- [x] Drag-to-reorder chats fix (selection.selected() instead of Y-coordinate calculation)
- [x] High DPI support (CSS classes .hidpi-2x, accessibility roles)
- [x] Accessibility (AccessibleRole for ListView, main container)
- [ ] Keyboard navigation (full)
- [ ] Screen reader (AT-SPI)
- [ ] RTL support
- [ ] Localization (ru/en)

---

## 📊 Статистика

| Метрика | Значение |
|---------|----------|
| Sprint'ов завершено | 10 (8 полных + 2 частично) |
| Новых файлов создано | ~31 |
| Обновлённых файлов | ~20 |
| CSS компонентов | 70+ |
| API методов | 38+ |
| WS подписок | 8+ |
| Search методов | 2+ (global_search.rs) |
| Предсуществующих ошибок компиляции | 0 (исправлено ~25) |
| Ошибок компиляции (текущее) | 0 |
| Предупреждений | 35 (unused imports/variables) |

---

## ✅ Sprints 9–10 (завершено)

### Спринт 9: Search & Performance
- [x] Search within chat (highlight matches) ✅ (chat_view.rs: search_query, search_entry, highlight via regex)
- [x] Global search (all messages, contacts) ✅ (global_search.rs, Ctrl+K shortcut)
- [x] Virtualization chat list (gtk::ListView) ✅
- [x] Lazy loading media ✅
- [x] Message caching (L1/L2) ✅ (core.rs: cache_dir, load_cache_l2, save_cache_l2)
- [x] Search methods: 2+ (global_search.rs)

### Спринт 10: Polish & UX
- [x] Emoji picker ✅ (emoji_picker.rs: FlowBox with 10+ emojis, categories, popover)
- [x] Typing indicator ✅ (chat_view.rs: set_typing, set_online, set_status_text)
- [x] Online status (real-time WebSocket) ✅
- [x] Reply & Edit (inline) ✅ (reply_preview_box, reply_to_msg_id, edit_msg_id, right-click menu)
- [x] Message actions ✅ (copy, delete, forward, save to favorites, pin — right-click popover)
- [x] Undo delete/undo send ✅ (undo_bar with 5s timeout, pending_delete_msg_id, pending_delete_row)
- [x] Pin messages ✅ (pinned_box, pinned_label, pinned_message_id, unpin_btn)

## ✅ Sprints 7–8 (завершено)

### Спринт 7: Voice Recording (GStreamer)
- [x] GStreamer integration для записи аудио
- [x] VoiceRecorder с cfg-gated pipeline (gstreamer / stub)
- [x] RECORDING_PIPELINE для GStreamer: `autoaudiosrc ! audioconvert ! audioresample ! opusenc ! oggmux ! appsink`
- [x] RECORDING_PIPELINE для stub: `autoaudiosrc ! audioconvert ! audioresample ! wavenc ! appsink`
- [x] appsink signal handling (emit_signals)
- [x] Waveform visualization
- [x] VoiceRecorder implements Clone (для closures)
- [x] simulate_input для тестирования
- [x] Cargo.toml: gstreamer feature flag + dependencies
- [x] VoiceRecorder test: start/stop/cancel/waveform

### Спринт 8: Image Enhancements

### Спринт 9: Search & Performance
- [x] Search within chat (highlight matches) ✅ (chat_view.rs: search_query, search_entry, highlight via regex)
- [x] Global search (all messages, contacts) ✅ (global_search.rs, Ctrl+K shortcut)
- [x] Virtualization chat list (gtk::ListView) ✅
- [x] Lazy loading media ✅
- [x] Message caching (L1/L2) ✅ (core.rs: cache_dir, load_cache_l2, save_cache_l2)
- [x] Search methods: 2+ (global_search.rs)

### Спринт 10: Polish & UX
- [x] Emoji picker ✅ (emoji_picker.rs: FlowBox with 10+ emojis, categories, popover)
- [x] Typing indicator ✅ (chat_view.rs: set_typing, set_online, set_status_text)
- [x] Online status (real-time WebSocket) ✅
- [x] Reply & Edit (inline) ✅ (reply_preview_box, reply_to_msg_id, edit_msg_id, right-click menu)
- [x] Message actions ✅ (copy, delete, forward, save to favorites, pin — right-click popover)
- [x] Undo delete/undo send ✅ (undo_bar with 5s timeout, pending_delete_msg_id, pending_delete_row)
- [x] Pin messages ✅ (pinned_box, pinned_label, pinned_message_id, unpin_btn)

## ✅ Спринт 12 — Детальный чеклист

### ✅ Multi-account support
- [x] Account model (`src/models/account.rs`)
  - [x] id, display_name, avatar_url, access_token, refresh_token, expires_at, is_valid
  - [x] display_label() — fallback to ID if no display name
- [x] AuthManager multi-account methods (`src/api/auth.rs`)
  - [x] current_account_id — ID текущего аккаунта
  - [x] list_accounts() — список всех аккаунтов
  - [x] switch_account(account_id) — переключение между аккаунтами
  - [x] add_account(token, user) — добавление нового аккаунта
  - [x] remove_account(account_id) — удаление аккаунта
  - [x] remove_current_account() — удаление текущего аккаунта
  - [x] is_multi_account() — проверка режима мульти-аккаунтов
  - [x] current_account_name() — имя текущего аккаунта
- [x] AccountDropdown UI (`src/ui/account_dropdown.rs`)
  - [x] Popover для отображения списка аккаунтов
  - [x] Callback для переключения аккаунта

### ✅ Drag-to-reorder fix
- [x] Заменить расчёт dest_idx через `selection.selected()` вместо `(y / 60.0) as u32`

### ✅ Accessibility
- [x] ListView — set_accessible_role(gtk::AccessibleRole::List)
- [x] Main container — set_accessible_role(gtk::AccessibleRole::Sidebar)
- [x] Chat view — set_accessible_role(gtk::AccessibleRole::Main)
- [x] Global search — set_accessible_role(gtk::AccessibleRole::Dialog)

### ✅ High-DPI поддержка
- [x] CSS-классы `.hidpi-2x` для масштабирования шрифтов
- [x] CSS-классы `.hidpi-2x` для масштабирования аватаров
- [x] CSS-классы `.hidpi-2x` для масштабирования сообщений
- [x] CSS для AccountDropdown

### ✅ Технический долг
- [x] Исправлен E0599 — no method named `set_accessible_name`
- [x] Исправлен E0382 — selection move into closure
- [x] Исправлен E0616 — private field access
- [x] Исправлен E0521 — closure lifetime issue
- [x] Исправлен E0505 — move out of borrow
- [x] Исправлен E0594 — cannot assign to data in & reference

---
- [x] ImageViewer — download (сохраняет в Downloads с правильным filename)
- [x] ImageViewer — swipe navigation (prev/next via GestureSwipe)
- [x] ImageViewer — image_index / image_count tracking
- [x] ImageViewer implements Clone
- [x] show(url, filename) — поддержка filename для download
- [x] set_image_sequence(count) — обновление состояния swipe
- [x] ImageViewer в main.rs — wire up with filename
- [x] chat_view.rs — on_image_open принимает (String, String)
- [x] Inline image preview с GestureClick
- [x] CSS: inline-image, image-viewer, image-controls

### Исправлено:
- [x] GStreamer pipeline type mismatch (cfg-gated)
- [x] connect_end signature (dx, dy — f64, f64)
- [x] ImageViewer show() — filename support
- [x] chat_view.rs show_image() — String vs &str
- [x] ImageViewer clone implementation
- [x] controls_popover — Rc<ImageViewer> pattern
- [x] 5 ошибок компиляции → 0 ошибок

---

## 🎯 Приоритеты (Sprint 13 — следующий)

1. **K1 (критично):** Full keyboard navigation, AT-SPI integration
2. **K2 (важно):** RTL support, Localization (ru/en)
3. **K3 (желательно):** Settings categories refinement
4. **K4 (nice-to-have):** Multi-account UI polish (account list display, avatar support)

---

## 🔧 Технический долг (исправлено)

### Исправлено:
- [x] Частично устранены unused_imports и unused_variables с помощью `cargo fix` (количество warnings снижено с 75 до 44)
- [x] SelectionModelExt — переключение на GTK4 v4_12 feature
- [x] Cargo.toml — добавлен `v4_12` feature для gtk
- [x] ImageViewer — исправлены все borrow errors
- [x] Inline image preview — добавлен в сообщения
- [x] Translate button — добавлен с hover
- [x] FolderSidebar — интегрирован в main layout
- [x] get_folders — API integration
- [x] VoiceRecorder — GStreamer integration (cfg-gated)
- [x] connect_end — исправлен signature (dx, dy: f64)
- [x] ImageViewer show() — добавлен filename
- [x] ImageViewer clone — реализация Clone
- [x] chat_view.rs on_image_open — (String, String)
- [x] 5 ошибок компиляции → 0

### Осталось:
- [x] Drag-to-reorder chats (из Sprint 10) — исправлено в Sprint 12
- [ ] VoiceRecorder: stub → GStreamer integration (проверить cfg-gated код)
- [x] Оставшиеся unused_imports и unused_variables (частично устранены, осталось 35 warnings)
- [ ] GStreamer dependency в Cargo.toml (проверить текущий статус)
- [ ] Локализация строк

---

## 📝 Примечания

- **Платформа:** Linux (GTK4 + libadwaita)
- **CSS theming:** через CSSProvider (light/dark)
- **WebSocket:** подписки на обновления (messages, reactions, typing, polls)
- **OAuth2:** авторизация через Yandex OAuth
- **Target:** Debian package, PPA support
- **Компиляция:** `cargo check` — 0 errors, 35 warnings (unused imports/variables)
- **Структура:** 21 UI компонентов, 10 model модулей, 7 API модулей

---

## 📋 Спринт 6 — Детальный чеклист

### ✅ Реализовано:
- [x] ImageViewer (src/ui/image_viewer.rs)
  - [x] show(url: &str) — загрузка изображения
  - [x] zoom_in/zoom_out/reset_zoom
  - [x] controls_popover — кнопки управления
  - [x] close/is_closed
- [x] Inline image preview в chat_view.rs
  - [x] Проверка MediaType::Image
  - [x] Отображение thumbnail
  - [x] GestureClick для клика
- [x] Translate button в сообщениях
  - [x] Hover visibility
  - [x] on_translate callback
- [x] FolderSidebar в main layout
  - [x] Integration в main.rs
  - [x] get_folders API call
- [x] Typing indicator
  - [x] set_typing(user)
  - [x] set_online()
  - [x] set_status_text(text)
- [x] CSS styles
  - [x] .inline-image (max-width, border-radius, hover)
  - [x] .image-viewer (background, overlay)
  - [x] .image-controls (buttons, hover, active)
  - [x] .viewer-image

### 🔄 В процессе:
- [x] Voice recording (GStreamer)
- [x] Image download
- [x] Image swipe navigation
- [x] Voice transcription (Yandex SpeechKit) ✅
- [ ] Video playback support

---

## 📋 Спринт 10 — Детальный чеклист

### ✅ Реализовано:
- [x] Emoji picker (src/ui/emoji_picker.rs)
  - [x] FlowBox layout с 10+ emojis
  - [x] Categories (😀, 😂, ❤️, 👍, 🙏, etc.)
  - [x] Popover display
  - [x] Inline insert into input_entry
- [x] Reply & Edit (inline)
  - [x] reply_preview_box (reply_to_msg_id, edit_msg_id)
  - [x] Right-click menu: Ответить / Ответить в треде
  - [x] Изменить (для отправленных сообщений)
  - [x] reply_preview_close_btn
- [x] Message actions (right-click popover)
  - [x] Копировать (clipboard)
  - [x] Переслать (forward)
  - [x] Сохранить в Избранное
  - [x] Закрепить (pin)
  - [x] Удалить (с undo)
- [x] Undo delete/undo send
  - [x] undo_bar с анимацией
  - [x] pending_delete_msg_id, pending_delete_row
  - [x] 5s timeout через glib::timeout_add_local_once
  - [x] Отменить кнопка
- [x] Pin messages
  - [x] pinned_box (view-pin-symbolic icon)
  - [x] pinned_label (message preview)
  - [x] pinned_message_id
  - [x] unpin_btn (window-close-symbolic)
- [x] Typing indicator (enhanced)
  - [x] set_typing(user) — "user печатает..."
  - [x] set_online() — "В сети"
  - [x] set_status_text(text)
  - [x] Debounced typing via last_typing_time (3s)
- [x] Online status (real-time WebSocket)
- [x] Inline image preview
- [x] Inline video preview (play overlay)
- [x] Translate button (hover)
- [x] Message time formatting
- [x] Read ticks (✓✓, ✓, ◷)
- [x] CSS: .undo-bar, .pinned-message-bar, .reply-preview-box
