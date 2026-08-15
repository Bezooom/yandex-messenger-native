# Test Plan

[Русская версия](TESTPLAN.ru.md)

## Current Status (2.173.0)

| Phase | Status |
|---|---|
| 0 — Sedimentation | Complete |
| 1 — API Integration | Complete (session login, files, reply/edit) |
| 2 — Calls / Telemost | Shell only — WebRTC is still a stub |
| Desktop trust | Notifications, tray, settings, ticks |

**Tests:** `cargo test --all-targets` (13 unit/smoke tests at last check).

---

## 1. Static checks

```bash
cargo fmt --check
cargo clippy --all-targets
cargo test --all-targets
```

## 2. Smoke run

```bash
make run
```

Check:

- Application window opens with the night theme and nheko-style split.
- OAuth / WebView login writes `~/.config/yandex-messenger-native/session.json` with `Session_id`.
- Chat list loads; opening a chat shows history and auto-scrolls to the latest message.
- Outbox: disable network → send text → pending bubble → restore network → message leaves (or after reconnect).
- Pagination: long chat → scroll up → older history loads; “Loading history…” is visible.
- Drafts: type text → switch chat → come back → text is still there.
- DnD: drop a file into the chat → send; Ctrl+V for a clipboard image.
- Attachment: Download / Open → file appears in Downloads.
- Offline cold start: chat list / history from SQLite after a previous session.
- UX: startup skeleton → list; no chat selected → welcome; empty chat → empty conversation.
- Search with no hits → empty “Nothing found”.
- Reaction → chip pop-in; Settings → “Reduce animations” kills shimmer/pop-in.
- Opening a chat auto-scrolls to the last message.

## 3. Messaging flow

- Send text with the button and Enter.
- Sent message appears in the list.
- No crash when switching chats.
- Reply: peer sees a quote (session/WS).
- Edit: peer sees the changed text.
- Mark as read: badge drops when the chat is opened.

## 4. File flow

- Attach action enters the upload pipeline.
- Download API returns bytes.
- Download / Open works via `xdg-open`.
- Errors show a proper notification.

## 5. Calls

- Call action opens the Telemost window.
- End closes the window without errors.
- Built-in WebView (`in_app_webview`) loads the Telemost page.
- Mute / Video toggle state and visuals.
- Fallback: without `in_app_webview`, a dialog with “Open in browser” appears.
- Native WebRTC is **not** expected in 2.173.0.

## 6. Desktop behavior

- Notifications via `notify-rust` (not for the currently focused chat; mute is respected).
- Dark / night theme applies when `dark_theme = true`.
- Close behavior: with `minimize_to_tray` the window hides, the tray stays.
- Tray: Show / Quit, unread tooltip.
- Settings: notifications / tray / dark theme / reduce animations.

## 7. Packaging and CI

- `debuild -us -uc` succeeds.
- `.github/workflows/ci.yml` passes on a clean runner.
- `make dist` produces `yandex-messenger-native_2.173.0-*`.
