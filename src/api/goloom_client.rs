//! Goloom signaling WebSocket client.
//!
//! Transport for the protocol in [`super::goloom`] (schemas from
//! `docs/apk-3.12.0.138/proto/`). Default endpoint [`crate::config::GOLOOM_WS_URL`].
//!
//! Session lifecycle per `signaling.proto` comments:
//! 1. WS open → [`goloom::hello_message`] MUST be the first message
//!    (else the server closes with `HELLO_SHOULD_BE_FIRST`).
//! 2. Server answers [`goloom::ServerHello`] (ICE servers, session secret,
//!    ping/ack periods, negotiated [`goloom::CapabilitiesAnswer`]).
//! 3. Steady state: protobuf `Ping` ⇄ `Ack` heartbeat (a missing Ack means the
//!    connection MUST be closed), SDP/ICE exchange, roster/slot updates.
//! 4. Reconnect carries `signaling_close_code` + `session_secret` auth.
//!
//! Transport auth (HTTP headers) is best-effort: the APK talks to the same
//! hosts with Passport cookies/OAuth, but the exact Goloom handshake headers
//! are not observable statically — verifiable only against a live server.
//! Everything the client *sends in-band* (Hello/capabilities/SDP/ICE) is exact.
//!
//! Structure: one background task per session owns the socket; all writes
//! (public outbox, heartbeat pings, Ping-acks) go through it, so frame order
//! on the wire is deterministic.

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use futures::{SinkExt, StreamExt};
use tokio::sync::{mpsc, Mutex};
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::protocol::CloseFrame;
use tokio_tungstenite::tungstenite::Message;

use super::goloom::{
    self, CapabilitiesAnswer, GoloomMessage, HelloAuth, MessageKind, ParticipantDescription,
    RtcConfiguration, StatusCode, UpdateParticipantMeta,
};
use crate::config;

const HANDSHAKE_TIMEOUT_SECS: u64 = 15;
const CONNECT_TIMEOUT_SECS: u64 = 15;
const DEFAULT_PING_INTERVAL_SECS: u64 = 5;
const DEFAULT_ACK_TIMEOUT_SECS: u64 = 5;
/// Keeps the ack-deadline branch pending while no ping is awaited.
const IDLE_DEADLINE_SECS: u64 = 3600;

/// Connection state (mirrors [`crate::api::WSState`] semantics for the UI).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GoloomState {
    Disconnected,
    Connecting,
    Handshaking,
    Connected,
    Reconnecting(u32),
    Closed { reason: String },
}

/// ICE server as delivered by `ServerHello.rtc_configuration`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IceServerInfo {
    pub urls: Vec<String>,
    pub username: String,
    pub credential: String,
}

/// Events pushed to the UI / call controller.
#[derive(Debug, Clone)]
pub enum GoloomEvent {
    StateChanged(GoloomState),
    Connected {
        session_secret: String,
        ice_servers: Vec<IceServerInfo>,
        capabilities: Option<CapabilitiesAnswer>,
        ping_interval_secs: u64,
        ack_timeout_secs: u64,
    },
    Disconnected {
        will_retry: bool,
        reason: String,
    },
    RosterUpsert(Vec<ParticipantDescription>),
    RosterRemove(Vec<String>),
    Slots(super::goloom::SlotsConfig),
    SlotsOffset(u32),
    SlotsMeta(Option<u32>),
    SlotsRules(super::goloom::SetSlots),
    MeUpdated(UpdateParticipantMeta),
    PinnedRequested(Vec<String>),
    SubscriberOffer {
        pc_seq: u32,
        sdp: String,
    },
    PublisherAnswer {
        pc_seq: u32,
        sdp: String,
    },
    IceCandidate(super::goloom::WebrtcIceCandidate),
    Quality {
        participants: Vec<super::goloom::ParticipantQualityReport>,
        self_score: i32,
    },
    Vad(bool),
    ServerNotification {
        id: String,
        payload: String,
    },
    ActiveCodecs {
        video: i32,
        audio: i32,
    },
    /// ACK for one of our messages, or an inbound server Ping surfaced as
    /// an Ok ack (the wire Ack for it is sent automatically).
    AckReceived {
        uid: String,
        code: i32,
        description: Option<String>,
    },
    /// Non-OK ack — surfaces e.g. `BAD_DESCRIPTION` right at the failed SDP.
    ServerError {
        uid: String,
        code: i32,
        description: String,
    },
    /// Server asked us to move (e.g. `MOVE_TO_NEW_MEDIA_SERVER`); the client
    /// already reconnects, this is for UI logging.
    ReconnectRequested {
        code: i32,
        reason: String,
    },
}

