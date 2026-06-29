use serde::{Deserialize, Serialize};

/// Chat Folder Model
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChatFolder {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub included_chats: Vec<String>,
    #[serde(default)]
    pub excluded_chats: Vec<String>,
    pub filter: Option<FolderFilter>,
    #[serde(default)]
    pub is_active: bool,
    #[serde(default)]
    pub unread_count: u32,
}

/// Filter settings for a folder
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct FolderFilter {
    #[serde(default)]
    pub include_contacts: bool,
    #[serde(default)]
    pub include_non_contacts: bool,
    #[serde(default)]
    pub include_groups: bool,
    #[serde(default)]
    pub include_channels: bool,
    #[serde(default)]
    pub include_bots: bool,
    #[serde(default)]
    pub exclude_muted: bool,
    #[serde(default)]
    pub exclude_read: bool,
    #[serde(default)]
    pub exclude_archived: bool,
}

impl ChatFolder {
    pub fn new(id: String, title: String) -> Self {
        Self {
            id,
            title,
            included_chats: Vec::new(),
            excluded_chats: Vec::new(),
            filter: None,
            is_active: false,
            unread_count: 0,
        }
    }
}
