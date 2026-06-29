#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use uuid::Uuid;

pub mod thread;
pub mod reaction;
pub mod voice_message;
pub mod poll;
pub mod sticker;
pub mod folder;
pub mod saved_message;
pub mod bot;
pub mod scheduled_message;
pub mod group;
pub mod account;

pub use thread::Thread;
pub use reaction::ExtendedReactionsConfig;
pub use voice_message::VoiceMessage;
pub use poll::{Poll, PollAnswer};
pub use sticker::{Sticker, StickerPack, StickerPackList};
pub use folder::ChatFolder;
pub use bot::BotInfo;
pub use bot::BotReplyMarkup;
pub use scheduled_message::ScheduledMessage;
pub use account::Account;

/// Chat type enumeration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ChatType {
    Private,
    Group,
    Channel,
    Bot,
    Unknown,
}

impl Default for ChatType {
    fn default() -> Self {
        ChatType::Unknown
    }
}

/// Chat participant
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Participant {
    pub id: String,
    pub name: Option<String>,
    pub avatar: Option<String>,
    pub status: Option<ParticipantStatus>,
    pub last_seen: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ParticipantStatus {
    Online,
    Offline,
    Away,
    DoNotDisturb,
}

/// Chat object
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Chat {
    #[serde(alias = "chat_id")]
    pub id: String,
    #[serde(alias = "name")]
    pub title: Option<String>,
    pub rid: Option<String>, // Request ID
    pub chat_type: ChatType,
    pub avatar_id: Option<String>,
    pub participants: Vec<Participant>,
    pub unread_count: u32,
    pub last_message: Option<Message>,
    pub pinned: bool,
    pub archived: bool,
    pub muted: bool,
    pub created: Option<DateTime<Utc>>,
    pub updated: Option<DateTime<Utc>>,
}

impl Chat {
    pub fn preview_text(&self) -> String {
        self.last_message
            .as_ref()
            .map(|m| m.preview())
            .unwrap_or_else(|| "No messages".to_string())
    }

    pub fn display_name(&self) -> String {
        self.title.clone().unwrap_or_else(|| "Unknown Chat".to_string())
    }
}

/// Message types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum MessageType {
    Text,
    Image,
    File,
    Voice,
    Video,
    Link,
    System,
    Reply,
    Forward,
    Pin,
    Unpin,
    Kick,
    Invite,
    Call,
    Telemost,
    Reaction,
    Poll,
    Location,
    Contact,
    Sticker,
    AnimatedEmoji,
    ScreenShare,
    Unknown,
}

impl Default for MessageType {
    fn default() -> Self {
        MessageType::Unknown
    }
}

/// Message object
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub chat_id: String,
    pub from_id: String,
    pub message_id: Option<String>, // Internal message ID
    pub rmid: Option<String>,       // Reply message ID
    pub type_: MessageType,
    pub text: Option<String>,
    pub entities: Vec<MessageEntity>,
    pub reply_to: Option<MessageId>,
    pub forward: Option<ForwardInfo>,
    pub media: Vec<MediaAttachment>,
    pub reactions: Vec<Reaction>,
    pub thread_id: Option<String>,
    pub has_thread: bool,
    pub pinned: bool,
    pub edited: bool,
    pub edited_at: Option<DateTime<Utc>>,
    pub sent: bool,
    pub delivered: bool,
    pub read: bool,
    pub created: DateTime<Utc>,
    pub updated: Option<DateTime<Utc>>,
    /// Poll data (if message is a poll)
    pub poll: Option<Poll>,
}

impl Message {
    pub fn preview(&self) -> String {
        match &self.text {
            Some(text) => {
                let truncated: String = text.chars().take(100).collect();
                if text.len() > 100 {
                    format!("{}...", truncated)
                } else {
                    truncated
                }
            }
            None => match self.type_ {
                MessageType::Image => "[Image]".to_string(),
                MessageType::File => "[File]".to_string(),
                MessageType::Voice => "[Voice message]".to_string(),
                MessageType::Video => "[Video]".to_string(),
                MessageType::Sticker => "[Sticker]".to_string(),
                MessageType::Poll => "[Poll]".to_string(),
                _ => "[Message]".to_string(),
            },
        }
    }

