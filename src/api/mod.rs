#![allow(dead_code)]

pub mod auth;
pub mod bot;
pub mod folder;
pub mod group;
pub mod saved_message;
pub mod scheduled_message;
pub mod session_store;
pub mod translation;
pub mod telemost;

/// Delivery/read status update from WebSocket or history.
#[derive(Debug, Clone)]
pub struct StatusUpdate {
    pub message_id: Option<String>,
    pub chat_id: Option<String>,
    pub delivered: bool,
    pub read: bool,
}

/// Normalize server epoch to **unix seconds**.
///
/// Yandex payloads mix units:
/// - seconds (~1e9)
/// - milliseconds (~1e12)
/// - microseconds (~1e15)
///
/// Bug we hit: treating milliseconds as microseconds → `ts/1e6` ≈ 1970 →
/// new messages sorted to the **top** of the chat.
pub fn normalize_unix_timestamp_secs(ts: i64) -> i64 {
    if ts <= 0 {
        return chrono::Utc::now().timestamp();
    }
    if ts > 1_000_000_000_000_000 {
        // microseconds
        ts / 1_000_000
    } else if ts > 10_000_000_000 {
        // milliseconds (1e10 covers 2286; current ~1.7e12)
        ts / 1_000
    } else {
        // seconds
        ts
    }
}

/// Same as [`normalize_unix_timestamp_secs`] but as `DateTime<Utc>`.
pub fn normalize_unix_timestamp(ts: i64) -> chrono::DateTime<chrono::Utc> {
    let secs = normalize_unix_timestamp_secs(ts);
    chrono::DateTime::from_timestamp(secs, 0).unwrap_or_else(chrono::Utc::now)
}

use reqwest::Client;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{oneshot, Mutex};

use crate::api::auth::AuthManager;
use crate::config;
use crate::models;

use futures::{SinkExt, StreamExt};
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::Message;

/// WebSocket state
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WSState {
    Disconnected,
    Connecting,
    Connected,
    Reconnecting(u32),
}

/// WebSocket message callback type
type MessageCallback = Box<dyn Fn(&models::WSMessage) + Send + Sync>;
type StateCallback = Box<dyn Fn(WSState) + Send + Sync>;

/// WebSocket client for Yandex Messenger API
pub struct WebSocketClient {
    auth: Arc<AuthManager>,
    state: Arc<Mutex<WSState>>,
    callbacks: Arc<Mutex<Vec<MessageCallback>>>,
    state_callbacks: Arc<Mutex<Vec<StateCallback>>>,
    seq_counter: Arc<Mutex<u64>>,
    tx: Arc<
        Mutex<Option<tokio::sync::mpsc::UnboundedSender<tokio_tungstenite::tungstenite::Message>>>,
    >,
    pending_requests: Arc<Mutex<HashMap<u64, oneshot::Sender<Result<serde_json::Value, String>>>>>,
    /// Current chat being subscribed to
    current_chat_id: Arc<Mutex<Option<String>>>,
}

#[derive(serde::Deserialize)]
struct ListResponse<T> {
    items: Option<Vec<T>>,
    chats: Option<Vec<T>>,
    messages: Option<Vec<T>>,
}

impl WebSocketClient {
    pub fn new(auth: Arc<AuthManager>) -> Self {
        Self {
            auth,
            state: Arc::new(Mutex::new(WSState::Disconnected)),
            callbacks: Arc::new(Mutex::new(Vec::new())),
            state_callbacks: Arc::new(Mutex::new(Vec::new())),
            seq_counter: Arc::new(Mutex::new(0)),
            tx: Arc::new(Mutex::new(None)),
            pending_requests: Arc::new(Mutex::new(HashMap::new())),
            current_chat_id: Arc::new(Mutex::new(None)),
        }
    }

    pub async fn on_message<F>(&self, callback: F)
    where
        F: Fn(&models::WSMessage) + Send + Sync + 'static,
    {
        let mut callbacks = self.callbacks.lock().await;
        callbacks.push(Box::new(callback));
    }

    pub async fn on_state_change<F>(&self, callback: F)
    where
        F: Fn(WSState) + Send + Sync + 'static,
    {
        let mut callbacks = self.state_callbacks.lock().await;
        callbacks.push(Box::new(callback));
    }

    fn get_session_cookies_and_uid() -> Option<(String, String)> {
        let data = session_store::load_session()?;
        let cookie_str = data.cookie_header();
        let uid = data.yuid().unwrap_or_default();
        if cookie_str.is_empty() || uid.is_empty() {
            None
        } else {
            Some((cookie_str, uid))
        }
    }

    fn serialize_push_header_with_method(seq: u64, method: &str) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.push(0x93);
        bytes.push(0x00);
        if seq < 128 {
            bytes.push(seq as u8);
        } else if seq < 256 {
            bytes.push(0xcc);
            bytes.push(seq as u8);
        } else if seq < 65536 {
            bytes.push(0xcd);
            bytes.extend_from_slice(&(seq as u16).to_be_bytes());
        } else {
            bytes.push(0xce);
            bytes.extend_from_slice(&(seq as u32).to_be_bytes());
        }

