#![allow(dead_code)]

pub mod db;
pub mod voice_recorder;

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::Mutex;

use crate::api::auth::AuthManager;
use crate::api::scheduled_message::ScheduledMessageClient;
use crate::api::{HttpClient, WebSocketClient};
use crate::models::bot::{BotInfo, BotMessage, BotReplyMarkup};
use crate::models::saved_message::SavedMessage;
use crate::models::scheduled_message::ScheduledMessage;
use crate::models::{Chat, Message};

#[derive(Debug, Clone)]
pub enum AppEvent {
    ChatsUpdated(Vec<Chat>),
    ChatSelected(String),
    MessagesUpdated(String, Vec<Message>),
    MessageSent(String, Message),
    Typing(String, String),
    Error(String),
}

#[derive(Default, Clone)]
pub struct AppState {
    pub chats: Vec<Chat>,
    pub selected_chat_id: Option<String>,
    pub messages_by_chat: HashMap<String, Vec<Message>>,
    pub saved_messages: Vec<SavedMessage>,
    pub bot_reply_markupes: HashMap<String, BotReplyMarkup>,
    pub scheduled_messages: Vec<ScheduledMessage>,
}

pub type SharedState = Arc<Mutex<AppState>>;

#[derive(Clone)]
pub struct AppController {
    auth: Arc<AuthManager>,
    http: Arc<HttpClient>,
    ws: Arc<WebSocketClient>,
    scheduled_client: Arc<ScheduledMessageClient>,
    state: SharedState,
}

impl AppController {
    pub fn auth(&self) -> Arc<AuthManager> {
        self.auth.clone()
    }

    pub fn set_token(&self, token: &str) {
        self.http.set_token(token);
    }

    pub fn new(auth: Arc<AuthManager>, access_token: String) -> Self {
        let http = Arc::new(HttpClient::new(auth.clone()).with_token(&access_token));
        let ws = Arc::new(WebSocketClient::new(auth.clone()));
        let scheduled_client = Arc::new(ScheduledMessageClient::new(auth.clone()));
        Self {
            auth,
            http,
            ws,
            scheduled_client,
            state: Arc::new(Mutex::new(AppState::default())),
        }
    }

    pub fn state(&self) -> SharedState {
        self.state.clone()
    }

    pub fn ws(&self) -> Arc<WebSocketClient> {
        self.ws.clone()
    }

    /// Get the selected chat ID from the app state
    pub async fn get_selected_chat_id(&self) -> Option<String> {
        let state = self.state.lock().await;
        state.selected_chat_id.clone()
    }

    pub async fn connect_realtime(&self) -> Result<(), String> {
        self.ws.connect().await
    }

    pub async fn load_sticker_packs(&self) -> Result<crate::models::StickerPackList, String> {
        // Catalog now goes bootstrap → public file CDN (no broken registry RPC).
        self.http.get_sticker_catalog(None).await
    }

    pub async fn load_chats(&self) -> Result<Vec<Chat>, String> {
        match self.http.get_chat_list(0, 50).await {
            Ok(chats) => {
                let mut state = self.state.lock().await;
                state.chats = chats.clone();
                // Seed in-memory message cache from last_message previews so
                // opening a chat is never completely empty while history loads.
                for chat in &chats {
                    if let Some(ref lm) = chat.last_message {
                        state
                            .messages_by_chat
                            .entry(chat.id.clone())
                            .or_insert_with(|| vec![lm.clone()]);
                    }
                }
                Ok(chats)
            }
            Err(e) => {
                eprintln!("Failed to load chats: {}", e);
                Err(e)
            }
        }
    }

    pub async fn select_chat(&self, chat_id: &str) -> Result<Vec<Message>, String> {
        let _ = self.ws.subscribe(chat_id).await;

        {
            let mut state = self.state.lock().await;
            if let Some(prev) = state.selected_chat_id.clone() {
                if prev != chat_id {
                    // let _ = self.ws.unsubscribe(&prev).await;
                }
            }
            state.selected_chat_id = Some(chat_id.to_string());
        }

        // Always fetch fresh history instead of relying on stale cache
        let messages = self.http.get_messages_fresh(chat_id, None, 0, 50).await?;
        let mut state = self.state.lock().await;
        state
            .messages_by_chat
            .insert(chat_id.to_string(), messages.clone());

        // Save to L2 Cache asynchronously in a background thread task
        let chat_id_str = chat_id.to_string();
        let msgs_clone = messages.clone();
        tokio::spawn(async move {
            Self::save_cache_l2_async(chat_id_str, msgs_clone).await;
        });

        Ok(messages)
    }

