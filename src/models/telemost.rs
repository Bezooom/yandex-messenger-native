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

// ── Messenger Cloud API: personal meetings (from APK 3.12.0.138) ──
// Method names verified in DEX (`create_personal_meeting`,
// `start_meeting_call[_ringing]`, `end_personal_meeting`, `meeting_info[s]`,
// `join_url`, `meetingId(s)`); HTTP paths/bodies are best-effort and must be
// confirmed against a live server. All payloads stay tolerant: unknown
// fields are kept in `extra` instead of failing the parse.

/// A personal meeting (a callable room).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonalMeeting {
    #[serde(default, alias = "meetingId")]
    pub meeting_id: String,
    #[serde(default, alias = "joinUrl")]
    pub join_url: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    /// Anything the server sends that we don't model yet.
    #[serde(default, flatten)]
    pub extra: serde_json::Value,
}

/// Credentials to join a meeting over Goloom signaling.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeetingCall {
    #[serde(default, alias = "callId")]
    pub call_id: Option<String>,
    /// Goloom room id (falls back to the meeting id when absent).
    #[serde(default, alias = "roomId")]
    pub room_id: Option<String>,
    /// In-band auth for the Goloom Hello.
    #[serde(default)]
    pub credentials: Option<String>,
    #[serde(default, alias = "participantId")]
    pub participant_id: Option<String>,
    #[serde(default, alias = "joinUrl")]
    pub join_url: Option<String>,
    #[serde(default, flatten)]
    pub extra: serde_json::Value,
}

impl MeetingCall {
    /// Room id for [`crate::api::goloom::hello_message`].
    pub fn effective_room_id<'a>(&'a self, meeting_id: &'a str) -> &'a str {
        self.room_id.as_deref().unwrap_or(meeting_id)
    }
}

/// Meeting metadata + state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeetingInfo {
    #[serde(default, alias = "meetingId")]
    pub meeting_id: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default, alias = "joinUrl")]
    pub join_url: Option<String>,
    #[serde(default)]
    pub participants: Vec<MeetingParticipant>,
    #[serde(default, flatten)]
    pub extra: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeetingParticipant {
    #[serde(default, alias = "userId")]
    pub user_id: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default, flatten)]
    pub extra: serde_json::Value,
}

/// Server-side validation failure (`UserErrors` envelope).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeetingUserError {
    #[serde(default)]
    pub code: Option<String>,
    #[serde(default)]
    pub message: Option<String>,
}

