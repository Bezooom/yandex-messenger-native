use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Thread — nested conversation attached to a message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Thread {
    pub thread_id: String,
    pub chat_id: String,
    pub parent_message_id: String,
    pub reply_count: u32,
    pub last_reply_at: Option<DateTime<Utc>>,
    pub is_muted: bool,
}

impl Thread {
    pub fn new(
        thread_id: String,
        chat_id: String,
        parent_message_id: String,
    ) -> Self {
        Self {
            thread_id,
            chat_id,
            parent_message_id,
            reply_count: 0,
            last_reply_at: None,
            is_muted: false,
        }
    }

    /// Returns true if the thread has replies
    pub fn has_replies(&self) -> bool {
        self.reply_count > 0
    }
}