    pub async fn get_cached_messages_async(&self, chat_id: String) -> Vec<Message> {
        if let Ok(state) = self.state.try_lock() {
            if let Some(msgs) = state.messages_by_chat.get(&chat_id) {
                return msgs.clone();
            }
        }
        Self::load_cache_l2_async(chat_id).await.unwrap_or_default()
    }

    /// Create a new group chat
    pub async fn send_voice_message(
        &self,
        chat_id: &str,
        audio_data: &[u8],
        duration: f64,
        waveform: Vec<f32>,
    ) -> Result<Message, String> {
        // 1. Upload voice to HTTP API
        let _voice_info = self
            .http
            .upload_voice_message(chat_id, audio_data, duration, waveform)
            .await?;

        // 2. Send via WS as a message
        let text = format!("Voice message ({:.1}s)", duration);
        let sent = self.ws.send_text_message(chat_id, &text, None).await?;
        Ok(sent)
    }

    pub async fn create_group(
        &self,
        title: &str,
        members: Vec<String>,
        is_public: bool,
    ) -> Result<crate::models::Chat, String> {
        self.http.create_group(title, members, is_public).await
    }

    /// Contacts for group member picker (real names, non-deleted).
    pub async fn get_contact_candidates(
        &self,
    ) -> Result<Vec<crate::models::ContactCandidate>, String> {
        self.http.get_contact_candidates().await
    }

    /// Create a new channel
    pub async fn create_channel(
        &self,
        title: &str,
        description: Option<String>,
        is_public: bool,
    ) -> Result<crate::models::Chat, String> {
        self.http
            .create_channel(title, description, is_public)
            .await
    }

    /// Get group information
    pub async fn get_group_info(
        &self,
        chat_id: &str,
    ) -> Result<crate::models::group::GroupSettings, String> {
        self.http.get_group_info(chat_id).await
    }

    /// Get channel information
    pub async fn get_channel_info(
        &self,
        chat_id: &str,
    ) -> Result<crate::models::group::ChannelSettings, String> {
        self.http.get_channel_info(chat_id).await
    }

