# Telemost Android Client Analysis v2

[Русская версия](telemost-android-analysis.ru.md)

Reverse-engineering notes for APK `ru.yandex.telemost` **3.12.0.138**. The APK itself is **not** in this repository. Current desktop client (2.173.0) still uses a WebView shell.

## APK Fingerprint
- Package: `ru.yandex.telemost`
- Version: `3.12.0.138`
- Min SDK: 26, Target SDK: 35
- DEX: classes.dex (main), classes2.dex (meetings/cloudapi), classes3.dex (config), classes4.dex (protobuf)
- Native: `libjingle_peerconnection_so.so`, `libquasar_daemons.so`, `librnnoise.so`

## Architecture
- **Call Framework**: `ru.yandex.goloom` (Goloom) — WebRTC-based
- **Signaling**: Protobuf over WebSocket
- **HTTP Client**: Ktor Client + OkHttp engine
- **JSON**: Moshi (GeneratedJsonAdapter)
- **Protobuf**: Square Wire + custom protobuf
- **WebView SDK**: `com.yandex.messenger.websdk.api`
- **Auth**: Yandex Passport SDK + OAuth2

## Key Activities
- `com.yandex.messaging.telemost.ui.MessengerTelemostActivity` — main call UI
- `com.yandex.messaging.telemost.ui.incoming.MessengerTelemostRingingActivity` — incoming call
- `com.yandex.messaging.telemost.ui.MessengerTelemostStarterActivity` — launcher
- `com.yandex.messaging.telemost.ui.invite.MessengerInviteToMeetingActivity` — invite
- `com.yandex.messaging.activity.calls.MessengerCallFeedbackActivity` — call feedback

## Key Services
- `ru.yandex.goloom.lib.impl.session.service.CallService` — core call service
- `ru.yandex.goloom.lib.impl.peer.hardware.device.ConfigurationObserverService` — device config
- `com.yandex.messaging.telemost.ringing.MessengerTelemostRingingService` — foreground ringing
- `com.yandex.telemost.newarch.screensharing.ScreenSharingService` — screen sharing
- `com.yandex.telemost.core.waitingroom.WaitService` — waiting room

## Permissions
- `android.permission.FOREGROUND_SERVICE_PHONE_CALL`
- `android.permission.FOREGROUND_SERVICE_MICROPHONE`
- `android.permission.READ_PHONE_STATE`
- `android.permission.MANAGE_OWN_CALLS`

## API Endpoints

### HTTP REST (Ktor/OkHttp)
- Base: `https://api.messenger.yandex.net/api/`
- Cloud API: `https://cloud-api.yandex.net/`
- Calendar: `https://mobapi.calendar.yandex.ru`
- Push: `https://push.yandex.ru/v2/subscribe/app`

### Telemost URLs
- Join: `https://telemost.yandex.ru/j/{meetingId}`
- Chat: `https://telemost.yandex.ru/c/{chatId}/{participantId}`
- Link: `https://telemost.yandex.ru/link#{meetingId}`
- DSP: `https://telemost.dsp.yandex.ru/j/`
- DST: `https://telemost.dst.yandex.ru/j/`
- Internal: `https://telemost.yandex-team.ru/j/`

### WebSocket
- Primary: `wss://goloom.strm.yandex.net/join`
- Fallback: `wss://uniproxy.alice.yandex.net/uni.ws`
- Beta: `wss://beta.uniproxy.alice.yandex.net/uni.ws`

## API Models (extracted)

### Create Personal Meeting
```
POST /api/telemost/create_personal_meeting
Request: CreatePersonalMeetingApiParams { userId }
Response: CreatePersonalMeetingResponse {
  Success { meetingId, joinUrl, ... }
  UserErrors { UserError { code, message } }
}
```

### Start Meeting Call
```
POST /api/telemost/start_meeting_call
Request: StartMeetingCallParams { meetingId }
Response: StartMeetingCallResponse {
  Success { callId, ... }
  UserError { code, message }
}
```

### End Personal Meeting
```
POST /api/telemost/end_personal_meeting
Request: EndPersonalMeetingApiParams { meetingId }
```

### Meeting Info
```
GET /api/telemost/meeting_info?meetingId={id}
Response: MeetingInfoResponse

POST /api/telemost/meeting_infos
Request: MeetingInfosParams { meetingIds[] }
Response: MeetingInfoListResponse
```

## WebRTC/Goloom Signaling Protocol

### Protobuf Definitions
- Package: `videoplatform/webrtc/common/proto/signaling/signaling.proto`
- Telemetry: `videoplatform/webrtc/common/proto/telemetry/events.proto`
- Capabilities: `videoplatform/webrtc/common/proto/capabilities/capabilities.proto`

### Signaling Messages
- `MeetingCallSettings` — call configuration
- `MeetingCallingMessage` — outgoing call
- `MeetingIncomingCallMessage` — incoming call
- `MeetingRingingMessage` / `MeetingEndRingingMessage` / `MeetingNotifyRingingMessage`
- `MeetingRingingEndedMessage` / `EndOutgoingRingingMessage` / `OutgoingRingingEndedMessage`
- `PersonalMeetingEndedMessage`
- `MeetingInfoResponse` / `MeetingInfoRegistryResponse` / `MeetingInfoFanoutResponse`