/// Connection parameters for one call/room.
#[derive(Debug, Clone)]
pub struct GoloomParams {
    pub ws_url: String,
    pub room_id: String,
    pub participant_id: String,
    pub credentials: Option<String>,
    pub display_name: Option<String>,
    pub send_audio: bool,
    pub send_video: bool,
    pub send_sharing: bool,
    pub app_version: String,
    /// Best-effort transport auth (see module docs).
    pub oauth_token: Option<String>,
    /// Best-effort transport auth (see module docs).
    pub cookies: Option<String>,
    pub max_attempts: u32,
}

impl GoloomParams {
    pub fn new(room_id: &str, participant_id: &str, app_version: &str) -> Self {
        Self {
            ws_url: config::GOLOOM_WS_URL.to_string(),
            room_id: room_id.to_string(),
            participant_id: participant_id.to_string(),
            credentials: None,
            display_name: None,
            send_audio: true,
            send_video: true,
            send_sharing: false,
            app_version: app_version.to_string(),
            oauth_token: None,
            cookies: None,
            max_attempts: config::WS_MAX_RECONNECT_ATTEMPTS,
        }
    }
}

/// Handle to a background signaling session.
pub struct GoloomHandle {
    events_rx: mpsc::UnboundedReceiver<GoloomEvent>,
    out_tx: mpsc::UnboundedSender<GoloomMessage>,
    state: Arc<Mutex<GoloomState>>,
    shutdown: Arc<AtomicBool>,
}

impl GoloomHandle {
    pub async fn next_event(&mut self) -> Option<GoloomEvent> {
        self.events_rx.recv().await
    }

    pub async fn state(&self) -> GoloomState {
        self.state.lock().await.clone()
    }

    pub fn send(&self, msg: GoloomMessage) -> Result<(), String> {
        self.out_tx
            .send(msg)
            .map_err(|e| format!("signaling task gone: {e}"))
    }

    /// Answer to an incoming [`GoloomEvent::SubscriberOffer`] (SDP from our PC).
    pub fn send_subscriber_answer(&self, pc_seq: u32, sdp: &str) -> Result<(), String> {
        self.send(GoloomMessage {
            uid: goloom::new_uid(),
            kind: Some(MessageKind::SubscriberSdpAnswer(
                super::goloom::SubscriberSdpAnswer {
                    pc_seq,
                    sdp: sdp.to_string(),
                },
            )),
        })
    }

    /// Offer from our publisher PC (local camera/mic tracks).
    pub fn send_publisher_offer(
        &self,
        pc_seq: u32,
        sdp: &str,
        tracks: Vec<super::goloom::PublisherTrackDescription>,
    ) -> Result<(), String> {
        self.send(GoloomMessage {
            uid: goloom::new_uid(),
            kind: Some(MessageKind::PublisherSdpOffer(
                super::goloom::PublisherSdpOffer {
                    pc_seq,
                    sdp: sdp.to_string(),
                    tracks,
                },
            )),
        })
    }

    pub fn send_ice(&self, cand: super::goloom::WebrtcIceCandidate) -> Result<(), String> {
        self.send(GoloomMessage {
            uid: goloom::new_uid(),
            kind: Some(MessageKind::IceCandidate(cand)),
        })
    }

    /// Mute/unmute and meta updates (`update_me`).
    pub fn send_update_me(
        &self,
        send_audio: bool,
        send_video: bool,
        send_sharing: bool,
    ) -> Result<(), String> {
        self.send(GoloomMessage {
            uid: goloom::new_uid(),
            kind: Some(MessageKind::UpdateMe(UpdateParticipantMeta {
                participant_attributes: std::collections::HashMap::new(),
                send_audio,
                send_video,
                send_sharing,
            })),
        })
    }

    /// Page change request (`set_slots_offset`).
    pub fn send_slots_offset(&self, offset: u32) -> Result<(), String> {
        self.send(GoloomMessage {
            uid: goloom::new_uid(),
            kind: Some(MessageKind::SetSlotsOffset(super::goloom::SetSlotsOffset {
                offset,
            })),
        })
    }

    pub fn send_client_error(&self, code: StatusCode, description: &str) -> Result<(), String> {
        self.send(GoloomMessage {
            uid: goloom::new_uid(),
            kind: Some(MessageKind::ClientError(super::goloom::ClientError {
                code: code as i32,
                description: description.to_string(),
                details: std::collections::HashMap::new(),
                client_timestamp: now_ms(),
            })),
        })
    }

    pub fn shutdown(&self) {
        self.shutdown.store(true, Ordering::Relaxed);
    }
}

fn now_ms() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64() * 1000.0)
        .unwrap_or(0.0)
}

/// WS close-code → retry policy.
///
/// Terminal: call is over (`Normal`), auth/capability problems no retry can
/// fix, kicked/duplicate session. Everything else (transport drops, `Away`,
/// `Restart`/`Again`, 4100-range moves, timeouts) → reconnect with backoff.
pub fn should_retry_close(code: u16) -> bool {
    if StatusCode::is_terminal(code as i32) {
        return false;
    }
    match code {
        // 1000 Normal: purpose fulfilled (call ended).
        1000 => false,
        4000 | 4002 | 4003 | 4007 => false, // denied / must-be-first / caps / validation
        _ => true,
    }
}