    /// Creates a new poll message
    pub fn new_poll(chat_id: String, poll: Poll) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            chat_id,
            from_id: "current_user".to_string(), // Will be set by API
            message_id: None,
            rmid: None,
            type_: MessageType::Poll,
            text: Some(poll.question.clone()),
            entities: vec![],
            reply_to: None,
            forward: None,
            media: vec![],
            reactions: vec![],
            thread_id: None,
            has_thread: false,
            pinned: false,
            edited: false,
            edited_at: None,
            sent: false,
            delivered: false,
            read: false,
            created: Utc::now(),
            updated: None,
            poll: Some(poll),
        }
    }
}

/// Message entity (bold, italic, link, etc.)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageEntity {
    pub offset: usize,
    pub length: usize,
    pub r#type: String, // "bold", "italic", "link", "mention", etc.
    pub url: Option<String>,
}

/// Media attachment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaAttachment {
    pub id: String,
    pub type_: MediaType,
    pub url: String,
    pub thumbnail_url: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub size: Option<u64>,
    pub duration: Option<u64>,
    pub filename: Option<String>,
    pub mime_type: Option<String>,
    pub waveform: Option<Vec<f32>>, // waveform data for voice messages
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum MediaType {
    Image,
    Video,
    Audio,
    Document,
    Voice,
    Sticker,
    AnimatedEmoji,
    Thumbnail,
    Unknown,
}

/// Forward info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForwardInfo {
    pub from_chat_id: Option<String>,
    pub from_message_id: Option<String>,
    pub from_name: Option<String>,
    pub date: DateTime<Utc>,
}

/// Reaction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reaction {
    pub emoji: String,
    pub count: u32,
    pub selected: bool,
    pub user_ids: Vec<String>,
    pub is_extended: bool,
}

/// Message ID reference
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageId {
    pub chat_id: String,
    pub message_id: String,
}

/// Telemost call info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemostCall {
    pub call_id: String,
    pub chat_id: String,
    pub initiator_id: String,
    pub participants: Vec<CallParticipant>,
    pub status: CallStatus,
    pub started_at: DateTime<Utc>,
    pub duration: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum CallStatus {
    Ringing,
    InProgress,
    Missed,
    Ended,
    Declined,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallParticipant {
    pub user_id: String,
    pub audio_enabled: bool,
    pub video_enabled: bool,
    pub screen_share: bool,
    pub is_muted: bool,
}

/// User profile
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: String,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub display_name: Option<String>,
    pub username: Option<String>,
    pub avatar_id: Option<String>,
    pub status: Option<String>,
    pub is_bot: bool,
    pub is_premium: bool,
}

impl User {
    pub fn full_name(&self) -> String {
        if let Some(name) = &self.display_name {
            return name.clone();
        }
        let mut name = String::new();
        if let Some(first) = &self.first_name {
            name.push_str(first);
        }
        if let Some(last) = &self.last_name {
            if !name.is_empty() {
                name.push(' ');
            }
            name.push_str(last);
        }
        if name.is_empty() {
            format!("User {}", &self.id[..8])
        } else {
            name
        }
    }
}

/// WebSocket message format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WSMessage {
    pub seq: u64,
    pub message: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WSResponse {
    pub seq: u64,
    pub result: Option<serde_json::Value>,
    pub error: Option<WSApiError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WSApiError {
    pub code: i32,
    pub message: String,
    pub details: Option<serde_json::Value>,
}

/// Типы WebSocket-сообщений
#[derive(Debug, Clone, PartialEq)]
pub enum WSMessageType {
    ThreadUpdate,
    ReactionUpdate,
    TypingEnhanced,
    PollUpdate,
    StoryView,
    CustomStatus,
    Unknown(String),
}

impl WSMessageType {
    pub fn from_json(value: &serde_json::Value) -> Self {
        if let Some(method) = value.get("method").and_then(|m| m.as_str()) {
            match method {
                "thread_update" => Self::ThreadUpdate,
                "reaction_update" => Self::ReactionUpdate,
                "typing_enhanced" => Self::TypingEnhanced,
                "poll_update" => Self::PollUpdate,
                "story_view" => Self::StoryView,
                "custom_status_update" => Self::CustomStatus,
                other => Self::Unknown(other.to_string()),
            }
        } else {
            Self::Unknown("unknown".to_string())
        }
    }
}

/// Notification
#[derive(Debug, Clone)]
pub struct Notification {
    pub chat_id: String,
    pub chat_title: String,
    pub sender_name: String,
    pub message_preview: String,
    pub timestamp: DateTime<Utc>,
}
