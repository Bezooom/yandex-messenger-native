//! Per-chat text drafts persisted to disk.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

#[derive(Debug, Default, Serialize, Deserialize)]
struct DraftFile {
    drafts: HashMap<String, String>,
}

pub struct DraftStore {
    path: PathBuf,
    drafts: Mutex<HashMap<String, String>>,
}

impl DraftStore {
    pub fn open() -> Self {
        let path = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("yandex-messenger-native")
            .join("drafts.json");
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let drafts = std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str::<DraftFile>(&s).ok())
            .map(|f| f.drafts)
            .unwrap_or_default();
        Self {
            path,
            drafts: Mutex::new(drafts),
        }
    }

    fn persist(&self, map: &HashMap<String, String>) {
        let file = DraftFile {
            drafts: map.clone(),
        };
        if let Ok(json) = serde_json::to_string_pretty(&file) {
            let _ = std::fs::write(&self.path, json);
        }
    }

    pub fn get(&self, chat_id: &str) -> Option<String> {
        self.drafts
            .lock()
            .unwrap()
            .get(chat_id)
            .cloned()
            .filter(|s| !s.trim().is_empty())
    }

    pub fn set(&self, chat_id: &str, text: &str) {
        let mut guard = self.drafts.lock().unwrap();
        if text.trim().is_empty() {
            guard.remove(chat_id);
        } else {
            guard.insert(chat_id.to_string(), text.to_string());
        }
        self.persist(&guard);
    }

    pub fn clear(&self, chat_id: &str) {
        let mut guard = self.drafts.lock().unwrap();
        if guard.remove(chat_id).is_some() {
            self.persist(&guard);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_draft_set_get_clear() {
        let dir = std::env::temp_dir().join(format!("ym_drafts_{}", uuid::Uuid::new_v4()));
        let _ = fs::create_dir_all(&dir);
        let store = DraftStore {
            path: dir.join("drafts.json"),
            drafts: Mutex::new(HashMap::new()),
        };
        store.set("c1", "hello");
        assert_eq!(store.get("c1").as_deref(), Some("hello"));
        store.set("c1", "  ");
        assert!(store.get("c1").is_none());
        store.set("c1", "x");
        store.clear("c1");
        assert!(store.get("c1").is_none());
        let _ = fs::remove_dir_all(&dir);
    }
}