enum SessionEnd {
    ClosedByServer { code: u16, reason: String },
    Transport(String),
    HandshakeTimeout,
    PingTimeout,
    Shutdown,
}

/// What one inbound message produces: an optional wire reply plus an
/// optional session end. The session loop sends the reply itself, so the
/// socket has a single writer.
struct DispatchOut {
    reply: Option<GoloomMessage>,
    end: Option<SessionEnd>,
}

impl DispatchOut {
    fn reply(msg: GoloomMessage) -> Self {
        Self {
            reply: Some(msg),
            end: None,
        }
    }
    fn quiet() -> Self {
        Self {
            reply: None,
            end: None,
        }
    }
}

/// Mutable per-session network state touched by dispatch.
struct SessionNet {
    handshook: bool,
    ping_interval: tokio::time::Interval,
    ack_timeout_secs: u64,
}

pub struct GoloomClient;

impl GoloomClient {
    /// Spawn the reconnect loop; returns immediately with the live handle.
    pub fn spawn(params: GoloomParams) -> GoloomHandle {
        let (events_tx, events_rx) = mpsc::unbounded_channel();
        let (out_tx, out_rx) = mpsc::unbounded_channel();
        let state = Arc::new(Mutex::new(GoloomState::Disconnected));
        let shutdown = Arc::new(AtomicBool::new(false));

        let task = Runner {
            params,
            events: events_tx,
            out_rx,
            state: state.clone(),
            shutdown: shutdown.clone(),
            session_secret: Arc::new(Mutex::new(None)),
        };
        tokio::spawn(async move { task.run().await });

        GoloomHandle {
            events_rx,
            out_tx,
            state,
            shutdown,
        }
    }
}

struct Runner {
    params: GoloomParams,
    events: mpsc::UnboundedSender<GoloomEvent>,
    out_rx: mpsc::UnboundedReceiver<GoloomMessage>,
    state: Arc<Mutex<GoloomState>>,
    shutdown: Arc<AtomicBool>,
    session_secret: Arc<Mutex<Option<String>>>,
}

impl Runner {
    async fn set_state(&self, s: GoloomState) {
        *self.state.lock().await = s.clone();
        let _ = self.events.send(GoloomEvent::StateChanged(s));
    }

    async fn run(mut self) {
        let mut attempts = 0u32;
        let mut last_close: Option<(u16, String)> = None;

        loop {
            if self.shutdown.load(Ordering::Relaxed) {
                self.set_state(GoloomState::Closed {
                    reason: "shutdown".into(),
                })
                .await;
                return;
            }
            if attempts >= self.params.max_attempts {
                let reason = format!("giving up after {attempts} attempts");
                let _ = self.events.send(GoloomEvent::Disconnected {
                    will_retry: false,
                    reason: reason.clone(),
                });
                self.set_state(GoloomState::Closed { reason }).await;
                return;
            }

            self.set_state(if attempts == 0 {
                GoloomState::Connecting
            } else {
                GoloomState::Reconnecting(attempts)
            })
            .await;

            let close_code = last_close.as_ref().map(|(c, _)| *c);
            match self.run_session(close_code).await {
                SessionEnd::Shutdown => {
                    self.set_state(GoloomState::Closed {
                        reason: "shutdown".into(),
                    })
                    .await;
                    return;
                }
                SessionEnd::ClosedByServer { code, reason } => {
                    last_close = Some((code, reason.clone()));
                    if !should_retry_close(code) {
                        let msg = format!("server closed session (code {code}): {reason}");
                        let _ = self.events.send(GoloomEvent::Disconnected {
                            will_retry: false,
                            reason: msg.clone(),
                        });
                        self.set_state(GoloomState::Closed { reason: msg }).await;
                        return;
                    }
                    if StatusCode::expects_reconnect(code as i32) {
                        let _ = self.events.send(GoloomEvent::ReconnectRequested {
                            code: code as i32,
                            reason: reason.clone(),
                        });
                    }
                    attempts += 1;
                    let _ = self.events.send(GoloomEvent::Disconnected {
                        will_retry: true,
                        reason: format!("server closed ({code}): {reason}"),
                    });
                }
                SessionEnd::Transport(reason) => {
                    last_close = None;
                    attempts += 1;
                    let _ = self.events.send(GoloomEvent::Disconnected {
                        will_retry: true,
                        reason,
                    });
                }
                SessionEnd::HandshakeTimeout => {
                    last_close = None;
                    attempts += 1;
                    let _ = self.events.send(GoloomEvent::Disconnected {
                        will_retry: true,
                        reason: "handshake timeout (no ServerHello)".into(),
                    });
                }
                SessionEnd::PingTimeout => {
                    last_close = None;
                    attempts += 1;
                    let _ = self.events.send(GoloomEvent::Disconnected {
                        will_retry: true,
                        reason: "ping ack timeout".into(),
                    });
                }
            }

            // Exponential backoff 1s → 30s cap (1012 Restart suggests 5–30s).
            let wait = (1u64 << attempts.min(5)).min(30);
            tokio::time::sleep(std::time::Duration::from_secs(wait)).await;
        }
    }

