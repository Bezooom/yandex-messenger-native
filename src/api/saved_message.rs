#![allow(dead_code)]

use crate::models::saved_message::{SavedFilter, SavedMessage};
use chrono::Utc;
use tokio::sync::Mutex;

/// In-memory storage for saved messages
pub struct SavedMessageStore {
    messages: Mutex<Vec<SavedMessage>>,
}

impl Default for SavedMessageStore {
    fn default() -> Self {
        Self {
            messages: Mutex::new(Vec::new()),
        }
    }
}

impl SavedMessageStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn load_from_json(&self, json_str: &str) -> Result<(), String> {
        let messages: Vec<SavedMessage> = serde_json::from_str(json_str)
            .map_err(|e| format!("Failed to parse saved messages: {}", e))?;
        let mut store = self.messages.lock().await;
        *store = messages;
        Ok(())
    }

    pub async fn save_to_json(&self) -> String {
        let store = self.messages.lock().await;
        serde_json::to_string_pretty(&*store).unwrap_or_default()
    }

    pub async fn get_messages(
        &self,
        limit: usize,
        offset: usize,
        filter: SavedFilter,
    ) -> Vec<SavedMessage> {
        let store = self.messages.lock().await;
        let mut msgs = store.clone();
        msgs.sort_by(|a, b| b.saved_at.cmp(&a.saved_at));
        
        let total = msgs.len();
        let start = offset.min(total);
        let end = (start + limit).min(total);
        let mut result: Vec<SavedMessage> = msgs[start..end].to_vec();

        // Apply filter
        if filter != SavedFilter::All {
            result.retain(|m| match filter {
                SavedFilter::Text => {
                    m.media_type.as_deref() == Some("text") || m.media_type.is_none()
                }
                SavedFilter::Images => m.media_type.as_deref() == Some("image"),
                SavedFilter::Links => m.media_type.as_deref() == Some("link"),
                SavedFilter::Files => m.media_type.as_deref() == Some("file"),
                _ => true,
            });
        }

        result
    }

    pub async fn add_message(&self, msg: SavedMessage) {
        let mut store = self.messages.lock().await;
        store.push(msg);
    }

    pub async fn remove_message(&self, message_id: &str) {
        let mut store = self.messages.lock().await;
        store.retain(|m| m.message_id != message_id);
    }

    pub async fn update_note(&self, message_id: &str, note: String) {
        let mut store = self.messages.lock().await;
        if let Some(msg) = store.iter_mut().find(|m| m.message_id == message_id) {
            msg.note = Some(note);
        }
    }

    pub async fn search(&self, query: &str, limit: usize) -> Vec<SavedMessage> {
        let store = self.messages.lock().await;
        let q_lower = query.to_lowercase();
        let mut results: Vec<SavedMessage> = store
            .iter()
            .filter(|m| {
                m.preview.as_deref().unwrap_or("").to_lowercase().contains(&q_lower)
                    || m.note.as_deref().unwrap_or("").to_lowercase().contains(&q_lower)
                    || m.source_message.contains(&q_lower)
            })
            .cloned()
            .collect();
        results.sort_by(|a, b| b.saved_at.cmp(&a.saved_at));
        let total = results.len();
        let end = limit.min(total);
        results.truncate(end);
        results
    }
}

/// Global saved messages store (singleton)
static STORE: std::sync::OnceLock<SavedMessageStore> = std::sync::OnceLock::new();

pub fn get_saved_messages_store() -> &'static SavedMessageStore {
    STORE.get_or_init(SavedMessageStore::new)
}

/// API methods for saved messages
pub struct SavedMessagesApi {
    store: SavedMessageStore,
}

impl SavedMessagesApi {
    pub fn new() -> Self {
        Self {
            store: SavedMessageStore::new(),
        }
    }

    /// Save a message
    pub async fn save_message(
        &self,
        chat_id: &str,
        message_id: &str,
        note: Option<String>,
    ) -> Result<SavedMessage, String> {
        let saved = SavedMessage {
            message_id: message_id.to_string(),
            source_chat_id: chat_id.to_string(),
            source_message: message_id.to_string(),
            saved_at: Utc::now(),
            note,
            media_type: None,
            preview: None,
        };
        self.store.add_message(saved.clone()).await;
        Ok(saved)
    }

    /// Get saved messages with pagination and filtering
    pub async fn get_saved_messages(
        &self,
        limit: usize,
        offset: usize,
        filter: SavedFilter,
    ) -> Result<Vec<SavedMessage>, String> {
        Ok(self.store.get_messages(limit, offset, filter).await)
    }

    /// Remove a message from saved
    pub async fn unsave_message(&self, message_id: &str) -> Result<(), String> {
        self.store.remove_message(message_id).await;
        Ok(())
    }

    /// Update note for a saved message
    pub async fn update_note(&self, message_id: &str, note: String) -> Result<(), String> {
        self.store.update_note(message_id, note).await;
        Ok(())
    }

    /// Search saved messages
    pub async fn search_saved(&self, query: &str, limit: usize) -> Result<Vec<SavedMessage>, String> {
        Ok(self.store.search(query, limit).await)
    }

    /// Get total count of saved messages
    pub async fn get_saved_count(&self) -> usize {
        let store = self.store.messages.lock().await;
        store.len()
    }
}