    /// Get group members
    pub async fn get_group_members(
        &self,
        chat_id: &str,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<crate::models::group::GroupMember>, String> {
        self.http.get_group_members(chat_id, limit, offset).await
    }

    /// Add a member to a group
    pub async fn add_group_member(&self, chat_id: &str, user_id: &str) -> Result<(), String> {
        self.http.add_group_member(chat_id, user_id).await
    }

    /// Remove a member from a group
    pub async fn remove_group_member(&self, chat_id: &str, user_id: &str) -> Result<(), String> {
        self.http.remove_group_member(chat_id, user_id).await
    }

    /// Update group settings
    pub async fn update_group_settings(
        &self,
        chat_id: &str,
        settings: crate::models::group::GroupSettings,
    ) -> Result<(), String> {
        self.http.update_group_settings(chat_id, settings).await
    }

    /// Update channel settings
    pub async fn update_channel_settings(
        &self,
        chat_id: &str,
        settings: crate::models::group::ChannelSettings,
    ) -> Result<(), String> {
        self.http.update_channel_settings(chat_id, settings).await
    }

    /// Generate an invite link for a group
    pub async fn generate_invite_link(&self, chat_id: &str) -> Result<String, String> {
        self.http.generate_invite_link(chat_id).await
    }

    /// Join a channel
    pub async fn join_channel(&self, chat_id: &str) -> Result<(), String> {
        self.http.join_channel(chat_id).await
    }

    /// Leave a group
    pub async fn leave_group(&self, chat_id: &str) -> Result<(), String> {
        self.http.leave_group(chat_id).await
    }

    /// Promote a member to admin
    pub async fn promote_to_admin(&self, chat_id: &str, user_id: &str) -> Result<(), String> {
        self.http.promote_to_admin(chat_id, user_id).await
    }

    /// Demote an admin to member
    pub async fn demote_from_admin(&self, chat_id: &str, user_id: &str) -> Result<(), String> {
        self.http.demote_from_admin(chat_id, user_id).await
    }

    /// Ban a member from the group
    pub async fn ban_member(&self, chat_id: &str, user_id: &str) -> Result<(), String> {
        self.http.ban_member(chat_id, user_id).await
    }

    /// Unban a member from the group
    pub async fn unban_member(&self, chat_id: &str, user_id: &str) -> Result<(), String> {
        self.http.unban_member(chat_id, user_id).await
    }

    pub async fn fetch_fresh_messages(&self, chat_id: &str) -> Result<Vec<Message>, String> {
        let messages = self.http.get_messages_fresh(chat_id, None, 0, 50).await?;
        let mut state = self.state.lock().await;
        state
            .messages_by_chat
            .insert(chat_id.to_string(), messages.clone());

        // Save to L2 Cache
        Self::save_cache_l2(chat_id, &messages);

        Ok(messages)
    }

    fn cache_dir() -> std::path::PathBuf {
        let mut path = dirs::cache_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
        path.push("yandex-messenger");
        path.push("chats");
        let _ = std::fs::create_dir_all(&path);
        path
    }

    fn load_cache_l2(chat_id: &str) -> Result<Vec<Message>, String> {
        let mut path = Self::cache_dir();
        path.push(format!("{}.json", chat_id));

        if !path.exists() {
            return Err("Cache miss".into());
        }

        let data = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
        let messages: Vec<Message> = serde_json::from_str(&data).map_err(|e| e.to_string())?;
        Ok(messages)
    }

    fn save_cache_l2(chat_id: &str, messages: &[Message]) {
        let mut path = Self::cache_dir();
        path.push(format!("{}.json", chat_id));

        if let Ok(data) = serde_json::to_string(messages) {
            let _ = std::fs::write(path, data);
        }
    }

    async fn save_cache_l2_async(chat_id: String, messages: Vec<Message>) {
        let (tx, rx) = tokio::sync::oneshot::channel();
        std::thread::spawn(move || {
            let mut path = Self::cache_dir();
            path.push(format!("{}.json", chat_id));
            if let Ok(data) = serde_json::to_string(&messages) {
                let _ = std::fs::write(path, data);
            }
            let _ = tx.send(());
        });
        let _ = rx.await;
    }

    async fn load_cache_l2_async(chat_id: String) -> Result<Vec<Message>, String> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        std::thread::spawn(move || {
            let res = (|| {
                let mut path = Self::cache_dir();
                path.push(format!("{}.json", chat_id));
                if !path.exists() {
                    return Err("Cache miss".to_string());
                }
                let data = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
                let messages: Vec<Message> =
                    serde_json::from_str(&data).map_err(|e| e.to_string())?;
                Ok(messages)
            })();
            let _ = tx.send(res);
        });
        rx.await.map_err(|_| "Thread panicked".to_string())?
    }

    pub async fn send_text_message(&self, chat_id: &str, text: &str) -> Result<Message, String> {
        // Send via WebSocket
        let sent = self.ws.send_text_message(chat_id, text, None).await?;

        let mut state = self.state.lock().await;
        state
            .messages_by_chat
            .entry(chat_id.to_string())
            .or_default()
            .push(sent.clone());
        Ok(sent)
    }

    pub async fn upload_file(
        &self,
        chat_id: &str,
        file_data: &[u8],
        filename: &str,
    ) -> Result<String, String> {
        self.http.upload_file(chat_id, file_data, filename).await
    }

    pub async fn download_file(&self, file_id: &str) -> Result<Vec<u8>, String> {
        self.http.download_file(file_id).await
    }

    /// Save a message to favorites
    pub async fn save_message(
        &self,
        chat_id: &str,
        message_id: &str,
        note: Option<String>,
    ) -> Result<SavedMessage, String> {
        let msg = self
            .http
            .get_messages(chat_id, Some(message_id), 0, 1)
            .await?;
        let preview = msg.first().and_then(|m| m.text.clone()).unwrap_or_default();

        let saved = SavedMessage {
            message_id: message_id.to_string(),
            source_chat_id: chat_id.to_string(),
            source_message: message_id.to_string(),
            saved_at: chrono::Utc::now(),
            note,
            media_type: Some("text".to_string()),
            preview: Some(preview),
        };

        let mut state = self.state.lock().await;
        state.saved_messages.push(saved.clone());

        Ok(saved)
    }

