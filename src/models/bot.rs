#![allow(dead_code)]

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Бот, с которым можно общаться
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BotInfo {
    #[serde(alias = "bot_id", alias = "id")]
    pub bot_id: String,
    pub username: Option<String>,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub description: Option<String>,
    pub avatar_id: Option<String>,
    pub commands: Vec<BotCommand>,
    pub can_reply: bool,
    pub inline_modes: Vec<InlineMode>,
    pub is_verified: bool,
}

impl BotInfo {
    pub fn display_name(&self) -> String {
        if let Some(name) = &self.first_name {
            if let Some(last) = &self.last_name {
                return format!("{} {}", name, last);
            }
            return name.clone();
        }
        self.username.clone().unwrap_or_else(|| "Бот".to_string())
    }
}

/// Команда бота
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BotCommand {
    pub command: String,
    pub description: String,
}

/// Режимы поддержки inline
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum InlineMode {
    Empty,
    OnlyInPM,
    Everywhere,
}

/// Inline-кнопка
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InlineButton {
    pub text: String,
    #[serde(default)]
    pub callback_data: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub web_app: Option<String>,
}

/// Reply-клавиатура
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ReplyKeyboard {
    #[serde(default)]
    pub rows: Vec<Vec<KeyboardButton>>,
    #[serde(default)]
    pub is_persistent: bool,
}

/// Одна кнопка reply-клавиатуры
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyboardButton {
    pub text: String,
    #[serde(default)]
    pub request_contact: bool,
    #[serde(default)]
    pub request_location: bool,
}

/// Reply-маркап (inline + reply)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BotReplyMarkup {
    #[serde(default)]
    pub inline_keyboard: Vec<Vec<InlineButton>>,
    #[serde(default)]
    pub keyboard: Option<ReplyKeyboard>,
}

/// Результат inline-калла
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InlineCallback {
    pub callback_id: String,
    pub chat_id: String,
    pub message_id: String,
    pub callback_data: String,
}

/// Результат запуска бота
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BotStartResult {
    pub success: bool,
    pub bot_id: String,
    pub start_param: String,
    pub message: Option<BotMessage>,
}

/// Сообщение от бота
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BotMessage {
    pub id: String,
    pub bot_id: String,
    pub text: Option<String>,
    pub reply_markup: Option<BotReplyMarkup>,
    pub created: DateTime<Utc>,
}
