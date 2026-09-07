# Changelog

[Русская версия](CHANGELOG.ru.md)

All notable changes to this project will be documented in this file.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/).

Current release: **2.173.0** (2026-08-15).

## Unreleased

### Added
* **Live Telemost calls**: Goloom signaling (`wss://goloom.strm.yandex.net/join`, prost types from the APK schemas) + WebRTC via `webrtcbin` (two PCs: publish/subscribe, ICE trickle, server-provided STUN/TURN).
* **Call window**: states, timer, participant roster, mute/end, in-window peer video (RGBA frames → `MemoryTexture`), incoming-call ringing bar, copy-link, open-in-browser fallback.
* **Meetings Cloud API** using APK method names (`create_personal_meeting` / `start_meeting_call` / `end_personal_meeting` / `meeting_info[s]`); the chat call button creates a meeting and opens the window.
* System package `gstreamer1.0-nice` is required for ICE (otherwise webrtcbin creates no transports).
* **Screen sharing**: third m-line built dynamically on first enable (re-offer, no negotiation stall); portal capture on Wayland (`YM_SHARE_PORTAL`, `portal` feature, picker dialog) with ximagesrc fallback.
* **Local camera preview** (PiP) from the same pipeline.
* **Incoming calls**: invites from WS traffic (direct methods + links with call markers; a bare link never rings) raise the ringing window with accept via the shared join flow.
* **Voice transcriptions**: "Recognize speech" button with retry, tolerant response parsing.
* **Real voice messages**: Opus/OGG recording with waveform meter (appsink polling, EOS finalization), playbin player (play/pause, progress, single active), voice-type send with attachment instead of text placeholder.
* **In-chat video player**: inline (play/pause, scrub, time, EOS), frames via appsink→Picture.
* **Light Yandex theme by default** + dark night: tokens split into `theme-tokens-{night,light}.css`, live switch in settings; token-parity test catches CSS parser errors.

### Known limitations
* Meetings HTTP paths are best-effort, pending live-server verification (debug dumps included).

## 2.173.0 - 2026-08-15

### Added
- **Telegram Desktop night theme**: color tokens from `night.tdesktop-theme` (`dialogsBg`, `msgIn` / `msgOut`, blue send button).
- **nheko-style split**: dialog list ~32% width (max 420px), dense rows, 54px avatars.
- **Solid selection** (`dialogsBgActive`) instead of glow chrome.
- **Adaptive sidebar**: shrinks and clamps instead of overflowing.
- **Auto-scroll** to the latest message when opening a chat.

### Changed
- Default visual language is now Telegram/nheko night, not the older Obsidian-like glow theme.

### Known Limitations
- Voice messages and Telemost WebRTC remain stubs (hidden unless feature flags are set).
- Video player is not implemented.
- Chat-action RPC names are best-effort reverse-engineered; the local UI still updates.

---

## 2.172.0 - 2026-08-08

### Added
- **New brand icon** in Yandex style: yellow squircle, dark bubble, «Я»; hicolor 16–512 plus scalable SVG.
- **Reaction pop-in**: staggered scale/opacity animation on chips and picker buttons.
- **Pagination loader**: spinner + “Loading history…” when scrolling up.
- **Reduce animations** setting (`reduced_motion`) disables decorative CSS.
- Package `yandex-messenger-native_2.172.0-1_amd64.deb` + tar.gz.

---

## 2.171.0 - 2026-08-08

### Added
- **Skeleton loaders**: chat list (8 rows) and message feed (bubble placeholders) with shimmer.
- **Empty states**: welcome (no chat selected), empty conversation, no chats / no search results.
- **Stack transitions**: crossfade list ↔ skeleton ↔ empty; message fade-in / slide-up / pop-in.

---

## 2.170.0 - 2026-08-08

### Added
- **Delivery / read ticks**: ◔ pending → ✓ delivered → ✓✓ read; parsed from history/WS; live update without a full refresh when possible.
- **Design v3**: YM-like palette, gradient sent bubbles, meta footer, ticks, denser list, composer/header polish, file cards.

---

## 2.169.0 - 2026-08-08

### Added
- **Download / Open** on document/file attachments: download to `~/Downloads`, open via `xdg-open`.
- **SQLite cache** (`cache.db`): upsert chats and messages; cold start from SQLite; fallback on network errors.

---

## 2.168.0 - 2026-08-08

### Added
- **History pagination**: scroll up → `load_older_messages` (session RPC `from_message_id`).
- **Drafts**: per-chat text in `drafts.json`, saved on chat switch, cleared after send.
- **File drag-and-drop** into the chat window + **Ctrl+V** for clipboard images → attach pipeline.

---

## 2.167.0 - 2026-08-08

### Added
- **In-login session capture** (no Python): WebView login stores Passport cookies (`session.json`) + CSRF; loads `yandex.ru/chat` when needed.
- **Outbox**: unsent messages written to disk (`outbox.json`), retried on WS Connected and every 45s; UI shows a pending bubble (`sent=false`).

### Changed
- `HttpClient.session_cookies` — Mutex + `reload_session()` / `apply_session()`.
- Startup hint when a session is missing.

---

## 2.166.0 - 2026-08-08

### Added
- **Reply / Edit**: replies and edits are sent to the server (`send_text_message_ex`).
- **Desktop notifications** via `notify-rust` (per-chat mute + global settings flag).
- **System tray** (StatusNotifierItem / `ksni`): show, quit, unread badge, close-to-tray.
- **Chat actions**: mark read / mute / pin / archive / delete (session RPC, best-effort) + local UI.
- **Mark as read** when opening a chat.
- **Settings window**: notifications, tray, dark theme.
- **RU message previews** (`📷 Photo`, `No messages`, …).
- **Phase 2 Telemost shell**: `webkit6` WebView (`in_app_webview`), Mute / Video / End bar, participant sidebar, fallback “Open in browser”.

