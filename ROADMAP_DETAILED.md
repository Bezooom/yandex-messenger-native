# Detailed improvement roadmap

**Project:** Yandex Messenger Native (Linux, Rust + GTK4/Libadwaita)  
**Plan baseline:** 2.165.0  
**Current release:** 2.173.0 (2026-08-15)  
**Sources:** [`GAP_AUDIT.md`](GAP_AUDIT.md), `src/`, [`ROADMAP.md`](ROADMAP.md)  
**Russian (full original):** [`ROADMAP_DETAILED.ru.md`](ROADMAP_DETAILED.ru.md)  
**Principle:** *stub ban* — a feature either works end-to-end or is hidden; docs status = code status.

---

## How to use this document

| Term | Meaning |
|------|---------|
| **Phase** | Large stage with a product outcome |
| **Sprint** | 1–2 weeks of focused work |
| **S / M / L / XL** | Effort: ~0.5–1d / 2–3d / 5–8d / 2+ weeks |
| **DoD** | Definition of Done — without it the task is not “✅” |
| **Gate** | Exit criterion (dogfood) |

**Statuses:** `TODO` · `IN PROGRESS` · `BLOCKED` · `DONE` · `WONTFIX`

Progress rule: P0 (trust) → P1 (daily use) → polish. Do not open new panels (bots/scheduled UI) until Gate A is closed.

```
Phase 0 ──► Phase A ──► Phase B ──► Phase C ──► Phase D ──► Phase E
 Prep       Trust       Daily       Media       Parity      Polish
 ~1 week    ~3–4 wk     ~3–4 wk     ~3–5 wk     ~4–6 wk     continuous
            MVP-0       MVP-1       MVP-2       “almost YM” release quality
```

| Milestone | Version (proposal) | User meaning |
|-----------|--------------------|--------------|
| Gate A | **2.170** | Worth leaving open: notify + tray + honest actions |
| Gate B | **2.180** | Daily text/files instead of the web client |
| Gate C | **2.190** | Voice/video/media previews are not embarrassing |
| Gate D | **2.200** | Groups, search, calls, offline — core parity |
| Gate E | **2.210+** | Polish, a11y, i18n, auto-update |

---

## Status as of 2.173.0

Shipped since the 2.165 baseline (Gate A and the start of Gate B):

| Area | State |
|------|--------|
| Honest feature flags + stub comments | DONE |
| Desktop notifications | DONE |
| System tray | DONE |
| Mark read / mute / pin / archive UI | DONE (RPC best-effort) |
| Settings window | DONE (+ reduce animations) |
| Session capture inside login | DONE |
| Outbox + drafts | DONE |
| History pagination | DONE |
| SQLite cache + cold start | DONE |
| File Download/Open, DnD, paste | DONE |
| Delivery / read ticks | DONE |
| Skeleton / empty states | DONE |
| Night theme + nheko-style list | DONE (2.173) |

Still open (Gate B–D):

- Voice record/play and video player
- Real Telemost WebRTC (not WebView)
- Reliable server-side chat-action RPCs
- Full offline, groups/channels parity, a11y, broader i18n

---

## Product goals

- **Phase A / MVP-0:** you can leave the app open all day (notify, tray, read state).
- **Phase B / MVP-1:** text and files replace the web client for daily 1:1 chat.
- **Phase C:** media (voice, video, rich previews) is usable.
- **Phase D:** groups, search, calls, offline approach official Messenger.
- **Phase E:** release quality (a11y, i18n, packaging, updates).

## Dogfood checklists

### Gate A (largely met in 2.170–2.173)

- [x] Notifications for new messages (respect mute)
- [x] Tray + close-to-tray
- [x] Mark as read on open
- [x] Settings for notify / tray / theme
- [ ] Chat-action RPCs confirmed against production (still best-effort)

### Gate B (in progress)

- [x] Session without an extra Python script
- [x] Outbox / drafts / pagination / SQLite
- [x] Files open, DnD, paste
- [ ] Voice messages
- [ ] Stable file types / progress UI for all media

### Gate C

- [ ] Voice + video playback
- [ ] Native Telemost (see [`TELEMOST_IMPLEMENTATION_PLAN.md`](TELEMOST_IMPLEMENTATION_PLAN.md))
- [ ] Call history

### Gate D / E

- [ ] Groups/channels parity
- [ ] Global search quality
- [ ] Offline-first
- [ ] WCAG / keyboard / RTL
- [ ] Auto-update / wider packaging

The Russian document keeps the original sprint-level task tables. Treat those rows as a backlog: many Phase 0/A items are done; do not re-open them without checking 2.173 code.
