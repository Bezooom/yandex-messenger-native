# Changelog

All notable changes to this project will be documented in this file.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/).

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