    /// One WS connection: Hello → ServerHello → steady state.
    async fn run_session(&mut self, prev_close_code: Option<u16>) -> SessionEnd {
        let mut request = match self.params.ws_url.clone().into_client_request() {
            Ok(r) => r,
            Err(e) => return SessionEnd::Transport(format!("bad ws url: {e}")),
        };
        let headers = request.headers_mut();
        headers.insert(
            "Origin",
            tokio_tungstenite::tungstenite::http::HeaderValue::from_static("https://yandex.ru"),
        );
        headers.insert(
            "User-Agent",
            tokio_tungstenite::tungstenite::http::HeaderValue::from_static("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36"),
        );
        if let Some(ref token) = self.params.oauth_token {
            let value = if token.starts_with("OAuth ") {
                token.clone()
            } else {
                format!("OAuth {token}")
            };
            if let Ok(v) = tokio_tungstenite::tungstenite::http::HeaderValue::from_str(&value) {
                headers.insert("Authorization", v);
            }
        }
        if let Some(ref cookies) = self.params.cookies {
            if let Ok(v) = tokio_tungstenite::tungstenite::http::HeaderValue::from_str(cookies) {
                headers.insert("Cookie", v);
            }
        }

        let (ws, _) = match tokio::time::timeout(
            std::time::Duration::from_secs(CONNECT_TIMEOUT_SECS),
            tokio_tungstenite::connect_async(request),
        )
        .await
        {
            Ok(Ok(pair)) => pair,
            Ok(Err(e)) => return SessionEnd::Transport(format!("connect failed: {e}")),
            Err(_) => return SessionEnd::Transport("connect timed out".into()),
        };

        self.set_state(GoloomState::Handshaking).await;
        let (mut write, mut read) = ws.split();

        // Hello MUST be first. Prefer session_secret auth on reconnect.
        let secret = self.session_secret.lock().await.clone();
        let mut hello = goloom::hello_message(
            &self.params.room_id,
            &self.params.participant_id,
            self.params.credentials.clone(),
            self.params.display_name.as_deref(),
            self.params.send_audio,
            self.params.send_video,
            self.params.send_sharing,
            &self.params.app_version,
        );
        if let Some(MessageKind::Hello(h)) = hello.kind.as_mut() {
            if secret.is_some() {
                h.auth = secret.map(HelloAuth::SessionSecret);
            }
            h.signaling_close_code = prev_close_code.map(|c| c as i32);
        }
        if write
            .send(Message::Binary(goloom::encode_message(&hello).into()))
            .await
            .is_err()
        {
            return SessionEnd::Transport("hello send failed".into());
        }

        let mut net = SessionNet {
            handshook: false,
            ping_interval: tokio::time::interval(std::time::Duration::from_secs(
                DEFAULT_PING_INTERVAL_SECS,
            )),
            ack_timeout_secs: DEFAULT_ACK_TIMEOUT_SECS,
        };
        let mut pending_ping: Option<(String, tokio::time::Instant)> = None;
        let handshake_deadline =
            tokio::time::Instant::now() + std::time::Duration::from_secs(HANDSHAKE_TIMEOUT_SECS);

        loop {
            if self.shutdown.load(Ordering::Relaxed) {
                let _ = write.close().await;
                return SessionEnd::Shutdown;
            }
            let ack_deadline = pending_ping.as_ref().map(|(_, d)| *d).unwrap_or_else(|| {
                tokio::time::Instant::now() + std::time::Duration::from_secs(IDLE_DEADLINE_SECS)
            });

            tokio::select! {
                biased;

                msg = self.out_rx.recv() => {
                    match msg {
                        Some(m) => {
                            if write.send(Message::Binary(goloom::encode_message(&m).into())).await.is_err() {
                                return SessionEnd::Transport("write failed".into());
                            }
                        }
                        None => return SessionEnd::Shutdown,
                    }
                }

                _ = net.ping_interval.tick(), if net.handshook => {
                    let ping = goloom::ping(None);
                    let uid = ping.uid.clone();
                    if write.send(Message::Binary(goloom::encode_message(&ping).into())).await.is_err() {
                        return SessionEnd::Transport("ping send failed".into());
                    }
                    pending_ping = Some((uid, tokio::time::Instant::now() + std::time::Duration::from_secs(net.ack_timeout_secs)));
                }

                msg = read.next() => {
                    match msg {
                        None => return SessionEnd::Transport("stream ended".into()),
                        Some(Err(e)) => {
                            use tokio_tungstenite::tungstenite::Error as WsErr;
                            match e {
                                WsErr::ConnectionClosed | WsErr::AlreadyClosed => {
                                    return SessionEnd::Transport("connection closed".into());
                                }
                                _ => return SessionEnd::Transport(format!("read error: {e}")),
                            }
                        }
                        Some(Ok(Message::Ping(payload))) => {
                            // WS-level ping: answer with pong (separate from proto Ping⇄Ack).
                            if write.send(Message::Pong(payload)).await.is_err() {
                                return SessionEnd::Transport("pong send failed".into());
                            }
                        }
                        Some(Ok(Message::Pong(_))) => {}
                        Some(Ok(Message::Text(_))) => {
                            log::debug!("goloom: unexpected text frame, ignoring");
                        }
                        Some(Ok(Message::Frame(_))) => {}
                        Some(Ok(Message::Close(frame))) => {
                            let (code, reason) = frame_to_code_reason(frame);
                            return SessionEnd::ClosedByServer { code, reason };
                        }
                        Some(Ok(Message::Binary(bin))) => {
                            let decoded = match goloom::decode_message(&bin) {
                                Ok(m) => m,
                                Err(e) => {
                                    log::warn!("goloom: undecodable frame ({} bytes): {e}", bin.len());
                                    continue;
                                }
                            };
                            // Consume the ack for our heartbeat ping.
                            if let Some(MessageKind::Ack(_)) = decoded.kind.as_ref() {
                                if pending_ping.as_ref().is_some_and(|(u, _)| *u == decoded.uid) {
                                    pending_ping = None;
                                    continue;
                                }
                            }
                            let out = self.dispatch(decoded, &mut net).await;
                            if let Some(reply) = out.reply {
                                if write.send(Message::Binary(goloom::encode_message(&reply).into())).await.is_err() {
                                    return SessionEnd::Transport("ack send failed".into());
                                }
                            }
                            if let Some(end) = out.end {
                                return end;
                            }
                        }
                    }
                }

                _ = tokio::time::sleep_until(ack_deadline) => {
                    if pending_ping.take().is_some() {
                        return SessionEnd::PingTimeout;
                    }
                }

                _ = tokio::time::sleep_until(handshake_deadline), if !net.handshook => {
                    return SessionEnd::HandshakeTimeout;
                }
            }
        }
    }

