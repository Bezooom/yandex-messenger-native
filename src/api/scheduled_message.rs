use std::sync::Arc;

use chrono::{DateTime, Utc};

use crate::api::auth::AuthManager;
use crate::models::scheduled_message::ScheduledMessage;

/// Клиент для работы с запланированными сообщениями
#[derive(Clone)]
pub struct ScheduledMessageClient {
    auth: Arc<AuthManager>,
}

impl ScheduledMessageClient {
    pub fn new(auth: Arc<AuthManager>) -> Self {
        Self { auth }
    }

    /// Получить список запланированных сообщений для чата
    pub async fn get_scheduled_messages(&self, chat_id: &str) -> Result<Vec<ScheduledMessage>, String> {
        // Сохраняем в кэш
        let cache_dir = dirs::config_dir()
            .map(|d| d.join("yandex-messenger-native").join("cache"))
            .unwrap_or_else(|| std::path::PathBuf::from("/tmp/yandex-messenger-cache"));
        
        std::fs::create_dir_all(&cache_dir).ok();
        let cache_file = cache_dir.join(format!("scheduled_{}.json", chat_id.replace("/", "_")));

        if let Ok(data) = std::fs::read_to_string(&cache_file) {
            if let Ok(messages) = serde_json::from_str::<Vec<ScheduledMessage>>(&data) {
                return Ok(messages);
            }
        }

        // Если кэша нет, возвращаем пустой список
        Ok(Vec::new())
    }

    /// Запланировать отправку сообщения
    pub async fn schedule_message(
        &self,
        chat_id: &str,
        text: &str,
        scheduled_at: DateTime<Utc>,
    ) -> Result<ScheduledMessage, String> {
        let message = ScheduledMessage::new(chat_id, text, scheduled_at);
        
        // Сохраняем в кэш
        let cache_dir = dirs::config_dir()
            .map(|d| d.join("yandex-messenger-native").join("cache"))
            .unwrap_or_else(|| std::path::PathBuf::from("/tmp/yandex-messenger-cache"));
        
        std::fs::create_dir_all(&cache_dir).ok();
        let cache_file = cache_dir.join(format!("scheduled_{}.json", chat_id.replace("/", "_")));

        let mut messages = if let Ok(data) = std::fs::read_to_string(&cache_file) {
            serde_json::from_str::<Vec<ScheduledMessage>>(&data).unwrap_or_default()
        } else {
            Vec::new()
        };

        messages.push(message.clone());
        
        if let Ok(json) = serde_json::to_string(&messages) {
            let _ = std::fs::write(&cache_file, json);
        }

        Ok(message)
    }

    /// Отменить запланированное сообщение
    pub async fn cancel_scheduled_message(
        &self,
        chat_id: &str,
        message_id: &str,
    ) -> Result<(), String> {
        let cache_dir = dirs::config_dir()
            .map(|d| d.join("yandex-messenger-native").join("cache"))
            .unwrap_or_else(|| std::path::PathBuf::from("/tmp/yandex-messenger-cache"));
        
        let cache_file = cache_dir.join(format!("scheduled_{}.json", chat_id.replace("/", "_")));

        let mut messages = if let Ok(data) = std::fs::read_to_string(&cache_file) {
            serde_json::from_str::<Vec<ScheduledMessage>>(&data).unwrap_or_default()
        } else {
            return Err("No scheduled messages found".to_string());
        };

        messages.retain(|m| m.message_id != message_id);
        
        if let Ok(json) = serde_json::to_string(&messages) {
            let _ = std::fs::write(&cache_file, json);
        }

        Ok(())
    }

    /// Обновить время отправки
    pub async fn update_scheduled_time(
        &self,
        chat_id: &str,
        message_id: &str,
        new_scheduled_at: DateTime<Utc>,
    ) -> Result<(), String> {
        let cache_dir = dirs::config_dir()
            .map(|d| d.join("yandex-messenger-native").join("cache"))
            .unwrap_or_else(|| std::path::PathBuf::from("/tmp/yandex-messenger-cache"));
        
        let cache_file = cache_dir.join(format!("scheduled_{}.json", chat_id.replace("/", "_")));

        let mut messages = if let Ok(data) = std::fs::read_to_string(&cache_file) {
            serde_json::from_str::<Vec<ScheduledMessage>>(&data).unwrap_or_default()
        } else {
            return Err("No scheduled messages found".to_string());
        };

        let found = messages.iter_mut().find(|m| m.message_id == message_id);
        if let Some(msg) = found {
            msg.scheduled_at = new_scheduled_at;
            if let Ok(json) = serde_json::to_string(&messages) {
                let _ = std::fs::write(&cache_file, json);
            }
            Ok(())
        } else {
            Err("Message not found".to_string())
        }
    }

    /// Получить быстрые пресеты
    pub fn get_quick_presets(&self) -> Vec<(String, u64)> {
        crate::models::scheduled_message::MessageSchedule::quick_presets()
    }

    /// Получить пресеты на сегодня
    pub fn get_today_presets(&self) -> Vec<(String, DateTime<Utc>)> {
        let now = chrono::Utc::now();
        crate::models::scheduled_message::MessageSchedule::generate_today_presets(&now)
    }
}
