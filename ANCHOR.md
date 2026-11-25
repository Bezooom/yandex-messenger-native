# Anchored Summary — yandex-messenger-native

> Last updated: 2026-11-25 — release **2.176.0**.  
> [Русская версия](ANCHOR.ru.md)

## Goal

Ship a usable unofficial Linux desktop client for Yandex Messenger (Rust + GTK4 + Libadwaita), with honest docs: a feature is either end-to-end or hidden.

## Constraints & Preferences

- Status in README/CHANGELOG must match the code.
- Feature flags hide stubs: `YM_ENABLE_VOICE`, `YM_ENABLE_TELEMOST_UI` (default off).
- Verify with `cargo build` and `cargo test --all-targets`.

## Progress (as of 2.176.0)

### Done

- Bootstrap chat list: `members_count`, `private`, pin/mute buckets, `members` → `Participant` with names.
- Real chat timestamps from `create_timestamp` / `version`; previews no longer collapse to "now".
- Unread lives in app state (window title, tray and list agree); own echo is ignored.
- Telemost home hub (new chat / new call / join by link) + Все / Личные / Новые filters with matching empty states.
- Window title follows the official «Яндекс Телемост — N новое сообщение» shape.
- Emoji and stickers live in a right-hand dock with tabs; the image viewer fits photos to the window and zooms for real.
- Stickers send end-to-end: `Image` + sticker CDN `FileInfo.Id2`; official clients render them; `stickers/` ids map to direct CDN URLs.
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
  The feature itself compiles green against the local dev prefix (see `YM-DEV` note below); runtime needs
  `gstreamer1.0-nice`, which is installed on the dev machine.
- Goloom signaling has two wire codecs (protobuf = APK, JSON envelope = desktop client, see
  `TELEMOST_MAC_REVERSE.md`); `config::GOLOOM_WIRE` picks one. Which one the live server accepts for our
  client identity is unverified; camelCase payload keys and exact `type` tags need a live-session smoke.
- Telemost meetings HTTP paths are best-effort against a live server.
- Chat-action RPC names are best-effort.
- Full group/channel parity, a11y, i18n beyond RU previews.

## YM-DEV: dev prefix without sudo

GTK/WebKit/GStreamer headers on this box come from `-dev` debs unpacked to `/tmp/gtk-prefix`
(`apt-get download` + `dpkg -x`), resolve missing `.pc` deps in a loop; `.so` link stubs are symlinked
from system runtime libs into `/tmp/ym-liblinks`. The stubs must point at runtime `.so` files, never
static `.a` archives (static libdbus drags undefined `sd_listen_fds`/`sd_is_socket` into the final link).
Build:
`PKG_CONFIG_PATH=/tmp/gtk-prefix/usr/lib/x86_64-linux-gnu/pkgconfig RUSTFLAGS="-L /tmp/ym-liblinks" cargo build --features gstreamer`.
The prefix lives in /tmp — rebuild it after a reboot.

## Verification floor

- `make check` (clippy `-D warnings` + `cargo fmt --check`) is green — it was chronically red until 2026-11-25 (fixed mechanically: clippy --fix, dead-code removal, Rc migration for main-thread UI handles, file-level `#![allow]` only for GTK `type_complexity`). `cargo test --bin yandex-messenger` passes (73 tests, 1 ignored).
- This machine has GTK4 runtime but no `gtk4.pc`/headers; the check ran against pkg-config files extracted from Ubuntu `.deb` packages into `/tmp/gtk-prefix` with `-L /tmp/ym-liblinks`. A normal dev box should `apt install libgtk-4-dev libadwaita-1-dev libwebkitgtk-6.0-dev` instead.

## Next Steps

- Gate B/C from [`ROADMAP_DETAILED.md`](ROADMAP_DETAILED.md): reliable daily media, then voice/video.
- Native Telemost signaling (see [`TELEMOST_IMPLEMENTATION_PLAN.md`](TELEMOST_IMPLEMENTATION_PLAN.md)).

## Relevant Files

- `README.md` — landing page (RU) + English summary
- `CHANGELOG.md` / `CHANGELOG.ru.md` — 2.165–2.175
- `src/api/session_store.rs`, `src/core/outbox.rs`, `src/core/drafts.rs`, `src/core/db.rs`
- `src/ui/theme.css`, `src/ui/chat_view.rs`, `src/ui/chat_list.rs`