/// Extract a `telemost.yandex.ru/j/<id>` (or `/c/…`, `/link…`) meeting id.
pub fn meeting_id_from_url(url: &str) -> Option<String> {
    let url = url.trim();
    for marker in ["/j/", "/c/", "link#", "link="] {
        if let Some(pos) = url.find(marker) {
            let tail = &url[pos + marker.len()..];
            let id: String = tail
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
                .collect();
            if !id.is_empty() {
                return Some(id);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn meeting_id_from_url_shapes() {
        assert_eq!(
            meeting_id_from_url("https://telemost.yandex.ru/j/abc-123").as_deref(),
            Some("abc-123")
        );
        assert_eq!(
            meeting_id_from_url("https://telemost.yandex.ru/link#xyz_9").as_deref(),
            Some("xyz_9")
        );
        assert_eq!(meeting_id_from_url("https://yandex.ru/chat"), None);
    }

    #[test]
    fn tolerant_personal_meeting_parse() {
        let m: PersonalMeeting = serde_json::from_value(serde_json::json!({
            "meetingId": "m1",
            "join_url": "https://telemost.yandex.ru/j/m1",
            "future_field": {"nested": 1},
        }))
        .expect("parse");
        assert_eq!(m.meeting_id, "m1");
        assert!(m.extra.get("future_field").is_some());
    }
}

// ── incoming call invites (push / chat traffic) ──

/// Someone rings us: a meeting to join.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallInvite {
    pub meeting_id: String,
    pub join_url: Option<String>,
    pub peer_name: Option<String>,
    pub chat_id: Option<String>,
}

/// Server methods that directly carry a call invite (best-effort names from
/// DEX: `start_meeting_call_ringing`, `meeting_ring*`, …).
const INVITE_METHODS: &[&str] = &[
    "call_invite",
    "incoming_call",
    "incoming_meeting_call",
    "meeting_call_started",
    "meeting_ringing",
    "ringing",
    "start_meeting_call_ringing",
    "telemost_invite",
    "call_started",
];

/// Words marking a chat text as a call (not a mere link share).
const CALL_MARKERS: &[&str] = &[
    "звон",
    "звонит",
    "входящ",
    "созвон",
    "подключ",
    "call",
    "ring",
    "join",
    "incoming",
];

/// Find all Telemost meeting ids mentioned in free text.
pub fn find_telemost_ids(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for token in text.split_whitespace() {
        let trimmed = token.trim_matches(|c: char| " \t\n\"'<>(),.!?;:".contains(c));
        if trimmed.contains("telemost.") {
            if let Some(id) = meeting_id_from_url(trimmed) {
                if !out.contains(&id) {
                    out.push(id);
                }
            }
        }
    }
    out
}

fn str_field(value: &serde_json::Value, keys: &[&str]) -> Option<String> {
    for key in keys {
        if let Some(s) = value.get(key).and_then(|v| v.as_str()) {
            if !s.trim().is_empty() {
                return Some(s.to_string());
            }
        }
    }
    None
}

fn invite_from_object(obj: &serde_json::Value, chat_id: Option<String>) -> Option<CallInvite> {
    let meeting_id = str_field(obj, &["meetingId", "meeting_id"])?;
    let join_url = str_field(obj, &["join_url", "joinUrl", "link", "url"])
        .or_else(|| Some(format!("https://telemost.yandex.ru/j/{meeting_id}")));
    let peer_name = str_field(
        obj,
        &[
            "display_name",
            "public_name",
            "contact_name",
            "name",
            "peer_name",
        ],
    )
    .or_else(|| {
        obj.get("user")
            .and_then(|u| str_field(u, &["display_name", "public_name", "contact_name", "name"]))
            .or_else(|| {
                obj.get("from").and_then(|u| {
                    str_field(u, &["display_name", "public_name", "contact_name", "name"])
                })
            })
    });
    Some(CallInvite {
        meeting_id,
        join_url,
        peer_name,
        chat_id: chat_id.or_else(|| str_field(obj, &["chat_id", "chatId"])),
    })
}

/// Extract an incoming-call invite from a push/WS payload.
///
/// - Direct invite methods → parsed fields (meeting id required).
/// - `new_message` → a message counts when it carries call keys
///   (`call`/`meeting_call`/`telemost` objects) or a Telemost URL *plus* a
///   call marker word. A bare shared link never rings (no spam).
/// - Anything else → `None`.
pub fn extract_invite(method: &str, payload: &serde_json::Value) -> Option<CallInvite> {
    if INVITE_METHODS.contains(&method) {
        let obj = payload.get("params").unwrap_or(payload);
        return invite_from_object(obj, None);
    }
    if method != "new_message" {
        return None;
    }
    let messages = payload.get("messages")?.as_array()?;
    for msg in messages {
        if msg.get("call").is_some()
            || msg.get("meeting_call").is_some()
            || msg.get("telemost").is_some()
        {
            if let Some(inv) = invite_from_object(
                msg.get("call")
                    .or_else(|| msg.get("meeting_call"))
                    .or_else(|| msg.get("telemost"))
                    .unwrap_or(msg),
                str_field(msg, &["chat_id", "chatId", "chatID"]),
            ) {
                return Some(inv);
            }
        }
        let text = str_field(msg, &["text", "body"]).unwrap_or_default();
        let ids = find_telemost_ids(&text);
        if ids.is_empty() {
            continue;
        }
        let lowered = text.to_lowercase();
        if !CALL_MARKERS.iter().any(|m| lowered.contains(m)) {
            continue;
        }
        let peer_name = msg
            .get("sender")
            .and_then(|s| str_field(s, &["display_name", "public_name", "contact_name", "name"]))
            .or_else(|| str_field(msg, &["sender_name", "author"]));
        return Some(CallInvite {
            join_url: Some(format!("https://telemost.yandex.ru/j/{}", ids[0])),
            meeting_id: ids[0].clone(),
            peer_name,
            chat_id: str_field(msg, &["chat_id", "chatId", "chatID"]),
        });
    }
    None
}

#[cfg(test)]
mod invite_tests {
    use super::*;

    #[test]
    fn direct_invite_methods() {
        let inv = extract_invite(
            "start_meeting_call_ringing",
            &serde_json::json!({
                "params": {"meetingId": "m7", "user": {"display_name": "Bob"}}
            }),
        )
        .expect("invite");
        assert_eq!(inv.meeting_id, "m7");
        assert_eq!(inv.peer_name.as_deref(), Some("Bob"));
        assert!(inv.join_url.unwrap().contains("m7"));
    }

    #[test]
    fn message_call_object_invites() {
        let inv = extract_invite(
            "new_message",
            &serde_json::json!({"messages": [{
                "chat_id": "c1",
                "call": {"meeting_id": "m9", "joinUrl": "https://telemost.yandex.ru/j/m9"},
            }]}),
        )
        .expect("invite");
        assert_eq!(inv.meeting_id, "m9");
        assert_eq!(inv.chat_id.as_deref(), Some("c1"));
    }

    #[test]
    fn message_link_with_marker_invites() {
        let inv = extract_invite(
            "new_message",
            &serde_json::json!({"messages": [{
                "chat_id": "c2",
                "text": "Звони сюда https://telemost.yandex.ru/j/abc-1",
                "sender": {"display_name": "Alice"},
            }]}),
        )
        .expect("invite");
        assert_eq!(inv.meeting_id, "abc-1");
        assert_eq!(inv.peer_name.as_deref(), Some("Alice"));
    }

    #[test]
    fn bare_link_share_does_not_ring() {
        assert!(extract_invite(
            "new_message",
            &serde_json::json!({"messages": [{
                "chat_id": "c3",
                "text": "запись встречи https://telemost.yandex.ru/j/zzz",
            }]}),
        )
        .is_none());
    }

    #[test]
    fn unknown_method_ignored() {
        assert!(extract_invite("typing_enhanced", &serde_json::json!({})).is_none());
    }
}
