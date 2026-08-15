use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConferenceParams {
    pub ws_uri: Option<String>,
    pub room_id: Option<String>,
    pub peer_id: Option<String>,
    pub phone: Option<String>,
    pub uri: Option<String>,
    pub client_config: Option<ClientConfig>,
    pub platform: Option<String>,
    pub conference_state: Option<String>,
    pub organization_id: Option<String>,
    pub session_id: Option<String>,
    pub credentials: Option<String>,
    pub peer_session_id: Option<String>,
    pub waiting_room_available: Option<bool>,
    pub conference_limit: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientConfig {
    pub url: Option<String>,
    pub properties: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ConferenceState {
    #[serde(rename = "CREATED")]
    Created,
    #[serde(rename = "STARTED")]
    Started,
    #[serde(rename = "FINISHED")]
    Finished,
    #[serde(rename = "UNKNOWN")]
    Unknown,
}

impl Default for ConferenceState {
    fn default() -> Self {
        ConferenceState::Unknown
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemostConference {
    pub id: String,
    pub chat_id: Option<String>,
    pub state: ConferenceState,
    pub participants: Vec<TelemostParticipant>,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub join_url: Option<String>,
    pub host_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemostParticipant {
    pub id: String,
    pub name: Option<String>,
    pub avatar_id: Option<String>,
    pub role: ParticipantRole,
    pub audio_enabled: Option<bool>,
    pub video_enabled: Option<bool>,
    pub screen_share: Option<bool>,
    pub joined_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ParticipantRole {
    Host,
    Participant,
    Viewer,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilitiesOffer {
    pub send_audio: bool,
    pub send_video: bool,
    pub send_sharing: bool,
    pub receive_audio: bool,
    pub receive_video: bool,
}

impl Default for CapabilitiesOffer {
    fn default() -> Self {
        Self {
            send_audio: true,
            send_video: true,
            send_sharing: false,
            receive_audio: true,
            receive_video: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SignalingMessage {
    Hello {
        capabilities_offer: Option<CapabilitiesOffer>,
        credentials: Option<String>,
        disable_publisher: Option<bool>,
        disable_subscriber_audio: Option<bool>,
        disable_subscriber: Option<bool>,
        participant_attributes: Option<serde_json::Value>,
        send_sharing: Option<bool>,
        signaling_close_code: Option<i32>,
        ref_participant_id: Option<String>,
    },
    ServerHello {
        participant_id: Option<String>,
        room_id: Option<String>,
        session_id: Option<String>,
    },
    PublisherSdpOffer {
        sdp: String,
        participant_id: Option<String>,
    },
    PublisherSdpAnswer {
        sdp: String,
        participant_id: Option<String>,
    },
    SubscriberSdpOffer {
        sdp: String,
        participant_id: Option<String>,
    },
    SubscriberSdpAnswer {
        sdp: String,
        participant_id: Option<String>,
    },
    IceCandidate {
        candidate: String,
        sdp_mid: Option<String>,
        sdp_mline_index: Option<u32>,
        participant_id: Option<String>,
    },
    RequestSubscription {
        participant_id: Option<String>,
    },
    SetSlots {
        slots: Vec<SlotInfo>,
    },
    Status {
        code: Option<i32>,
        message: Option<String>,
    },
    Notification {
        event: String,
        payload: Option<serde_json::Value>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlotInfo {
    pub slot_id: Option<String>,
    pub participant_id: Option<String>,
    pub kind: SlotKind,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SlotKind {
    Audio,
    Video,
    ScreenShare,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateConferenceRequest {
    pub chat_id: Option<String>,
    pub title: Option<String>,
    pub waiting_room_enabled: Option<bool>,
    pub max_participants: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateConferenceResponse {
    pub conference: TelemostConference,
    pub params: ConferenceParams,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndConferenceRequest {
    pub conference_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JoinConferenceRequest {
    pub conference_id: String,
    pub capabilities: Option<CapabilitiesOffer>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JoinConferenceResponse {
    pub conference: TelemostConference,
    pub params: ConferenceParams,
}