        let len = method.len();
        if len < 32 {
            bytes.push(0xa0 + len as u8);
        } else if len < 256 {
            bytes.push(0xd9);
            bytes.push(len as u8);
        } else if len < 65536 {
            bytes.push(0xda);
            bytes.extend_from_slice(&(len as u16).to_be_bytes());
        } else {
            bytes.push(0xdb);
            bytes.extend_from_slice(&(len as u32).to_be_bytes());
        }
        bytes.extend_from_slice(method.as_bytes());
        bytes
    }

    fn serialize_push_header(seq: u64) -> Vec<u8> {
        Self::serialize_push_header_with_method(seq, "push")
    }

    fn parse_msgpack_seq(bin: &[u8]) -> Option<(u64, usize)> {
        if bin.len() < 3 {
            return None;
        }
        let array_header = bin[1];
        if array_header != 0x92 && array_header != 0x93 && array_header != 0x94 {
            return None;
        }

        let offset = 2;
        let type_byte = bin[offset];
        if type_byte <= 0x7f {
            Some((type_byte as u64, offset + 1))
        } else if type_byte == 0xcc {
            if bin.len() < offset + 2 {
                return None;
            }
            Some((bin[offset + 1] as u64, offset + 2))
        } else if type_byte == 0xcd {
            if bin.len() < offset + 3 {
                return None;
            }
            let val = u16::from_be_bytes([bin[offset + 1], bin[offset + 2]]);
            Some((val as u64, offset + 3))
        } else if type_byte == 0xce {
            if bin.len() < offset + 5 {
                return None;
            }
            let val = u32::from_be_bytes([
                bin[offset + 1],
                bin[offset + 2],
                bin[offset + 3],
                bin[offset + 4],
            ]);
            Some((val as u64, offset + 5))
        } else if type_byte == 0xcf {
            if bin.len() < offset + 9 {
                return None;
            }
            let val = u64::from_be_bytes([
                bin[offset + 1],
                bin[offset + 2],
                bin[offset + 3],
                bin[offset + 4],
                bin[offset + 5],
                bin[offset + 6],
                bin[offset + 7],
                bin[offset + 8],
            ]);
            Some((val, offset + 9))
        } else {
            None
        }
    }

    fn extract_json_payload(bin: &[u8]) -> Option<String> {
        let scan_start = 50.min(bin.len());
        if let Some(pos) = bin[scan_start..].windows(2).position(|w| w == b"{\"") {
            let actual_pos = scan_start + pos;
            String::from_utf8(bin[actual_pos..].to_vec()).ok()
        } else {
            if let Some(pos) = bin.windows(2).position(|w| w == b"{\"") {
                String::from_utf8(bin[pos..].to_vec()).ok()
            } else {
                None
            }
        }
    }

    fn find_json_field(val: &serde_json::Value, key: &str) -> Option<String> {
        if let Some(obj) = val.as_object() {
            if let Some(v) = obj.get(key) {
                if let Some(s) = v.as_str() {
                    return Some(s.to_string());
                }
                if let Some(n) = v.as_u64() {
                    return Some(n.to_string());
                }
            }
            for (_, child) in obj {
                if let Some(res) = Self::find_json_field(child, key) {
                    return Some(res);
                }
            }
        } else if let Some(arr) = val.as_array() {
            for child in arr {
                if let Some(res) = Self::find_json_field(child, key) {
                    return Some(res);
                }
            }
        }
        None
    }

    /// Connects to the WebSocket server with automatic reconnection.
    /// Spawns a background task that handles reconnection with exponential backoff.
    pub fn connect(&self) -> impl std::future::Future<Output = Result<(), String>> + '_ {
        let client = self;
        async move {
            let mut attempts = 0u32;
            let mut interval_ms = 1000u64; // Start with 1 second, double each time (max 32s)

            loop {
                match client.do_connect().await {
                    Ok(()) => {
                        attempts = 0;
                        interval_ms = 1000;
                        // Connection established — run forever until disconnected
                        client.run_forever().await;
                        // Reconnect loop
                        attempts += 1;
                        log::info!(
                            "WebSocket disconnected, reconnecting in {}s (attempt {})",
                            interval_ms / 1000,
                            attempts
                        );
                        tokio::time::sleep(std::time::Duration::from_millis(interval_ms)).await;
                        interval_ms = (interval_ms * 2).min(32_000);
                    }
                    Err(e) => {
                        attempts += 1;
                        log::warn!("WebSocket connection failed (attempt {}): {}", attempts, e);
                        tokio::time::sleep(std::time::Duration::from_millis(interval_ms)).await;
                        interval_ms = (interval_ms * 2).min(32_000);
                    }
                }
            }
        }
    }

    /// Perform a single WebSocket connection attempt
    async fn do_connect(&self) -> Result<(), String> {
        let (cookies_str, uid) = Self::get_session_cookies_and_uid().ok_or_else(|| {
            "Failed to load session cookies/UID from session.json. Please re-login."
                .to_string()
        })?;

        // OAuth is REQUIRED for push.yandex.ru — cookies alone → close 4401 "no credentials"
        let oauth = self
            .auth
            .get_access_token()
            .map_err(|e| {
                format!(
                    "No OAuth access token ({e}). Re-login so messenger can open the push WebSocket."
                )
            })?;
        let auth_header = if oauth.starts_with("OAuth ") {
            oauth
        } else {
            format!("OAuth {}", oauth)
        };

        let xiva_session = format!(
            "{}-{}-{}-{}",
            &uuid::Uuid::new_v4().simple().to_string()[..4],
            &uuid::Uuid::new_v4().simple().to_string()[..4],
            &uuid::Uuid::new_v4().simple().to_string()[..4],
            &uuid::Uuid::new_v4().simple().to_string()[..4]
        );

        let ws_url = format!(
            "wss://push.yandex.ru/v2/subscribe/websocket?service=messenger-prod%3Aversion5*common%2Bversion5*main&session={}&client=web_main&user={}",
            xiva_session, uid
        );

        let mut request = ws_url.into_client_request().map_err(|e| e.to_string())?;
        request.headers_mut().insert(
            "Cookie",
            tokio_tungstenite::tungstenite::http::HeaderValue::from_str(&cookies_str)
                .map_err(|e| e.to_string())?,
        );
        request.headers_mut().insert(
            "Authorization",
            tokio_tungstenite::tungstenite::http::HeaderValue::from_str(&auth_header)
                .map_err(|e| e.to_string())?,
        );
        request.headers_mut().insert(
            "Origin",
            tokio_tungstenite::tungstenite::http::HeaderValue::from_static("https://yandex.ru"),
        );
        request.headers_mut().insert(
            "User-Agent",
            tokio_tungstenite::tungstenite::http::HeaderValue::from_static("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
        );

        let (ws_stream, _response) = tokio::time::timeout(
            std::time::Duration::from_secs(15),
            tokio_tungstenite::connect_async(request),
        )
        .await
        .map_err(|_| "WebSocket connection timed out".to_string())?
        .map_err(|e| format!("WebSocket connect failed: {}", e))?;

        let (mut write, mut read) = ws_stream.split();
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Message>();

        *self.tx.lock().await = Some(tx.clone());

        {
            let mut state = self.state.lock().await;
            *state = WSState::Connected;
        }
        self.notify_state(WSState::Connected).await;

        let callbacks = self.callbacks.clone();
        let state_callbacks = self.state_callbacks.clone();
        let state = self.state.clone();
        let pending = self.pending_requests.clone();
        let tx_for_ping = self.tx.clone();

        // Write loop & heartbeat
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(
                crate::config::WS_HEARTBEAT_INTERVAL,
            ));
            loop {
                tokio::select! {
                    msg = rx.recv() => {
                        match msg {
                            Some(ws_msg) => {
                                if write.send(ws_msg).await.is_err() {
                                    break;
                                }
                            }
                            None => break,
                        }
                    }
                    _ = interval.tick() => {
                        if write.send(Message::Ping(vec![].into())).await.is_err() {
                            break;
                        }
                    }
                }
            }
        });

        // Read loop — when it ends, the caller (connect) will reconnect
        let seq_counter_clone = self.seq_counter.clone();
        let tx_clone = self.tx.clone();
        tokio::spawn(async move {
            while let Some(msg) = read.next().await {
                match msg {
                    Ok(Message::Text(text)) => {
                        if text.contains("\"operation\":\"ping\"") {
                            let tx_guard = tx_for_ping.lock().await;
                            if let Some(ref sender) = *tx_guard {
                                let _ =
                                    sender.send(Message::Text("{\"operation\":\"pong\"}".into()));
                            }
                        }

                        // Handle pending requests
                        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) {
                            if let Some(seq) = value.get("seq").and_then(|s| s.as_u64()) {
                                let mut p_reqs = pending.lock().await;
                                if let Some(sender) = p_reqs.remove(&seq) {
                                    if let Some(err) = value.get("error") {
                                        let _ = sender.send(Err(err.to_string()));
                                    } else if let Some(res) = value.get("result") {
                                        let _ = sender.send(Ok(res.clone()));
                                    } else if let Some(msg) = value.get("message") {
                                        let _ = sender.send(Ok(msg.clone()));
                                    } else {
                                        let _ = sender.send(Ok(value.clone()));
                                    }
                                }
                            }
                        }

                        if let Ok(ws_msg) = serde_json::from_str::<crate::models::WSMessage>(&text)
                        {
                            let cbs = callbacks.lock().await;
                            for cb in cbs.iter() {
                                cb(&ws_msg);
                            }
                        }
                    }
                    Ok(Message::Binary(bin)) => {
                        if bin.is_empty() {
                            continue;
                        }
                        let first_byte = bin[0];
                        if first_byte == 0x01 || first_byte == 0x02 {
                            // RPC response (0x01 for success, 0x02 for error/control)
                            if let Some((seq, json_start_offset)) = Self::parse_msgpack_seq(&bin) {
                                // Sync sequence counter!
                                {
                                    let mut counter = seq_counter_clone.lock().await;
                                    *counter = std::cmp::max(*counter, seq + 1);
                                }

                                let mut was_handled = false;
                                {
                                    let mut p_reqs = pending.lock().await;
                                    if let Some(sender) = p_reqs.remove(&seq) {
                                        was_handled = true;
                                        let trailing_json = if bin.len() > json_start_offset {
                                            Self::extract_json_payload(&bin[json_start_offset..])
                                        } else {
                                            None
                                        };

                                        if let Some(json_str) = trailing_json {
                                            if let Ok(val) =
                                                serde_json::from_str::<serde_json::Value>(&json_str)
                                            {
                                                let _ = sender.send(Ok(val));
                                            } else {
                                                let _ = sender
                                                    .send(Ok(serde_json::json!({"status": "ok"})));
                                            }
                                        } else {
                                            let _ = sender
                                                .send(Ok(serde_json::json!({"status": "ok"})));
                                        }
                                    }
                                }

                                // If it is a server push notification (not an RPC response to our request)
                                // we must send an ACK frame back to the server to maintain session status!
                                if !was_handled && first_byte == 0x01 {
                                    let ack = Self::make_ack_packet(seq);
                                    let tx_guard = tx_clone.lock().await;
                                    if let Some(ref sender) = *tx_guard {
                                        let _ = sender.send(Message::Binary(ack));
                                    }
                                }
                            }
                        } else if first_byte == 0x03 {
                            // Passive push delivery
                            if let Some(json_str) = Self::extract_json_payload(&bin) {
                                if let Ok(parsed_json) =
                                    serde_json::from_str::<serde_json::Value>(&json_str)
                                {
                                    let mut method = "";
                                    let mut mapped_msg = serde_json::json!({});

                                    if let Some(client_msg) = parsed_json.get("ClientMessage") {
                                        if let Some(plain) = client_msg.get("Plain") {
                                            let chat_id = plain
                                                .get("ChatId")
                                                .and_then(|v| v.as_str())
                                                .unwrap_or("");
                                            let text_obj = plain.get("Text");
                                            let text = text_obj
                                                .and_then(|t| t.get("MessageText"))
                                                .and_then(|t| t.as_str())
                                                .unwrap_or("");
                                            let payload_id = plain
                                                .get("PayloadId")
                                                .and_then(|p| p.as_str())
                                                .unwrap_or("");
                                            let from_guid = parsed_json
                                                .get("ServerMessageInfo")
                                                .and_then(|smi| smi.get("From"))
                                                .and_then(|f| f.get("Guid"))
                                                .and_then(|g| g.as_str())
                                                .or_else(|| {
                                                    parsed_json
                                                        .get("ServerMessageInfo")
                                                        .and_then(|smi| smi.get("FromGuid"))
                                                        .and_then(|fg| fg.as_str())
                                                })
                                                .unwrap_or("system");
                                            let timestamp = parsed_json
                                                .get("ServerMessageInfo")
                                                .and_then(|smi| smi.get("Timestamp"))
                                                .and_then(|t| t.as_i64())
                                                .unwrap_or_else(|| {
                                                    chrono::Utc::now().timestamp_millis()
                                                });
                                            let created_secs =
                                                normalize_unix_timestamp_secs(timestamp);

                                            if !chat_id.is_empty() && !text.is_empty() {
                                                method = "new_message";
                                                let msg_id_val: String = if !payload_id.is_empty() {
                                                    payload_id.to_string()
                                                } else {
                                                    uuid::Uuid::new_v4().simple().to_string()
                                                };
                                                mapped_msg = serde_json::json!({
                                                    "method": "new_message",
                                                    "messages": [{
                                                        "id": format!("{}_{}", created_secs, msg_id_val),
                                                        "chat_id": chat_id,
                                                        "from_id": from_guid,
                                                        "message_id": Some(msg_id_val.clone()),
                                                        "type": "text",
                                                        "text": text,
                                                        "created": created_secs
                                                    }]
                                                });
                                            }
                                        }
                                    } else if parsed_json.get("ServerMessage").is_some() {
                                        let chat_id = Self::find_json_field(&parsed_json, "ChatId")
                                            .unwrap_or_default();
                                        let text = Self::find_json_field(&parsed_json, "Text")
                                            .unwrap_or_default();
                                        let from_guid =
                                            Self::find_json_field(&parsed_json, "FromGuid")
                                                .unwrap_or_else(|| "system".to_string());
                                        let message_id =
                                            Self::find_json_field(&parsed_json, "MessageId")
                                                .or_else(|| {
                                                    Self::find_json_field(&parsed_json, "PayloadId")
                                                })
                                                .unwrap_or_else(|| {
                                                    uuid::Uuid::new_v4().simple().to_string()
                                                });
                                        let timestamp = parsed_json
                                            .get("ServerMessageInfo")
                                            .and_then(|smi| smi.get("Timestamp"))
                                            .and_then(|t| t.as_i64())
                                            .unwrap_or_else(|| {
                                                chrono::Utc::now().timestamp_millis()
                                            });
                                        let created_secs =
                                            normalize_unix_timestamp_secs(timestamp);

                                        if !chat_id.is_empty() && !text.is_empty() {
                                            method = "new_message";
                                            mapped_msg = serde_json::json!({
                                                "method": "new_message",
                                                "messages": [{
                                                    "id": format!("{}_{}", created_secs, message_id),
                                                    "chat_id": chat_id,
                                                    "from_id": from_guid,
                                                    "message_id": Some(message_id),
                                                    "type": "text",
                                                    "text": text,
                                                    "created": created_secs
                                                }]
                                            });
                                        }
                                    }

                                    if !method.is_empty() {
                                        let ws_msg = crate::models::WSMessage {
                                            seq: 0,
                                            message: mapped_msg,
                                        };
                                        let cbs = callbacks.lock().await;
                                        for cb in cbs.iter() {
                                            cb(&ws_msg);
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Ok(Message::Close(_)) => {
                        break;
                    }
                    Err(_) => break,
                    _ => {}
                }
            }
            let mut st = state.lock().await;
            *st = WSState::Disconnected;
            drop(st);
            let scbs = state_callbacks.lock().await;
            for cb in scbs.iter() {
                cb(WSState::Disconnected);
            }
        });

        Ok(())
    }

    /// Run forever while connected — subscribes to current chat and handles WS messages
    async fn run_forever(&self) {
        // Subscribe to the current chat if we have one
        if let Some(chat_id) = self.current_chat_id.lock().await.clone() {
            let _ = self.subscribe(&chat_id).await;
        }

        // Spawn a task that keeps reading WS messages
        let _callbacks = self.callbacks.clone();
        let _state_callbacks = self.state_callbacks.clone();
        let state = self.state.clone();
        let _pending = self.pending_requests.clone();

        // We need to reconnect the read loop — we'll do this by periodically checking
        // if we're still connected and re-subscribing
        let mut last_state = WSState::Connected;

        loop {
            // Check if still connected
            let current_state = {
                let s = state.lock().await;
                s.clone()
            };

            if current_state != WSState::Connected {
                break;
            }

            // Re-subscribe if state changed
            if current_state != last_state {
                log::info!(
                    "WebSocket state changed to {:?}, re-subscribing",
                    current_state
                );
                last_state = current_state;
                if let Some(chat_id) = self.current_chat_id.lock().await.clone() {
                    let _ = self.subscribe(&chat_id).await;
                }
            }

            // Heartbeat check — if no heartbeat for 60s, the connection is dead
            // We'll rely on the ping/pong from the write loop to keep it alive
            tokio::time::sleep(std::time::Duration::from_secs(30)).await;
        }
    }

    /// Set the current chat to subscribe to
    pub async fn set_current_chat(&self, chat_id: Option<String>) {
        let mut current = self.current_chat_id.lock().await;
        if current.as_deref() != chat_id.as_deref() {
            // Unsubscribe from previous
            if let Some(prev) = current.take() {
                let _ = self.unsubscribe(&prev).await;
            }
            *current = chat_id.clone();
            // Subscribe to new
            if let Some(id) = chat_id {
                let _ = self.subscribe(&id).await;
            }
        }
    }

    /// Whether the push WebSocket is currently connected.
    pub async fn is_connected(&self) -> bool {
        let state = self.state.lock().await;
        *state == WSState::Connected
    }

    /// Force reconnect by dropping the active WebSocket connection
    pub async fn force_reconnect(&self) {
        let mut tx_opt = self.tx.lock().await;
        if let Some(tx) = tx_opt.take() {
            // Drop sender to close receiver channel, which breaks the write loop and closes connection
            drop(tx);
        }
        let mut state = self.state.lock().await;
        *state = WSState::Disconnected;
    }

    /// Send a message via WebSocket (using the Yandex binary protocol)
    pub async fn send_message(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<u64, String> {
        let mut counter = self.seq_counter.lock().await;
        let seq = *counter;
        *counter += 1;
        drop(counter);

        let msgpack_header = Self::serialize_push_header_with_method(seq, method);

        let mut bin_header = Vec::new();
        bin_header.push(0x05);
        bin_header.extend_from_slice(&0u64.to_be_bytes()); // 0 timestamp
        bin_header.extend_from_slice(&[0, 0, 0]); // Fixed: Must be exactly 3 bytes (total header size = 12 bytes)

        let json_bytes = serde_json::to_vec(&params).unwrap_or_default();

        let mut packet = Vec::new();
        packet.push(0x01);
        packet.extend_from_slice(&msgpack_header);
        packet.extend_from_slice(&bin_header);
        packet.extend_from_slice(&json_bytes);

        let state = self.state.lock().await;
        if *state != WSState::Connected {
            return Err("Not connected".to_string());
        }
        drop(state);

        let tx_guard = self.tx.lock().await;
        if let Some(tx) = tx_guard.as_ref() {
            let _ = tx.send(Message::Binary(packet));
        }

        Ok(seq)
    }

    /// Send a message via WebSocket and wait for a response
    pub async fn send_request(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        let (tx_resp, rx_resp) = oneshot::channel();

        let seq = self.send_message(method, params).await?;

        {
            let mut pending = self.pending_requests.lock().await;
            pending.insert(seq, tx_resp);
        }

        tokio::time::timeout(std::time::Duration::from_secs(10), rx_resp)
            .await
            .map_err(|_| "WebSocket request timed out".to_string())?
            .map_err(|_| "WebSocket request channel closed".to_string())?
    }

    pub async fn get_chat_list(
        &self,
        _offset: usize,
        _limit: usize,
    ) -> Result<Vec<models::Chat>, String> {
        let response = self
            .send_request(
                "bootstrap",
                serde_json::json!({
                    "flags": {
                        "with_deleted": true,
                        "compact": false
                    }
                }),
            )
            .await?;

        log::info!(
            "Bootstrap response: {}",
            serde_json::to_string_pretty(&response).unwrap_or_default()
        );

        // Extract chats from the response
        if let Some(chats_val) = response.get("chats") {
            let chats: Vec<models::Chat> = serde_json::from_value(chats_val.clone())
                .map_err(|e| format!("Failed to parse chats: {}", e))?;
            return Ok(chats);
        }

        Err("No chats found in bootstrap response".to_string())
    }

    pub async fn get_messages(
        &self,
        chat_id: &str,
        msg_id: Option<&str>,
        _offset: usize,
        limit: usize,
    ) -> Result<Vec<models::Message>, String> {
        let mut params = serde_json::json!({
            "chatId": chat_id,
            "limit": limit
        });
        if let Some(mid) = msg_id {
            params["mid"] = serde_json::json!(mid);
        }

        let response = self.send_request("get_history", params).await?;

        log::info!(
            "History response: {}",
            serde_json::to_string_pretty(&response).unwrap_or_default()
        );

        if let Some(messages_val) = response.get("messages") {
            let messages: Vec<models::Message> = serde_json::from_value(messages_val.clone())
                .map_err(|e| format!("Failed to parse messages: {}", e))?;
            return Ok(messages);
        }

        Err("No messages found in history response".to_string())
    }

    fn get_yuid_from_session() -> Option<String> {
        session_store::load_session()?.yuid()
    }

    fn make_ack_packet(seq: u64) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.push(0x02); // Packet type 2
                          // MsgPack array of 2 elements [seq, 0]
        bytes.push(0x92);
        // Serialize seq
        if seq < 128 {
            bytes.push(seq as u8);
        } else if seq < 256 {
            bytes.push(0xcc);
            bytes.push(seq as u8);
        } else if seq < 65536 {
            bytes.push(0xcd);
            bytes.extend_from_slice(&(seq as u16).to_be_bytes());
        } else {
            bytes.push(0xce);
            bytes.extend_from_slice(&(seq as u32).to_be_bytes());
        }
        bytes.push(0x00); // 0 (success status)
        bytes
    }

    pub async fn send_text_message(
        &self,
        chat_id: &str,
        text: &str,
        reply_to: Option<&str>,
    ) -> Result<models::Message, String> {
        let payload_id = uuid::Uuid::new_v4().simple().to_string();

        let mut counter = self.seq_counter.lock().await;
        let seq = *counter;
        *counter += 1;
        drop(counter);

        let yuid = Self::get_yuid_from_session().ok_or_else(|| {
            "Нет session yuid (session.json). Запустите scripts/login_browser.py или перелогиньтесь."
                .to_string()
        })?;
        let custom_payload_json = serde_json::json!({
            "service": {
                "serviceName": "WEB",
                "region": "Санкт-Петербург",
                "yuid": yuid,
                "isHistory": true,
                "ui": "desktop",
                "ua": "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
                "id": format!("{}-{}-{}-{}",
                    &uuid::Uuid::new_v4().simple().to_string()[..4],
                    &uuid::Uuid::new_v4().simple().to_string()[..4],
                    &uuid::Uuid::new_v4().simple().to_string()[..4],
                    &uuid::Uuid::new_v4().simple().to_string()[..4]
                ),
                "version": "3.18.0"
            }
        });

        let custom_payload_str = serde_json::to_string(&custom_payload_json).unwrap_or_default();
        use base64::Engine;
        let custom_payload_b64 =
            base64::engine::general_purpose::STANDARD.encode(custom_payload_str);

        let mut plain = serde_json::json!({
            "ChatId": chat_id,
            "Text": {
                "MessageText": text
            },
            "PayloadId": payload_id,
            "CustomPayload": custom_payload_b64
        });

        if let Some(rtid) = reply_to {
            plain["ReplyTo"] = serde_json::json!(rtid);
        }

        let payload = serde_json::json!({
            "ClientMessage": {
                "Plain": plain
            }
        });

        let state = self.state.lock().await;
        if *state != WSState::Connected {
            return Err("Not connected".to_string());
        }
        drop(state);

        let msgpack_header = Self::serialize_push_header(seq);

        let mut bin_header = Vec::new();
        bin_header.push(0x05);
        bin_header.extend_from_slice(&0u64.to_be_bytes()); // Fixed: Use 0 timestamp to match relative time format expected by Yandex
        bin_header.extend_from_slice(&[0, 0, 0]); // Fixed: Must be exactly 3 bytes (total header size = 12 bytes)

        let json_bytes = serde_json::to_vec(&payload).unwrap_or_default();

        let mut packet = Vec::new();
        packet.push(0x01);
        packet.extend_from_slice(&msgpack_header);
        packet.extend_from_slice(&bin_header);
        packet.extend_from_slice(&json_bytes);

        let tx_guard = self.tx.lock().await;
        if let Some(tx) = tx_guard.as_ref() {
            let _ = tx.send(Message::Binary(packet));
        }

        // Optimistic local message for UI
        let reply_to = reply_to.map(|rtid| models::MessageId {
            chat_id: chat_id.to_string(),
            message_id: rtid.to_string(),
        });
        Ok(models::Message {
            id: payload_id.clone(),
            chat_id: chat_id.to_string(),
            from_id: self.auth.get_current_account_id().await.unwrap_or_default(),
            message_id: Some(payload_id.clone()),
            rmid: None,
            type_: models::MessageType::Text,
            text: Some(text.to_string()),
            entities: vec![],
            reply_to,
            forward: None,
            media: vec![],
            reactions: vec![],
            thread_id: None,
            has_thread: false,
            pinned: false,
            edited: false,
            edited_at: None,
            sent: true,
            // Optimistic: server accepted the push → delivered until proven otherwise
            delivered: true,
            read: false,
            created: chrono::Utc::now(),
            updated: None,
            poll: None,
        })
    }

    /// Send an already-uploaded file as ClientMessage (Image or MiscFile).
    /// `file_id` comes from `HttpClient::upload_file_for_message` and must have been
    /// uploaded with the same `payload_id` as `messageId` path segment.
    pub async fn send_file_message(
        &self,
        chat_id: &str,
        file_id: &str,
        filename: &str,
        size: u64,
        mime: &str,
        width: Option<u32>,
        height: Option<u32>,
        payload_id: &str,
    ) -> Result<models::Message, String> {
        let mut counter = self.seq_counter.lock().await;
        let seq = *counter;
        *counter += 1;
        drop(counter);

        let yuid = Self::get_yuid_from_session().ok_or_else(|| {
            "Нет session yuid (session.json). Перелогиньтесь, чтобы отправлять файлы."
                .to_string()
        })?;
        let custom_payload_json = serde_json::json!({
            "service": {
                "serviceName": "WEB",
                "region": "Санкт-Петербург",
                "yuid": yuid,
                "isHistory": true,
                "ui": "desktop",
                "ua": "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
                "id": format!("{}-{}-{}-{}",
                    &uuid::Uuid::new_v4().simple().to_string()[..4],
                    &uuid::Uuid::new_v4().simple().to_string()[..4],
                    &uuid::Uuid::new_v4().simple().to_string()[..4],
                    &uuid::Uuid::new_v4().simple().to_string()[..4]
                ),
                "version": "3.18.0"
            }
        });
        let custom_payload_str = serde_json::to_string(&custom_payload_json).unwrap_or_default();
        use base64::Engine;
        let custom_payload_b64 =
            base64::engine::general_purpose::STANDARD.encode(custom_payload_str);

        // FileSource.MDS = 0 (web client default for media_upload)
        let file_info = serde_json::json!({
            "Id2": file_id,
            "Name": filename,
            "Size": size,
            "Source": 0
        });

        let is_image = mime.starts_with("image/");
        let mut plain = serde_json::json!({
            "ChatId": chat_id,
            "PayloadId": payload_id,
            "CustomPayload": custom_payload_b64
        });

        if is_image {
            plain["Image"] = serde_json::json!({
                "Width": width.unwrap_or(0),
                "Height": height.unwrap_or(0),
                "FileInfo": file_info,
                "Animated": mime == "image/gif" || filename.to_ascii_lowercase().ends_with(".gif")
            });
        } else {
            plain["MiscFile"] = serde_json::json!({
                "FileInfo": file_info
            });
            // Caption-like text optional; many clients still set empty Text
            plain["Text"] = serde_json::json!({
                "MessageText": filename
            });
        }

        let payload = serde_json::json!({
            "ClientMessage": {
                "Plain": plain
            }
        });

        let state = self.state.lock().await;
        if *state != WSState::Connected {
            return Err("Not connected".to_string());
        }
        drop(state);

        let msgpack_header = Self::serialize_push_header(seq);
        let mut bin_header = Vec::new();
        bin_header.push(0x05);
        bin_header.extend_from_slice(&0u64.to_be_bytes());
        bin_header.extend_from_slice(&[0, 0, 0]);
        let json_bytes = serde_json::to_vec(&payload).unwrap_or_default();
        let mut packet = Vec::new();
        packet.push(0x01);
        packet.extend_from_slice(&msgpack_header);
        packet.extend_from_slice(&bin_header);
        packet.extend_from_slice(&json_bytes);

        let tx_guard = self.tx.lock().await;
        if let Some(tx) = tx_guard.as_ref() {
            let _ = tx.send(Message::Binary(packet));
        } else {
            return Err("WebSocket send channel missing".to_string());
        }

        let media_type = if is_image {
            models::MediaType::Image
        } else {
            models::MediaType::Document
        };
        // Prefer shortterm path (works with OAuth in preview loader)
        let media_url = format!(
            "{}/file_shortterm/{}",
            crate::config::FILE_PUBLIC_HOST.trim_end_matches('/'),
            file_id.trim_start_matches('/')
        );

        Ok(models::Message {
            id: payload_id.to_string(),
            chat_id: chat_id.to_string(),
            from_id: self.auth.get_current_account_id().await.unwrap_or_default(),
            message_id: Some(payload_id.to_string()),
            rmid: None,
            type_: if is_image {
                models::MessageType::Image
            } else {
                models::MessageType::File
            },
            text: if is_image {
                None
            } else {
                Some(filename.to_string())
            },
            entities: vec![],
            reply_to: None,
            forward: None,
            media: vec![models::MediaAttachment {
                id: file_id.to_string(),
                type_: media_type,
                url: media_url,
                thumbnail_url: None,
                width,
                height,
                size: Some(size),
                duration: None,
                filename: Some(filename.to_string()),
                mime_type: Some(mime.to_string()),
                waveform: None,
            }],
            reactions: vec![],
            thread_id: None,
            has_thread: false,
            pinned: false,
            edited: false,
            edited_at: None,
            sent: true,
            delivered: true,
            read: false,
            created: chrono::Utc::now(),
            updated: None,
            poll: None,
        })
    }

    pub async fn subscribe(&self, chat_id: &str) -> Result<u64, String> {
        self.send_message("subscribe", serde_json::json!({ "chatId": chat_id }))
            .await
    }

    pub async fn unsubscribe(&self, chat_id: &str) -> Result<u64, String> {
        self.send_message("unsubscribe", serde_json::json!({ "chatId": chat_id }))
            .await
    }

    // ============================================================
    // Thread subscriptions
    // ============================================================

    /// Подписаться на обновления thread-а
    pub async fn subscribe_thread(&self, thread_id: &str) -> Result<u64, String> {
        self.send_message(
            "subscribe_thread",
            serde_json::json!({ "threadId": thread_id }),
        )
        .await
    }

    /// Подписаться на обновления reactions
    pub async fn subscribe_reaction_updates(&self, message_id: &str) -> Result<u64, String> {
        self.send_message(
            "subscribe_reaction_updates",
            serde_json::json!({ "messageId": message_id }),
        )
        .await
    }

    /// Подписаться на typing-индикатор (enhanced)
    pub async fn subscribe_typing_enhanced(&self, chat_id: &str) -> Result<u64, String> {
        self.send_message(
            "subscribe_typing_enhanced",
            serde_json::json!({ "chatId": chat_id }),
        )
        .await
    }

    // ============================================================
    // Poll WS methods
    // ============================================================

    /// Подписаться на обновления опроса
    pub async fn subscribe_poll_updates(&self, poll_id: &str) -> Result<u64, String> {
        self.send_message(
            "subscribe_poll_updates",
            serde_json::json!({ "pollId": poll_id }),
        )
        .await
    }

    /// Отправить голос через WS
    pub async fn send_poll_vote_ws(
        &self,
        poll_id: &str,
        answer_ids: Vec<String>,
    ) -> Result<u64, String> {
        self.send_message(
            "send_poll_vote",
            serde_json::json!({
                "pollId": poll_id,
                "answerIds": answer_ids
            }),
        )
        .await
    }

    // ============================================================
    // Reaction WS methods
    // ============================================================

    /// Отправить реакцию через WebSocket
    pub async fn send_add_reaction(&self, message_id: &str, emoji: &str) -> Result<u64, String> {
        self.send_message(
            "add_reaction",
            serde_json::json!({
                "messageId": message_id,
                "emoji": emoji
            }),
        )
        .await
    }

    /// Убрать реакцию через WebSocket
    pub async fn send_remove_reaction(&self, message_id: &str, emoji: &str) -> Result<u64, String> {
        self.send_message(
            "remove_reaction",
            serde_json::json!({
                "messageId": message_id,
                "emoji": emoji
            }),
        )
        .await
    }

    /// Отправить сообщение в thread через WebSocket
    pub async fn send_thread_message_ws(
        &self,
        thread_id: &str,
        chat_id: &str,
        text: &str,
    ) -> Result<u64, String> {
        self.send_message(
            "send_thread_message",
            serde_json::json!({
                "threadId": thread_id,
                "chatId": chat_id,
                "text": text
            }),
        )
        .await
    }

    // ============================================================
    // Telemost WebSocket methods
    // ============================================================

    /// Подписаться на обновления звонка
    pub async fn subscribe_call_updates(&self, call_id: &str) -> Result<u64, String> {
        self.send_message(
            "subscribe_call_updates",
            serde_json::json!({ "callId": call_id }),
        )
        .await
    }

    /// Отправить событие звонка через WebSocket (mute/video/end)
    pub async fn send_call_event_ws(
        &self,
        call_id: &str,
        event: &str,
        params: serde_json::Value,
    ) -> Result<u64, String> {
        let mut payload = serde_json::json!({
            "callId": call_id,
            "event": event
        });
        if let Some(obj) = payload.as_object_mut() {
            if let Some(p) = params.as_object() {
                for (k, v) in p {
                    obj.insert(k.clone(), v.clone());
                }
            }
        }
        self.send_message("call_event", payload).await
    }

    async fn notify_state(&self, state: WSState) {
        let callbacks = self.state_callbacks.lock().await;
        for cb in callbacks.iter() {
            cb(state.clone());
        }
    }
}

pub struct HttpClient {
    auth: Arc<AuthManager>,
    client: Client,
    base_url: String,
    token: std::sync::Mutex<Option<String>>,
    /// Session cookies from Yandex Passport (for methods like 'messages' that require session auth)
    session_cookies: std::sync::Mutex<Option<String>>,
    /// CSRF token obtained during browser login
    csrf_token: std::sync::Mutex<Option<String>>,
}

impl HttpClient {
    pub fn new(auth: Arc<AuthManager>) -> Self {
        let (session_cookies, csrf_token) = Self::load_session_cookies();
        if session_cookies.is_some() {
            log::info!("Loaded session cookies for session API access");
        }

        Self {
            auth,
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(15))
                .user_agent("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
                .build()
                .unwrap_or_default(),
            base_url: config::API_BASE_URL.to_string(),
            token: std::sync::Mutex::new(None),
            session_cookies: std::sync::Mutex::new(session_cookies),
            csrf_token: std::sync::Mutex::new(csrf_token),
        }
    }

    /// Load session cookies from disk (in-app login or scripts/login_browser.py)
    fn load_session_cookies() -> (Option<String>, Option<String>) {
        match session_store::load_session() {
            Some(data) => {
                if data.saved_at > 0 {
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::SystemTime::UNIX_EPOCH)
                        .map(|d| d.as_secs())
                        .unwrap_or(0);
                    if now > data.saved_at {
                        let age_days = (now - data.saved_at) / 86400;
                        if age_days > 30 {
                            log::warn!(
                                "Session cookies are {} days old — re-login if history fails",
                                age_days
                            );
                        }
                    }
                }
                (Some(data.cookie_header()), data.csrf_token)
            }
            None => (None, None),
        }
    }

    /// Reload session from disk (call after in-app session capture).
    pub fn reload_session(&self) {
        let (cookies, csrf) = Self::load_session_cookies();
        *self.session_cookies.lock().unwrap() = cookies;
        *self.csrf_token.lock().unwrap() = csrf;
        if self.has_session() {
            log::info!("Session cookies reloaded into HttpClient");
        }
    }

    /// Drop in-memory Passport cookies (logout).
    pub fn clear_session_cookies(&self) {
        *self.session_cookies.lock().unwrap() = None;
        *self.csrf_token.lock().unwrap() = None;
        log::info!("Session cookies cleared from HttpClient");
    }

    /// Apply session data in-memory (and optionally already saved to disk).
    pub fn apply_session(&self, cookie_header: String, csrf: Option<String>) {
        *self.session_cookies.lock().unwrap() = Some(cookie_header);
        if csrf.is_some() {
            *self.csrf_token.lock().unwrap() = csrf;
        }
    }

    /// Check if session cookies are available
    pub fn has_session(&self) -> bool {
        self.session_cookies.lock().unwrap().is_some()
    }

    pub fn get_token_header(&self) -> String {
        self.token
            .lock()
            .unwrap()
            .as_deref()
            .unwrap_or("")
            .to_string()
    }

    pub fn get_token_raw(&self) -> String {
        let token = self.token.lock().unwrap();
        let raw = token.as_deref().unwrap_or("");
        if let Some(stripped) = raw.strip_prefix("OAuth ") {
            stripped.to_string()
        } else {
            raw.to_string()
        }
    }

    pub fn set_token(&self, token: &str) {
        let mut t = self.token.lock().unwrap();
        if token.starts_with("OAuth ") {
            *t = Some(token.to_string());
        } else {
            *t = Some(format!("OAuth {}", token));
        }
    }

    pub fn with_token(self, token: &str) -> Self {
        self.set_token(token);
        self
    }

    /// Helper: GET request, returns response body as string
    async fn get(&self, url: &str) -> Result<String, String> {
        let auth_header = self.get_token_header();
        let response = self
            .client
            .get(url)
            .header("Authorization", &auth_header)
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        response
            .text()
            .await
            .map_err(|e| format!("Response read failed: {}", e))
    }

    /// Helper: POST request with JSON body, returns response body as string
    async fn post(&self, url: &str, body: serde_json::Value) -> Result<String, String> {
        let auth_header = self.get_token_header();
        let response = self
            .client
            .post(url)
            .header("Authorization", &auth_header)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        response
            .text()
            .await
            .map_err(|e| format!("Response read failed: {}", e))
    }

    /// Helper: RPC request to Yandex Messenger backend (yamb)
    /// Uses multipart/form-data with a 'request' field as the real web client does.
    pub async fn rpc_request(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        let body = serde_json::json!({
            "method": method,
            "params": params
        });
        let body_str =
            serde_json::to_string(&body).map_err(|e| format!("JSON serialize failed: {}", e))?;

        let form = reqwest::multipart::Form::new().text("request", body_str);

        let auth_header = self.get_token_header();
        let response = self
            .client
            .post(&self.base_url)
            .header("Authorization", &auth_header)
            .header("Origin", "https://yandex.ru")
            .header("Referer", "https://yandex.ru/chat")
            .multipart(form)
            .send()
            .await
            .map_err(|e| format!("RPC request failed: {}", e))?;

        let status = response.status();
        log::info!("RPC {} response: HTTP {}", method, status);

        let text = response
            .text()
            .await
            .map_err(|e| format!("RPC response read failed: {}", e))?;

        if text.is_empty() {
            log::error!("RPC {} empty response (HTTP {})", method, status);
            return Err(format!("RPC empty response (HTTP {})", status));
        }

        log::info!(
            "RPC {} response body length: {} chars (first 500: {})",
            method,
            text.len(),
            &text[..text.len().min(500)]
        );

        let json: serde_json::Value = match serde_json::from_str(&text) {
            Ok(v) => v,
            Err(e) => {
                log::error!(
                    "RPC {} JSON parse failed: {} (body: {})",
                    method,
                    e,
                    &text[..text.len().min(500)]
                );
                return Err(format!(
                    "RPC JSON parse failed: {} (body: {})",
                    e,
                    &text[..text.len().min(500)]
                ));
            }
        };

        if json.get("status").and_then(|s| s.as_str()) == Some("error") {
            log::error!("RPC {} API error: {:?}", method, json.get("data"));
            return Err(format!("RPC error: {:?}", json.get("data")));
        }

        log::info!(
            "RPC {} parsed OK, top-level keys: {:?}",
            method,
            json.get("data").map(|d| d
                .as_object()
                .map(|o| {
                    let keys: Vec<&str> = o.keys().map(|k| k.as_str()).collect();
                    keys
                })
                .unwrap_or_default())
        );

        Ok(json["data"].clone())
    }

    /// Session-based RPC request — uses Passport session cookies + CSRF token.
    /// Required for methods like 'messages' that return 418 with OAuth auth.
    pub async fn session_rpc_request(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        let cookies_guard = self.session_cookies.lock().unwrap();
        let cookies = cookies_guard.as_ref().ok_or_else(|| {
            "No session cookies. Re-login in the app (session is captured automatically)."
                .to_string()
        })?.clone();
        drop(cookies_guard);

        let body = serde_json::json!({
            "method": method,
            "params": params
        });
        let body_str =
            serde_json::to_string(&body).map_err(|e| format!("JSON serialize failed: {}", e))?;

        let mut req = self
            .client
            .post(&self.base_url)
            .header("Cookie", cookies.as_str())
            .header("Origin", "https://yandex.ru")
            .header("Referer", "https://yandex.ru/chat");

        // Add CSRF token if available
        let current_csrf = {
            let guard = self.csrf_token.lock().unwrap();
            guard.clone()
        };
        if let Some(ref csrf) = current_csrf {
            if !csrf.is_empty() {
                req = req.header("X-Csrf-Token", csrf.as_str());
            }
        }

        let form = reqwest::multipart::Form::new().text("request", body_str.clone());
        let response = req
            .try_clone()
            .unwrap()
            .multipart(form)
            .send()
            .await
            .map_err(|e| format!("Session RPC request failed: {}", e))?;

        let status = response.status();
        let text = response
            .text()
            .await
            .map_err(|e| format!("Session RPC response read failed: {}", e))?;

        if text.is_empty() {
            return Err(format!("Session RPC empty response (HTTP {})", status));
        }

        let mut json: serde_json::Value = serde_json::from_str(&text).map_err(|e| {
            format!(
                "Session RPC parse failed: {} (body: {})",
                e,
                &text[..text.len().min(200)]
            )
        })?;

        if json.get("status").and_then(|s| s.as_str()) == Some("error") {
            if let Some(data) = json.get("data") {
                if data.get("code").and_then(|c| c.as_str()) == Some("bad_csrf_token") {
                    log::warn!("CSRF token expired, refreshing...");
                    if let Some(new_csrf) = self.refresh_csrf_token().await {
                        // Update in memory
                        if let Ok(mut guard) = self.csrf_token.lock() {
                            *guard = Some(new_csrf.clone());
                        }

                        let retry_req = self
                            .client
                            .post(&self.base_url)
                            .header("Cookie", cookies.as_str())
                            .header("Origin", "https://yandex.ru")
                            .header("Referer", "https://yandex.ru/chat")
                            .header("X-Csrf-Token", new_csrf.as_str());

                        let form2 = reqwest::multipart::Form::new().text("request", body_str);
                        let retry_resp = retry_req
                            .multipart(form2)
                            .send()
                            .await
                            .map_err(|e| format!("Session RPC retry failed: {}", e))?;

                        let retry_text = retry_resp
                            .text()
                            .await
                            .map_err(|e| format!("Retry read failed: {}", e))?;
                        json = serde_json::from_str(&retry_text)
                            .map_err(|e| format!("Retry parse failed: {}", e))?;
                    }
                }
            }
        }

        if json.get("status").and_then(|s| s.as_str()) == Some("error") {
            return Err(format!("Session RPC error: {:?}", json.get("data")));
        }

        Ok(json["data"].clone())
    }

    /// Refresh CSRF token using session cookies
    async fn refresh_csrf_token(&self) -> Option<String> {
        let cookies = self.session_cookies.lock().unwrap().clone()?;
        let url = "https://yandex.ru/messenger/api/registry/csrf-token/".to_string();
        let resp = self
            .client
            .get(&url)
            .header("Cookie", cookies.as_str())
            .header("Origin", "https://yandex.ru")
            .send()
            .await
            .ok()?;
        let text = resp.text().await.ok()?;
        log::info!("CSRF token response: {}", text);
        let json: serde_json::Value = serde_json::from_str(&text).ok()?;
        json.get("token")
            .and_then(|t| t.as_str())
            .map(|s| s.to_string())
    }

    /// Get CSRF token
    pub async fn get_csrf_token(&self) -> Result<String, String> {
        let url = "https://yandex.ru/messenger/api/registry/csrf-token/".to_string();
        let auth_header = self.get_token_header();
        let response = self
            .client
            .get(&url)
            .header("Authorization", &auth_header)
            .send()
            .await
            .map_err(|e| format!("CSRF request failed: {}", e))?;

        let json: Value = response
            .json()
            .await
            .map_err(|e| format!("CSRF parse failed: {}", e))?;

        json["token"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| "CSRF token not found".to_string())
    }

    /// Fetch user profile
    pub async fn get_user_profile(&self) -> Result<models::User, String> {
        let url = format!("{}api/get_profile", self.base_url);
        let auth_header = self.get_token_header();
        let response = self
            .client
            .get(&url)
            .header("Authorization", &auth_header)
            .send()
            .await
            .map_err(|e| format!("Profile request failed: {}", e))?;

        let json: Value = response
            .json()
            .await
            .map_err(|e| format!("Profile parse failed: {}", e))?;

        serde_json::from_value(json).map_err(|e| format!("Profile deserialization failed: {}", e))
    }

    /// Load contacts for member pickers (create group, add member, forward…).
    /// Returns non-deleted contacts with real names (contact_name preferred).
    pub async fn get_contact_candidates(&self) -> Result<Vec<models::ContactCandidate>, String> {
        let data = self
            .rpc_request(
                "bootstrap",
                serde_json::json!({
                    "flags": {
                        "with_deleted": true,
                        "compact": false
                    }
                }),
            )
            .await?;

        let current_user_id = data
            .get("user")
            .and_then(|u| u.get("guid"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let mut by_guid: std::collections::HashMap<String, models::ContactCandidate> =
            std::collections::HashMap::new();

        let mut ingest = |obj: &serde_json::Value, from_contacts: bool| {
            let guid = obj
                .get("guid")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if guid.is_empty() || guid == current_user_id {
                return;
            }
            let contact_name = obj
                .get("contact_name")
                .and_then(|v| v.as_str())
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty());
            let display_name = obj
                .get("display_name")
                .and_then(|v| v.as_str())
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty());
            let public_name = obj
                .get("public_name")
                .and_then(|v| v.as_str())
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty());
            let avatar_id = obj
                .get("avatar_id")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let deleted = obj
                .get("deleted")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            // Skip bots / system-ish accounts without human names
            let nick = obj.get("nickname").and_then(|v| v.as_str()).unwrap_or("");
            if nick.ends_with("_bot") || nick.contains("bot") {
                // still allow if has contact_name
                if contact_name.is_none() && display_name.is_none() {
                    return;
                }
            }

            let entry = by_guid
                .entry(guid.clone())
                .or_insert_with(|| models::ContactCandidate {
                    guid: guid.clone(),
                    contact_name: None,
                    display_name: None,
                    public_name: None,
                    avatar_id: None,
                    deleted: false,
                });

            if from_contacts {
                if contact_name.is_some() {
                    entry.contact_name = contact_name;
                }
                entry.deleted = deleted;
            }
            if entry.display_name.is_none() {
                entry.display_name = display_name;
            } else if from_contacts {
                // Prefer fresher display_name from contacts when present
                if let Some(d) = display_name {
                    entry.display_name = Some(d);
                }
            }
            if entry.public_name.is_none() {
                entry.public_name = public_name;
            }
            if entry.avatar_id.is_none() {
                entry.avatar_id = avatar_id;
            }
        };

        if let Some(users) = data.get("users").and_then(|v| v.as_array()) {
            for u in users {
                ingest(u, false);
            }
        }
        if let Some(contacts) = data.get("contacts").and_then(|v| v.as_array()) {
            for c in contacts {
                ingest(c, true);
            }
        }

        let mut out: Vec<models::ContactCandidate> = by_guid
            .into_values()
            .filter(|c| !c.deleted)
            .filter(|c| {
                // Must have at least one human-readable name field
                c.contact_name.is_some() || c.display_name.is_some() || c.public_name.is_some()
            })
            .collect();

        out.sort_by(|a, b| {
            a.primary_name()
                .to_lowercase()
                .cmp(&b.primary_name().to_lowercase())
        });

        log::info!("Loaded {} contact candidates for picker", out.len());
        Ok(out)
    }

    /// Get chat list
    pub async fn get_chat_list(
        &self,
        _offset: usize,
        _limit: usize,
    ) -> Result<Vec<models::Chat>, String> {
        let data = self
            .rpc_request(
                "bootstrap",
                serde_json::json!({
                    "flags": {
                        "with_deleted": true,
                        "compact": false
                    }
                }),
            )
            .await?;

        if let Ok(json_str) = serde_json::to_string_pretty(&data) {
            let _ = std::fs::write(
                "/home/bezoom/storage/Projects/Messenger/bootstrap_debug.json",
                json_str,
            );
        }

        let mut out_chats = Vec::new();

        let mut user_names = std::collections::HashMap::new();
        let mut user_avatars = std::collections::HashMap::new();
        let current_user_id = data
            .get("user")
            .and_then(|u| u.get("guid"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if !current_user_id.is_empty() {
            self.auth.set_user_id(current_user_id);
        }

        // Prefer contact book name (real name for the local user), then account names.
        fn extract_display_name(obj: &serde_json::Value) -> Option<String> {
            for key in ["contact_name", "display_name", "public_name"] {
                if let Some(name) = obj.get(key).and_then(|v| v.as_str()) {
                    let trimmed = name.trim();
                    if !trimmed.is_empty() {
                        return Some(trimmed.to_string());
                    }
                }
            }
            None
        }

        if let Some(user) = data.get("user") {
            if let Some(guid) = user.get("guid").and_then(|v| v.as_str()) {
                if let Some(name) = extract_display_name(user) {
                    user_names.insert(guid.to_string(), name.clone());
                    // Keep account header in sync with real messenger profile
                    let avatar = user
                        .get("avatar_id")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                    let auth = self.auth.clone();
                    let name_for_acc = name.clone();
                    let avatar_for_acc = avatar.clone();
                    // Fire-and-forget profile update (bootstrap is already async)
                    tokio::spawn(async move {
                        if let Err(e) = auth
                            .update_current_profile(Some(name_for_acc), avatar_for_acc)
                            .await
                        {
                            log::warn!("Failed to update account profile from bootstrap: {}", e);
                        }
                    });
                }
                if let Some(avatar) = user.get("avatar_id").and_then(|v| v.as_str()) {
                    user_avatars.insert(guid.to_string(), avatar.to_string());
                }
            }
        }

        // Parse users for display names
        if let Some(users_val) = data.get("users").and_then(|v| v.as_array()) {
            for u in users_val {
                let guid = u.get("guid").and_then(|v| v.as_str()).unwrap_or("");
                if guid.is_empty() {
                    continue;
                }
                if let Some(name) = extract_display_name(u) {
                    user_names.entry(guid.to_string()).or_insert(name);
                }
                if let Some(avatar) = u.get("avatar_id").and_then(|v| v.as_str()) {
                    user_avatars
                        .entry(guid.to_string())
                        .or_insert(avatar.to_string());
                }
            }
        }

        if let Some(contacts_val) = data.get("contacts").and_then(|v| v.as_array()) {
            for contact in contacts_val {
                let guid = contact.get("guid").and_then(|v| v.as_str()).unwrap_or("");
                if guid.is_empty() {
                    continue;
                }
                // Contact book names override generic account display names
                if let Some(name) = extract_display_name(contact) {
                    user_names.insert(guid.to_string(), name);
                }
                if let Some(avatar) = contact.get("avatar_id").and_then(|v| v.as_str()) {
                    user_avatars
                        .entry(guid.to_string())
                        .or_insert(avatar.to_string());
                }
            }
        }

        // Parse recent messages for last_message preview
        let mut recent_messages = std::collections::HashMap::new();
        if let Some(messages_val) = data.get("messages").and_then(|v| v.as_array()) {
            for m in messages_val {
                let mut chat_id = "";
                if let Some(info) = m.get("server_message_info") {
                    // Sometimes it's snake_case
                    chat_id = info.get("chat_id").and_then(|v| v.as_str()).unwrap_or("");
                } else if let Some(info) = m.get("ServerMessageInfo") {
                    chat_id = info.get("ChatId").and_then(|v| v.as_str()).unwrap_or("");
                }

                let mut text = None;
                if let Some(client_msg) = m.get("message") {
                    text = client_msg
                        .get("text")
                        .and_then(|t| t.as_str())
                        .map(|s| s.to_string());
                } else if let Some(client_msg) = m.get("ClientMessage") {
                    if let Some(plain) = client_msg.get("Plain") {
                        text = plain
                            .get("Text")
                            .and_then(|t| t.get("MessageText"))
                            .and_then(|t| t.as_str())
                            .map(|s| s.to_string());
                    }
                }

                if let Some(txt) = text {
                    recent_messages.insert(chat_id.to_string(), txt);
                }
            }
        }

        // Parse chats
        if let Some(chats_val) = data.get("chats").and_then(|v| v.as_array()) {
            for c in chats_val {
                let id = c
                    .get("chat_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let mut title = c
                    .get("name")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                let mut avatar_id = c
                    .get("avatar_id")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                let mut chat_type = models::ChatType::Group;

                // Handle private chats
                if id.contains('_') && title.is_none() {
                    chat_type = models::ChatType::Private;
                    // id is usually formatted like "0/0/guid1_guid2"
                    let clean_id = id.rsplit('/').next().unwrap_or(&id);
                    let parts: Vec<&str> = clean_id.split('_').collect();
                    if parts.len() == 2 {
                        if parts[0] == parts[1] {
                            title = Some("Избранное".to_string());
                            if let Some(avatar) = user_avatars.get(current_user_id) {
                                avatar_id = Some(avatar.clone());
                            }
                        } else {
                            let other_id = if parts[0] == current_user_id {
                                parts[1]
                            } else {
                                parts[0]
                            };
                            if let Some(name) = user_names.get(other_id) {
                                title = Some(name.clone());
                            }
                            if let Some(avatar) = user_avatars.get(other_id) {
                                avatar_id = Some(avatar.clone());
                            }
                        }
                    }
                }
                let mut last_message = None;
                let mut text = None;
                if let Some(lm) = c.get("last_message") {
                    if let Some(txt) = lm.get("text").and_then(|v| v.as_str()) {
                        text = Some(txt.to_string());
                    } else if let Some(txt) = lm.get("message").and_then(|v| v.as_str()) {
                        text = Some(txt.to_string());
                    } else if let Some(client_msg) = lm.get("ClientMessage") {
                        if let Some(plain) = client_msg.get("Plain") {
                            text = plain
                                .get("Text")
                                .and_then(|t| t.get("MessageText"))
                                .and_then(|t| t.as_str())
                                .map(|s| s.to_string());
                        }
                    }
                }

                if text.is_none() {
                    if let Some(recent_text) = recent_messages.get(&id) {
                        text = Some(recent_text.clone());
                    } else {
                        // Avoid rendering a blank message if we don't have text.
                        // Chat list will check if text is None and display "Нет сообщений".
                    }
                }

                if let Some(final_text) = text {
                    last_message = Some(models::Message {
                        id: "".to_string(),
                        chat_id: id.clone(),
                        from_id: "".to_string(),
                        message_id: None,
                        rmid: None,
                        type_: models::MessageType::Text,
                        text: Some(final_text),
                        created: chrono::Utc::now(),
                        updated: None,
                        entities: vec![],
                        reply_to: None,
                        forward: None,
                        media: vec![],
                        reactions: vec![],
                        thread_id: None,
                        has_thread: false,
                        pinned: false,
                        edited: false,
                        edited_at: None,
                        sent: true,
                        delivered: true,
                        read: true,
                        poll: None,
                    });
                }

                out_chats.push(models::Chat {
                    id,
                    title,
                    rid: None,
                    chat_type,
                    avatar_id,
                    participants: vec![],
                    unread_count: 0,
                    last_message,
                    pinned: c.get("pinned").and_then(|v| v.as_bool()).unwrap_or(false)
                        || c.get("is_pinned")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false)
                        || c.get("pin").and_then(|v| v.as_bool()).unwrap_or(false),
                    archived: false,
                    muted: false,
                    created: None,
                    updated: None,
                });
            }
        }

        // Fetch last messages in parallel for the first 20 chats to avoid rate limits
        let mut fetch_futs = Vec::new();
        for chat in out_chats.iter().take(20) {
            if chat.last_message.is_none() {
                let chat_id = chat.id.clone();
                let self_ref = self; // Since self is a reference (&HttpClient), copying it into async move is fine!
                fetch_futs.push(async move {
                    let msgs = self_ref
                        .get_messages(&chat_id, None, 0, 1)
                        .await
                        .unwrap_or_default();
                    (chat_id, msgs)
                });
            }
        }

        let results = futures::future::join_all(fetch_futs).await;
        for (chat_id, mut msgs) in results {
            if let Some(msg) = msgs.pop() {
                if let Some(chat) = out_chats.iter_mut().find(|c| c.id == chat_id) {
                    chat.last_message = Some(msg);
                }
            }
        }

        log::info!(
            "Parsed {} chats (first chat: {:?}, last chat: {:?})",
            out_chats.len(),
            out_chats.first().map(|c| (c.id.clone(), c.title.clone())),
            out_chats.last().map(|c| (c.id.clone(), c.title.clone()))
        );

        if out_chats.is_empty() {
            Err("No chats found in bootstrap data".to_string())
        } else {
            Ok(out_chats)
        }
    }

    /// Get chat messages via the search API.
    /// The 'messages' RPC method is blocked by Yandex (HTTP 418) for all auth types.
    /// Search with common word queries is the only working retrieval method.
    pub async fn get_messages(
        &self,
        chat_id: &str,
        msg_id: Option<&str>,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<models::Message>, String> {
        self.get_messages_internal(chat_id, msg_id, offset, limit, false)
            .await
    }

    pub async fn get_messages_fresh(
        &self,
        chat_id: &str,
        msg_id: Option<&str>,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<models::Message>, String> {
        self.get_messages_internal(chat_id, msg_id, offset, limit, true)
            .await
    }

    async fn get_messages_internal(
        &self,
        chat_id: &str,
        msg_id: Option<&str>,
        _offset: usize,
        limit: usize,
        skip_cache: bool,
    ) -> Result<Vec<models::Message>, String> {
        let cache_dir = dirs::config_dir()
            .map(|d| d.join("yandex-messenger-native").join("cache"))
            .unwrap_or_else(|| std::path::PathBuf::from("/tmp/yandex-messenger-cache"));

        std::fs::create_dir_all(&cache_dir).ok();
        let cache_file = cache_dir.join(format!("messages_{}.json", chat_id.replace("/", "_")));

        let mut cached_msgs = None;
        if let Ok(data) = std::fs::read_to_string(&cache_file) {
            if let Ok(msgs) = serde_json::from_str::<Vec<models::Message>>(&data) {
                cached_msgs = Some(msgs);
            }
        }

        // Fast path: return disk cache only when not asked for fresh data
        if !skip_cache {
            if let Some(ref msgs) = cached_msgs {
                return Ok(msgs.clone());
            }
        }

        // 1) Session RPC — full chronological history
        if self.has_session() {
            match self.get_messages_via_session(chat_id, msg_id, limit).await {
                Ok(msgs) if !msgs.is_empty() => {
                    if let Ok(json) = serde_json::to_string(&msgs) {
                        std::fs::write(&cache_file, json).ok();
                    }
                    log::info!(
                        "Loaded {} fresh messages via session for {}",
                        msgs.len(),
                        chat_id
                    );
                    return Ok(msgs);
                }
                Ok(_) => {
                    log::warn!("Session messages empty for {}", chat_id);
                }
                Err(e) => {
                    log::warn!("get_messages_via_session failed: {}", e);
                }
            }
        } else {
            log::warn!(
                "No session cookies — history may be incomplete (run scripts/login_browser.py)"
            );
        }

        // 2) Search fallback (OAuth) — always try when fresh was requested
        match self.get_messages_via_search(chat_id).await {
            Ok(msgs) if !msgs.is_empty() => {
                if let Ok(json) = serde_json::to_string(&msgs) {
                    std::fs::write(&cache_file, json).ok();
                }
                log::info!("Loaded {} messages via search for {}", msgs.len(), chat_id);
                return Ok(msgs);
            }
            Ok(_) => log::warn!("Search returned no messages for {}", chat_id),
            Err(e) => log::warn!("get_messages_via_search failed: {}", e),
        }

        // 3) Stale cache as last resort
        if let Some(msgs) = cached_msgs {
            log::info!(
                "Returning cached messages as last-resort fallback for chat {}",
                chat_id
            );
            return Ok(msgs);
        }

        Err(format!("No messages available for chat {}", chat_id))
    }

    /// Fetch messages using the session-based 'messages' RPC method.
    /// This returns full chronological history but requires Passport session cookies.
    async fn get_messages_via_session(
        &self,
        chat_id: &str,
        msg_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<models::Message>, String> {
        let mut params = serde_json::json!({
            "chat_id": chat_id,
            "limit": limit.min(50),
            "direction": "older"
        });
        if let Some(mid) = msg_id {
            params["from_message_id"] = serde_json::json!(mid);
        }

        let data = self.session_rpc_request("messages", params).await?;

        // Parse the messages array from the response
        let mut messages = Vec::new();

        if let Some(items) = data.get("messages").and_then(|v| v.as_array()) {
            for item in items {
                if let Some(msg) = Self::parse_session_message(item, chat_id) {
                    messages.push(msg);
                }
            }
        }

        // Sort by creation time (oldest first)
        messages.sort_by_key(|m| m.created);

        Ok(messages)
    }

    /// Parse a message from the session-based 'messages' API response.
    fn parse_session_message(item: &serde_json::Value, chat_id: &str) -> Option<models::Message> {
        // The 'messages' API may return data in a different format than search.
        // Try both ServerMessageInfo/ClientMessage format and flat format.

        // Format 1: ServerMessageInfo/ClientMessage (same as search)
        if let Some(data) = item.get("data") {
            if data.get("ServerMessageInfo").is_some() {
                return Self::parse_search_message(item, chat_id);
            }
        }

        // Format 2: Direct format with ServerMessageInfo at top level
        if item.get("ServerMessageInfo").is_some() {
            let wrapper = serde_json::json!({"data": item});
            return Self::parse_search_message(&wrapper, chat_id);
        }

        // Format 3: Flat message format
        let from_guid = item
            .get("from")
            .or_else(|| item.get("from_guid"))
            .or_else(|| item.get("sender"))
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();

        let timestamp = item
            .get("timestamp")
            .or_else(|| item.get("ts"))
            .or_else(|| item.get("created_at"))
            .or_else(|| item.get("created"))
            .and_then(|v| v.as_i64())
            .unwrap_or(0);

        // Server may send seconds, milliseconds, or microseconds — never treat ms as µs
        // (that mapped ~2026 → 1970 and sorted new messages to the top of the list).
        let created = normalize_unix_timestamp(timestamp);

        let text = item
            .get("text")
            .or_else(|| item.get("message_text"))
            .or_else(|| item.get("body"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let msg_id = item
            .get("message_id")
            .or_else(|| item.get("id"))
            .or_else(|| item.get("payload_id"))
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();

        let seq_no = item.get("seq_no").and_then(|v| v.as_u64()).unwrap_or(0);
        let reactions = Self::parse_reactions_from_message(item);

        Some(models::Message {
            id: format!("{}_{}", seq_no, msg_id),
            chat_id: chat_id.to_string(),
            from_id: from_guid,
            message_id: Some(msg_id),
            rmid: None,
            type_: models::MessageType::Text,
            text,
            entities: vec![],
            reply_to: None,
            forward: None,
            media: vec![],
            reactions,
            thread_id: None,
            has_thread: false,
            pinned: false,
            edited: false,
            edited_at: None,
            sent: true,
            delivered: true,
            read: true,
            created,
            updated: None,
            poll: None,
        })
    }

    /// Fallback: fetch messages using the search API (OAuth-compatible).
    async fn get_messages_via_search(&self, chat_id: &str) -> Result<Vec<models::Message>, String> {
        // Use a smaller set of more effective search queries to avoid rate limits and unnecessary noise
        let queries = [
            "привет",
            "как дела",
            "до свидания",
            "спасибо",
            "хорошо",
            "ок",
            "да",
            "нет",
            "встреча",
            "завтра",
        ];

        let mut search_futures = Vec::new();
        for &query in &queries {
            let params = serde_json::json!({
                "entities": ["messages"],
                "chat_id": chat_id,
                "query": query
            });
            search_futures.push(async move { (query, self.rpc_request("search", params).await) });
        }

        let results = futures::future::join_all(search_futures).await;

        let mut all_messages: Vec<models::Message> = Vec::new();
        let mut seen_ids: std::collections::HashSet<String> = std::collections::HashSet::new();

        for (query, res) in results {
            match res {
                Ok(data) => {
                    if let Some(items) = data
                        .get("messages")
                        .and_then(|m| m.get("items"))
                        .and_then(|i| i.as_array())
                    {
                        for item in items {
                            if let Some(msg) = Self::parse_search_message(item, chat_id) {
                                if seen_ids.insert(msg.id.clone()) {
                                    all_messages.push(msg);
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    log::warn!("Search query '{}' failed: {}", query, e);
                }
            }
        }

        all_messages.sort_by_key(|m| m.created);

        if all_messages.is_empty() {
            log::info!("No messages found via search for chat {}", chat_id);
        } else {
            log::info!(
                "Found {} messages via search for chat {}",
                all_messages.len(),
                chat_id
            );
        }

        Ok(all_messages)
    }

    fn parse_single_reaction(value: &serde_json::Value) -> Option<models::Reaction> {
        let emoji = value
            .get("emoji")
            .or_else(|| value.get("Emoji"))
            .and_then(|v| v.as_str())?;

        let count = value
            .get("count")
            .or_else(|| value.get("Count"))
            .and_then(|v| v.as_u64())
            .unwrap_or(1) as u32;

        let selected = value
            .get("selected")
            .or_else(|| value.get("Selected"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let user_ids = value
            .get("user_ids")
            .or_else(|| value.get("userIds"))
            .or_else(|| value.get("UserGuids"))
            .or_else(|| value.get("user_guids"))
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|id| id.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();

        let is_extended = value
            .get("is_extended")
            .or_else(|| value.get("isExtended"))
            .or_else(|| value.get("IsExtended"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        Some(models::Reaction {
            emoji: emoji.to_string(),
            count,
            selected,
            user_ids,
            is_extended,
        })
    }

    fn parse_reactions_array(value: &serde_json::Value) -> Vec<models::Reaction> {
        value
            .as_array()
            .map(|arr| arr.iter().filter_map(Self::parse_single_reaction).collect())
            .unwrap_or_default()
    }

    fn parse_reactions_from_message(data: &serde_json::Value) -> Vec<models::Reaction> {
        let candidates = [
            data.get("reactions"),
            data.get("Reactions"),
            data.get("ServerMessageInfo")
                .and_then(|info| info.get("reactions")),
            data.get("ServerMessageInfo")
                .and_then(|info| info.get("Reactions")),
            data.get("ClientMessage")
                .and_then(|msg| msg.get("reactions")),
            data.get("ClientMessage")
                .and_then(|msg| msg.get("Reactions")),
        ];

        for candidate in candidates.into_iter().flatten() {
            let reactions = Self::parse_reactions_array(candidate);
            if !reactions.is_empty() {
                return reactions;
            }
        }

        Vec::new()
    }

    pub fn parse_reaction_update_payload(
        payload: &serde_json::Value,
    ) -> Option<(String, Vec<models::Reaction>)> {
        let message_id = payload
            .get("message_id")
            .or_else(|| payload.get("messageId"))
            .and_then(|v| v.as_str())?;

        let reactions = payload
            .get("reactions")
            .or_else(|| payload.get("Reactions"))
            .map(Self::parse_reactions_array)
            .filter(|reactions| !reactions.is_empty())
            .or_else(|| {
                let reactions = Self::parse_reactions_from_message(payload);
                if reactions.is_empty() {
                    None
                } else {
                    Some(reactions)
                }
            })?;

        Some((message_id.to_string(), reactions))
    }

    pub fn parse_ws_incoming_messages(value: &serde_json::Value) -> Vec<models::Message> {
        let messages_val = match value.get("messages").and_then(|v| v.as_array()) {
            Some(messages) => messages,
            None => return Vec::new(),
        };

        let mut parsed = Vec::new();
        for item in messages_val {
            if let Ok(msg) = serde_json::from_value::<models::Message>(item.clone()) {
                if !msg.id.is_empty() && !msg.chat_id.is_empty() {
                    parsed.push(msg);
                    continue;
                }
            }

            let chat_id = item
                .get("chat_id")
                .or_else(|| item.get("chatId"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if chat_id.is_empty() {
                continue;
            }

            let text = item
                .get("text")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let from_id = item
                .get("from_id")
                .or_else(|| item.get("fromId"))
                .and_then(|v| v.as_str())
                .unwrap_or("system")
                .to_string();
            let message_id = item
                .get("message_id")
                .or_else(|| item.get("messageId"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let id = item
                .get("id")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| {
                    message_id
                        .clone()
                        .unwrap_or_else(|| uuid::Uuid::new_v4().simple().to_string())
                });
            let created = item
                .get("created")
                .and_then(|v| {
                    if let Some(n) = v.as_i64() {
                        Some(normalize_unix_timestamp(n))
                    } else if let Some(n) = v.as_u64() {
                        Some(normalize_unix_timestamp(n as i64))
                    } else if let Some(text) = v.as_str() {
                        chrono::DateTime::parse_from_rfc3339(text)
                            .ok()
                            .map(|dt| dt.with_timezone(&chrono::Utc))
                    } else {
                        None
                    }
                })
                .unwrap_or_else(chrono::Utc::now);
            let reactions = Self::parse_reactions_from_message(item);

            let (delivered, read) = Self::parse_delivery_flags(item);
            parsed.push(models::Message {
                id,
                chat_id: chat_id.to_string(),
                from_id,
                message_id,
                rmid: None,
                type_: models::MessageType::Text,
                text,
                entities: vec![],
                reply_to: None,
                forward: None,
                media: vec![],
                reactions,
                thread_id: None,
                has_thread: false,
                pinned: false,
                edited: false,
                edited_at: None,
                sent: true,
                delivered,
                read,
                created,
                updated: None,
                poll: None,
            });
        }

        parsed
    }

    /// Parse delivery/read flags from various server payload shapes.
    /// Returns `(delivered, read)`.
    pub fn parse_delivery_flags(item: &serde_json::Value) -> (bool, bool) {
        // Nested ServerMessageInfo
        let sources = [
            Some(item),
            item.get("ServerMessageInfo"),
            item.get("data").and_then(|d| d.get("ServerMessageInfo")),
            item.get("message"),
            item.get("Message"),
        ];

        for src in sources.into_iter().flatten() {
            if src.get("read").and_then(|v| v.as_bool()) == Some(true)
                || src.get("is_read").and_then(|v| v.as_bool()) == Some(true)
                || src.get("IsRead").and_then(|v| v.as_bool()) == Some(true)
            {
                return (true, true);
            }
            if let Some(s) = src
                .get("status")
                .or_else(|| src.get("Status"))
                .or_else(|| src.get("delivery_status"))
                .and_then(|v| v.as_str())
            {
                let s = s.to_ascii_lowercase();
                if s == "read" || s == "seen" || s == "viewed" {
                    return (true, true);
                }
                if s == "delivered" || s == "sent" || s == "received" {
                    return (true, false);
                }
                if s == "pending" || s == "sending" || s == "queued" {
                    return (false, false);
                }
            }
            if src.get("delivered").and_then(|v| v.as_bool()) == Some(true)
                || src.get("is_delivered").and_then(|v| v.as_bool()) == Some(true)
            {
                return (true, false);
            }
            // views > 0 often means read in channels
            if let Some(views) = src.get("views").or_else(|| src.get("Views")).and_then(|v| v.as_u64()) {
                if views > 0 {
                    return (true, true);
                }
            }
        }
        // History from server: assume at least delivered
        (true, false)
    }

    /// Parse a status-update WS payload → (message_id, delivered, read) or chat-wide read.
    pub fn parse_status_update_payload(
        payload: &serde_json::Value,
    ) -> Option<StatusUpdate> {
        let msg_id = payload
            .get("message_id")
            .or_else(|| payload.get("messageId"))
            .or_else(|| payload.get("msg_id"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let chat_id = payload
            .get("chat_id")
            .or_else(|| payload.get("chatId"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let (delivered, read) = Self::parse_delivery_flags(payload);

        // Explicit method-style flags
        let read = read
            || payload.get("event").and_then(|e| e.as_str()) == Some("read")
            || payload.get("type").and_then(|e| e.as_str()) == Some("read");
        let delivered = delivered || read;

        if msg_id.is_some() || chat_id.is_some() {
            Some(StatusUpdate {
                message_id: msg_id,
                chat_id,
                delivered,
                read,
            })
        } else {
            None
        }
    }

    /// Parse a single search result item into our Message model.
    fn parse_search_message(item: &serde_json::Value, chat_id: &str) -> Option<models::Message> {
        let data = item.get("data")?;
        let server_info = data.get("ServerMessageInfo")?;
        let client_msg = data.get("ClientMessage")?;
        let plain = client_msg.get("Plain")?;

        let from_guid = server_info
            .get("From")
            .and_then(|f| f.get("Guid"))
            .and_then(|g| g.as_str())
            .unwrap_or("unknown")
            .to_string();

        let timestamp = server_info
            .get("Timestamp")
            .and_then(|t| t.as_i64())
            .unwrap_or(0);
        // Prefer microsecond ServerMessageInfo.Timestamp; fall back to ms/s heuristics
        let created = if timestamp > 1_000_000_000_000_000 {
            let secs = timestamp / 1_000_000;
            let nanos = ((timestamp % 1_000_000) * 1000) as u32;
            chrono::DateTime::from_timestamp(secs, nanos).unwrap_or_else(chrono::Utc::now)
        } else {
            normalize_unix_timestamp(timestamp)
        };

        let text = plain
            .get("Text")
            .and_then(|t| t.get("MessageText"))
            .and_then(|t| t.as_str())
            .map(|s| s.to_string());

        let payload_id = plain
            .get("PayloadId")
            .and_then(|p| p.as_str())
            .unwrap_or("unknown")
            .to_string();

        let seq_no = server_info
            .get("SeqNo")
            .and_then(|s| s.as_u64())
            .unwrap_or(0);

        let reactions = Self::parse_reactions_from_message(data);

        let (delivered, read) = Self::parse_delivery_flags(item);
        Some(models::Message {
            id: format!("{}_{}", seq_no, payload_id),
            chat_id: chat_id.to_string(),
            from_id: from_guid,
            message_id: Some(payload_id),
            rmid: None,
            type_: models::MessageType::Text,
            text,
            entities: vec![],
            reply_to: None,
            forward: None,
            media: vec![],
            reactions,
            thread_id: None,
            has_thread: false,
            pinned: false,
            edited: server_info
                .get("Version")
                .and_then(|v| v.as_u64())
                .unwrap_or(1)
                > 1,
            edited_at: None,
            sent: true,
            delivered,
            read,
            created,
            updated: None,
            poll: None,
        })
    }

    /// Send text message
    pub async fn send_message(
        &self,
        chat_id: &str,
        text: &str,
        reply_to: Option<&str>,
    ) -> Result<models::Message, String> {
        let payload_id = uuid::Uuid::new_v4().simple().to_string();
        let mut params = serde_json::json!({
            "chat_id": chat_id,
            "text": text,
            "payload_id": payload_id
        });

        if let Some(rtid) = reply_to {
            params["reply_to"] = serde_json::json!(rtid);
        }

        // Must use session_rpc_request because send_message is blocked for pure OAuth
        let data = self.session_rpc_request("send_message", params).await?;

        if let Some(message_val) = data.get("message") {
            let message: models::Message = serde_json::from_value(message_val.clone())
                .map_err(|e| format!("Failed to parse message: {}", e))?;
            return Ok(message);
        }

        Err("Send message response missing 'message' object".to_string())
    }

    /// Edit an existing message (session RPC).
    pub async fn edit_message(
        &self,
        chat_id: &str,
        message_id: &str,
        text: &str,
    ) -> Result<(), String> {
        let params = serde_json::json!({
            "chat_id": chat_id,
            "message_id": message_id,
            "text": text,
        });
        match self.session_rpc_request("edit_message", params.clone()).await {
            Ok(_) => Ok(()),
            Err(e1) => {
                // Alternate param casing used by some clients
                let alt = serde_json::json!({
                    "chatId": chat_id,
                    "messageId": message_id,
                    "text": text,
                });
                self.session_rpc_request("edit_message", alt)
                    .await
                    .map(|_| ())
                    .map_err(|e2| format!("edit_message failed: {}; alt: {}", e1, e2))
            }
        }
    }

    /// Delete a message (session RPC).
    pub async fn delete_message(&self, chat_id: &str, message_id: &str) -> Result<(), String> {
        let params = serde_json::json!({
            "chat_id": chat_id,
            "message_id": message_id,
        });
        self.session_rpc_request("delete_message", params)
            .await
            .map(|_| ())
            .map_err(|e| format!("delete_message failed: {}", e))
    }

    /// Mark chat as read (session RPC method `read`).
    pub async fn mark_chat_read(
        &self,
        chat_id: &str,
        last_message_id: Option<&str>,
    ) -> Result<(), String> {
        let mut params = serde_json::json!({ "chat_id": chat_id });
        if let Some(mid) = last_message_id {
            params["message_id"] = serde_json::json!(mid);
        }
        match self.session_rpc_request("read", params.clone()).await {
            Ok(_) => Ok(()),
            Err(e1) => {
                let alt = serde_json::json!({
                    "chatId": chat_id,
                    "messageId": last_message_id,
                });
                self.session_rpc_request("read", alt)
                    .await
                    .map(|_| ())
                    .map_err(|e2| format!("read failed: {}; alt: {}", e1, e2))
            }
        }
    }

    /// Mute / unmute chat (best-effort session RPC).
    pub async fn set_chat_muted(&self, chat_id: &str, muted: bool) -> Result<(), String> {
        let attempts = [
            (
                "set_chat_settings",
                serde_json::json!({"chat_id": chat_id, "muted": muted}),
            ),
            (
                "set_chat_settings",
                serde_json::json!({"chatId": chat_id, "muted": muted}),
            ),
            (
                "update_chat",
                serde_json::json!({"chat_id": chat_id, "muted": muted}),
            ),
            (
                "mute_chat",
                serde_json::json!({"chat_id": chat_id, "mute": muted}),
            ),
        ];
        let mut last_err = String::new();
        for (method, params) in attempts {
            match self.session_rpc_request(method, params).await {
                Ok(_) => return Ok(()),
                Err(e) => last_err = format!("{}: {}", method, e),
            }
        }
        Err(format!("set_chat_muted failed: {}", last_err))
    }

    /// Pin / unpin chat in the list.
    pub async fn set_chat_pinned(&self, chat_id: &str, pinned: bool) -> Result<(), String> {
        let attempts = [
            (
                "pin_chat",
                serde_json::json!({"chat_id": chat_id, "pin": pinned}),
            ),
            (
                "set_chat_settings",
                serde_json::json!({"chat_id": chat_id, "pinned": pinned}),
            ),
            (
                "update_chat",
                serde_json::json!({"chat_id": chat_id, "pinned": pinned}),
            ),
        ];
        let mut last_err = String::new();
        for (method, params) in attempts {
            match self.session_rpc_request(method, params).await {
                Ok(_) => return Ok(()),
                Err(e) => last_err = format!("{}: {}", method, e),
            }
        }
        Err(format!("set_chat_pinned failed: {}", last_err))
    }

    /// Archive / unarchive chat.
    pub async fn set_chat_archived(&self, chat_id: &str, archived: bool) -> Result<(), String> {
        let attempts = [
            (
                "archive_chat",
                serde_json::json!({"chat_id": chat_id, "archive": archived}),
            ),
            (
                "set_chat_settings",
                serde_json::json!({"chat_id": chat_id, "archived": archived}),
            ),
            (
                "hide_chat",
                serde_json::json!({"chat_id": chat_id, "hide": archived}),
            ),
        ];
        let mut last_err = String::new();
        for (method, params) in attempts {
            match self.session_rpc_request(method, params).await {
                Ok(_) => return Ok(()),
                Err(e) => last_err = format!("{}: {}", method, e),
            }
        }
        Err(format!("set_chat_archived failed: {}", last_err))
    }

    /// Leave / delete chat from list.
    pub async fn delete_chat(&self, chat_id: &str) -> Result<(), String> {
        let attempts = [
            ("leave", serde_json::json!({"chat_id": chat_id})),
            ("delete_chat", serde_json::json!({"chat_id": chat_id})),
            ("hide_chat", serde_json::json!({"chat_id": chat_id, "hide": true})),
        ];
        let mut last_err = String::new();
        for (method, params) in attempts {
            match self.session_rpc_request(method, params).await {
                Ok(_) => return Ok(()),
                Err(e) => last_err = format!("{}: {}", method, e),
            }
        }
        Err(format!("delete_chat failed: {}", last_err))
    }

    /// Guess MIME from filename extension (used by upload + send).
    pub fn guess_mime_type(filename: &str) -> &'static str {
        let ext = filename
            .split('.')
            .last()
            .map(|s| s.to_ascii_lowercase())
            .unwrap_or_default();
        match ext.as_str() {
            "jpg" | "jpeg" => "image/jpeg",
            "png" => "image/png",
            "gif" => "image/gif",
            "webp" => "image/webp",
            "mp4" => "video/mp4",
            "webm" => "video/webm",
            "mp3" => "audio/mpeg",
            "ogg" => "audio/ogg",
            "pdf" => "application/pdf",
            "zip" => "application/zip",
            _ => "application/octet-stream",
        }
    }

    /// True if this MIME should be sent as `Image` / gallery rather than `MiscFile`.
    pub fn is_image_mime(mime: &str) -> bool {
        mime.starts_with("image/")
    }

    /// Upload file to Yandex Files and return `file_id` (e.g. `file/<uuid>`).
    ///
    /// Official web client:
    /// `POST https://files.messenger.yandex.ru/media_upload/{chatId}/{messageId}/{filename}`
    /// with raw body + `Authorization: OAuth …` + `Content-Type: <mime>`.
    /// PUT is rejected with 405 Method Not Allowed.
    pub async fn upload_file(
        &self,
        chat_id: &str,
        file_data: &[u8],
        filename: &str,
    ) -> Result<String, String> {
        let message_id = uuid::Uuid::new_v4().simple().to_string();
        self.upload_file_for_message(chat_id, file_data, filename, &message_id)
            .await
    }

    /// Same as [`upload_file`], but uses a pre-generated `message_id` (must match
    /// the PayloadId of the subsequent ClientMessage).
    pub async fn upload_file_for_message(
        &self,
        chat_id: &str,
        file_data: &[u8],
        filename: &str,
        message_id: &str,
    ) -> Result<String, String> {
        if file_data.is_empty() {
            return Err("Empty file".to_string());
        }
        if file_data.len() as u64 > config::MAX_FILE_SIZE {
            return Err(format!(
                "File too large ({} > {} bytes)",
                file_data.len(),
                config::MAX_FILE_SIZE
            ));
        }

        let auth_header = self.get_token_header();
        if auth_header.is_empty() {
            return Err(
                "Нет OAuth токена — перелогиньтесь (файлы требуют Authorization: OAuth)."
                    .to_string(),
            );
        }

        // encodeURI(filename) as web client: keep path-safe basename only
        let safe_name = filename
            .rsplit(['/', '\\'])
            .next()
            .unwrap_or(filename)
            .trim();
        let safe_name = if safe_name.is_empty() {
            "file.bin"
        } else {
            safe_name
        };
        // encodeURIComponent-style for the filename segment only; chatId keeps `/`
        let filename_enc = urlencoding::encode(safe_name);

        let mime = Self::guess_mime_type(safe_name);
        // Prefer private host (web template: filePrivateHost), then public.
        let hosts = [
            config::FILE_PRIVATE_HOST,
            config::FILE_PUBLIC_HOST,
        ];

        let mut last_err = String::new();
        for host in hosts {
            // Do NOT fully-encode chatId: group ids are `0/0/<uuid>` and must stay path segments.
            let upload_url = format!(
                "{}/media_upload/{}/{}/{}",
                host.trim_end_matches('/'),
                chat_id,
                message_id,
                filename_enc
            );

            log::info!(
                "Uploading {} bytes to {} (mime={})",
                file_data.len(),
                upload_url,
                mime
            );

            // OAuth only — Cookie without CSRF on .net often yields 401.
            let response = self
                .client
                .post(&upload_url)
                .header("Authorization", &auth_header)
                .header("Content-Type", mime)
                .header("Origin", "https://yandex.ru")
                .header("Referer", "https://yandex.ru/chat")
                .body(file_data.to_vec())
                .send()
                .await
                .map_err(|e| format!("Upload request failed: {}", e))?;

            let status = response.status();
            let text = response
                .text()
                .await
                .map_err(|e| format!("Upload read failed: {}", e))?;

            if !status.is_success() {
                last_err = format!("HTTP {} from {}: {}", status, host, &text[..text.len().min(200)]);
                log::warn!("media_upload failed: {}", last_err);
                continue;
            }

            let json: Value = serde_json::from_str(&text).map_err(|e| {
                format!(
                    "Upload JSON parse failed: {} (body: {})",
                    e,
                    &text[..text.len().min(200)]
                )
            })?;

            // Web client expects data.file_id (sometimes file_ids[0] for other endpoints)
            let file_id = json
                .pointer("/data/file_id")
                .and_then(|v| v.as_str())
                .or_else(|| json.get("file_id").and_then(|v| v.as_str()))
                .or_else(|| {
                    json.pointer("/data/file_ids/0")
                        .and_then(|v| v.as_str())
                })
                .or_else(|| json.get("fileId").and_then(|v| v.as_str()))
                .map(|s| s.to_string());

            if let Some(id) = file_id {
                log::info!("Upload OK file_id={}", id);
                return Ok(id);
            }

            last_err = format!("No file_id in upload response: {}", &text[..text.len().min(200)]);
        }

        Err(format!("File upload failed: {}", last_err))
    }

    /// Upload only (sending goes via push WS ClientMessage — registry has no send_message path).
    /// Kept for callers that still expect a Message; prefer `AppController::send_file_message`.
    pub async fn send_message_with_file(
        &self,
        chat_id: &str,
        file_data: &[u8],
        filename: &str,
    ) -> Result<models::Message, String> {
        let payload_id = uuid::Uuid::new_v4().simple().to_string();
        let file_id = self
            .upload_file_for_message(chat_id, file_data, filename, &payload_id)
            .await?;
        let mime = Self::guess_mime_type(filename);
        // Without WS we cannot deliver the attachment; return optimistic local message.
        Ok(models::Message {
            id: payload_id.clone(),
            chat_id: chat_id.to_string(),
            from_id: String::new(),
            message_id: Some(payload_id),
            rmid: None,
            type_: if Self::is_image_mime(mime) {
                models::MessageType::Text
            } else {
                models::MessageType::Text
            },
            text: Some(format!("[Файл: {}]", filename)),
            entities: vec![],
            reply_to: None,
            forward: None,
            media: vec![models::MediaAttachment {
                id: file_id.clone(),
                type_: if Self::is_image_mime(mime) {
                    models::MediaType::Image
                } else {
                    models::MediaType::Document
                },
                url: format!("{}/{}", config::FILE_PUBLIC_HOST, file_id),
                thumbnail_url: None,
                width: None,
                height: None,
                size: Some(file_data.len() as u64),
                duration: None,
                filename: Some(filename.to_string()),
                mime_type: Some(mime.to_string()),
                waveform: None,
            }],
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
        })
    }

    /// Download file by Yandex Files id.
    pub async fn download_file(&self, file_id: &str) -> Result<Vec<u8>, String> {
        let candidates = [
            format!("{}/file_shortterm/{}", config::FILE_PUBLIC_HOST, file_id),
            format!("{}/file_shortterm/{}", config::FILE_PRIVATE_HOST, file_id),
            format!("{}/{}", config::FILE_PUBLIC_HOST, file_id),
        ];
        let mut last_err = String::new();
        for url in &candidates {
            match self.download_url(url).await {
                Ok(bytes) if !bytes.is_empty() => return Ok(bytes),
                Ok(_) => last_err = format!("empty body from {}", url),
                Err(e) => last_err = e,
            }
        }
        Err(format!("Download failed for {}: {}", file_id, last_err))
    }

    /// Download arbitrary HTTPS URL (with OAuth if set).
    pub async fn download_url(&self, url: &str) -> Result<Vec<u8>, String> {
        let auth_header = self.get_token_header();
        let mut req = self.client.get(url);
        if !auth_header.is_empty() {
            req = req.header("Authorization", &auth_header);
        }
        // Session cookies help for some CDN paths
        if let Some(cookies) = self.session_cookies.lock().unwrap().clone() {
            req = req.header("Cookie", cookies);
        }
        let response = req
            .send()
            .await
            .map_err(|e| format!("Download failed: {}", e))?;
        if !response.status().is_success() {
            return Err(format!("Download HTTP {}", response.status()));
        }
        response
            .bytes()
            .await
            .map(|b| b.to_vec())
            .map_err(|e| format!("Download read failed: {}", e))
    }

    /// Get avatar URL
    pub fn avatar_url(&self, avatar_id: &str, size: u32) -> String {
        let normalized_id = avatar_id.replace("user_avatar/", "");
        let size_str = if size <= 100 {
            "islands-middle"
        } else {
            "islands-200"
        };
        format!(
            "https://avatars.mds.yandex.net/get-{}/{}",
            normalized_id, size_str
        )
    }

    /// Get Telemost URL for a call
    pub fn telemost_url(&self, chat_id: &str, call_id: Option<&str>) -> String {
        let mut url = format!("{}/?chatId={}", config::TELEMOST_URL, chat_id);
        if let Some(cid) = call_id {
            url = format!("{}&callId={}", url, cid);
        }
        url
    }

    // ============================================================
    // Telemost call API methods
    // ============================================================

    /// Start a Telemost call in a chat
    pub async fn start_call(&self, chat_id: &str) -> Result<models::TelemostCall, String> {
        let data = self
            .rpc_request(
                "start_call",
                serde_json::json!({
                    "chatId": chat_id
                }),
            )
            .await?;

        if let Some(call_val) = data.get("call") {
            let call: models::TelemostCall = serde_json::from_value(call_val.clone())
                .map_err(|e| format!("Failed to parse call: {}", e))?;
            return Ok(call);
        }

        if let Ok(call) = serde_json::from_value::<models::TelemostCall>(data.clone()) {
            return Ok(call);
        }

        Err("Start call response missing 'call' object".to_string())
    }

    /// End an active Telemost call
    pub async fn end_call(&self, call_id: &str) -> Result<(), String> {
        let _ = self
            .rpc_request(
                "end_call",
                serde_json::json!({
                    "callId": call_id
                }),
            )
            .await?;
        Ok(())
    }

    /// Get current status of a Telemost call
    pub async fn get_call_status(&self, call_id: &str) -> Result<models::CallStatus, String> {
        let data = self
            .rpc_request(
                "get_call_status",
                serde_json::json!({
                    "callId": call_id
                }),
            )
            .await?;

        if let Some(status_str) = data.get("status").and_then(|s| s.as_str()) {
            let status = match status_str {
                "ringing" => models::CallStatus::Ringing,
                "in_progress" => models::CallStatus::InProgress,
                "missed" => models::CallStatus::Missed,
                "ended" => models::CallStatus::Ended,
                "declined" => models::CallStatus::Declined,
                _ => models::CallStatus::Ended,
            };
            return Ok(status);
        }

        Err("Get call status response missing 'status' field".to_string())
    }

    // ============================================================
    // Voice message methods
    // ============================================================

    /// Загрузить голосовое сообщение
    pub async fn upload_voice_message(
        &self,
        chat_id: &str,
        audio_data: &[u8],
        duration: f64,
        waveform: Vec<f32>,
    ) -> Result<crate::models::VoiceMessage, String> {
        let url = format!("{}api/upload_voice", self.base_url);
        let auth_header = self.get_token_header();

        let response = self
            .client
            .post(&url)
            .header("Authorization", &auth_header)
            .header("Content-Type", "application/octet-stream")
            .body(audio_data.to_vec())
            .query(&[
                ("chatId", chat_id),
                ("duration", &duration.to_string()),
                (
                    "waveform",
                    &serde_json::to_string(&waveform)
                        .map_err(|e| format!("Waveform serialize failed: {}", e))?,
                ),
            ])
            .send()
            .await
            .map_err(|e| format!("Upload voice failed: {}", e))?;

        if !response.status().is_success() {
            return Err(format!(
                "Upload voice failed with status: {}",
                response.status()
            ));
        }

        let json: Value = response
            .json()
            .await
            .map_err(|e| format!("Upload voice parse failed: {}", e))?;

        // Parse response - handle both direct VoiceMessage and wrapped formats
        if let Ok(voice) = serde_json::from_value::<crate::models::VoiceMessage>(json.clone()) {
            return Ok(voice);
        }

        // Try extracting from a wrapper object
        if let Some(msg) = json.get("message") {
            if let Ok(voice) = serde_json::from_value::<crate::models::VoiceMessage>(msg.clone()) {
                return Ok(voice);
            }
        }

        Err("Upload voice response has unsupported format".to_string())
    }

    /// Получить транскрипцию голосового сообщения
    pub async fn get_transcription(&self, message_id: &str) -> Result<Option<String>, String> {
        let url = format!(
            "{}api/get_transcription?messageId={}",
            self.base_url, message_id
        );
        let auth_header = self.get_token_header();

        let response = self
            .client
            .get(&url)
            .header("Authorization", &auth_header)
            .send()
            .await
            .map_err(|e| format!("Get transcription failed: {}", e))?;

        if response.status() == 404 {
            return Ok(None);
        }

        if !response.status().is_success() {
            return Err(format!(
                "Get transcription failed with status: {}",
                response.status()
            ));
        }

        let json: Value = response
            .json()
            .await
            .map_err(|e| format!("Transcription parse failed: {}", e))?;

        // Parse response - handle direct text or wrapped format
        if let Some(text) = json.get("text").and_then(|t| t.as_str()) {
            if !text.is_empty() {
                return Ok(Some(text.to_string()));
            }
        }

        // Try extracting from a wrapper
        if let Some(msg) = json.get("message") {
            if let Some(text) = msg.get("text").and_then(|t| t.as_str()) {
                if !text.is_empty() {
                    return Ok(Some(text.to_string()));
                }
            }
        }

        Ok(None)
    }

    // ============================================================
    // Voice transcription (Yandex SpeechKit)
    // ============================================================

    /// Транскрипция голосового сообщения через Yandex SpeechKit
    pub async fn transcribe_voice(
        &self,
        audio_data: &[u8],
        _audio_format: &str,
    ) -> Result<String, String> {
        let token = self.get_token_raw();
        if token.is_empty() {
            return Err("No authentication token".to_string());
        }
        let url = format!("{}/api/v1/stt", crate::config::SPEECHKIT_API_URL);

        let response = self
            .client
            .post(&url)
            .header("Authorization", &format!("OAuth {}", token))
            .header("Content-Type", "audio/webm;codecs=opus")
            .body(audio_data.to_vec())
            .send()
            .await
            .map_err(|e| format!("SpeechKit transcription failed: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(format!("SpeechKit error (HTTP {}): {}", status, body));
        }

        let json: Value = response
            .json()
            .await
            .map_err(|e| format!("SpeechKit parse failed: {}", e))?;

        // SpeechKit returns { result: "..." } with the transcribed text
        if let Some(text) = json.get("result").and_then(|r| r.as_str()) {
            if !text.is_empty() {
                return Ok(text.to_string());
            }
        }

        Err("SpeechKit response has no 'result' field".to_string())
    }

    /// Start transcription for an existing voice message (server-side via Yandex SpeechKit)
    pub async fn start_voice_transcription(
        &self,
        message_id: &str,
        voice_url: &str,
    ) -> Result<String, String> {
        let token = self.get_token_raw();
        if token.is_empty() {
            return Err("No authentication token".to_string());
        }

        let body = serde_json::json!({
            "messageId": message_id,
            "audioUrl": voice_url,
            "lang": crate::config::SPEECHKIT_LANG,
            "format": crate::config::SPEECHKIT_ENCODING,
        });

        let url = format!("{}api/start_voice_transcription", self.base_url);
        let response = self
            .client
            .post(&url)
            .header("Authorization", &format!("OAuth {}", token))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Start transcription failed: {}", e))?;

        if !response.status().is_success() {
            return Err(format!(
                "Start transcription failed with status: {}",
                response.status()
            ));
        }

        let json: Value = response
            .json()
            .await
            .map_err(|e| format!("Transcription start parse failed: {}", e))?;

        // Return task ID or the transcribed text
        if let Some(task_id) = json.get("taskId").and_then(|t| t.as_str()) {
            return Ok(task_id.to_string());
        }

        if let Some(text) = json.get("text").and_then(|t| t.as_str()) {
            if !text.is_empty() {
                return Ok(text.to_string());
            }
        }

        Err("Start transcription response has no taskId or text".to_string())
    }

    /// Check transcription status for a voice message
    pub async fn get_transcription_status(
        &self,
        task_id: &str,
    ) -> Result<crate::models::voice_message::TranscribeStatus, String> {
        let token = self.get_token_raw();
        if token.is_empty() {
            return Err("No authentication token".to_string());
        }

        let url = format!(
            "{}api/get_transcription_status?taskId={}",
            self.base_url, task_id
        );

        let response = self
            .client
            .get(&url)
            .header("Authorization", &format!("OAuth {}", token))
            .send()
            .await
            .map_err(|e| format!("Get transcription status failed: {}", e))?;

        if response.status() == 404 {
            return Ok(crate::models::voice_message::TranscribeStatus::Error(
                "Not found".to_string(),
            ));
        }

        let json: Value = response
            .json()
            .await
            .map_err(|e| format!("Transcription status parse failed: {}", e))?;

        let status = json
            .get("status")
            .and_then(|s| s.as_str())
            .unwrap_or("pending");

        match status {
            "ready" => {
                if let Some(_text) = json.get("result").and_then(|t| t.as_str()) {
                    Ok(crate::models::voice_message::TranscribeStatus::Completed)
                } else {
                    Ok(crate::models::voice_message::TranscribeStatus::Completed)
                }
            }
            "failed" => {
                let err_msg = json
                    .get("error")
                    .and_then(|e| e.as_str())
                    .map(|e| e.to_string())
                    .unwrap_or_else(|| "Unknown error".to_string());
                Ok(crate::models::voice_message::TranscribeStatus::Error(
                    err_msg,
                ))
            }
            _ => Ok(crate::models::voice_message::TranscribeStatus::InProgress),
        }
    }
}

// ============================================================
// Thread API methods
// ============================================================

impl HttpClient {
    /// Get messages for a thread (private, requires token)
    async fn get_thread_messages(
        &self,
        thread_id: &str,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<models::Message>, String> {
        let auth_header = self.get_token_header();
        if auth_header.is_empty() {
            return Err("No authentication token".to_string());
        }
        let url = format!(
            "{}api/get_thread_messages?threadId={}&offset={}&limit={}",
            self.base_url, thread_id, offset, limit
        );
        let response = self
            .client
            .get(&url)
            .header("Authorization", &auth_header)
            .send()
            .await
            .map_err(|e| format!("Get thread messages failed: {}", e))?;

        let json: Value = response
            .json()
            .await
            .map_err(|e| format!("Thread messages parse failed: {}", e))?;

        if let Ok(messages) = serde_json::from_value::<Vec<models::Message>>(json.clone()) {
            return Ok(messages);
        }
        if let Ok(wrapper) = serde_json::from_value::<ListResponse<models::Message>>(json.clone()) {
            if let Some(messages) = wrapper.items.or(wrapper.messages) {
                return Ok(messages);
            }
        }
        Err("Thread messages response has unsupported format".to_string())
    }

    /// Send a message within a thread (private, requires token)
    async fn send_thread_message(
        &self,
        thread_id: &str,
        chat_id: &str,
        text: &str,
    ) -> Result<models::Message, String> {
        let auth_header = self.get_token_header();
        if auth_header.is_empty() {
            return Err("No authentication token".to_string());
        }
        let url = format!("{}api/send_thread_message", self.base_url);
        let body = serde_json::json!({
            "threadId": thread_id,
            "chatId": chat_id,
            "text": text
        });

        let response = self
            .client
            .post(&url)
            .header("Authorization", &auth_header)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Send thread message failed: {}", e))?;

        let json: Value = response
            .json()
            .await
            .map_err(|e| format!("Send thread message parse failed: {}", e))?;

        if let Ok(message) = serde_json::from_value::<models::Message>(json.clone()) {
            return Ok(message);
        }
        if let Ok(mut list) = serde_json::from_value::<Vec<models::Message>>(json.clone()) {
            if let Some(first) = list.pop() {
                return Ok(first);
            }
        }
        Err("Send thread message response has unsupported format".to_string())
    }

    /// Get thread summary (private, requires token)
    async fn get_thread_summary(&self, thread_id: &str) -> Result<models::Thread, String> {
        let auth_header = self.get_token_header();
        if auth_header.is_empty() {
            return Err("No authentication token".to_string());
        }
        let url = format!("{}api/thread_summary?threadId={}", self.base_url, thread_id);
        let response = self
            .client
            .get(&url)
            .header("Authorization", &auth_header)
            .send()
            .await
            .map_err(|e| format!("Get thread summary failed: {}", e))?;

        let json: Value = response
            .json()
            .await
            .map_err(|e| format!("Thread summary parse failed: {}", e))?;

        serde_json::from_value(json)
            .map_err(|e| format!("Thread summary deserialization failed: {}", e))
    }

    // ============================================================
    // Reaction API methods
    // ============================================================

    /// Get reactions configuration
    pub async fn get_reactions_config_public(
        &self,
    ) -> Result<models::ExtendedReactionsConfig, String> {
        self.get_reactions_config().await
    }

    /// Get reactions configuration (private, requires token)
    async fn get_reactions_config(&self) -> Result<models::ExtendedReactionsConfig, String> {
        let auth_header = self.get_token_header();
        if auth_header.is_empty() {
            return Err("No authentication token".to_string());
        }
        let url = format!("{}api/reactions_config", self.base_url);
        let response = self
            .client
            .get(&url)
            .header("Authorization", &auth_header)
            .send()
            .await
            .map_err(|e| format!("Get reactions config failed: {}", e))?;

        let json: Value = response
            .json()
            .await
            .map_err(|e| format!("Reactions config parse failed: {}", e))?;

        serde_json::from_value(json)
            .map_err(|e| format!("Reactions config deserialization failed: {}", e))
    }

    /// Add a reaction to a message (private, requires token)
    async fn add_reaction(
        &self,
        message_id: &str,
        emoji: &str,
    ) -> Result<models::Reaction, String> {
        let auth_header = self.get_token_header();
        if auth_header.is_empty() {
            return Err("No authentication token".to_string());
        }
        let url = format!("{}api/add_reaction", self.base_url);
        let body = serde_json::json!({
            "messageId": message_id,
            "emoji": emoji
        });

        let response = self
            .client
            .post(&url)
            .header("Authorization", &auth_header)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Add reaction failed: {}", e))?;

        let json: Value = response
            .json()
            .await
            .map_err(|e| format!("Add reaction parse failed: {}", e))?;

        serde_json::from_value(json)
            .map_err(|e| format!("Add reaction deserialization failed: {}", e))
    }

    /// Remove a reaction from a message (private, requires token)
    async fn remove_reaction(&self, message_id: &str, emoji: &str) -> Result<(), String> {
        let auth_header = self.get_token_header();
        if auth_header.is_empty() {
            return Err("No authentication token".to_string());
        }
        let url = format!("{}api/remove_reaction", self.base_url);
        let body = serde_json::json!({
            "messageId": message_id,
            "emoji": emoji
        });

        let response = self
            .client
            .post(&url)
            .header("Authorization", &auth_header)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Remove reaction failed: {}", e))?;

        if !response.status().is_success() {
            return Err(format!(
                "Remove reaction failed with status: {}",
                response.status()
            ));
        }

        Ok(())
    }

    /// Get reactions for a message (private, requires token)
    async fn get_message_reactions(
        &self,
        message_id: &str,
    ) -> Result<Vec<models::Reaction>, String> {
        let auth_header = self.get_token_header();
        if auth_header.is_empty() {
            return Err("No authentication token".to_string());
        }
        let url = format!(
            "{}api/message_reactions?messageId={}",
            self.base_url, message_id
        );
        let response = self
            .client
            .get(&url)
            .header("Authorization", &auth_header)
            .send()
            .await
            .map_err(|e| format!("Get message reactions failed: {}", e))?;

        let json: Value = response
            .json()
            .await
            .map_err(|e| format!("Message reactions parse failed: {}", e))?;

        if let Ok(reactions) = serde_json::from_value::<Vec<models::Reaction>>(json.clone()) {
            return Ok(reactions);
        }
        if let Ok(wrapper) = serde_json::from_value::<ListResponse<models::Reaction>>(json.clone())
        {
            if let Some(reactions) = wrapper.items {
                return Ok(reactions);
            }
        }
        Err("Message reactions response has unsupported format".to_string())
    }

    // ============================================================
    // Public wrappers
    // ============================================================

    /// Add a reaction to a message (public wrapper with token check)
    pub async fn add_reaction_public(
        &self,
        message_id: &str,
        emoji: &str,
    ) -> Result<models::Reaction, String> {
        self.add_reaction(message_id, emoji).await
    }

    /// Remove a reaction from a message (public wrapper with token check)
    pub async fn remove_reaction_public(
        &self,
        message_id: &str,
        emoji: &str,
    ) -> Result<(), String> {
        self.remove_reaction(message_id, emoji).await
    }

    // ============================================================
    // Poll API methods
    // ============================================================

    /// Создать опрос
    pub async fn create_poll(
        &self,
        chat_id: &str,
        poll: &crate::models::Poll,
    ) -> Result<crate::models::Poll, String> {
        let auth_header = self.get_token_header();
        if auth_header.is_empty() {
            return Err("No authentication token".to_string());
        }
        let url = format!("{}api/create_poll", self.base_url);

        let answers: Vec<serde_json::Value> = poll
            .answers
            .iter()
            .map(|a| serde_json::json!({ "text": a.text }))
            .collect();

        let body = serde_json::json!({
            "chatId": chat_id,
            "question": poll.question,
            "answers": answers,
            "isAnonymous": poll.is_anonymous,
            "isMultiSelect": poll.is_multi_select,
            "quizMode": poll.quiz_mode,
            "correctAnswerIds": poll.correct_answer_ids,
            "expiresAt": poll.expires_at.map(|d| d.to_rfc3339()),
        });

        let response = self
            .client
            .post(&url)
            .header("Authorization", &auth_header)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Create poll failed: {}", e))?;

        let json: Value = response
            .json()
            .await
            .map_err(|e| format!("Create poll parse failed: {}", e))?;

        serde_json::from_value(json)
            .map_err(|e| format!("Create poll deserialization failed: {}", e))
    }

    /// Проголосовать в опросе
    pub async fn vote_poll(
        &self,
        poll_id: &str,
        answer_ids: &[String],
    ) -> Result<crate::models::Poll, String> {
        let auth_header = self.get_token_header();
        if auth_header.is_empty() {
            return Err("No authentication token".to_string());
        }
        let url = format!("{}api/vote_poll", self.base_url);

        let body = serde_json::json!({
            "pollId": poll_id,
            "answerIds": answer_ids,
        });

        let response = self
            .client
            .post(&url)
            .header("Authorization", &auth_header)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Vote poll failed: {}", e))?;

        let json: Value = response
            .json()
            .await
            .map_err(|e| format!("Vote poll parse failed: {}", e))?;

        serde_json::from_value(json).map_err(|e| format!("Vote poll deserialization failed: {}", e))
    }

    /// Получить результаты опроса
    pub async fn get_poll_results(&self, poll_id: &str) -> Result<crate::models::Poll, String> {
        let auth_header = self.get_token_header();
        if auth_header.is_empty() {
            return Err("No authentication token".to_string());
        }
        let url = format!("{}api/poll_results?pollId={}", self.base_url, poll_id);

        let response = self
            .client
            .get(&url)
            .header("Authorization", &auth_header)
            .send()
            .await
            .map_err(|e| format!("Get poll results failed: {}", e))?;

        let json: Value = response
            .json()
            .await
            .map_err(|e| format!("Poll results parse failed: {}", e))?;

        serde_json::from_value(json)
            .map_err(|e| format!("Poll results deserialization failed: {}", e))
    }

    // ============================================================
    // Sticker API methods
    //
    // Yandex Messenger serves sticker metadata from the public file host:
    //   GET {FILE_PUBLIC_HOST}/stickers/packs?id={packId}&id={packId2}
    // Sticker images:
    //   GET {FILE_PUBLIC_HOST}/{stickerId}?size=small|large
    // Pack IDs themselves come from the bootstrap `sticker_packs` bucket.
    // Fake registry RPC methods (get_sticker_catalog etc.) return "No such path".
    // ============================================================

    fn sticker_cdn_url(sticker_id: &str, size: &str) -> String {
        format!(
            "{}/{}?size={}",
            crate::config::FILE_PUBLIC_HOST,
            sticker_id.trim_start_matches('/'),
            size
        )
    }

    /// Parse CDN response packs into our StickerPack model.
    fn parse_cdn_sticker_packs(
        packs_val: &serde_json::Value,
    ) -> Result<crate::models::StickerPackList, String> {
        let arr = packs_val
            .as_array()
            .ok_or_else(|| "Sticker packs payload is not an array".to_string())?;

        let mut packs = Vec::new();
        for pack_node in arr {
            let pack_id = pack_node
                .get("id")
                .or_else(|| pack_node.get("packId"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if pack_id.is_empty() {
                continue;
            }

            let title = pack_node
                .get("title")
                .and_then(|v| v.as_str())
                .unwrap_or("Стикеры")
                .to_string();

            let stickers_arr = pack_node
                .get("stickers")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();

            let mut stickers = Vec::with_capacity(stickers_arr.len());
            for s in &stickers_arr {
                let sticker_id = s
                    .get("id")
                    .or_else(|| s.get("stickerId"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                if sticker_id.is_empty() {
                    continue;
                }
                let emoji = s
                    .get("text")
                    .or_else(|| s.get("emoji"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                stickers.push(crate::models::Sticker {
                    sticker_id: sticker_id.clone(),
                    pack_id: pack_id.clone(),
                    file_url: Self::sticker_cdn_url(&sticker_id, "large"),
                    thumb_url: Self::sticker_cdn_url(&sticker_id, "small"),
                    width: 512,
                    height: 512,
                    emoji,
                    file_size: 0,
                    is_animated: sticker_id.ends_with(".json") || sticker_id.ends_with(".tgs"),
                    is_text_sticker: false,
                    text: None,
                });
            }

            let sticker_count = stickers.len() as u32;
            let thumb_url = stickers
                .first()
                .map(|s| s.thumb_url.clone())
                .unwrap_or_default();

            packs.push(crate::models::StickerPack {
                pack_id,
                title,
                stickers,
                is_installed: true,
                is_featured: false,
                category: pack_node
                    .get("description")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                thumb_url,
                sticker_count,
            });
        }

        if packs.is_empty() {
            Err("No sticker packs parsed from CDN".to_string())
        } else {
            Ok(crate::models::StickerPackList {
                packs,
                next_cursor: None,
            })
        }
    }

    /// Fetch sticker packs by IDs from the public file host (no auth required).
    pub async fn fetch_sticker_packs_by_ids(
        &self,
        pack_ids: &[String],
    ) -> Result<crate::models::StickerPackList, String> {
        if pack_ids.is_empty() {
            return Err("No sticker pack IDs".to_string());
        }

        // CDN accepts repeated ?id= params; batch in chunks to keep URLs short
        let mut all_packs = Vec::new();
        for chunk in pack_ids.chunks(12) {
            let query: String = chunk
                .iter()
                .map(|id| format!("id={}", urlencoding::encode(id)))
                .collect::<Vec<_>>()
                .join("&");
            let url = format!(
                "{}/stickers/packs?{}",
                crate::config::FILE_PUBLIC_HOST,
                query
            );

            let body = self
                .client
                .get(&url)
                .header("Origin", "https://yandex.ru")
                .header("Referer", "https://yandex.ru/chat")
                .send()
                .await
                .map_err(|e| format!("Sticker packs CDN request failed: {}", e))?
                .text()
                .await
                .map_err(|e| format!("Sticker packs CDN read failed: {}", e))?;

            let json: serde_json::Value = serde_json::from_str(&body)
                .map_err(|e| format!("Sticker packs CDN parse failed: {}", e))?;

            if json.get("status").and_then(|s| s.as_str()) == Some("error") {
                log::warn!(
                    "Sticker packs CDN error for {:?}: {:?}",
                    chunk,
                    json.get("data")
                );
                continue;
            }

            let data = json
                .get("data")
                .cloned()
                .unwrap_or(serde_json::Value::Array(vec![]));

            if let Ok(list) = Self::parse_cdn_sticker_packs(&data) {
                all_packs.extend(list.packs);
            }
        }

        if all_packs.is_empty() {
            Err("CDN returned no sticker packs".to_string())
        } else {
            Ok(crate::models::StickerPackList {
                packs: all_packs,
                next_cursor: None,
            })
        }
    }

    async fn get_sticker_pack_by_id(
        &self,
        pack_id: &str,
    ) -> Result<crate::models::StickerPack, String> {
        let list = self
            .fetch_sticker_packs_by_ids(&[pack_id.to_string()])
            .await?;
        list.packs
            .into_iter()
            .next()
            .ok_or_else(|| format!("No pack data for {}", pack_id))
    }

    /// Extract installed pack IDs from bootstrap, then load full packs from CDN.
    pub async fn get_sticker_catalog_from_bootstrap(
        &self,
    ) -> Result<crate::models::StickerPackList, String> {
        let data = self
            .rpc_request(
                "bootstrap",
                serde_json::json!({
                    "flags": {
                        "with_deleted": true,
                        "compact": true
                    }
                }),
            )
            .await?;

        let pack_ids: Vec<String> = data
            .get("buckets")
            .and_then(|buckets| buckets.as_array())
            .into_iter()
            .flatten()
            .filter(|bucket| {
                bucket.get("bucket_name").and_then(|v| v.as_str()) == Some("sticker_packs")
            })
            .flat_map(|bucket| {
                bucket
                    .get("bucket_value")
                    .and_then(|value| value.get("sticker_packs"))
                    .and_then(|packs| packs.as_array())
                    .into_iter()
                    .flatten()
                    .filter_map(|pack_id| pack_id.as_str().map(|s| s.to_string()))
                    .collect::<Vec<_>>()
            })
            .collect();

        if pack_ids.is_empty() {
            return Err("No sticker pack IDs in bootstrap".to_string());
        }

        log::info!(
            "Loading {} sticker packs from CDN: {:?}",
            pack_ids.len().min(16),
            &pack_ids[..pack_ids.len().min(8)]
        );

        // Load first 16 installed packs (enough for the picker UI)
        let ids: Vec<String> = pack_ids.into_iter().take(16).collect();
        self.fetch_sticker_packs_by_ids(&ids).await
    }

    /// Получить каталог стикеров (CDN via bootstrap pack IDs).
    pub async fn get_sticker_catalog(
        &self,
        _cursor: Option<&str>,
    ) -> Result<crate::models::StickerPackList, String> {
        // Official web client does not use registry RPC for stickers — go straight
        // to bootstrap IDs + public file host to avoid "No such path" spam.
        self.get_sticker_catalog_from_bootstrap().await
    }

    /// Поиск стикеров по запросу (локальный фильтр по уже загруженным пакам / title+emoji).
    pub async fn search_stickers(
        &self,
        query: &str,
    ) -> Result<crate::models::StickerPackList, String> {
        let catalog = self.get_sticker_catalog(None).await?;
        let q = query.to_lowercase();
        if q.is_empty() {
            return Ok(catalog);
        }

        let packs: Vec<crate::models::StickerPack> = catalog
            .packs
            .into_iter()
            .filter_map(|mut pack| {
                if pack.title.to_lowercase().contains(&q) {
                    return Some(pack);
                }
                pack.stickers
                    .retain(|s| s.emoji.to_lowercase().contains(&q) || s.sticker_id.contains(&q));
                if pack.stickers.is_empty() {
                    None
                } else {
                    pack.sticker_count = pack.stickers.len() as u32;
                    Some(pack)
                }
            })
            .collect();

        if packs.is_empty() {
            Err("No stickers match query".to_string())
        } else {
            Ok(crate::models::StickerPackList {
                packs,
                next_cursor: None,
            })
        }
    }

    /// Установить пакет стикеров (best-effort; may not be available on all accounts)
    pub async fn install_sticker_pack(&self, pack_id: &str) -> Result<(), String> {
        // Prefer silent failure over spammy registry RPC
        let _ = pack_id;
        log::info!("install_sticker_pack is not supported via public API; pack stays local");
        Ok(())
    }

    /// Получить информацию о стикере (CDN URL only — no metadata endpoint)
    pub async fn get_sticker(&self, sticker_id: &str) -> Result<crate::models::Sticker, String> {
        Ok(crate::models::Sticker {
            sticker_id: sticker_id.to_string(),
            pack_id: String::new(),
            file_url: Self::sticker_cdn_url(sticker_id, "large"),
            thumb_url: Self::sticker_cdn_url(sticker_id, "small"),
            width: 512,
            height: 512,
            emoji: String::new(),
            file_size: 0,
            is_animated: sticker_id.ends_with(".json") || sticker_id.ends_with(".tgs"),
            is_text_sticker: false,
            text: None,
        })
    }

    /// Отправить стикер в чат
    pub async fn send_sticker(
        &self,
        chat_id: &str,
        sticker_id: &str,
        caption: Option<&str>,
    ) -> Result<crate::models::Message, String> {
        let url = format!("{}api/send_sticker", self.base_url);
        let mut payload = serde_json::json!({
            "chatId": chat_id,
            "stickerId": sticker_id,
        });
        if let Some(cap) = caption {
            payload["caption"] = serde_json::json!(cap);
        }
        let body = self.post(&url, payload).await?;
        let json: serde_json::Value =
            serde_json::from_str(&body).map_err(|e| format!("Send sticker parse failed: {}", e))?;
        let data = json
            .get("data")
            .and_then(|v| v.as_object())
            .and_then(|o| o.get("message"))
            .ok_or("No data in send sticker response")?;
        let message: crate::models::Message = serde_json::from_value(data.clone())
            .map_err(|e| format!("Send sticker deserialization failed: {}", e))?;
        Ok(message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn test_config_values() {
        // Default OAuth client_id is empty — set via YANDEX_CLIENT_ID (32-char hex).
        assert!(
            config::OAUTH_CLIENT_ID.is_empty()
                || config::OAUTH_CLIENT_ID.chars().all(|c| c.is_ascii_hexdigit()),
            "OAUTH_CLIENT_ID must be empty or a hex OAuth app id, not a package name"
        );
        assert_eq!(config::TELEMOST_URL, "https://telemost.yandex.ru");
        assert_eq!(
            config::FILE_PUBLIC_HOST,
            "https://files.messenger.yandex.net"
        );
    }

    #[tokio::test]
    #[ignore]
    async fn test_api_send() {
        let auth = Arc::new(AuthManager::new().unwrap());
        let token = auth.get_token().await.unwrap();
        let http = HttpClient::new(auth.clone()).with_token(&token.access_token);
        println!("=== TEST: Has session cookies: {} ===", http.has_session());
        match http.get_chat_list(0, 10).await {
            Ok(chats) => {
                println!("=== TEST: Chats count: {} ===", chats.len());
                for chat in chats.iter().take(5) {
                    println!("=== TEST: Chat: ID={}, Title={:?} ===", chat.id, chat.title);
                }
                if let Some(first_chat) = chats.first() {
                    println!(
                        "=== TEST: Trying to send message to: {:?} ===",
                        first_chat.title
                    );
                    match http
                        .send_message(&first_chat.id, "Test message from Kilo CLI test", None)
                        .await
                    {
                        Ok(msg) => {
                            println!(
                                "=== TEST: Successfully sent! Message ID: {:?} ===",
                                msg.message_id
                            );
                        }
                        Err(e) => {
                            println!("=== TEST: Failed to send: {} ===", e);
                        }
                    }
                }
            }
            Err(e) => {
                println!("=== TEST: Failed to get chat list: {} ===", e);
            }
        }
    }

    #[tokio::test]
    async fn test_ws_send() {
        let auth = Arc::new(AuthManager::new().unwrap());
        let ws = WebSocketClient::new(auth.clone());

        let ws_clone = Arc::new(ws);
        let ws_spawn = ws_clone.clone();

        ws_clone
            .on_message(|msg| {
                println!(
                    "=== TEST WS RECV MESSAGE: {} ===",
                    serde_json::to_string(msg).unwrap()
                );
            })
            .await;

        tokio::spawn(async move {
            let _ = ws_spawn.connect().await;
        });

        let mut connected = false;
        for _ in 0..50 {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            let state = ws_clone.state.lock().await;
            if *state == WSState::Connected {
                connected = true;
                break;
            }
        }

        println!("=== TEST WS: Connected: {} ===", connected);
        if connected {
            let chat_id = "0/0/aca75cd7-0c98-409b-ba27-3e27c823e1dd";

            // Method 1: JSON-RPC over text frame
            println!("=== TEST WS: Trying Method 1: JSON-RPC ===");
            let payload_id = uuid::Uuid::new_v4().simple().to_string();
            let rpc_params = serde_json::json!({
                "chat_id": chat_id,
                "text": "Hello from Kilo WS JSON-RPC test",
                "payload_id": payload_id
            });
            match ws_clone.send_request("send_message", rpc_params).await {
                Ok(res) => {
                    println!("=== TEST WS: Method 1 (JSON-RPC) Success: {} ===", res);
                }
                Err(e) => {
                    println!("=== TEST WS: Method 1 (JSON-RPC) Error: {} ===", e);
                }
            }

            // Method 2: Binary frame (send_text_message)
            println!("=== TEST WS: Trying Method 2: Binary ===");
            match ws_clone
                .send_text_message(chat_id, "Hello from Kilo WS Binary test", None)
                .await
            {
                Ok(msg) => {
                    println!(
                        "=== TEST WS: Method 2 (Binary) Success: {:?} ===",
                        msg.message_id
                    );
                }
                Err(e) => {
                    println!("=== TEST WS: Method 2 (Binary) Error: {} ===", e);
                }
            }

            tokio::time::sleep(std::time::Duration::from_secs(3)).await;
        }
    }
}
