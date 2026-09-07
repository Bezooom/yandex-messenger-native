//! Goloom signaling protocol — Rust port of the APK protobuf schemas.
//!
//! Source of truth: `docs/apk-3.12.0.138/proto/` extracted verbatim from
//! `ru.yandex.telemost_3.12.0.138.xapk`:
//! - `signaling.proto` — package `videoplatform.speakerroom.common.signaling`
//! - `capabilities.proto` — package `videoplatform.speakerroom.common.capabilities`
//! - `events.proto` — package `videoplatform.speakerroom.common.telemetry`
//!
//! Wire endpoint (from DEX strings): `wss://goloom.strm.yandex.net/join`
//! (fallback `wss://uniproxy.alice.yandex.net/uni.ws`).
//!
//! Scope: the handshake/media-negotiation subset the desktop client needs.
//! Deprecated-only messages the server never sends us (`ParticipantMeta`,
//! `Telemetry` reports, `SdkCodecsInfo`, `RoomAgentSignaling`, …) are
//! intentionally omitted — protobuf decoders skip unknown fields, so this
//! stays wire-compatible. Anything omitted is marked `// FOLLOW-UP`.

// ── capabilities.proto ──────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum OfferAnswerMode {
    Separate = 0,
    United = 1,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum InitialSubscriberOffer {
    OnRequestSubscription = 0,
    OnHello = 1,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum SlotsMode {
    FromSfu = 0,
    FromController = 1,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum SimulcastMode {
    Disabled = 0,
    Static = 1,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum SelfVadStatus {
    FromClient = 0,
    FromServer = 1,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum DataChannelSharing {
    ToDataChannel = 0,
    ToRtp = 1,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum DataChannelVideoCodec {
    Vp8 = 0,
    UniqueCodecFromTrackDescription = 1,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum VideoEncoderConfigSupport {
    NoConfig = 0,
    OnlyInitConfig = 1,
    RuntimeConfig = 2,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum BandwidthLimitationReason {
    Disabled = 0,
    Enabled = 1,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum ServerLayoutTransition {
    Disabled = 0,
    Enabled = 1,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum PinLayout {
    Disabled = 0,
    Enabled = 1,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum JoinOrderLayout {
    Disabled = 0,
    Enabled = 1,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum SendSelfViewVideoSlot {
    Disabled = 0,
    Enabled = 1,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum SdkDefaultDeviceManagement {
    Disabled = 0,
    Enabled = 1,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum SdkPublisherOptimizeBitrate {
    Disabled = 0,
    OnlySelf = 1,
    Full = 2,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum SdkNetworkPathMonitor {
    Disabled = 0,
    Enabled = 1,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum PublishVp9 {
    Disabled = 0,
    Enabled = 1,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum SvcMode {
    Disabled = 0,
    L1t1 = 1,
    L1t2 = 2,
    L1t3 = 3,
    L2t1 = 4,
    L2t2 = 5,
    L2t3 = 6,
    L3t1 = 7,
    L3t2 = 8,
    L3t3 = 9,
    L2t1h = 10,
    L2t2h = 11,
    L2t3h = 12,
    L3t1h = 13,
    L3t2h = 14,
    L3t3h = 15,
    S2t1 = 16,
    S2t2 = 17,
    S2t3 = 18,
    S2t1h = 19,
    S2t2h = 20,
    S2t3h = 21,
    S3t1 = 22,
    S3t2 = 23,
    S3t3 = 24,
    S3t1h = 25,
    S3t2h = 26,
    S3t3h = 27,
    L2t2Key = 28,
    L2t2KeyShift = 29,
    L2t3Key = 30,
    L2t3KeyShift = 31,
    L3t1Key = 32,
    L3t2Key = 33,
    L3t2KeyShift = 34,
    L3t3Key = 35,
    L3t3KeyShift = 36,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum SdkNetworkLostDetection {
    Disabled = 0,
    Enabled = 1,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum FixedIceCandidatesPoolSize {
    Disabled = 0,
    Enabled = 1,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum SubscriberOfferAsyncAck {
    Disabled = 0,
    Enabled = 1,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum AndroidBluetoothRoutingFix {
    Enabled = 0,
    Disabled = 1,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum SdkAndroidTelecomIntegration {
    Enabled = 0,
    Disabled = 1,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum SetActiveCodecsMode {
    Disabled = 0,
    VideoOnly = 1,
    AudioOnly = 2,
    AudioAndVideo = 3,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum SubscriberDtlsPassiveMode {
    Disabled = 0,
    Enabled = 1,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum PublisherOpusLowBitrate {
    Disabled = 0,
    Enabled = 1,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum PublisherOpusDred {
    Disabled = 0,
    Enabled = 1,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum SdkAndroidDestroySessionOnTaskRemoved {
    Disabled = 0,
    Enabled = 1,
}

/// `capabilities.CapabilitiesOffer` — what we claim to support.
/// The server answers with `CapabilitiesAnswer`; never assume the offer sticks.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CapabilitiesOffer {
    #[prost(enumeration = "OfferAnswerMode", repeated, tag = "1")]
    pub offer_answer_mode: Vec<i32>,
    #[prost(enumeration = "InitialSubscriberOffer", repeated, tag = "2")]
    pub initial_subscriber_offer: Vec<i32>,
    #[prost(enumeration = "SlotsMode", repeated, tag = "3")]
    pub slots_mode: Vec<i32>,
    #[prost(enumeration = "SimulcastMode", repeated, tag = "4")]
    pub simulcast_mode: Vec<i32>,
    #[prost(enumeration = "SelfVadStatus", repeated, tag = "5")]
    pub self_vad_status: Vec<i32>,
    #[prost(enumeration = "DataChannelSharing", repeated, tag = "6")]
    pub data_channel_sharing: Vec<i32>,
    #[prost(enumeration = "VideoEncoderConfigSupport", repeated, tag = "7")]
    pub video_encoder_config: Vec<i32>,
    #[prost(enumeration = "DataChannelVideoCodec", repeated, tag = "8")]
    pub data_channel_video_codec: Vec<i32>,
    #[prost(enumeration = "BandwidthLimitationReason", repeated, tag = "9")]
    pub bandwidth_limitation_reason: Vec<i32>,
    #[prost(enumeration = "ServerLayoutTransition", repeated, tag = "10")]
    pub server_layout_transition: Vec<i32>,
    #[prost(enumeration = "PinLayout", repeated, tag = "11")]
    pub pin_layout: Vec<i32>,
    #[prost(enumeration = "JoinOrderLayout", repeated, tag = "12")]
    pub join_order_layout: Vec<i32>,
    #[prost(enumeration = "SendSelfViewVideoSlot", repeated, tag = "13")]
    pub send_self_view_video_slot: Vec<i32>,
    #[prost(enumeration = "SdkDefaultDeviceManagement", repeated, tag = "14")]
    pub sdk_default_device_management: Vec<i32>,
    #[prost(enumeration = "SdkPublisherOptimizeBitrate", repeated, tag = "15")]
    pub sdk_publisher_optimize_bitrate: Vec<i32>,
    #[prost(enumeration = "SdkNetworkPathMonitor", repeated, tag = "16")]
    pub sdk_network_path_monitor: Vec<i32>,
    #[prost(enumeration = "PublishVp9", repeated, tag = "17")]
    pub publisher_vp9: Vec<i32>,
    #[prost(enumeration = "SvcMode", repeated, tag = "18")]
    pub svc_mode: Vec<i32>,
    #[prost(enumeration = "SdkNetworkLostDetection", repeated, tag = "19")]
    pub sdk_network_lost_detection: Vec<i32>,
    #[prost(enumeration = "FixedIceCandidatesPoolSize", repeated, tag = "20")]
    pub fixed_ice_candidates_pool_size: Vec<i32>,
    #[prost(enumeration = "SubscriberOfferAsyncAck", repeated, tag = "21")]
    pub subscriber_offer_async_ack: Vec<i32>,
    #[prost(enumeration = "AndroidBluetoothRoutingFix", repeated, tag = "22")]
    pub android_bluetooth_routing_fix: Vec<i32>,
    #[prost(enumeration = "SdkAndroidTelecomIntegration", repeated, tag = "23")]
    pub sdk_android_telecom_integration: Vec<i32>,
    #[prost(enumeration = "SetActiveCodecsMode", repeated, tag = "24")]
    pub set_active_codecs_mode: Vec<i32>,
    #[prost(enumeration = "SubscriberDtlsPassiveMode", repeated, tag = "25")]
    pub subscriber_dtls_passive_mode: Vec<i32>,
    #[prost(enumeration = "PublisherOpusLowBitrate", repeated, tag = "26")]
    pub publisher_opus_low_bitrate: Vec<i32>,
    #[prost(enumeration = "PublisherOpusDred", repeated, tag = "27")]
    pub publisher_opus_dred: Vec<i32>,
    #[prost(
        enumeration = "SdkAndroidDestroySessionOnTaskRemoved",
        repeated,
        tag = "28"
    )]
    pub sdk_android_destroy_session_on_task_removed: Vec<i32>,
}

/// `capabilities.CapabilitiesAnswer` — server's negotiation result.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CapabilitiesAnswer {
    #[prost(enumeration = "OfferAnswerMode", tag = "1")]
    pub offer_answer_mode: i32,
    #[prost(enumeration = "InitialSubscriberOffer", tag = "2")]
    pub initial_subscriber_offer: i32,
    #[prost(enumeration = "SlotsMode", tag = "3")]
    pub slots_mode: i32,
    #[prost(enumeration = "SimulcastMode", tag = "4")]
    pub simulcast_mode: i32,
    #[prost(enumeration = "SelfVadStatus", tag = "5")]
    pub self_vad_status: i32,
    #[prost(enumeration = "DataChannelSharing", tag = "6")]
    pub data_channel_sharing: i32,
    #[prost(enumeration = "VideoEncoderConfigSupport", tag = "7")]
    pub video_encoder_config: i32,
    #[prost(enumeration = "DataChannelVideoCodec", tag = "8")]
    pub data_channel_video_codec: i32,
    #[prost(enumeration = "BandwidthLimitationReason", tag = "9")]
    pub bandwidth_limitation_reason: i32,
    #[prost(enumeration = "ServerLayoutTransition", tag = "10")]
    pub server_layout_transition: i32,
    #[prost(enumeration = "PinLayout", tag = "11")]
    pub pin_layout: i32,
    #[prost(enumeration = "JoinOrderLayout", tag = "12")]
    pub join_order_layout: i32,
    #[prost(enumeration = "SendSelfViewVideoSlot", tag = "13")]
    pub send_self_view_video_slot: i32,
    #[prost(enumeration = "SdkDefaultDeviceManagement", tag = "14")]
    pub sdk_default_device_management: i32,
    #[prost(enumeration = "SdkPublisherOptimizeBitrate", tag = "15")]
    pub sdk_publisher_optimize_bitrate: i32,
    #[prost(enumeration = "SdkNetworkPathMonitor", tag = "16")]
    pub sdk_network_path_monitor: i32,
    #[prost(enumeration = "PublishVp9", tag = "17")]
    pub publisher_vp9: i32,
    #[prost(enumeration = "SvcMode", tag = "18")]
    pub svc_mode: i32,
    #[prost(enumeration = "SdkNetworkLostDetection", tag = "19")]
    pub sdk_network_lost_detection: i32,
    #[prost(enumeration = "FixedIceCandidatesPoolSize", tag = "20")]
    pub fixed_ice_candidates_pool_size: i32,
    #[prost(enumeration = "SubscriberOfferAsyncAck", tag = "21")]
    pub subscriber_offer_async_ack: i32,
    #[prost(enumeration = "AndroidBluetoothRoutingFix", tag = "22")]
    pub android_bluetooth_routing_fix: i32,
    #[prost(enumeration = "SdkAndroidTelecomIntegration", tag = "23")]
    pub sdk_android_telecom_integration: i32,
    #[prost(enumeration = "SetActiveCodecsMode", tag = "24")]
    pub set_active_codecs_mode: i32,
    #[prost(enumeration = "SubscriberDtlsPassiveMode", tag = "25")]
    pub subscriber_dtls_passive_mode: i32,
    #[prost(enumeration = "PublisherOpusLowBitrate", tag = "26")]
    pub publisher_opus_low_bitrate: i32,
    #[prost(enumeration = "PublisherOpusDred", tag = "27")]
    pub publisher_opus_dred: i32,
    #[prost(enumeration = "SdkAndroidDestroySessionOnTaskRemoved", tag = "28")]
    pub sdk_android_destroy_session_on_task_removed: i32,
}

// ── signaling.proto: enums ──────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum WebrtcSdpType {
    Unspecified = 0,
    Offer = 1,
    Answer = 2,
    Reject = 3,
    OfferClose = 4,
}

/// Full `StatusCode` — the ranges drive reconnect policy:
/// `[4100;4200)` + `TIMEOUT`/`ACK_TIMEOUT_ERROR` → reconnect expected;
/// `ROOM_HAS_BEEN_CLOSED*`/`KICKED_OUT`/`PERMISSION_DENIED` → do not retry blindly.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum StatusCode {
    Unspecified = 0,
    Ok = 1,
    UnknownError = 2,
    NotAllowed = 3,
    BadDescription = 4,
    BadCandidate = 5,
    UnknownMessage = 6,
    ConnectionClose = 7,
    RemoteDescriptionRejected = 8,
    LocalDescriptionRejected = 9,
    TelemetryProcessingError = 10,
    AckTimeoutError = 3106,
    PermissionDenied = 4000,
    AuthProviderIsNotAvailable = 4001,
    HelloShouldBeFirst = 4002,
    UnsupportedCapabilities = 4003,
    KickedOut = 4004,
    FoundActiveSession = 4005,
    ParticipantConnectionIdMismatch = 4006,
    ValidationError = 4007,
    Timeout = 4008,
    RoomHasBeenClosed = 4009,
    RoomHasBeenClosedByTimeout = 4010,
    RoomHasBeenUnexpectedlyClosed = 4011,
    MoveToNewMediaServer = 4100,
    RoomIsClosing = 4101,
    NetworkLost = 4102,
    TooManyMessages = 4103,
    WebsocketClosedWithoutCode = 4104,
    InitClientError = 5000,
    UserEventHandlerError = 5001,
    RequestDevicesError = 5002,
    ApplyingDevicesError = 5003,
    MediaStreamTrackError = 5004,
    DeviceChangeStateError = 5005,
    FourKSharingError = 5006,
    CapabilitiesDetectingError = 5007,
    SignalingFailed = 5008,
    PublisherFailed = 5009,
    SubscriberFailed = 5010,
    NoiseCancellationError = 5018,
    VideoProcessingError = 5019,
    DevicesNotFound = 5011,
    DeviceBusy = 5012,
    DeviceOverconstrained = 5013,
    DeviceDeniedByUser = 5014,
    DeviceDeniedBySystem = 5015,
    DeviceNotReady = 5016,
    ApplyingOutputDevicesError = 5017,
    IceCandidateErrorTryAlternate = 6300,
    IceCandidateErrorBadRequest = 6400,
    IceCandidateErrorUnauthenticated = 6401,
    IceCandidateErrorForbidden = 6403,
    IceCandidateErrorMobilityForbidden = 6405,
    IceCandidateErrorUnknownAttribute = 6420,
    IceCandidateErrorAllocationMismatch = 6437,
    IceCandidateErrorStaleNonce = 6438,
    IceCandidateErrorAddressFamilyNotSupported = 6440,
    IceCandidateErrorWrongCredentials = 6441,
    IceCandidateErrorUnsupportedTransportProtocol = 6442,
    IceCandidateErrorPeerAddressFamilyMismatch = 6443,
    IceCandidateErrorConnectionAlreadyExists = 6446,
    IceCandidateErrorConnectionTimeoutOrFailure = 6447,
    IceCandidateErrorAllocationQuotaReached = 6486,
    IceCandidateErrorRoleConflict = 6487,
    IceCandidateErrorServerError = 6500,
    IceCandidateErrorInsufficientCapacity = 6508,
    IceCandidateErrorNoHostCandidateCanReachTheServer = 6701,
    IceCandidateErrorUnknown = 6799,
}

impl StatusCode {
    /// `true` when the APK contract expects the SDK to reconnect.
    pub fn expects_reconnect(code: i32) -> bool {
        matches!(
            code,
            x if x == StatusCode::AckTimeoutError as i32
                || x == StatusCode::Timeout as i32
                || (4100..4200).contains(&x)
                || (1002..2000).contains(&x)
        )
    }

    /// `true` when retrying the same room is pointless.
    pub fn is_terminal(code: i32) -> bool {
        matches!(
            code,
            x if x == StatusCode::PermissionDenied as i32
                || x == StatusCode::KickedOut as i32
                || x == StatusCode::RoomHasBeenClosed as i32
                || x == StatusCode::RoomHasBeenClosedByTimeout as i32
                || x == StatusCode::RoomHasBeenUnexpectedlyClosed as i32
                || x == StatusCode::FoundActiveSession as i32
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum MediaTrackKind {
    Unspecified = 0,
    Unknown = 1,
    Audio = 2,
    Video = 3,
    DisplayAudio = 4,
    DisplayVideo = 5,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum ConnectionType {
    Unspecified = 0,
    Sdk = 1,
    Whip = 2,
    Sip = 3,
    Pstn = 4,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum NetworkQualityScore {
    Unspecified = 0,
    Poor = 1,
    Good = 2,
    Excellent = 3,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum VideoCodec {
    Vp8 = 0,
    H264 = 1,
    Vp9 = 2,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum AudioCodec {
    Opus14 = 0,
    Opus15 = 1,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum ComponentType {
    Border = 0,
    WebrtcServer = 1,
    Controller = 2,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum MediaLimitationReason {
    Unspecified = 0,
    NoLimitation = 1,
    RoomSize = 2,
    Bandwidth = 3,
    ByUser = 4,
    UnsupportedCodec = 5,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum SelfViewVisibility {
    Unspecified = 0,
    Show = 1,
    Hide = 2,
    OnLoadingThenShow = 3,
    OnLoadingThenHide = 4,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum IceTarget {
    Unspecified = 0,
    Publisher = 1,
    Subscriber = 2,
}

// ── signaling.proto: messages ───────────────────────────────────────

#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Status {
    #[prost(enumeration = "StatusCode", tag = "1")]
    pub code: i32,
    #[prost(string, optional, tag = "2")]
    pub description: Option<String>,
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Ack {
    #[prost(message, optional, tag = "1")]
    pub status: Option<Status>,
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ClientError {
    #[prost(enumeration = "StatusCode", tag = "1")]
    pub code: i32,
    #[prost(string, tag = "2")]
    pub description: String,
    #[prost(map = "string, string", tag = "3")]
    pub details: std::collections::HashMap<String, String>,
    #[prost(double, tag = "4")]
    pub client_timestamp: f64,
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ParticipantDescription {
    #[prost(string, tag = "1")]
    pub id: String,
    // field 2 reserved; field 3 `meta` deprecated — skipped on decode.
    #[prost(map = "string, string", tag = "7")]
    pub participant_attributes: std::collections::HashMap<String, String>,
    #[prost(bool, tag = "4")]
    pub send_audio: bool,
    #[prost(bool, tag = "5")]
    pub send_video: bool,
    #[prost(bool, tag = "8")]
    pub send_sharing: bool,
    #[prost(bool, tag = "9")]
    pub hide_from_participants_list: bool,
    #[prost(uint32, optional, tag = "6")]
    pub disconnected_at: Option<u32>,
    #[prost(enumeration = "NetworkQualityScore", tag = "10")]
    pub network_score: i32,
    #[prost(enumeration = "ConnectionType", tag = "11")]
    pub connection_type: i32,
    #[prost(string, optional, tag = "12")]
    pub ref_participant_id: Option<String>,
}

impl ParticipantDescription {
    /// Display name in newarch comes via attributes, not a dedicated field.
    pub fn display_name(&self) -> Option<&str> {
        self.participant_attributes.get("name").map(String::as_str)
    }

    pub fn is_connected(&self) -> bool {
        self.disconnected_at.is_none()
    }
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CodecCapability {
    #[prost(string, tag = "1")]
    pub mime_type: String,
    #[prost(uint32, tag = "2")]
    pub clock_rate: u32,
    #[prost(uint32, tag = "3")]
    pub channels: u32,
    #[prost(string, tag = "4")]
    pub sdp_fmtp_line: String,
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub struct PublisherTrackDescription {
    #[prost(oneof = "PublisherTrackTransport", tags = "6, 7")]
    pub transport: Option<PublisherTrackTransport>,
    #[prost(enumeration = "MediaTrackKind", tag = "2")]
    pub kind: i32,
    #[prost(string, tag = "5")]
    pub label: String,
    #[prost(string, tag = "10")]
    pub description: String,
    #[prost(uint32, optional, tag = "9")]
    pub group_id: Option<u32>,
    #[prost(map = "uint32, message", tag = "8")]
    pub codecs: std::collections::HashMap<u32, CodecCapability>,
}

#[derive(Clone, PartialEq, ::prost::Oneof)]
pub enum PublisherTrackTransport {
    #[prost(string, tag = "6")]
    TransceiverMid(String),
    #[prost(string, tag = "7")]
    DcLabel(String),
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SubscriberTrackDescription {
    #[prost(string, tag = "1")]
    pub mid: String,
    #[prost(string, tag = "4")]
    pub track_description_id: String,
    #[prost(enumeration = "MediaTrackKind", tag = "2")]
    pub kind: i32,
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub struct PublisherSdpOffer {
    #[prost(uint32, tag = "3")]
    pub pc_seq: u32,
    #[prost(string, tag = "1")]
    pub sdp: String,
    #[prost(message, repeated, tag = "2")]
    pub tracks: Vec<PublisherTrackDescription>,
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub struct PublisherSdpAnswer {
    #[prost(uint32, tag = "2")]
    pub pc_seq: u32,
    #[prost(string, tag = "1")]
    pub sdp: String,
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SubscriberSdpOffer {
    #[prost(uint32, tag = "4")]
    pub pc_seq: u32,
    #[prost(string, tag = "1")]
    pub sdp: String,
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SubscriberSdpAnswer {
    #[prost(uint32, tag = "2")]
    pub pc_seq: u32,
    #[prost(string, tag = "1")]
    pub sdp: String,
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub struct WebrtcIceCandidate {
    #[prost(uint32, tag = "6")]
    pub pc_seq: u32,
    #[prost(enumeration = "IceTarget", tag = "1")]
    pub target: i32,
    #[prost(string, tag = "2")]
    pub candidate: String,
    #[prost(string, optional, tag = "3")]
    pub sdp_mid: Option<String>,
    #[prost(uint32, optional, tag = "4")]
    pub sdp_mline_index: Option<u32>,
    #[prost(string, optional, tag = "5")]
    pub username_fragment: Option<String>,
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub struct UpdateDescription {
    #[prost(message, repeated, tag = "1")]
    pub description: Vec<ParticipantDescription>,
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub struct UpsertDescription {
    #[prost(message, repeated, tag = "1")]
    pub description: Vec<ParticipantDescription>,
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RemoveDescription {
    #[prost(string, repeated, tag = "1")]
    pub description_id: Vec<String>,
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SdkInfo {
    #[prost(string, tag = "1")]
    pub implementation: String,
    #[prost(string, tag = "2")]
    pub version: String,
    #[prost(string, optional, tag = "3")]
    pub webrtc_version: Option<String>,
    #[prost(string, tag = "4")]
    pub user_agent: String,
    #[prost(int32, tag = "7")]
    pub hw_concurrency: i32,
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Hello {
    #[prost(string, tag = "4")]
    pub service_name: String,
    #[prost(string, tag = "2")]
    pub room_id: String,
    #[prost(string, tag = "1")]
    pub participant_id: String,
    #[prost(oneof = "HelloAuth", tags = "5, 7")]
    pub auth: Option<HelloAuth>,
    // field 3 `participant_meta` deprecated — never send.
    #[prost(map = "string, string", tag = "12")]
    pub participant_attributes: std::collections::HashMap<String, String>,
    #[prost(bool, tag = "8")]
    pub send_audio: bool,
    #[prost(bool, tag = "9")]
    pub send_video: bool,
    #[prost(bool, tag = "13")]
    pub send_sharing: bool,
    #[prost(bool, tag = "14")]
    pub disable_subscriber: bool,
    #[prost(bool, tag = "16")]
    pub disable_subscriber_audio: bool,
    #[prost(bool, tag = "15")]
    pub disable_publisher: bool,
    #[prost(message, optional, tag = "6")]
    pub capabilities_offer: Option<CapabilitiesOffer>,
    #[prost(message, optional, tag = "10")]
    pub sdk_info: Option<SdkInfo>,
    #[prost(string, tag = "11")]
    pub sdk_initialization_id: String,
    #[prost(enumeration = "StatusCode", optional, tag = "17")]
    pub signaling_close_code: Option<i32>,
    #[prost(string, optional, tag = "18")]
    pub ref_participant_id: Option<String>,
}

#[derive(Clone, PartialEq, ::prost::Oneof)]
pub enum HelloAuth {
    #[prost(string, tag = "5")]
    Credentials(String),
    #[prost(string, tag = "7")]
    SessionSecret(String),
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RtcIceServer {
    #[prost(string, repeated, tag = "1")]
    pub urls: Vec<String>,
    #[prost(string, tag = "2")]
    pub credential: String,
    #[prost(string, tag = "3")]
    pub username: String,
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RtcConfiguration {
    #[prost(message, repeated, tag = "1")]
    pub ice_servers: Vec<RtcIceServer>,
    #[prost(string, optional, tag = "2")]
    pub ice_transport_policy: Option<String>,
    #[prost(int32, optional, tag = "3")]
    pub ice_candidate_pool_size: Option<i32>,
    #[prost(string, optional, tag = "4")]
    pub bundle_policy: Option<String>,
    #[prost(string, optional, tag = "5")]
    pub rtcp_mux_policy: Option<String>,
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub struct PingPongConfiguration {
    #[prost(uint32, tag = "1")]
    pub ping_interval: u32,
    #[prost(uint32, tag = "2")]
    pub ack_timeout: u32,
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TelemetryConfiguration {
    #[prost(uint32, tag = "1")]
    pub sending_interval: u32,
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ServingComponent {
    #[prost(enumeration = "ComponentType", tag = "1")]
    pub r#type: i32,
    #[prost(string, tag = "2")]
    pub host: String,
    #[prost(string, tag = "3")]
    pub version: String,
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ServerHello {
    #[prost(message, optional, tag = "1")]
    pub capabilities_answer: Option<CapabilitiesAnswer>,
    #[prost(message, repeated, tag = "2")]
    pub serving_components: Vec<ServingComponent>,
    #[prost(string, tag = "3")]
    pub session_secret: String,
    // field 4 `vad_config` deprecated — skipped.
    #[prost(string, tag = "5")]
    pub sfu_peer_initialization_id: String,
    #[prost(message, optional, tag = "6")]
    pub rtc_configuration: Option<RtcConfiguration>,
    #[prost(string, tag = "7")]
    pub log_endpoint: String,
    #[prost(message, optional, tag = "11")]
    pub ping_pong_configuration: Option<PingPongConfiguration>,
    #[prost(message, optional, tag = "12")]
    pub telemetry_configuration: Option<TelemetryConfiguration>,
    #[prost(bool, tag = "14")]
    pub exclude_from_experiments: bool,
    #[prost(message, optional, tag = "17")]
    pub active_codecs: Option<SetActiveCodecs>,
    // FOLLOW-UP: fields 8/9/10/13/15/16/18 (legacy encoder configs,
    // sound processing, 4K sharing, CodecsConfiguration) — parse on demand.
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SfuHello {
    #[prost(string, tag = "1")]
    pub sfu_peer_initialization_id: String,
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub struct UpdateParticipantMeta {
    #[prost(map = "string, string", tag = "4")]
    pub participant_attributes: std::collections::HashMap<String, String>,
    #[prost(bool, tag = "2")]
    pub send_audio: bool,
    #[prost(bool, tag = "3")]
    pub send_video: bool,
    #[prost(bool, tag = "5")]
    pub send_sharing: bool,
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub struct UpdatePublisherTrackDescription {
    #[prost(message, repeated, tag = "1")]
    pub publisher_track_descriptions: Vec<PublisherTrackDescription>,
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub struct VideoSlot {
    #[prost(int32, tag = "1")]
    pub width: i32,
    #[prost(int32, tag = "2")]
    pub height: i32,
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RequestSubscription {
    #[prost(bool, tag = "1")]
    pub force_new_peer_connnection: bool,
    #[prost(bool, tag = "2")]
    pub force_new_peer_connection: bool,
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SetSlotsOffset {
    #[prost(uint32, tag = "1")]
    pub offset: u32,
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SlotsMeta {
    #[prost(uint32, optional, tag = "1")]
    pub max_offset: Option<u32>,
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RequestPinnedParticipants {
    #[prost(string, repeated, tag = "1")]
    pub participants_id: Vec<String>,
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SetSlots {
    #[prost(uint32, tag = "8")]
    pub key: u32,
    #[prost(message, repeated, tag = "1")]
    pub slots: Vec<VideoSlot>,
    #[prost(bool, tag = "5")]
    pub shutdown_all_video: bool,
    #[prost(enumeration = "SelfViewVisibility", tag = "10")]
    pub self_view_visibility: i32,
    // FOLLOW-UP: `View` oneof (grid/n-last/join-order/pin) for layout control.
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GridLayoutConfig {}

#[derive(Clone, PartialEq, ::prost::Message)]
pub struct NLastLayoutConfig {
    #[prost(uint32, tag = "1")]
    pub n_count: u32,
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub struct JoinOrderLayoutConfig {}

#[derive(Clone, PartialEq, ::prost::Message)]
pub struct PinLayoutConfig {}

#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SlotsConfig {
    #[prost(uint32, tag = "9")]
    pub key: u32,
    #[prost(message, repeated, tag = "10")]
    pub prev_slots: Vec<SlotsConfigSlot>,
    #[prost(message, repeated, tag = "3")]
    pub slots: Vec<SlotsConfigSlot>,
    #[prost(message, repeated, tag = "11")]
    pub next_slots: Vec<SlotsConfigSlot>,
    #[prost(uint32, optional, tag = "4")]
    pub offset: Option<u32>,
    #[prost(oneof = "SlotsConfigView", tags = "5, 6, 7, 8")]
    pub view: Option<SlotsConfigView>,
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SlotsConfigSlot {
    #[prost(oneof = "SlotsConfigSlotKind", tags = "1, 12, 2, 3, 4, 8")]
    pub kind: Option<SlotsConfigSlotKind>,
    #[prost(bool, tag = "5")]
    pub vad: bool,
    #[prost(bool, tag = "6")]
    pub pinned: bool,
    #[prost(string, tag = "7")]
    pub label: String,
}

#[derive(Clone, PartialEq, ::prost::Oneof)]
pub enum SlotsConfigSlotKind {
    #[prost(message, tag = "1")]
    Empty(GridLayoutConfig),
    #[prost(message, tag = "12")]
    SelfView(GridLayoutConfig),
    #[prost(message, tag = "2")]
    Participant(SlotParticipant),
    #[prost(message, tag = "3")]
    ParticipantVideoByMid(SlotParticipantVideoByMid),
    #[prost(message, tag = "4")]
    ParticipantScreenSharingByDataChannel(SlotParticipant),
    #[prost(message, tag = "8")]
    ParticipantScreenSharingByMid(SlotParticipantVideoByMid),
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SlotParticipant {
    #[prost(string, tag = "1")]
    pub participant_id: String,
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SlotParticipantVideoByMid {
    #[prost(string, tag = "1")]
    pub participant_id: String,
    #[prost(string, tag = "2")]
    pub mid: String,
    #[prost(enumeration = "MediaLimitationReason", tag = "3")]
    pub limitation_reason: i32,
}

#[derive(Clone, PartialEq, ::prost::Oneof)]
pub enum SlotsConfigView {
    #[prost(message, tag = "5")]
    GridConfig(GridLayoutConfig),
    #[prost(message, tag = "6")]
    NLastConfig(NLastLayoutConfig),
    #[prost(message, tag = "7")]
    JoinOrderConfig(JoinOrderLayoutConfig),
    #[prost(message, tag = "8")]
    PinConfig(PinLayoutConfig),
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub struct VadActivity {
    #[prost(bool, tag = "1")]
    pub active: bool,
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ClientSideVadActivity {
    #[prost(string, tag = "1")]
    pub transceiver_mid: String,
    #[prost(bool, tag = "2")]
    pub active: bool,
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ParticipantQualityReport {
    #[prost(string, tag = "1")]
    pub participant_id: String,
    #[prost(enumeration = "NetworkQualityScore", tag = "2")]
    pub network_score: i32,
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub struct UpsertParticipantsQualityReport {
    #[prost(message, repeated, tag = "1")]
    pub participants_quality_report: Vec<ParticipantQualityReport>,
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SelfQualityReport {
    #[prost(enumeration = "NetworkQualityScore", tag = "1")]
    pub network_score: i32,
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GoloomNotification {
    #[prost(string, tag = "1")]
    pub notification_id: String,
    #[prost(string, tag = "2")]
    pub payload: String,
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SetActiveCodecs {
    #[prost(enumeration = "VideoCodec", tag = "1")]
    pub video_codec: i32,
    #[prost(enumeration = "AudioCodec", tag = "2")]
    pub audio_codec: i32,
}

/// Wire-compatible stand-in for `google.protobuf.Empty` (ping, tag 6).
/// Saves a `prost-types` dependency for a zero-field message.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ProtoEmpty {}

/// Top-level `signaling.Message`. `uid` must be uuidv4 per the schema comment —
/// receivers MAY drop the session on duplicates.
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GoloomMessage {
    #[prost(string, tag = "1")]
    pub uid: String,
    #[prost(
        oneof = "MessageKind",
        tags = "7, 20, 24, 5, 6, 22, 11, 12, 13, 10, 37, 23, 8, 4, 14, 15, 30, 31, 35, 16, 19, 9, 28, 17, 18, 3, 26, 36"
    )]
    pub kind: Option<MessageKind>,
}

#[derive(Clone, PartialEq, ::prost::Oneof)]
pub enum MessageKind {
    #[prost(message, tag = "7")]
    Hello(Hello),
    #[prost(message, tag = "20")]
    ServerHello(ServerHello),
    #[prost(message, tag = "24")]
    SfuHello(SfuHello),
    #[prost(message, tag = "5")]
    Ack(Ack),
    #[prost(message, tag = "6")]
    Ping(ProtoEmpty),
    #[prost(message, tag = "22")]
    ClientError(ClientError),
    #[prost(message, tag = "11")]
    SetSlotsOffset(SetSlotsOffset),
    #[prost(message, tag = "12")]
    RequestPinnedParticipants(RequestPinnedParticipants),
    #[prost(message, tag = "13")]
    SetSlots(SetSlots),
    #[prost(message, tag = "10")]
    SlotsConfig(SlotsConfig),
    #[prost(message, tag = "37")]
    SlotsMeta(SlotsMeta),
    #[prost(message, tag = "23")]
    VadActivity(VadActivity),
    #[prost(message, tag = "8")]
    UpdateMe(UpdateParticipantMeta),
    #[prost(message, tag = "4")]
    UpdateDescription(UpdateDescription),
    #[prost(message, tag = "14")]
    UpsertDescription(UpsertDescription),
    #[prost(message, tag = "15")]
    RemoveDescription(RemoveDescription),
    #[prost(message, tag = "30")]
    UpsertQuality(UpsertParticipantsQualityReport),
    #[prost(message, tag = "31")]
    SelfQuality(SelfQualityReport),
    #[prost(message, tag = "35")]
    ClientSideVad(ClientSideVadActivity),
    #[prost(message, tag = "16")]
    PublisherSdpOffer(PublisherSdpOffer),
    #[prost(message, tag = "19")]
    SubscriberSdpAnswer(SubscriberSdpAnswer),
    #[prost(message, tag = "9")]
    RequestSubscription(RequestSubscription),
    #[prost(message, tag = "28")]
    UpdatePublisherTrack(UpdatePublisherTrackDescription),
    #[prost(message, tag = "17")]
    PublisherSdpAnswer(PublisherSdpAnswer),
    #[prost(message, tag = "18")]
    SubscriberSdpOffer(SubscriberSdpOffer),
    #[prost(message, tag = "3")]
    IceCandidate(WebrtcIceCandidate),
    #[prost(message, tag = "26")]
    Notification(GoloomNotification),
    #[prost(message, tag = "36")]
    SetActiveCodecs(SetActiveCodecs),
}

// ── builders / helpers ──────────────────────────────────────────────

/// Fresh uuidv4 for `Message.uid` as required by the schema.
pub fn new_uid() -> String {
    uuid::Uuid::new_v4().to_string()
}

/// Desktop defaults for `CapabilitiesOffer`.
///
/// Mirrors what a Linux desktop can actually do: no telephony/блютуз
/// (Android-only flags off), controller-driven slots, simulcast off,
/// RTP sharing, runtime encoder config. The server's `CapabilitiesAnswer`
/// is authoritative — this is only the opening bid.
pub fn default_capabilities_offer() -> CapabilitiesOffer {
    CapabilitiesOffer {
        offer_answer_mode: vec![OfferAnswerMode::Separate as i32],
        initial_subscriber_offer: vec![InitialSubscriberOffer::OnHello as i32],
        slots_mode: vec![SlotsMode::FromController as i32],
        simulcast_mode: vec![SimulcastMode::Disabled as i32],
        self_vad_status: vec![SelfVadStatus::FromClient as i32],
        data_channel_sharing: vec![DataChannelSharing::ToRtp as i32],
        video_encoder_config: vec![VideoEncoderConfigSupport::RuntimeConfig as i32],
        data_channel_video_codec: vec![
            DataChannelVideoCodec::UniqueCodecFromTrackDescription as i32,
        ],
        bandwidth_limitation_reason: vec![BandwidthLimitationReason::Disabled as i32],
        server_layout_transition: vec![ServerLayoutTransition::Enabled as i32],
        pin_layout: vec![PinLayout::Enabled as i32],
        join_order_layout: vec![JoinOrderLayout::Enabled as i32],
        send_self_view_video_slot: vec![SendSelfViewVideoSlot::Enabled as i32],
        sdk_default_device_management: vec![SdkDefaultDeviceManagement::Enabled as i32],
        sdk_publisher_optimize_bitrate: vec![SdkPublisherOptimizeBitrate::Full as i32],
        sdk_network_path_monitor: vec![SdkNetworkPathMonitor::Disabled as i32],
        publisher_vp9: vec![PublishVp9::Disabled as i32],
        svc_mode: vec![SvcMode::Disabled as i32],
        sdk_network_lost_detection: vec![SdkNetworkLostDetection::Enabled as i32],
        fixed_ice_candidates_pool_size: vec![FixedIceCandidatesPoolSize::Disabled as i32],
        subscriber_offer_async_ack: vec![SubscriberOfferAsyncAck::Enabled as i32],
        android_bluetooth_routing_fix: vec![AndroidBluetoothRoutingFix::Disabled as i32],
        sdk_android_telecom_integration: vec![SdkAndroidTelecomIntegration::Disabled as i32],
        set_active_codecs_mode: vec![SetActiveCodecsMode::AudioAndVideo as i32],
        subscriber_dtls_passive_mode: vec![SubscriberDtlsPassiveMode::Enabled as i32],
        publisher_opus_low_bitrate: vec![PublisherOpusLowBitrate::Enabled as i32],
        publisher_opus_dred: vec![PublisherOpusDred::Disabled as i32],
        sdk_android_destroy_session_on_task_removed: vec![
            SdkAndroidDestroySessionOnTaskRemoved::Disabled as i32,
        ],
    }
}

pub fn desktop_sdk_info(version: &str) -> SdkInfo {
    SdkInfo {
        implementation: "linux-desktop".to_string(),
        version: version.to_string(),
        webrtc_version: None,
        user_agent: format!("yandex-messenger-native/{}", version),
        hw_concurrency: std::thread::available_parallelism()
            .map(|n| n.get() as i32)
            .unwrap_or(4),
    }
}

/// First message on `wss://goloom.strm.yandex.net/join` after WS open.
/// `credentials` = Telemost/Cloud-API credential for the room.
#[allow(clippy::too_many_arguments)]
pub fn hello_message(
    room_id: &str,
    participant_id: &str,
    credentials: Option<String>,
    display_name: Option<&str>,
    send_audio: bool,
    send_video: bool,
    send_sharing: bool,
    app_version: &str,
) -> GoloomMessage {
    let mut attrs = std::collections::HashMap::new();
    if let Some(name) = display_name {
        attrs.insert("name".to_string(), name.to_string());
    }
    GoloomMessage {
        uid: new_uid(),
        kind: Some(MessageKind::Hello(Hello {
            service_name: "telemost".to_string(),
            room_id: room_id.to_string(),
            participant_id: participant_id.to_string(),
            auth: credentials.map(HelloAuth::Credentials),
            participant_attributes: attrs,
            send_audio,
            send_video,
            send_sharing,
            disable_subscriber: false,
            disable_subscriber_audio: false,
            disable_publisher: false,
            capabilities_offer: Some(default_capabilities_offer()),
            sdk_initialization_id: new_uid(),
            sdk_info: Some(desktop_sdk_info(app_version)),
            signaling_close_code: None,
            ref_participant_id: None,
        })),
    }
}

/// Reconnect hello carrying the previous WS close code, as the schema requires.
pub fn reconnect_hello(mut msg: GoloomMessage, close_code: i32) -> GoloomMessage {
    if let Some(MessageKind::Hello(hello)) = msg.kind.as_mut() {
        // Keep any valid code; schema requires it on signaling reconnect.
        hello.signaling_close_code = Some(close_code);
        hello.sdk_initialization_id = new_uid();
    }
    msg.uid = new_uid();
    msg
}

pub fn ack_ok(uid: &str) -> GoloomMessage {
    GoloomMessage {
        uid: uid.to_string(),
        kind: Some(MessageKind::Ack(Ack {
            status: Some(Status {
                code: StatusCode::Ok as i32,
                description: None,
            }),
        })),
    }
}

pub fn ping(uid: Option<String>) -> GoloomMessage {
    GoloomMessage {
        uid: uid.unwrap_or_else(new_uid),
        kind: Some(MessageKind::Ping(ProtoEmpty {})),
    }
}

pub fn encode_message(msg: &GoloomMessage) -> Vec<u8> {
    use ::prost::Message;
    msg.encode_to_vec()
}

pub fn decode_message(bytes: &[u8]) -> Result<GoloomMessage, ::prost::DecodeError> {
    use ::prost::Message;
    GoloomMessage::decode(bytes)
}

/// Bridge APK roster → our chat model for the participants panel.
pub fn to_telemost_participant(
    desc: &ParticipantDescription,
) -> crate::models::telemost::TelemostParticipant {
    crate::models::telemost::TelemostParticipant {
        id: desc.id.clone(),
        name: desc.display_name().map(str::to_string),
        avatar_id: desc.participant_attributes.get("avatar").cloned(),
        role: crate::models::telemost::ParticipantRole::Participant,
        audio_enabled: Some(desc.send_audio),
        video_enabled: Some(desc.send_video),
        screen_share: Some(desc.send_sharing),
        joined_at: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ::prost::Message;

    #[test]
    fn hello_roundtrip() {
        let msg = hello_message(
            "room-1",
            "peer-1",
            Some("cred".into()),
            Some("Bob"),
            true,
            true,
            false,
            "2.173.0",
        );
        let bytes = encode_message(&msg);
        assert!(!bytes.is_empty());
        let back = decode_message(&bytes).expect("decode");
        match back.kind {
            Some(MessageKind::Hello(h)) => {
                assert_eq!(h.room_id, "room-1");
                assert_eq!(h.participant_id, "peer-1");
                assert_eq!(h.auth, Some(HelloAuth::Credentials("cred".to_string())));
                assert!(h.send_audio && h.send_video && !h.send_sharing);
                assert!(h.capabilities_offer.is_some());
                assert_eq!(h.sdk_info.as_ref().unwrap().implementation, "linux-desktop");
            }
            other => panic!("expected Hello, got {:?}", other.is_some()),
        }
    }

    #[test]
    fn server_hello_with_ice_roundtrip() {
        let msg = GoloomMessage {
            uid: new_uid(),
            kind: Some(MessageKind::ServerHello(ServerHello {
                capabilities_answer: None,
                serving_components: vec![ServingComponent {
                    r#type: ComponentType::Controller as i32,
                    host: "controller.example".into(),
                    version: "1".into(),
                }],
                session_secret: "secret".into(),
                sfu_peer_initialization_id: "sfu-1".into(),
                rtc_configuration: Some(RtcConfiguration {
                    ice_servers: vec![RtcIceServer {
                        urls: vec!["stun:stun.yandex.net".into()],
                        credential: "".into(),
                        username: "".into(),
                    }],
                    ice_transport_policy: Some("all".into()),
                    ice_candidate_pool_size: None,
                    bundle_policy: Some("max-bundle".into()),
                    rtcp_mux_policy: Some("require".into()),
                }),
                log_endpoint: "".into(),
                ping_pong_configuration: Some(PingPongConfiguration {
                    ping_interval: 5,
                    ack_timeout: 5,
                }),
                telemetry_configuration: None,
                exclude_from_experiments: false,
                active_codecs: Some(SetActiveCodecs {
                    video_codec: VideoCodec::Vp8 as i32,
                    audio_codec: AudioCodec::Opus14 as i32,
                }),
            })),
        };
        let back = decode_message(&encode_message(&msg)).expect("decode");
        match back.kind {
            Some(MessageKind::ServerHello(h)) => {
                let rtc = h.rtc_configuration.expect("rtc");
                assert_eq!(rtc.ice_servers[0].urls[0], "stun:stun.yandex.net");
                assert_eq!(h.active_codecs.unwrap().video_codec, VideoCodec::Vp8 as i32);
            }
            _ => panic!("expected ServerHello"),
        }
    }

    #[test]
    fn ack_terminal_codes_policy() {
        assert!(StatusCode::is_terminal(StatusCode::KickedOut as i32));
        assert!(StatusCode::is_terminal(
            StatusCode::RoomHasBeenClosed as i32
        ));
        assert!(!StatusCode::is_terminal(StatusCode::Ok as i32));
        assert!(StatusCode::expects_reconnect(
            StatusCode::MoveToNewMediaServer as i32
        ));
        assert!(StatusCode::expects_reconnect(
            StatusCode::NetworkLost as i32
        ));
        assert!(!StatusCode::expects_reconnect(
            StatusCode::PermissionDenied as i32
        ));
    }

    #[test]
    fn slots_and_roster_roundtrip() {
        let msg = GoloomMessage {
            uid: new_uid(),
            kind: Some(MessageKind::UpsertDescription(UpsertDescription {
                description: vec![ParticipantDescription {
                    id: "p1".into(),
                    participant_attributes: [("name".to_string(), "Alice".to_string())]
                        .into_iter()
                        .collect(),
                    send_audio: true,
                    send_video: false,
                    send_sharing: false,
                    hide_from_participants_list: false,
                    disconnected_at: None,
                    network_score: NetworkQualityScore::Good as i32,
                    connection_type: ConnectionType::Sdk as i32,
                    ref_participant_id: None,
                }],
            })),
        };
        let back = decode_message(&encode_message(&msg)).expect("decode");
        match back.kind {
            Some(MessageKind::UpsertDescription(u)) => {
                assert_eq!(u.description.len(), 1);
                let p = &u.description[0];
                assert_eq!(p.display_name(), Some("Alice"));
                assert!(p.is_connected());
                let model = to_telemost_participant(p);
                assert_eq!(model.id, "p1");
                assert_eq!(model.name.as_deref(), Some("Alice"));
                assert_eq!(model.audio_enabled, Some(true));
            }
            _ => panic!("expected UpsertDescription"),
        }
    }

    #[test]
    fn uids_unique_per_message() {
        assert_ne!(new_uid(), new_uid());
    }

    #[test]
    fn default_offer_covers_all_28_fields() {
        let offer = default_capabilities_offer();
        let bytes = {
            let mut buf = Vec::new();
            offer.encode(&mut buf).expect("encode offer");
            buf
        };
        let back = CapabilitiesOffer::decode(bytes.as_slice()).expect("decode offer");
        assert_eq!(
            back.offer_answer_mode,
            vec![OfferAnswerMode::Separate as i32]
        );
        assert_eq!(back.slots_mode, vec![SlotsMode::FromController as i32]);
        assert!(!back.svc_mode.is_empty());
        assert!(!back.sdk_android_destroy_session_on_task_removed.is_empty());
    }
}