    /// Maps one inbound message to UI events (+ optional wire reply).
    /// Never ends the session itself — server-driven moves arrive as
    /// `ReconnectRequested` and the close frame that follows does the rest.
    async fn dispatch(&self, msg: GoloomMessage, net: &mut SessionNet) -> DispatchOut {
        let uid = msg.uid.clone();
        let kind = match msg.kind {
            Some(k) => k,
            None => return DispatchOut::quiet(),
        };
        match kind {
            MessageKind::ServerHello(h) => {
                *self.session_secret.lock().await = if h.session_secret.is_empty() {
                    None
                } else {
                    Some(h.session_secret.clone())
                };
                let ice = h
                    .rtc_configuration
                    .as_ref()
                    .map(map_ice)
                    .unwrap_or_default();
                let (ping_secs, ack_secs) = h
                    .ping_pong_configuration
                    .as_ref()
                    .map(|c| {
                        (
                            nonzero_or(c.ping_interval as u64, DEFAULT_PING_INTERVAL_SECS),
                            nonzero_or(c.ack_timeout as u64, DEFAULT_ACK_TIMEOUT_SECS),
                        )
                    })
                    .unwrap_or((DEFAULT_PING_INTERVAL_SECS, DEFAULT_ACK_TIMEOUT_SECS));
                net.ack_timeout_secs = ack_secs;
                net.ping_interval =
                    tokio::time::interval(std::time::Duration::from_secs(ping_secs));
                net.ping_interval.reset();
                net.handshook = true;
                self.set_state(GoloomState::Connected).await;
                let _ = self.events.send(GoloomEvent::Connected {
                    session_secret: h.session_secret.clone(),
                    ice_servers: ice,
                    capabilities: h.capabilities_answer.clone(),
                    ping_interval_secs: ping_secs,
                    ack_timeout_secs: ack_secs,
                });
                DispatchOut::quiet()
            }
            MessageKind::Ping(_) => {
                // Schema: answer with Ack ASAP, same uid.
                let _ = self.events.send(GoloomEvent::AckReceived {
                    uid: uid.clone(),
                    code: StatusCode::Ok as i32,
                    description: None,
                });
                DispatchOut::reply(goloom::ack_ok(&uid))
            }
            MessageKind::Ack(a) => {
                let (code, desc) = a
                    .status
                    .as_ref()
                    .map(|s| (s.code, s.description.clone()))
                    .unwrap_or((StatusCode::Ok as i32, None));
                if code == StatusCode::Ok as i32 {
                    let _ = self.events.send(GoloomEvent::AckReceived {
                        uid,
                        code,
                        description: desc,
                    });
                } else {
                    let _ = self.events.send(GoloomEvent::ServerError {
                        uid,
                        code,
                        description: desc.unwrap_or_default(),
                    });
                }
                DispatchOut::quiet()
            }
            MessageKind::UpdateDescription(u) => {
                let _ = self.events.send(GoloomEvent::RosterUpsert(u.description));
                DispatchOut::quiet()
            }
            MessageKind::UpsertDescription(u) => {
                let _ = self.events.send(GoloomEvent::RosterUpsert(u.description));
                DispatchOut::quiet()
            }
            MessageKind::RemoveDescription(r) => {
                let _ = self
                    .events
                    .send(GoloomEvent::RosterRemove(r.description_id));
                DispatchOut::quiet()
            }
            MessageKind::SlotsConfig(s) => {
                let _ = self.events.send(GoloomEvent::Slots(s));
                DispatchOut::quiet()
            }
            MessageKind::SetSlotsOffset(o) => {
                let _ = self.events.send(GoloomEvent::SlotsOffset(o.offset));
                DispatchOut::quiet()
            }
            MessageKind::SlotsMeta(m) => {
                let _ = self.events.send(GoloomEvent::SlotsMeta(m.max_offset));
                DispatchOut::quiet()
            }
            MessageKind::SetSlots(s) => {
                let _ = self.events.send(GoloomEvent::SlotsRules(s));
                DispatchOut::quiet()
            }
            MessageKind::UpdateMe(m) => {
                let _ = self.events.send(GoloomEvent::MeUpdated(m));
                DispatchOut::quiet()
            }
            MessageKind::RequestPinnedParticipants(r) => {
                let _ = self
                    .events
                    .send(GoloomEvent::PinnedRequested(r.participants_id));
                DispatchOut::quiet()
            }
            MessageKind::SubscriberSdpOffer(o) => {
                let _ = self.events.send(GoloomEvent::SubscriberOffer {
                    pc_seq: o.pc_seq,
                    sdp: o.sdp,
                });
                DispatchOut::quiet()
            }
            MessageKind::PublisherSdpAnswer(a) => {
                let _ = self.events.send(GoloomEvent::PublisherAnswer {
                    pc_seq: a.pc_seq,
                    sdp: a.sdp,
                });
                DispatchOut::quiet()
            }
            MessageKind::IceCandidate(c) => {
                let _ = self.events.send(GoloomEvent::IceCandidate(c));
                DispatchOut::quiet()
            }
            MessageKind::UpsertQuality(q) => {
                let _ = self.events.send(GoloomEvent::Quality {
                    participants: q.participants_quality_report,
                    self_score: super::goloom::NetworkQualityScore::Unspecified as i32,
                });
                DispatchOut::quiet()
            }
            MessageKind::SelfQuality(q) => {
                let _ = self.events.send(GoloomEvent::Quality {
                    participants: Vec::new(),
                    self_score: q.network_score,
                });
                DispatchOut::quiet()
            }
            MessageKind::VadActivity(v) => {
                let _ = self.events.send(GoloomEvent::Vad(v.active));
                DispatchOut::quiet()
            }
            MessageKind::ClientSideVad(_) => DispatchOut::quiet(),
            MessageKind::Notification(n) => {
                let _ = self.events.send(GoloomEvent::ServerNotification {
                    id: n.notification_id,
                    payload: n.payload,
                });
                DispatchOut::quiet()
            }
            MessageKind::SetActiveCodecs(c) => {
                let _ = self.events.send(GoloomEvent::ActiveCodecs {
                    video: c.video_codec,
                    audio: c.audio_codec,
                });
                DispatchOut::quiet()
            }
            // Client-originated or SFU-internal: never dispatched inbound.
            MessageKind::PublisherSdpOffer(_)
            | MessageKind::SubscriberSdpAnswer(_)
            | MessageKind::RequestSubscription(_)
            | MessageKind::UpdatePublisherTrack(_)
            | MessageKind::Hello(_)
            | MessageKind::SfuHello(_)
            | MessageKind::ClientError(_) => {
                log::debug!("goloom: ignoring inbound client-side message (uid {uid})");
                DispatchOut::quiet()
            }
        }
    }
}

