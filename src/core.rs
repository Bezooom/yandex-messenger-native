#![allow(dead_code)]

pub mod db;
pub mod drafts;
pub mod outbox;
pub mod voice_recorder;

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::Mutex;

use crate::api::auth::AuthManager;
use crate::api::scheduled_message::ScheduledMessageClient;
use crate::api::telemost::TelemostClient;
use crate::api::{HttpClient, WebSocketClient};
use crate::core::db::Database;
use crate::core::drafts::DraftStore;
use crate::core::outbox::Outbox;
use crate::models::bot::{BotInfo, BotMessage, BotReplyMarkup};
use crate::models::saved_message::SavedMessage;
use crate::models::scheduled_message::ScheduledMessage;
use crate::models::{CallStatus, Chat, Message, TelemostCall};

#[derive(Debug, Clone)]
pub enum AppEvent {
    ChatsUpdated(Vec<Chat>),
    ChatSelected(String),
    MessagesUpdated(String, Vec<Message>),
    MessageSent(String, Message),
    Typing(String, String),
    Error(String),
    CallStatusUpdated(String, CallStatus),
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
    telemost_client: Arc<TelemostClient>,
    outbox: Arc<Outbox>,
    drafts: Arc<DraftStore>,
    db: Arc<Database>,
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
        let telemost_client = Arc::new(TelemostClient::new(auth.clone()));
        let outbox = Arc::new(Outbox::open());
        let drafts = Arc::new(DraftStore::open());
        let db = match Database::open() {
            Ok(d) => Arc::new(d),
            Err(e) => {
                log::error!("SQLite cache open failed ({}): using temp fallback", e);
                let path = std::env::temp_dir()
                    .join(format!("ym-cache-fallback-{}.db", std::process::id()));
                let conn = rusqlite::Connection::open(&path).expect("temp sqlite open");
                Arc::new(Database::from_connection(conn).expect("temp sqlite schema"))
            }
        };
        Self {
            auth,
            http,
            ws,
            scheduled_client,
            telemost_client,
            outbox,
            drafts,
            db,
            state: Arc::new(Mutex::new(AppState::default())),
        }
    }

    pub fn outbox(&self) -> Arc<Outbox> {
        self.outbox.clone()
    }

    pub fn drafts(&self) -> Arc<DraftStore> {
        self.drafts.clone()
    }

    pub fn http(&self) -> Arc<HttpClient> {
        self.http.clone()
    }

    /// Reload Passport session cookies from disk into HTTP client.
    pub fn reload_session(&self) {
        self.http.reload_session();
    }

    /// Clear in-memory session cookies (after logout / before re-login).
    pub fn clear_session_cookies(&self) {
        self.http.clear_session_cookies();
    }

    /// Load older messages (pagination) using the oldest known message id as cursor.
    /// Returns newly loaded messages (oldest-first), already merged into state.
    pub async fn load_older_messages(
        &self,
        chat_id: &str,
        before_message_id: &str,
        limit: usize,
    ) -> Result<Vec<Message>, String> {
        // Prefer raw message_id if our id is composite like "seq_mid"
        let cursor = before_message_id
            .split('_')
            .last()
            .unwrap_or(before_message_id);

        let older = self
            .http
            .get_messages(chat_id, Some(cursor), 0, limit)
            .await?;

        if older.is_empty() {
            return Ok(vec![]);
        }

        let mut state = self.state.lock().await;
        let existing = state
            .messages_by_chat
            .entry(chat_id.to_string())
            .or_default();
        let existing_ids: std::collections::HashSet<String> =
            existing.iter().map(|m| m.id.clone()).collect();

        let mut new_ones: Vec<Message> = older
            .into_iter()
            .filter(|m| !existing_ids.contains(&m.id))
            .collect();
        new_ones.sort_by(|a, b| a.created.cmp(&b.created));

        if !new_ones.is_empty() {
            let mut merged = new_ones.clone();
            merged.extend(existing.iter().cloned());
            merged.sort_by(|a, b| a.created.cmp(&b.created));
            // dedup by id
            let mut seen = std::collections::HashSet::new();
            merged.retain(|m| seen.insert(m.id.clone()));
            *existing = merged;
            let full = existing.clone();
            drop(state);
            Self::save_cache_l2(chat_id, &full);
            let db = self.db.clone();
            let cid = chat_id.to_string();
            let full2 = full;
            let _ = tokio::task::spawn_blocking(move || {
                if let Err(e) = db.upsert_messages(&cid, &full2) {
                    log::warn!("SQLite upsert older msgs: {}", e);
                }
            });
        }

        Ok(new_ones)
    }

    pub fn state(&self) -> SharedState {
        self.state.clone()
    }

    pub fn ws(&self) -> Arc<WebSocketClient> {
        self.ws.clone()
    }

    pub fn telemost_client(&self) -> Arc<TelemostClient> {
        self.telemost_client.clone()
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
                drop(state);
                // Persist to SQLite
                let db = self.db.clone();
                let chats_for_db = chats.clone();
                tokio::task::spawn_blocking(move || {
                    if let Err(e) = db.upsert_chats(&chats_for_db) {
                        log::warn!("SQLite upsert_chats: {}", e);
                    }
                });
                Ok(chats)
            }
            Err(e) => {
                eprintln!("Failed to load chats: {}", e);
                // Fallback to SQLite
                match self.db.get_chats() {
                    Ok(cached) if !cached.is_empty() => {
                        log::info!("Loaded {} chats from SQLite cache", cached.len());
                        let mut state = self.state.lock().await;
                        state.chats = cached.clone();
                        Ok(cached)
                    }
                    _ => Err(e),
                }
            }
        }
    }

    /// Instant chat list from SQLite (for cold start UI).
    pub fn load_chats_from_db(&self) -> Vec<Chat> {
        self.db.get_chats().unwrap_or_default()
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
        match self.http.get_messages_fresh(chat_id, None, 0, 50).await {
            Ok(messages) => {
                let mut state = self.state.lock().await;
                state
                    .messages_by_chat
                    .insert(chat_id.to_string(), messages.clone());
                drop(state);

                let chat_id_str = chat_id.to_string();
                let msgs_clone = messages.clone();
                let db = self.db.clone();
                tokio::spawn(async move {
                    Self::save_cache_l2_async(chat_id_str.clone(), msgs_clone.clone()).await;
                    let _ = tokio::task::spawn_blocking(move || {
                        if let Err(e) = db.upsert_messages(&chat_id_str, &msgs_clone) {
                            log::warn!("SQLite upsert_messages: {}", e);
                        }
                    })
                    .await;
                });

                Ok(messages)
            }
            Err(e) => {
                // Fallback: SQLite then JSON L2
                if let Ok(cached) = self.db.get_messages(chat_id, Some(200)) {
                    if !cached.is_empty() {
                        log::info!(
                            "select_chat network failed ({}), using {} SQLite msgs",
                            e,
                            cached.len()
                        );
                        let mut state = self.state.lock().await;
                        state
                            .messages_by_chat
                            .insert(chat_id.to_string(), cached.clone());
                        return Ok(cached);
                    }
                }
                if let Ok(cached) = Self::load_cache_l2(chat_id) {
                    if !cached.is_empty() {
                        return Ok(cached);
                    }
                }
                Err(e)
            }
        }
    }

    pub async fn get_cached_messages_async(&self, chat_id: String) -> Vec<Message> {
        if let Ok(state) = self.state.try_lock() {
            if let Some(msgs) = state.messages_by_chat.get(&chat_id) {
                if !msgs.is_empty() {
                    return msgs.clone();
                }
            }
        }
        // Prefer SQLite
        if let Ok(msgs) = self.db.get_messages(&chat_id, Some(200)) {
            if !msgs.is_empty() {
                return msgs;
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

    /// Persist messages to SQLite (sync; call from blocking context or spawn_blocking).
    pub fn save_messages_sqlite(&self, chat_id: &str, messages: &[Message]) {
        if let Err(e) = self.db.upsert_messages(chat_id, messages) {
            log::warn!("save_messages_sqlite: {}", e);
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
        self.send_text_message_ex(chat_id, text, None, None).await
    }

    /// Send or edit a text message.
    /// - `reply_to`: optional message id being replied to
    /// - `edit_id`: if set, edits that message instead of sending a new one
    pub async fn send_text_message_ex(
        &self,
        chat_id: &str,
        text: &str,
        reply_to: Option<&str>,
        edit_id: Option<&str>,
    ) -> Result<Message, String> {
        if let Some(mid) = edit_id {
            self.http.edit_message(chat_id, mid, text).await?;
            let mut state = self.state.lock().await;
            if let Some(msgs) = state.messages_by_chat.get_mut(chat_id) {
                if let Some(msg) = msgs
                    .iter_mut()
                    .find(|m| m.id == mid || m.message_id.as_deref() == Some(mid))
                {
                    msg.text = Some(text.to_string());
                    msg.edited = true;
                    msg.edited_at = Some(chrono::Utc::now());
                    return Ok(msg.clone());
                }
            }
            // Fallback optimistic edited message
            return Ok(Message {
                id: mid.to_string(),
                chat_id: chat_id.to_string(),
                from_id: self.auth.get_current_account_id().await.unwrap_or_default(),
                message_id: Some(mid.to_string()),
                rmid: None,
                type_: crate::models::MessageType::Text,
                text: Some(text.to_string()),
                entities: vec![],
                reply_to: None,
                forward: None,
                media: vec![],
                reactions: vec![],
                thread_id: None,
                has_thread: false,
                pinned: false,
                edited: true,
                edited_at: Some(chrono::Utc::now()),
                sent: true,
                delivered: true,
                read: false,
                created: chrono::Utc::now(),
                updated: Some(chrono::Utc::now()),
                poll: None,
            });
        }

        // Text send path (Yandex):
        // 1) WebSocket binary ClientMessage (primary — registry has no send_message path)
        // 2) Session RPC only as experimental fallback (often "No such path")
        // On total failure — enqueue outbox and return optimistic pending message.
        let send_result = match self.ws.send_text_message(chat_id, text, reply_to).await {
            Ok(m) => Ok(m),
            Err(ws_err) => {
                log::warn!("WS send failed: {}", ws_err);
                if self.http.has_session() {
                    match self.http.send_message(chat_id, text, reply_to).await {
                        Ok(m) => Ok(m),
                        Err(e) => {
                            log::warn!("session send_message failed: {}", e);
                            Err(format!("ws: {}; session: {}", ws_err, e))
                        }
                    }
                } else {
                    Err(format!("ws: {}; no session for HTTP fallback", ws_err))
                }
            }
        };

        let sent = match send_result {
            Ok(mut m) => {
                // Network round-trip succeeded → at least delivered
                m.sent = true;
                m.delivered = true;
                m
            }
            Err(e) => {
                log::warn!("Send failed, queuing outbox: {}", e);
                let item = self
                    .outbox
                    .enqueue(chat_id, text, reply_to, Some(e.clone()));
                let pending = Message {
                    id: item.id.clone(),
                    chat_id: chat_id.to_string(),
                    from_id: self.auth.get_current_account_id().await.unwrap_or_default(),
                    message_id: Some(item.id.clone()),
                    rmid: None,
                    type_: crate::models::MessageType::Text,
                    text: Some(text.to_string()),
                    entities: vec![],
                    reply_to: reply_to.map(|rt| crate::models::MessageId {
                        chat_id: chat_id.to_string(),
                        message_id: rt.to_string(),
                    }),
                    forward: None,
                    media: vec![],
                    reactions: vec![],
                    thread_id: None,
                    has_thread: false,
                    pinned: false,
                    edited: false,
                    edited_at: None,
                    sent: false,
                    delivered: false,
                    read: false,
                    created: chrono::Utc::now(),
                    updated: None,
                    poll: None,
                };
                let mut state = self.state.lock().await;
                state
                    .messages_by_chat
                    .entry(chat_id.to_string())
                    .or_default()
                    .push(pending.clone());
                return Ok(pending);
            }
        };

        let mut state = self.state.lock().await;
        state
            .messages_by_chat
            .entry(chat_id.to_string())
            .or_default()
            .push(sent.clone());
        Ok(sent)
    }

    /// Retry all pending outbox messages. Returns (sent_ok, still_pending).
    pub async fn flush_outbox(&self) -> (usize, usize) {
        self.outbox.purge_dead(20);
        let items = self.outbox.list();
        if items.is_empty() {
            return (0, 0);
        }
        log::info!("Flushing outbox ({} items)", items.len());
        let mut ok = 0usize;
        for item in items {
            let res = if self.http.has_session() {
                match self
                    .http
                    .send_message(&item.chat_id, &item.text, item.reply_to.as_deref())
                    .await
                {
                    Ok(m) => Ok(m),
                    Err(e) => self
                        .ws
                        .send_text_message(&item.chat_id, &item.text, item.reply_to.as_deref())
                        .await
                        .map_err(|e2| format!("{}; {}", e, e2)),
                }
            } else {
                self.ws
                    .send_text_message(&item.chat_id, &item.text, item.reply_to.as_deref())
                    .await
            };

            match res {
                Ok(msg) => {
                    self.outbox.remove(&item.id);
                    let mut state = self.state.lock().await;
                    if let Some(msgs) = state.messages_by_chat.get_mut(&item.chat_id) {
                        // Replace pending placeholder if present
                        if let Some(slot) = msgs.iter_mut().find(|m| m.id == item.id) {
                            *slot = msg.clone();
                        } else {
                            msgs.push(msg);
                        }
                    }
                    ok += 1;
                }
                Err(e) => {
                    log::warn!("Outbox item {} failed: {}", item.id, e);
                    self.outbox.mark_attempt(&item.id, Some(e));
                }
            }
        }
        (ok, self.outbox.len())
    }

    pub async fn mark_chat_read(&self, chat_id: &str) -> Result<(), String> {
        let last_id = {
            let state = self.state.lock().await;
            state
                .messages_by_chat
                .get(chat_id)
                .and_then(|m| m.last())
                .map(|m| m.id.clone())
        };
        let res = self.http.mark_chat_read(chat_id, last_id.as_deref()).await;
        if let Ok(()) = res {
            let mut state = self.state.lock().await;
            if let Some(chat) = state.chats.iter_mut().find(|c| c.id == chat_id) {
                chat.unread_count = 0;
            }
        }
        res
    }

    pub async fn set_chat_muted(&self, chat_id: &str, muted: bool) -> Result<(), String> {
        let res = self.http.set_chat_muted(chat_id, muted).await;
        // Always update local state for responsive UI
        {
            let mut state = self.state.lock().await;
            if let Some(chat) = state.chats.iter_mut().find(|c| c.id == chat_id) {
                chat.muted = muted;
            }
        }
        res
    }

    pub async fn set_chat_pinned(&self, chat_id: &str, pinned: bool) -> Result<(), String> {
        let res = self.http.set_chat_pinned(chat_id, pinned).await;
        {
            let mut state = self.state.lock().await;
            if let Some(chat) = state.chats.iter_mut().find(|c| c.id == chat_id) {
                chat.pinned = pinned;
            }
        }
        res
    }

    pub async fn set_chat_archived(&self, chat_id: &str, archived: bool) -> Result<(), String> {
        let res = self.http.set_chat_archived(chat_id, archived).await;
        {
            let mut state = self.state.lock().await;
            if let Some(chat) = state.chats.iter_mut().find(|c| c.id == chat_id) {
                chat.archived = archived;
            }
        }
        res
    }

    pub async fn delete_chat(&self, chat_id: &str) -> Result<(), String> {
        let res = self.http.delete_chat(chat_id).await;
        {
            let mut state = self.state.lock().await;
            state.chats.retain(|c| c.id != chat_id);
            state.messages_by_chat.remove(chat_id);
            if state.selected_chat_id.as_deref() == Some(chat_id) {
                state.selected_chat_id = None;
            }
        }
        res
    }

    pub async fn delete_message(&self, chat_id: &str, message_id: &str) -> Result<(), String> {
        self.http.delete_message(chat_id, message_id).await?;
        let mut state = self.state.lock().await;
        if let Some(msgs) = state.messages_by_chat.get_mut(chat_id) {
            msgs.retain(|m| m.id != message_id && m.message_id.as_deref() != Some(message_id));
        }
        Ok(())
    }

    /// Total unread across chats (for tray badge).
    pub async fn total_unread(&self) -> u32 {
        let state = self.state.lock().await;
        state.chats.iter().map(|c| c.unread_count).sum()
    }

    pub async fn is_chat_muted(&self, chat_id: &str) -> bool {
        let state = self.state.lock().await;
        state
            .chats
            .iter()
            .find(|c| c.id == chat_id)
            .map(|c| c.muted)
            .unwrap_or(false)
    }

    /// Apply delivery/read status to messages in memory (and SQLite best-effort).
    /// Returns list of message ids that changed.
    pub async fn apply_status_update(&self, update: crate::api::StatusUpdate) -> Vec<String> {
        let mut changed = Vec::new();
        let mut state = self.state.lock().await;

        let chat_ids: Vec<String> = if let Some(ref cid) = update.chat_id {
            vec![cid.clone()]
        } else {
            state.messages_by_chat.keys().cloned().collect()
        };

        for cid in chat_ids {
            if let Some(msgs) = state.messages_by_chat.get_mut(&cid) {
                for msg in msgs.iter_mut() {
                    let chat_wide = update.message_id.is_none();
                    let match_id = update.message_id.as_ref().map_or(false, |mid| {
                        msg.id == *mid
                            || msg.message_id.as_deref() == Some(mid.as_str())
                            || msg.id.ends_with(&format!("_{}", mid))
                    });
                    if !chat_wide && !match_id {
                        continue;
                    }
                    // Chat-wide updates apply to already-outgoing messages only
                    if chat_wide && !(msg.sent || msg.delivered) {
                        continue;
                    }
                    let mut dirty = false;
                    if update.delivered && !msg.delivered {
                        msg.delivered = true;
                        dirty = true;
                    }
                    if update.read && !msg.read {
                        msg.read = true;
                        msg.delivered = true;
                        dirty = true;
                    }
                    if dirty {
                        changed.push(msg.id.clone());
                    }
                }
            }
        }
        changed
    }

    /// Send a message with a file attachment:
    /// 1) POST media_upload (OAuth) → file_id
    /// 2) push WS ClientMessage Image/MiscFile (same PayloadId as upload messageId)
    pub async fn send_file_message(
        &self,
        chat_id: &str,
        file_data: &[u8],
        filename: &str,
    ) -> Result<Message, String> {
        let payload_id = uuid::Uuid::new_v4().simple().to_string();
        let mime = crate::api::HttpClient::guess_mime_type(filename);

        // Wait briefly for WS (same as text send)
        if !self.ws.is_connected().await {
            for _ in 0..20 {
                if self.ws.is_connected().await {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
        }

        let file_id = self
            .http
            .upload_file_for_message(chat_id, file_data, filename, &payload_id)
            .await?;

        let is_image = crate::api::HttpClient::is_image_mime(mime);
        // Cheap dimensions (no full decode when possible) — full decode freezes on 4K pastes
        let (width, height) = if is_image {
            image_dimensions_fast(file_data)
        } else {
            (None, None)
        };

        let mut sent = self
            .ws
            .send_file_message(
                chat_id,
                &file_id,
                filename,
                file_data.len() as u64,
                mime,
                width,
                height,
                &payload_id,
            )
            .await
            .map_err(|e| format!("Файл загружен, но отправка в чат не удалась: {}", e))?;

        // Local downscaled preview so bubble doesn't re-fetch full file over network
        if is_image {
            if let Some(local) = cache_outgoing_image_preview(file_data, &file_id) {
                if let Some(m) = sent.media.get_mut(0) {
                    m.thumbnail_url = Some(local.clone());
                    // Prefer shortterm URL for open/download with OAuth later
                    m.url = format!(
                        "{}/file_shortterm/{}",
                        crate::config::FILE_PUBLIC_HOST.trim_end_matches('/'),
                        file_id.trim_start_matches('/')
                    );
                }
            }
        }

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

    pub async fn download_url(&self, url: &str) -> Result<Vec<u8>, String> {
        self.http.download_url(url).await
    }

    /// Download attachment bytes (prefer file id, fall back to URL).
    pub async fn download_attachment(&self, file_id: &str, url: &str) -> Result<Vec<u8>, String> {
        if !file_id.is_empty() && !file_id.starts_with("http") {
            match self.download_file(file_id).await {
                Ok(b) if !b.is_empty() => return Ok(b),
                Ok(_) => {}
                Err(e) => log::debug!("download_file id failed: {}", e),
            }
        }
        if url.starts_with("http") {
            return self.download_url(url).await;
        }
        Err("No downloadable url/id".into())
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

    pub fn telemost_url(&self, chat_id: &str, call_id: Option<&str>) -> String {
        self.http.telemost_url(chat_id, call_id)
    }

    /// Start a Telemost call in the given chat.
    pub async fn start_call(&self, chat_id: &str) -> Result<TelemostCall, String> {
        self.http.start_call(chat_id).await
    }

    /// End an active Telemost call.
    pub async fn end_call(&self, call_id: &str) -> Result<(), String> {
        self.http.end_call(call_id).await
    }

    /// Get the current status of a Telemost call.
    pub async fn get_call_status(&self, call_id: &str) -> Result<CallStatus, String> {
        self.http.get_call_status(call_id).await
    }

    /// Subscribe to real-time call status updates via WebSocket.
    pub async fn subscribe_call_updates(&self, call_id: &str) -> Result<u64, String> {
        self.ws.subscribe_call_updates(call_id).await
    }

    /// Send a call event (e.g. participant joined/left) over WebSocket.
    pub async fn send_call_event_ws(
        &self,
        call_id: &str,
        event: &str,
        params: serde_json::Value,
    ) -> Result<u64, String> {
        self.ws.send_call_event_ws(call_id, event, params).await
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

/// Read image size without full pixel buffer when the decoder supports it.
fn image_dimensions_fast(data: &[u8]) -> (Option<u32>, Option<u32>) {
    use std::io::Cursor;
    let reader = image::ImageReader::new(Cursor::new(data)).with_guessed_format();
    match reader {
        Ok(r) => match r.into_dimensions() {
            Ok((w, h)) => (Some(w), Some(h)),
            Err(_) => (None, None),
        },
        Err(_) => (None, None),
    }
}

/// Write a small PNG preview under cache for instant bubble display (file://).
fn cache_outgoing_image_preview(file_data: &[u8], file_id: &str) -> Option<String> {
    use std::io::Cursor;

    let cache = dirs::cache_dir()?
        .join("yandex-messenger-native")
        .join("previews");
    std::fs::create_dir_all(&cache).ok()?;
    let safe: String = file_id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let path = cache.join(format!("{}.png", safe));

    // Prefer small thumbnail for UI; skip on failure
    let mut reader = image::ImageReader::new(Cursor::new(file_data))
        .with_guessed_format()
        .ok()?;
    reader.no_limits();
    let img = reader.decode().ok()?;
    let thumb = if img.width() > 480 || img.height() > 480 {
        img.thumbnail(480, 480)
    } else {
        img
    };
    let mut out = Vec::new();
    {
        let mut c = Cursor::new(&mut out);
        thumb.write_to(&mut c, image::ImageFormat::Png).ok()?;
    }
    std::fs::write(&path, &out).ok()?;
    // file:// URI for load_inline_image
    Some(format!("file://{}", path.display()))
}
