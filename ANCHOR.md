# Anchored Summary — yandex-messenger-native

> Last updated: 2026-11-14 — release **2.175.3**.  
> [Русская версия](ANCHOR.ru.md)

## Goal

Ship a usable unofficial Linux desktop client for Yandex Messenger (Rust + GTK4 + Libadwaita), with honest docs: a feature is either end-to-end or hidden.

## Constraints & Preferences

- Status in README/CHANGELOG must match the code.
- Feature flags hide stubs: `YM_ENABLE_VOICE`, `YM_ENABLE_TELEMOST_UI` (default off).
- Verify with `cargo build` and `cargo test --all-targets`.

## Progress (as of 2.175.3)

### Done

- Bootstrap chat list: `members_count`, `private`, pin/mute buckets, `members` → `Participant` with names.
- Real chat timestamps from `create_timestamp` / `version`; previews no longer collapse to "now".
- Unread lives in app state (window title, tray and list agree); own echo is ignored.
- Telemost home hub (new chat / new call / join by link) + Все / Личные / Новые filters with matching empty states.
- Window title follows the official «Яндекс Телемост — N новое сообщение» shape.
- Emoji and stickers live in a right-hand dock with tabs; the image viewer fits photos to the window and zooms for real.
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

- Voice, video and Telemost media need the `gstreamer` feature; the default and CI binary do not compile it.
- Telemost meetings HTTP paths are best-effort against a live server.
- Chat-action RPC names are best-effort.
- Full group/channel parity, a11y, i18n beyond RU previews.

## Verification floor

- `cargo check --bin yandex-messenger` and `cargo test --bin yandex-messenger` pass (68 tests, 1 ignored).
- This machine has GTK4 runtime but no `gtk4.pc`/headers; the check ran against pkg-config files extracted from Ubuntu `.deb` packages into `/tmp/gtk-prefix` with `-L /tmp/ym-liblinks`. A normal dev box should `apt install libgtk-4-dev libadwaita-1-dev libwebkitgtk-6.0-dev` instead.

## Next Steps

- Gate B/C from [`ROADMAP_DETAILED.md`](ROADMAP_DETAILED.md): reliable daily media, then voice/video.
- Native Telemost signaling (see [`TELEMOST_IMPLEMENTATION_PLAN.md`](TELEMOST_IMPLEMENTATION_PLAN.md)).

## Relevant Files

- `README.md` / `README.ru.md` — landing pages (Best-README-Template style)
- `CHANGELOG.md` / `CHANGELOG.ru.md` — 2.165–2.175
- `src/api/session_store.rs`, `src/core/outbox.rs`, `src/core/drafts.rs`, `src/core/db.rs`
- `src/ui/theme.css`, `src/ui/chat_view.rs`, `src/ui/chat_list.rs`
