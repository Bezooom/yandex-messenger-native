use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Poll {
    pub poll_id: String,
    pub message_id: String, // сообщение с опросом
    pub chat_id: String,
    pub question: String,
    pub answers: Vec<PollAnswer>,
    pub total_voters: u32,
    pub is_anonymous: bool,
    pub is_multi_select: bool,
    pub quiz_mode: bool,
    pub correct_answer_ids: Vec<String>,
    pub created_by: String,
    pub expires_at: Option<DateTime<Utc>>,
    pub is_closed: bool,
    pub final_results: Option<Vec<PollAnswer>>, // после закрытия
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PollAnswer {
    pub answer_id: String,
    pub text: String,
    pub votes: u32,
    pub is_correct: bool,
    pub is_selected: bool, // проголосовал ли текущий пользователь
}

impl Poll {
    pub fn total_percentage(&self, answer_idx: usize) -> f64 {
        if self.total_voters == 0 {
            return 0.0;
        }
        if answer_idx >= self.answers.len() {
            return 0.0;
        }
        (self.answers[answer_idx].votes as f64 / self.total_voters as f64) * 100.0
    }

    pub fn can_vote(&self) -> bool {
        !self.is_closed && !self.is_anonymous || !self.is_closed
    }
}
