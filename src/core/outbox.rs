//! Persistent outbox for unsent text messages (retry on reconnect).

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutboxItem {
    pub id: String,
    pub chat_id: String,
    pub text: String,
    pub reply_to: Option<String>,
    pub created_at: u64,
    pub attempts: u32,
    pub last_error: Option<String>,
}

pub struct Outbox {
    path: PathBuf,
    items: Mutex<Vec<OutboxItem>>,
}

impl Outbox {
    pub fn open() -> Self {
        let path = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("yandex-messenger-native")
            .join("outbox.json");
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let items = Self::load_from(&path).unwrap_or_default();
        Self {
            path,
            items: Mutex::new(items),
        }
    }

    fn load_from(path: &PathBuf) -> Option<Vec<OutboxItem>> {
        let data = std::fs::read_to_string(path).ok()?;
        serde_json::from_str(&data).ok()
    }

    fn persist(&self, items: &[OutboxItem]) {
        if let Ok(json) = serde_json::to_string_pretty(items) {
            let _ = std::fs::write(&self.path, json);
        }
    }

    pub fn enqueue(
        &self,
        chat_id: &str,
        text: &str,
        reply_to: Option<&str>,
        error: Option<String>,
    ) -> OutboxItem {
        let item = OutboxItem {
            id: Uuid::new_v4().to_string(),
            chat_id: chat_id.to_string(),
            text: text.to_string(),
            reply_to: reply_to.map(|s| s.to_string()),
            created_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
            attempts: 0,
            last_error: error,
        };
        let mut guard = self.items.lock().unwrap();
        guard.push(item.clone());
        self.persist(&guard);
        log::info!(
            "Outbox enqueue id={} chat={} (queue={})",
            item.id,
            chat_id,
            guard.len()
        );
        item
    }

    pub fn list(&self) -> Vec<OutboxItem> {
        self.items.lock().unwrap().clone()
    }

    pub fn len(&self) -> usize {
        self.items.lock().unwrap().len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.lock().unwrap().is_empty()
    }

    pub fn mark_attempt(&self, id: &str, error: Option<String>) {
        let mut guard = self.items.lock().unwrap();
        if let Some(item) = guard.iter_mut().find(|i| i.id == id) {
            item.attempts = item.attempts.saturating_add(1);
            item.last_error = error;
        }
        self.persist(&guard);
    }

    pub fn remove(&self, id: &str) {
        let mut guard = self.items.lock().unwrap();
        guard.retain(|i| i.id != id);
        self.persist(&guard);
    }

    /// Drop items with too many failed attempts (default 20).
    pub fn purge_dead(&self, max_attempts: u32) -> usize {
        let mut guard = self.items.lock().unwrap();
        let before = guard.len();
        guard.retain(|i| i.attempts < max_attempts);
        let removed = before - guard.len();
        if removed > 0 {
            self.persist(&guard);
        }
        removed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_outbox_enqueue_remove() {
        let dir = std::env::temp_dir().join(format!("ym_outbox_test_{}", Uuid::new_v4()));
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("outbox.json");
        let box_ = Outbox {
            path: path.clone(),
            items: Mutex::new(Vec::new()),
        };
        let item = box_.enqueue("c1", "hello", None, Some("net".into()));
        assert_eq!(box_.len(), 1);
        assert_eq!(box_.list()[0].text, "hello");
        box_.mark_attempt(&item.id, Some("retry".into()));
        assert_eq!(box_.list()[0].attempts, 1);
        box_.remove(&item.id);
        assert!(box_.is_empty());
        let _ = fs::remove_dir_all(&dir);
    }
}
