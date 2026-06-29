use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Статус запланированного сообщения
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ScheduledStatus {
    /// Ожидает отправки
    Pending,
    /// Отправляется
    Sending,
    /// Отправлено
    Sent,
    /// Ошибка отправки
    Failed,
}

impl Default for ScheduledStatus {
    fn default() -> Self {
        ScheduledStatus::Pending
    }
}

/// Запланированное сообщение
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduledMessage {
    /// Уникальный ID сообщения
    pub message_id: String,
    /// ID чата
    pub chat_id: String,
    /// Время отправки (UTC)
    pub scheduled_at: DateTime<Utc>,
    /// Статус
    pub status: ScheduledStatus,
    /// Текст сообщения
    pub text: String,
    /// ID отправителя
    pub from_id: String,
    /// ID оригинального сообщения (после отправки)
    pub original_message_id: Option<String>,
    /// Метка для отмены
    pub cancel_token: String,
}

impl ScheduledMessage {
    pub fn new(chat_id: &str, text: &str, scheduled_at: DateTime<Utc>) -> Self {
        Self {
            message_id: uuid::Uuid::new_v4().to_string(),
            chat_id: chat_id.to_string(),
            scheduled_at,
            status: ScheduledStatus::Pending,
            text: text.to_string(),
            from_id: "".to_string(),
            original_message_id: None,
            cancel_token: uuid::Uuid::new_v4().to_string(),
        }
    }
}

/// Быстрые пресеты для планирования
pub struct MessageSchedule {
    /// Пресеты: (название, длительность в секундах)
    pub quick_presets: Vec<(String, u64)>,
    /// Пресеты на сегодня: (время, DateTime)
    pub today_presets: Vec<(String, DateTime<Utc>)>,
}

impl MessageSchedule {
    pub fn new() -> Self {
        let now = Utc::now();
        let today_presets = Self::generate_today_presets(&now);
        Self {
            quick_presets: Self::quick_presets(),
            today_presets,
        }
    }

    /// Быстрые пресеты
    pub fn quick_presets() -> Vec<(String, u64)> {
        vec![
            ("Через 5 мин".to_string(), 5 * 60),
            ("Через 15 мин".to_string(), 15 * 60),
            ("Через час".to_string(), 60 * 60),
            ("Через 3 часа".to_string(), 3 * 60 * 60),
            ("Через день".to_string(), 24 * 60 * 60),
        ]
    }

    /// Пресеты на сегодня
    pub fn generate_today_presets(now: &DateTime<Utc>) -> Vec<(String, DateTime<Utc>)> {
        let today = now.date_naive();
        vec![
            ("09:00".to_string(), Self::to_datetime(today, 9, 0, now)),
            ("12:00".to_string(), Self::to_datetime(today, 12, 0, now)),
            ("14:00".to_string(), Self::to_datetime(today, 14, 0, now)),
            ("18:00".to_string(), Self::to_datetime(today, 18, 0, now)),
            ("20:00".to_string(), Self::to_datetime(today, 20, 0, now)),
            ("23:59".to_string(), Self::to_datetime(today, 23, 59, now)),
        ]
    }

    fn to_datetime(
        date: chrono::NaiveDate,
        hour: i32,
        minute: i32,
        now: &DateTime<Utc>,
    ) -> DateTime<Utc> {
        let dt = date.and_hms_opt(hour as u32, minute as u32, 0).unwrap();
        let result = dt.and_utc();
        // Если время уже прошло, добавляем день
        if result <= *now {
            result + chrono::Duration::days(1)
        } else {
            result
        }
    }
}
