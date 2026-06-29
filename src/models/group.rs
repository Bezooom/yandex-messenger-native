use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Group settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupSettings {
    pub chat_id: String,
    pub title: String,
    pub description: Option<String>,
    pub admin_ids: Vec<String>,
    pub join_policy: JoinPolicy,
    pub invite_link: Option<String>,
    pub member_count: u32,
    pub is_public: bool,
    pub is_signatory: bool,
}

/// Join policy for groups
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum JoinPolicy {
    Open,
    Request,
    InviteOnly,
}

/// Channel settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelSettings {
    pub chat_id: String,
    pub title: String,
    pub description: Option<String>,
    pub admin_ids: Vec<String>,
    pub subscribers: Vec<String>,
    pub subscriber_count: u32,
    pub is_signatory: bool,
    pub join_to_send: bool,
}

/// Group member
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupMember {
    pub user_id: String,
    pub role: MemberRole,
    #[serde(with = "chrono::serde::ts_seconds")]
    pub joined_at: DateTime<Utc>,
    #[serde(with = "chrono::serde::ts_seconds_option")]
    pub last_seen: Option<DateTime<Utc>>,
}

/// Member role in a group
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum MemberRole {
    Member,
    Admin,
    Creator,
}

/// Group invite link
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupInvite {
    pub invite_link: String,
    #[serde(with = "chrono::serde::ts_seconds")]
    pub created_at: DateTime<Utc>,
    #[serde(with = "chrono::serde::ts_seconds_option")]
    pub expires_at: Option<DateTime<Utc>>,
    pub usage_limit: Option<u32>,
    pub usage_count: u32,
}