### Participant Description
```protobuf
ParticipantDescription {
  id: string
  meta: ParticipantMeta { name, role, description, avatar }
  participant_attributes: map<string, string>
  send_audio: bool
  send_video: bool
  send_sharing: bool
  hide_from_participants_list: bool
  disconnected_at: timestamp
  network_score: NetworkQualityScore
  connection_type: ConnectionType
  ref_participant_id: string
}
```

### Track Description
```protobuf
PublisherTrackDescription {
  mid: string
  transceiver_mid: string
  dc_label: string
  kind: MediaTrackKind (VIDEO/AUDIO)
  priority: int
  label: string
  description: string
  group_id: int64
  codecs: map<int64, CodecCapability>
}

SubscriberTrackDescription {
  mid: string
  kind: MediaTrackKind
}
```

### SDP Exchange
```protobuf
PublisherSdpOffer { pc_seq: int64, sdp: string }
PublisherSdpAnswer { pc_seq: int64, sdp: string }
SubscriberSdpOffer { pc_seq: int64, sdp: string }
SubscriberSdpAnswer { pc_seq: int64, sdp: string }
WebrtcIceCandidate { pc_seq: int64, target: Target, candidate: string }
```

### Capabilities Negotiation
```protobuf
CapabilitiesOffer {
  offer_answer_mode: OfferAnswerMode (SEPARATE/UNITED)
  initial_subscriber_offer: InitialSubscriberOffer (ON_REQUEST_SUBSCRIPTION/ON_HELLO)
  slots_mode: SlotsMode (FROM_SFU/FROM_CONTROLLER)
  simulcast_mode: SimulcastMode (DISABLED/STATIC)
  self_vad_status: SelfVADStatus (FROM_CLIENT/FROM_SERVER)
  data_channel_sharing: DataChannelSharing (TO_DATA_CHANNEL/TO_RTP)
  video_encoder_config: VideoEncoderConfigSupport
  data_channel_video_codec: DataChannelVideoCodec (VP8/UNIQUE_CODEC_FROM_TRACK_DESCRIPTION)
  svc_mode: SVCMode
  publish_vp9: bool
  publisher_opus_low_bitrate: bool
  publisher_opus_dred: bool
  // ... 20+ more capabilities
}
```

## Cloud API Classes (com.yandex.telemost.core.cloudapi)
- `AccessLevel` — conference access control
- `BroadcastData` / `BroadcastInfoState` — broadcast status
- `CallHistoryRawData` — call history with Direction, Origin, Status
- `ClientConfig` — client configuration
- `CloudRecordingStatus` — recording state
- `ConferenceState` — conference state machine
- `ConnectToWaitingRoomReason` — waiting room flow
- `ExperimentsInfo` — A/B experiments

## Auth Flow
1. Yandex Passport SDK (`com.yandex.passport`)
2. OAuth2 via `https://oauth.yandex.ru/authorize`
3. Client ID: `ru.yandex.yamb`
4. Tokens: `access_token`, `refresh_token`, `authToken`, `authenticationToken`
5. Headers: `Authorization`, `Ya-Consumer-Authorization`, `Proxy-Authorization`
6. OAuth discovery: `/.well-known/oauth/openid/keys/`

## WebView Bridge
- JS bridge: `androidMessengerChannel` via `MessageChannel`
- Methods: `openPort()`, `postMessage()`, `receiveMessage(JSON.stringify(e.data))`
- Ping: `sentPing = '@@@@ping_'`
- MiniApp: `miniappChannel.port1.onmessage`

## WebSocket Message Format
Based on protobuf definitions and WebSocket usage:
1. Connect to `wss://goloom.strm.yandex.net/join`
2. Send join message with meeting ID
3. Exchange protobuf-encoded signaling messages:
   - SDP offer/answer
   - ICE candidates
   - Track descriptions
   - Capabilities negotiation
4. Media via WebRTC (RTP/RTCP)

## Key Findings for Desktop Rust Messenger

### Reusable Protocol Patterns
1. **WebSocket endpoint**: `wss://goloom.strm.yandex.net/join` — primary signaling
2. **Protobuf schema**: Available from extracted strings — can be ported to Rust
3. **Capabilities negotiation**: Full schema extracted (25+ flags)
4. **Meeting lifecycle**: create → start → call → end
5. **Auth**: OAuth2 with Yandex Passport, token-based

### Implementation Strategy
1. Port protobuf definitions to Rust `prost`/`protobuf`
2. Implement WebSocket client with `tokio-tungstenite`
3. Implement Ktor-compatible HTTP client with `reqwest`/`hyper`
4. Follow capabilities negotiation flow
5. Integrate with existing WebRTC stack (if available)

### Missing Pieces
- Exact protobuf schema files (need full DEX decompilation or jadx)
- WebSocket message binary format
- Auth token refresh flow details
- Error codes and handling

## Next Steps
1. Install jadx for full Java decompilation
2. Extract full protobuf schemas from assets
3. Trace WebSocket message flow in CallService
4. Document auth token lifecycle
5. Synthesize with existing desktop implementation
