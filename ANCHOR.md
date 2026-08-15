# Anchored Summary — yandex-messenger-native

> Last updated: 2026-08-15 — release **2.173.0**.  
> [Русская версия](ANCHOR.ru.md)

## Goal

Ship a usable unofficial Linux desktop client for Yandex Messenger (Rust + GTK4 + Libadwaita), with honest docs: a feature is either end-to-end or hidden.

## Constraints & Preferences

- Status in README/CHANGELOG must match the code.
- Feature flags hide stubs: `YM_ENABLE_VOICE`, `YM_ENABLE_TELEMOST_UI` (default off).
- Verify with `cargo build` and `cargo test --all-targets`.

## Progress (as of 2.173.0)

### Done

- Session-in-login (WebView → `session.json` + CSRF), no Python required for the happy path.
- Outbox (`outbox.json`) + drafts (`drafts.json`).
- History pagination, SQLite cache (`cache.db`) with cold-start hydrate.
- Files: upload/send, Download/Open, DnD, Ctrl+V images.
- Reply/edit, delivery/read ticks, mark-as-read, mute/pin/archive/delete UI.
- Desktop notifications (`notify-rust`) and system tray (`ksni`).
- Settings: notifications, tray, dark theme, reduce animations.
- UX: skeletons, empty states, reaction pop-in, pagination loader.
- Night theme (Telegram Desktop tokens) + nheko-style dense list.

### Still stubs / open

- Voice messages and video player.
- Telemost WebRTC (UI shell + WebView only).
- Chat-action RPC names are best-effort.
- Full group/channel parity, a11y, i18n beyond RU previews.

## Next Steps

- Gate B/C from [`ROADMAP_DETAILED.md`](ROADMAP_DETAILED.md): reliable daily media, then voice/video.
- Native Telemost signaling (see [`TELEMOST_IMPLEMENTATION_PLAN.md`](TELEMOST_IMPLEMENTATION_PLAN.md)).

## Relevant Files

- `README.md` / `README.ru.md` — feature matrix
- `CHANGELOG.md` / `CHANGELOG.ru.md` — 2.165–2.173
- `src/api/session_store.rs`, `src/core/outbox.rs`, `src/core/drafts.rs`, `src/core/db.rs`
- `src/ui/theme.css`, `src/ui/chat_view.rs`, `src/ui/chat_list.rs`
