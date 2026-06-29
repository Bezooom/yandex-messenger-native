#![allow(dead_code)]

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Saved message stored in the user's "Saved Messages"
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedMessage {
    pub message_id: String,
    pub source_chat_id: String,
    pub source_message: String, // The original message ID
    pub saved_at: DateTime<Utc>,
    pub note: Option<String>,
    pub media_type: Option<String>,  // "text", "image", "link", "file"
    pub preview: Option<String>,
}

/// Filter type for saved messages list
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SavedFilter {
    All,
    Text,
    Images,
    Links,
    Files,
}

impl Default for SavedFilter {
    fn default() -> Self {
        SavedFilter::All
    }
}

impl SavedFilter {
    pub fn label(&self) -> &'static str {
        match self {
            SavedFilter::All => "Все",
            SavedFilter::Text => "Текст",
            SavedFilter::Images => "Изображения",
            SavedFilter::Links => "Ссылки",
            SavedFilter::Files => "Файлы",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            SavedFilter::All => "view-list-symbolic",
            SavedFilter::Text => "document-open-symbolic",
            SavedFilter::Images => "image-x-generic-symbolic",
            SavedFilter::Links => "x-office-document-symbolic",
            SavedFilter::Files => "package-x-generic-symbolic",
        }
    }
}
