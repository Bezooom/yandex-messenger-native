# Telemost implementation plan for Yandex Messenger Native

[Русская версия](TELEMOST_IMPLEMENTATION_PLAN.md)

> Based on reverse-engineering APK `ru.yandex.telemost` v3.12.0.138  
> As of **2.173.0** there is no native WebRTC — only a WebView/browser shell.

## Current state

| Component | Status | Path |
|-----------|--------|------|
| `TelemostWindow` | Stub (WebView/browser) | `src/ui/telemost.rs` |
| `start_call` / `end_call` / `get_call_status` | REST stub | `src/api/mod.rs` |
| `subscribe_call_updates` / `send_call_event_ws` | WebSocket stub | `src/api/mod.rs` |
| `TelemostCall`, `CallStatus`, `CallParticipant` | Models exist | `src/models/mod.rs` / `src/models/telemost.rs` |
| `TELEMOST_URL` | `https://telemost.yandex.ru` | `src/config.rs` |
| `YM_ENABLE_TELEMOST_UI` | Feature flag | `src/config.rs` |

## Findings from the APK

| Finding | Value / format |
|---------|----------------|
| Goloom WS | `wss://goloom.strm.yandex.net/join` |
| Uniproxy WS | `wss://uniproxy.messenger.yandex.ru/uni.ws` |
| Cloud API | `https://api.messenger.yandex.net` |
| Signaling | 245 protobuf classes (`ru.yandex.goloom.lib.model.signaling`) |
| Capabilities | 27 types (AUDIO, VIDEO, SCREEN_SHARE, BROADCAST…) |
| ConferenceParams | `wsUri`, `roomId`, `peerId`, `sessionId`, `credentials`, `clientConfig` |
| Hello fields | `capabilities_offer`, `credentials`, `send_audio`, `send_video`, `send_sharing` |
| Native WebRTC | `libjingle_peerconnection_so.so` (16 MB) |
| Noise suppression | `librnnoise.so` |

## Goals

### P0 (minimum viable call)

1. Create a conference via Cloud API → `ConferenceParams`
2. Connect to Goloom WebSocket signaling
3. Basic audio call (WebRTC)
4. UI: call window with mute/hangup

### P1 (full call)

5. Video streams
6. Screen share
7. UI: participant layout, indicators

### P2 (extra)

8. Incoming calls (ringing)
9. Call recording
10. Broadcast
11. AI features (summary, Alice Pro)

## Phases

### Phase 1 — Foundation (weeks 1–2)

- Config: `GOLOOM_WS_URL`, Cloud API base, Telemost path
- Models in `src/models/telemost.rs`: `ConferenceParams`, `ConferenceState`, `Capabilities`, signaling messages
- HTTP: `create_conference`, `get_conference`, `end_conference`, `update_participant`
- Dependencies: `prost`, `webrtc`, optional PipeWire

### Phase 2 — Signaling (weeks 3–4)

- `src/api/goloom_ws.rs` client and Hello/SFU state machine
- Protobuf or JSON-over-WS parser
- `RTCPeerConnection`, Opus audio, VP8/VP9 video

### Phase 3 — UI (weeks 5–6)

- Replace WebView with GTK video widgets and a control bar
- Incoming-call notification and call history in chat
- Device / screen-share pickers in settings

### Phase 4 — Polish (week 7+)

- Signaling unit tests, WebRTC loopback, UI E2E
- Adaptive bitrate, pause video when minimized
- Architecture and protocol docs

## Risks

| Risk | Mitigation |
|------|------------|
| Unknown protobuf schemas | APK RE + JSON fallback |
| WebRTC in Rust is hard | `webrtc` crate, audio first |
| Screen share on Linux | PipeWire, X11 fallback |
| Compatibility | Keep `YM_ENABLE_TELEMOST_UI` |

## Next steps

1. Agree the plan
2. Start with Phase 1.1 (config)
3. Implement `create_conference`
4. Connect to Goloom WS and receive Hello
