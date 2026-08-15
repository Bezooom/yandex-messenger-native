# Gap audit: what is missing vs Yandex Messenger

> ⚠️ **Historical snapshot from 2026-08-08 (~2.165.x).** Several P0/P1 items from that audit shipped in **2.173.0** (notify, tray, session, outbox, SQLite, DnD, ticks, night theme). Current status: [README.md](README.md). Original Russian audit: [GAP_AUDIT.md](GAP_AUDIT.md).

**Snapshot date:** 2026-08-08  
**Project:** Yandex Messenger Native (unofficial Linux client, Rust + GTK4/Libadwaita)  
**Code version at audit time:** ~2.165.x  

The 2.165 verdict was: the app was not yet a daily driver. Main gaps were trust (notifications/tray/read state), session friction, files, offline cache, and honest docs.

**Closed or largely closed by 2.173.0**

- Desktop notifications and system tray
- Mark-as-read, mute/pin/archive UI
- In-login session capture
- Outbox, drafts, history pagination
- SQLite cache and cold start
- File Download/Open, drag-and-drop, clipboard paste
- Delivery/read ticks
- Skeleton/empty states and reduce-animations
- Night theme / dense chat list

**Still open (see [ROADMAP_DETAILED.md](ROADMAP_DETAILED.md))**

- Voice messages and video playback
- Native Telemost (WebRTC), call history
- Production-confirmed chat-action RPCs
- Groups/channels parity, richer search
- Offline-first, a11y, broader i18n

The detailed tables in the Russian file remain a useful backlog, not a live scoreboard.