    /// Get all saved messages
    pub async fn get_saved_messages(
        &self,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<SavedMessage>, String> {
        let state = self.state.lock().await;
        let mut msgs = state.saved_messages.clone();
        // Sort by saved_at descending
        msgs.sort_by(|a, b| b.saved_at.cmp(&a.saved_at));
        let total = msgs.len();
        let start = offset.min(total);
        let end = (start + limit).min(total);
        Ok(msgs[start..end].to_vec())
    }

    /// Get total count of saved messages
    pub async fn get_saved_count(&self) -> usize {
        let state = self.state.lock().await;
        state.saved_messages.len()
    }

    /// Unsave a message
    pub async fn unsave_message(&self, message_id: &str) -> Result<(), String> {
        let mut state = self.state.lock().await;
        state.saved_messages.retain(|m| m.message_id != message_id);
        Ok(())
    }

    pub fn telemost_url(&self, chat_id: &str) -> String {
        self.http.telemost_url(chat_id, None)
    }

    // ── Bot methods ──

    pub async fn get_bot_info(&self, bot_id: &str) -> Result<BotInfo, String> {
        self.http.get_bot_info(bot_id).await
    }

    pub async fn send_bot_command(
        &self,
        bot_id: &str,
        command: &str,
        params: serde_json::Value,
    ) -> Result<BotMessage, String> {
        self.http.send_bot_command(bot_id, command, params).await
    }

    pub async fn get_bot_messages(
        &self,
        bot_id: &str,
        limit: usize,
    ) -> Result<Vec<BotMessage>, String> {
        self.http.get_bot_messages(bot_id, limit).await
    }

    pub async fn update_bot_reply_markup(
        &self,
        bot_id: &str,
        markup: BotReplyMarkup,
    ) -> Result<(), String> {
        let _ = self.http.send_inline_callback(bot_id, "").await?;
        // Store markup in state
        let mut state = self.state.lock().await;
        state.bot_reply_markupes.insert(bot_id.to_string(), markup);
        Ok(())
    }

    pub async fn token_available(&self) -> bool {
        self.auth.is_authenticated().await
    }

    pub async fn get_folders(&self) -> Result<Vec<crate::models::ChatFolder>, String> {
        self.http.get_folders().await
    }

    pub fn send_typing(&self, chat_id: &str) {
        log::info!("Sending typing event to chat {}", chat_id);
    }

    pub async fn get_reactions_config(
        &self,
    ) -> Result<crate::models::ExtendedReactionsConfig, String> {
        self.http.get_reactions_config_public().await
    }

    pub async fn add_reaction(&self, message_id: &str, emoji: &str) -> Result<(), String> {
        if self.ws.send_add_reaction(message_id, emoji).await.is_ok() {
            return Ok(());
        }
        self.http
            .add_reaction_public(message_id, emoji)
            .await
            .map(|_| ())
    }

    pub async fn remove_reaction(&self, message_id: &str, emoji: &str) -> Result<(), String> {
        if self
            .ws
            .send_remove_reaction(message_id, emoji)
            .await
            .is_ok()
        {
            return Ok(());
        }
        self.http.remove_reaction_public(message_id, emoji).await
    }

    // ── Scheduled message methods ──

    /// Запланировать сообщение
    pub async fn schedule_message(
        &self,
        chat_id: &str,
        text: &str,
        scheduled_at: chrono::DateTime<chrono::Utc>,
    ) -> Result<ScheduledMessage, String> {
        let message = self
            .scheduled_client
            .schedule_message(chat_id, text, scheduled_at)
            .await?;
        let mut state = self.state.lock().await;
        state.scheduled_messages.push(message.clone());
        Ok(message)
    }

    /// Получить запланированные сообщения для чата
    pub async fn get_scheduled_messages(
        &self,
        chat_id: &str,
    ) -> Result<Vec<ScheduledMessage>, String> {
        let messages = self
            .scheduled_client
            .get_scheduled_messages(chat_id)
            .await?;
        let _state = self.state.lock().await;
        let filtered: Vec<ScheduledMessage> = messages
            .into_iter()
            .filter(|m| {
                m.chat_id == chat_id
                    || m.status != crate::models::scheduled_message::ScheduledStatus::Sent
            })
            .collect();
        Ok(filtered)
    }

    /// Отменить запланированное сообщение
    pub async fn cancel_scheduled_message(
        &self,
        chat_id: &str,
        message_id: &str,
    ) -> Result<(), String> {
        self.scheduled_client
            .cancel_scheduled_message(chat_id, message_id)
            .await?;
        let mut state = self.state.lock().await;
        state
            .scheduled_messages
            .retain(|m| !(m.chat_id == chat_id && m.message_id == message_id));
        Ok(())
    }

    /// Обновить время отправки
    pub async fn update_scheduled_time(
        &self,
        chat_id: &str,
        message_id: &str,
        new_scheduled_at: chrono::DateTime<chrono::Utc>,
    ) -> Result<(), String> {
        self.scheduled_client
            .update_scheduled_time(chat_id, message_id, new_scheduled_at)
            .await?;
        let mut state = self.state.lock().await;
        if let Some(msg) = state
            .scheduled_messages
            .iter_mut()
            .find(|m| m.chat_id == chat_id && m.message_id == message_id)
        {
            msg.scheduled_at = new_scheduled_at;
        }
        Ok(())
    }

    /// Запланировать сообщение с быстрым пресетом
    pub async fn schedule_with_preset(
        &self,
        chat_id: &str,
        text: &str,
        preset_seconds: u64,
    ) -> Result<ScheduledMessage, String> {
        let scheduled_at = chrono::Utc::now() + chrono::Duration::seconds(preset_seconds as i64);
        self.schedule_message(chat_id, text, scheduled_at).await
    }

    /// Запланировать на сегодня (быстрый пресет)
    pub async fn schedule_today(
        &self,
        chat_id: &str,
        text: &str,
        hour: u32,
        minute: u32,
    ) -> Result<ScheduledMessage, String> {
        let now = chrono::Utc::now();
        let today = now.date_naive();
        let scheduled_at = today
            .and_hms_opt(hour, minute, 0)
            .map(|dt| dt.and_utc())
            .unwrap_or(chrono::Utc::now());
        self.schedule_message(chat_id, text, scheduled_at).await
    }

    /// Проверить и отправить ожидающие сообщения
    pub async fn send_pending_scheduled(&self) -> Result<Vec<Message>, String> {
        let now = chrono::Utc::now();
        let mut state = self.state.lock().await;
        let mut sent_messages = Vec::new();

        let pending: Vec<String> = state
            .scheduled_messages
            .iter()
            .filter(|m| {
                m.status == crate::models::scheduled_message::ScheduledStatus::Pending
                    && m.scheduled_at <= now
            })
            .map(|m| m.message_id.clone())
            .collect();

        let pending_clone = pending.clone();
        for msg_id in pending_clone {
            if let Some(msg) = state
                .scheduled_messages
                .iter_mut()
                .find(|m| m.message_id == msg_id)
            {
                msg.status = crate::models::scheduled_message::ScheduledStatus::Sending;
            }
        }

        // Отправляем сообщения
        for msg_id in pending {
            if let Some(msg) = state
                .scheduled_messages
                .iter()
                .find(|m| m.message_id == msg_id)
            {
                let chat_id = msg.chat_id.clone();
                let text = msg.text.clone();
                if let Ok(sent) = self.ws.send_text_message(&chat_id, &text, None).await {
                    if let Some(m) = state
                        .scheduled_messages
                        .iter_mut()
                        .find(|m| m.message_id == msg_id)
                    {
                        m.status = crate::models::scheduled_message::ScheduledStatus::Sent;
                        m.original_message_id = Some(sent.id.clone());
                    }
                    sent_messages.push(sent);
                } else {
                    if let Some(m) = state
                        .scheduled_messages
                        .iter_mut()
                        .find(|m| m.message_id == msg_id)
                    {
                        m.status = crate::models::scheduled_message::ScheduledStatus::Failed;
                    }
                }
            }
        }

        Ok(sent_messages)
    }
}
