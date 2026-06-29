use serde::{Deserialize, Serialize};

/// Голосовое сообщение (Voice Message)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceMessage {
    pub message_id: String,
    pub url: String,
    pub duration: f64,      // секунды
    pub waveform: Vec<f32>, // визуализация звуковой волны (амплитуды 0.0-1.0)
    pub transcribed_text: Option<String>,
    pub is_transcribing: bool,
    pub transcribe_error: Option<String>,
}

impl VoiceMessage {
    /// Создать новое голосовое сообщение
    pub fn new(message_id: String, url: String, duration: f64, waveform: Vec<f32>) -> Self {
        Self {
            message_id,
            url,
            duration,
            waveform,
            transcribed_text: None,
            is_transcribing: false,
            transcribe_error: None,
        }
    }

    /// Форматировать длительность в строку вида "m:ss" или "mm:ss"
    pub fn format_duration(&self) -> String {
        let secs = self.duration as u64;
        let mins = secs / 60;
        let s = secs % 60;
        if mins > 0 {
            format!("{:02}:{:02}", mins, s)
        } else {
            format!("0:{:02}", s)
        }
    }

    /// Проверить, доступна ли транскрипция
    pub fn has_transcription(&self) -> bool {
        self.transcribed_text
            .as_ref()
            .map_or(false, |t| !t.is_empty())
    }

    /// Статус транскрипции для отображения в UI
    pub fn transcribe_status(&self) -> TranscribeStatus {
        if self.is_transcribing {
            TranscribeStatus::InProgress
        } else if let Some(ref err) = self.transcribe_error {
            TranscribeStatus::Error(err.clone())
        } else if self.has_transcription() {
            TranscribeStatus::Completed
        } else {
            TranscribeStatus::None
        }
    }
}

/// Статус транскрипции голосового сообщения
#[derive(Debug, Clone, PartialEq)]
pub enum TranscribeStatus {
    None,
    InProgress,
    Completed,
    Error(String),
}

/// Параметры записи голосового сообщения
#[derive(Debug, Clone)]
pub struct VoiceRecordParams {
    pub sample_rate: u32,
    pub channels: u16,
    pub duration_secs: f64,
    pub format: VoiceFormat,
}

impl Default for VoiceRecordParams {
    fn default() -> Self {
        Self {
            sample_rate: crate::config::VOICE_SAMPLE_RATE,
            channels: 1,
            duration_secs: 0.0,
            format: VoiceFormat::Opus,
        }
    }
}

/// Формат кодирования голосового сообщения
#[derive(Debug, Clone, PartialEq)]
pub enum VoiceFormat {
    Opus,
    WebM,
    Ogg,
}