fn map_ice(rtc: &RtcConfiguration) -> Vec<IceServerInfo> {
    rtc.ice_servers
        .iter()
        .map(|s| IceServerInfo {
            urls: s.urls.clone(),
            username: s.username.clone(),
            credential: s.credential.clone(),
        })
        .collect()
}

fn nonzero_or(v: u64, dflt: u64) -> u64 {
    if v == 0 {
        dflt
    } else {
        v
    }
}

fn frame_to_code_reason(frame: Option<CloseFrame<'_>>) -> (u16, String) {
    match frame {
        Some(f) => (u16::from(f.code), f.reason.to_string()),
        None => (1006, "no close frame (transport drop)".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::TcpListener;

    #[test]
    fn close_policy_matrix() {
        // Terminal: never retry.
        for code in [
            1000u16, 4000, 4002, 4003, 4004, 4005, 4007, 4009, 4010, 4011,
        ] {
            assert!(!should_retry_close(code), "code {code} must be terminal");
        }
        // Retryable: drops, restarts, moves, timeouts.
        for code in [
            1001u16, 1006, 1011, 1012, 1013, 4001, 4006, 4008, 4100, 4101, 4102, 4103, 4104,
        ] {
            assert!(should_retry_close(code), "code {code} must retry");
        }
    }

    type WsStream = tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>;

    async fn recv_proto(
        read: &mut futures::stream::SplitStream<WsStream>,
    ) -> Option<GoloomMessage> {
        let msg = tokio::time::timeout(std::time::Duration::from_secs(10), read.next())
            .await
            .ok()??;
        match msg.ok()? {
            Message::Binary(bin) => goloom::decode_message(&bin).ok(),
            _ => None,
        }
    }

    async fn send_proto(
        write: &mut futures::stream::SplitSink<WsStream, Message>,
        msg: &GoloomMessage,
    ) {
        write
            .send(Message::Binary(goloom::encode_message(msg)))
            .await
            .expect("server send");
    }

    fn server_hello_msg() -> GoloomMessage {
        GoloomMessage {
            uid: goloom::new_uid(),
            kind: Some(MessageKind::ServerHello(super::goloom::ServerHello {
                capabilities_answer: None,
                serving_components: Vec::new(),
                session_secret: "s3".to_string(),
                sfu_peer_initialization_id: "sfu-1".to_string(),
                rtc_configuration: Some(super::goloom::RtcConfiguration {
                    ice_servers: vec![super::goloom::RtcIceServer {
                        urls: vec!["stun:stun.test".to_string()],
                        credential: String::new(),
                        username: String::new(),
                    }],
                    ice_transport_policy: None,
                    ice_candidate_pool_size: None,
                    bundle_policy: None,
                    rtcp_mux_policy: None,
                }),
                log_endpoint: String::new(),
                ping_pong_configuration: Some(super::goloom::PingPongConfiguration {
                    ping_interval: 60,
                    ack_timeout: 60,
                }),
                telemetry_configuration: None,
                exclude_from_experiments: false,
                active_codecs: None,
            })),
        }
    }

    /// Full handshake: Hello → ServerHello → Ping⇄Ack → roster → terminal close.
    #[tokio::test]
    async fn loopback_handshake_ping_ack_terminal() {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("addr");

        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.expect("accept");
            let ws = tokio_tungstenite::accept_async(stream).await.expect("ws");
            let (mut write, mut read) = ws.split();

            // 1. Hello must be first and exact in-band.
            let hello = recv_proto(&mut read).await.expect("hello");
            match hello.kind.expect("hello kind") {
                MessageKind::Hello(h) => {
                    assert_eq!(h.room_id, "room-1");
                    assert_eq!(h.participant_id, "peer-1");
                    assert_eq!(h.auth, Some(HelloAuth::Credentials("cred".to_string())));
                    assert!(h.capabilities_offer.is_some());
                    assert!(h.signaling_close_code.is_none());
                }
                other => panic!("expected Hello, got {other:?}"),
            }

            // 2. Answer with ServerHello.
            send_proto(&mut write, &server_hello_msg()).await;

            // 3. Auto-ack client heartbeat pings (none expected at 60s, robust anyway).
            // 4. Scripted: our Ping → client Ack with same uid.
            let ping = goloom::ping(Some("srv-ping-1".to_string()));
            send_proto(&mut write, &ping).await;
            let ack = tokio::time::timeout(std::time::Duration::from_secs(10), async {
                loop {
                    let m = recv_proto(&mut read).await.expect("ack frame");
                    match m.kind {
                        Some(MessageKind::Ping(_)) => {
                            send_proto(&mut write, &goloom::ack_ok(&m.uid)).await;
                        }
                        Some(MessageKind::Ack(_)) => return m,
                        _ => {}
                    }
                }
            })
            .await
            .expect("ack timeout");
            assert_eq!(ack.uid, "srv-ping-1");

            // 5. Roster push.
            let mut attrs = std::collections::HashMap::new();
            attrs.insert("name".to_string(), "Alice".to_string());
            send_proto(
                &mut write,
                &GoloomMessage {
                    uid: goloom::new_uid(),
                    kind: Some(MessageKind::UpsertDescription(
                        super::goloom::UpsertDescription {
                            description: vec![ParticipantDescription {
                                id: "p1".to_string(),
                                participant_attributes: attrs,
                                send_audio: true,
                                send_video: false,
                                send_sharing: false,
                                hide_from_participants_list: false,
                                disconnected_at: None,
                                network_score: super::goloom::NetworkQualityScore::Good as i32,
                                connection_type: super::goloom::ConnectionType::Sdk as i32,
                                ref_participant_id: None,
                            }],
                        },
                    )),
                },
            )
            .await;

            // 6. Terminal close: room over.
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            write
                .send(Message::Close(Some(CloseFrame {
                    code: 4009u16.into(),
                    reason: "done".into(),
                })))
                .await
                .ok();
        });

        let mut params = GoloomParams::new("room-1", "peer-1", "test");
        params.ws_url = format!("ws://{addr}/join");
        params.credentials = Some("cred".to_string());
        params.max_attempts = 3;
        let mut handle = GoloomClient::spawn(params);

        let mut saw_connected = false;
        let mut saw_ping_ack = false;
        let mut saw_roster = false;
        let mut saw_terminal = false;
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(25);
        while tokio::time::Instant::now() < deadline {
            let ev = tokio::time::timeout(std::time::Duration::from_secs(5), handle.next_event())
                .await
                .expect("event STARVED — client stalled")
                .expect("event stream closed early");
            match ev {
                GoloomEvent::Connected {
                    session_secret,
                    ice_servers,
                    ..
                } => {
                    assert_eq!(session_secret, "s3");
                    assert_eq!(ice_servers.len(), 1);
                    assert_eq!(ice_servers[0].urls[0], "stun:stun.test");
                    saw_connected = true;
                }
                GoloomEvent::AckReceived { uid, code, .. } if uid == "srv-ping-1" => {
                    assert_eq!(code, StatusCode::Ok as i32);
                    saw_ping_ack = true;
                }
                GoloomEvent::RosterUpsert(list) => {
                    assert_eq!(list.len(), 1);
                    assert_eq!(list[0].display_name(), Some("Alice"));
                    saw_roster = true;
                }
                GoloomEvent::Disconnected { will_retry, .. } => {
                    assert!(!will_retry, "4009 must be terminal");
                    saw_terminal = true;
                    break;
                }
                _ => {}
            }
        }
        assert!(saw_connected && saw_ping_ack && saw_roster && saw_terminal);
        assert!(matches!(handle.state().await, GoloomState::Closed { .. }));
        server.await.expect("server task");
    }

    /// Transport drop (no close frame) → reconnect with a fresh Hello.
    #[tokio::test]
    async fn loopback_reconnect_after_drop() {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("addr");
        let conns = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let conns_srv = conns.clone();

        let server = tokio::spawn(async move {
            for _ in 0..2 {
                let (stream, _) = listener.accept().await.expect("accept");
                conns_srv.fetch_add(1, Ordering::SeqCst);
                let ws = tokio_tungstenite::accept_async(stream).await.expect("ws");
                let (mut write, mut read) = ws.split();
                let hello = recv_proto(&mut read).await.expect("hello");
                match hello.kind.expect("kind") {
                    MessageKind::Hello(h) => assert_eq!(h.room_id, "room-9"),
                    other => panic!("expected Hello, got {other:?}"),
                }
                if conns_srv.load(Ordering::SeqCst) == 1 {
                    // Abrupt drop: no close frame, but keep listening.
                    continue;
                }
                send_proto(&mut write, &server_hello_msg()).await;
                tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                write
                    .send(Message::Close(Some(CloseFrame {
                        code: 4009u16.into(),
                        reason: "over".into(),
                    })))
                    .await
                    .ok();
            }
        });

        let mut params = GoloomParams::new("room-9", "peer-9", "test");
        params.ws_url = format!("ws://{addr}/join");
        params.max_attempts = 5;
        let mut handle = GoloomClient::spawn(params);

        let mut saw_retry_disconnect = false;
        let mut saw_reconnected = false;
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(25);
        while tokio::time::Instant::now() < deadline {
            let ev = tokio::time::timeout(std::time::Duration::from_secs(8), handle.next_event())
                .await
                .expect("event STARVED")
                .expect("stream closed early");
            match ev {
                GoloomEvent::Disconnected { will_retry, .. } if will_retry => {
                    saw_retry_disconnect = true;
                }
                GoloomEvent::Connected { .. } => {
                    saw_reconnected = true;
                }
                GoloomEvent::Disconnected { will_retry, .. } if !will_retry => break,
                _ => {}
            }
        }
        assert_eq!(conns.load(Ordering::SeqCst), 2, "must reconnect once");
        assert!(saw_retry_disconnect && saw_reconnected);
        server.await.expect("server task");
    }
}