### Fixed
- Hardcoded yuid fallback removed — send fails explicitly without a session yuid.
- Telemost UI build (GTK4 children API).

### Known Limitations
- `session.json` is still required for full history/WS/files.
- Voice / video / Telemost WebRTC remain stubs.
- Chat-action RPC names may not match the server — the UI still updates locally.

---

## 2.165.0 - 2026-08-08

### Added
* **Feature Flags for Voice and Telemost**:
  - Added `YM_ENABLE_VOICE` environment variable to show/hide voice message UI elements (waveform, play/record buttons).
  - Added `YM_ENABLE_TELEMOST_UI` environment variable to show/hide Telemost (video calls) UI elements in the chat header.
  - When a flag is off, the corresponding stub UI elements are hidden so users see a cleaner interface.
* **Stubs marked with comments**:
  - All stub implementations are now annotated with `// STUB:` comments for easy identification during development.
* **Smoke tests for message preview and settings**:
  - Added smoke tests covering message preview rendering and settings persistence.
* **Code cleanup**:
  - Removed unused imports across the codebase.
  - Added `#[allow]` attributes to resolve lint warnings.

### Changed
* 0 build warnings, 13 tests passing.

### Known Limitations
* **Voice messages**: stub (hidden by default, enable with `YM_ENABLE_VOICE`)
* **Video calls (Telemost)**: stub (hidden by default, enable with `YM_ENABLE_TELEMOST_UI`)
* **Notifications**: stub (stdout/stderr only, no desktop notifications)
* **System tray**: stub (not implemented)
* **Chat list context menus**: stub (not implemented)
* **File upload/download**: not implemented
* **Call history**: not implemented
* **Message reactions**: not implemented

---

## 2.162.0 - 2026-06-29

### Added
* **Libadwaita Integration**:
  - Migrated the application structure to use `libadwaita` (`adw::Application` / `adw::ApplicationWindow`).
  - Replaced standard avatar placeholders with `adw::Avatar`, offering automatic color palettes and initials support.
  - Added `adw::HeaderBar` to the top of the interface.
* **Chat List Enhancements**:
  - Implemented in-memory avatar texture caching (`AVATAR_CACHE`), eliminating scrolling flicker and redundant downloads.
  - Added pinned chat indicators (pin icon 📌) to list rows.
  - Implemented automatic sorting: pinned chats always reside at the top of the list, followed by other chats sorted by the last message timestamp.
* **Editor & Message Input**:
  - Replaced the simple entry field with a multi-line `TextView` inside a `ScrolledWindow` with adaptive height limits (up to 120px).
  - Configured message submission on `Enter` and new line insertion on `Shift+Enter`.
  - Added a quick edit feature: pressing the `Up` arrow when the input is empty instantly edits the last sent message.
* **Chat & History Improvements**:
  - Implemented local message row caching (`message_rows`) to optimize rendering and avoid redundant UI rebuilds.
  - Integrated asynchronous JSON-based L2 caching (`load_cache_l2_async` / `save_cache_l2_async`) running on a background thread.
  - Chat history displays immediately from the cache while background network synchronization runs.
  - Added automatic scrolling to the latest message on chat switching or when new messages arrive.
  - Added date separators ("Today", "Yesterday", and formatted date strings).
  - Added sticker support rendering images at 128x128px.
  - Added "emoji-only" mode (large 40px emojis without chat bubbles when sending 1 to 3 emojis).
  - Integrated attachment popups (file share, poll creation, scheduling) and header menus (chat info, notification toggle).

### Changed
* Migrated the sidebar and chat panels to `gtk::Paned` with adjustable boundaries and a compact sidebar mode (collapsing text when width is < 180px).
* Reduced default sidebar width from 320px to 260px.
* Configured fallback built-in mock stickers in case of server-side sticker index failures.

### Fixed
* Fixed a critical type mismatch issue `E0308 mismatched types` in `src/ui/chat_list.rs` during callback setup.
* Solved GTK layout issues (infinite size calculation cycles expanding widgets to `1048576px`) by configuring `hexpand`/`vexpand` options.
* Corrected message dispatching and callback binding in `ChatView` via `bind_callbacks`.

---

## 2.160.0 - 2026-05-10

### Changed
* **Non-blocking OAuth Dialog**:
  - Re-routed the local callback server to run on a background thread, preventing GTK main loop freezes.
  - The authorization button updates to "Awaiting approval..." with a loading `Spinner` while waiting for login.
  - Increased the authorization timeout to 180 seconds.
  - Correctly handled Implicit Grant flow token parsing from URL fragments.
* **Streamlined Login UI**:
  - Moved the manual token entry field under a collapsed `Expander`.
* **Visual Styling**:
  - Added button gradients, radial lighting to the login panel background, and soft shadows on entry fields.

### Fixed
* Fixed panics and Tokio runtime initialization issues in `src/ui/auth_dialog.rs`.

---

## 2.158.0 - 2026-05-04

### Added
* **Search & Performance**:
  - Added lazy loading for media attachments.
  - Implemented a global message and contact search overlay (`GlobalSearch`), triggered with `Ctrl+K`.
  - Added in-memory message caching (L1) with background server synchronization.

---

## 2.156.0 - 2026-04-25

### Added
* Drafted initial architecture documentation (`ARCHITECTURE.md`) and security policy (`SECURITY.md`).
* Implemented automatic access token refresh 5 minutes before expiration.
