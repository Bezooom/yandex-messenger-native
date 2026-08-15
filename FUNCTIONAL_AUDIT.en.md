# Functional audit (what actually works)

> ⚠️ **Historical snapshot from 2026-08-08 (tree ~2.165).** Current status is in [README.md](README.md) and [CHANGELOG.md](CHANGELOG.md) for **2.173.0**. Original Russian audit: [FUNCTIONAL_AUDIT.md](FUNCTIONAL_AUDIT.md).

**Snapshot date:** 2026-08-08  
**Tree at audit time:** 2.165.0 + uncommitted changes  
**Method:** code review, `cargo check` / `cargo test`, README/CHANGELOG vs `src/`  
**Related:** [`GAP_AUDIT.md`](GAP_AUDIT.md), [`ROADMAP_DETAILED.md`](ROADMAP_DETAILED.md)

The original audit found:

- Build was briefly broken by GTK3 APIs in `telemost_window.rs` (fixed in-tree).
- Notifications, tray, chat context menus, and SQLite were still stubs.
- File attach was wired but not E2E-verified; Telemost was a placeholder (no WebRTC).
- Session cookies were required for history/send/files.

**Superseded by 2.173.0:** notifications, tray, SQLite cache, session-in-login, outbox, drafts, pagination, Download/Open, DnD/paste, delivery ticks, settings, night theme.  
**Still true:** voice/video/Telemost WebRTC are stubs; chat-action RPC names are best-effort.

Do not treat the matrices below the banner in the Russian file as current — they describe 2.165.
