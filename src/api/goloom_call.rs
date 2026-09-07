//! Call controller: binds Goloom signaling to media engines.
//!
//! The protocol splits a call into two directions (see
//! [`EngineRole`](super::goloom_media::EngineRole)), so a call owns two
//! engines: `publish` (local capture → `PublisherSdpOffer`) and `subscribe`
//! (remote `SubscriberSdpOffer` → answers). Wiring:
//! - publish [`MediaEvent::LocalOffer`](super::goloom_media::MediaEvent) →
//!   `PublisherSdpOffer` on the wire; server `PublisherSdpAnswer` routes back
//!   to the publish engine;
//! - server `SubscriberSdpOffer` → subscribe engine → `LocalAnswer` →
//!   `SubscriberSdpAnswer` with the **same** `pc_seq`;
//! - ICE trickles per direction; outgoing candidates are tagged by the engine
//!   they came from (`PUBLISHER` / `SUBSCRIBER`), incoming ones are routed by
//!   their tag (`mline_index` carries the intra-PC routing).
//!
//! Glare is impossible by construction: publishers never answer, subscribers
//! never offer, and renegotiation from our side is never needed (mute keeps
//! the m-lines via pad probes + `update_me` signaling).

use std::collections::HashMap;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use tokio::sync::mpsc;

use super::goloom::{self, IceTarget, MediaTrackKind, PublisherTrackTransport};
use super::goloom_client::{GoloomClient, GoloomEvent, GoloomHandle, GoloomParams, IceServerInfo};
use super::goloom_media::{IceServer, MediaEngine, MediaEvent, MediaTrack, PeerState, VideoFrame};
use crate::models::telemost::TelemostParticipant;

/// Parameters for one call. `room_id` + `credentials` come from the Telemost
/// Cloud API (`create_personal_meeting` / `start_meeting_call`); the REST →
/// Goloom handoff is the caller's job (see `docs/apk-3.12.0.138/`).
#[derive(Debug, Clone)]
pub struct CallParams {
    pub room_id: String,
    pub participant_id: String,
    pub credentials: Option<String>,
    pub display_name: Option<String>,
    pub send_audio: bool,
    pub send_video: bool,
    pub send_sharing: bool,
    pub app_version: String,
    pub oauth_token: Option<String>,
    pub cookies: Option<String>,
    /// Test override for the signaling URL.
    pub ws_url: Option<String>,
}

impl CallParams {
    /// Build call params from a joined meeting. `app_version` should be the
    /// client version (sent as `SdkInfo`); `display_name` is shown to peers.
    pub fn from_meeting(
        meeting_id: &str,
        call: &crate::models::telemost::MeetingCall,
        display_name: Option<String>,
        app_version: &str,
    ) -> Self {
        Self {
            room_id: call.effective_room_id(meeting_id).to_string(),
            participant_id: call.participant_id.clone().unwrap_or_else(|| {
                format!(
                    "desktop-{}",
                    &uuid::Uuid::new_v4().simple().to_string()[..8]
                )
            }),
            credentials: call.credentials.clone(),
            display_name,
            send_audio: true,
            send_video: true,
            send_sharing: false,
            app_version: app_version.to_string(),
            oauth_token: None,
            cookies: None,
            ws_url: None,
        }
    }

    fn goloom_params(&self) -> GoloomParams {
        let mut p = GoloomParams::new(&self.room_id, &self.participant_id, &self.app_version);
        p.credentials = self.credentials.clone();
        p.display_name = self.display_name.clone();
        p.send_audio = self.send_audio;
        p.send_video = self.send_video;
        p.send_sharing = self.send_sharing;
        p.oauth_token = self.oauth_token.clone();
        p.cookies = self.cookies.clone();
        if let Some(ref url) = self.ws_url {
            p.ws_url = url.clone();
        }
        p
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CallState {
    Joining,
    Joined,
    InCall,
    Reconnecting,
    Ended { reason: String },
    Failed { reason: String },
}

/// UI-facing call events.
#[derive(Debug, Clone)]
pub enum CallEvent {
    State(CallState),
    Roster(Vec<TelemostParticipant>),
    Media(PeerState),
    /// Decoded remote video frame for the call window.
    RemoteFrame(VideoFrame),
    /// Local camera preview frame for the call window.
    PreviewFrame(VideoFrame),
    Error(String),
}

#[derive(Debug, Clone)]
enum CallCommand {
    MuteAudio(bool),
    MuteVideo(bool),
    Share(bool),
    End,
}

/// UI handle to a live call.
pub struct CallHandle {
    events_rx: mpsc::UnboundedReceiver<CallEvent>,
    cmd_tx: mpsc::UnboundedSender<CallCommand>,
    shutdown: Arc<AtomicBool>,
}

/// Cheap-clone command endpoint for UI buttons (the event stream stays with
/// [`CallHandle`], so there is no `RefCell` sharing hazard).
#[derive(Debug, Clone)]
pub struct CallControl {
    cmd: mpsc::UnboundedSender<CallCommand>,
}

impl CallControl {
    pub fn mute_audio(&self, enabled: bool) -> Result<(), String> {
        self.cmd
            .send(CallCommand::MuteAudio(enabled))
            .map_err(|e| format!("call task gone: {e}"))
    }

    pub fn mute_video(&self, enabled: bool) -> Result<(), String> {
        self.cmd
            .send(CallCommand::MuteVideo(enabled))
            .map_err(|e| format!("call task gone: {e}"))
    }

    pub fn share(&self, enabled: bool) -> Result<(), String> {
        self.cmd
            .send(CallCommand::Share(enabled))
            .map_err(|e| format!("call task gone: {e}"))
    }

    pub fn end(&self) -> Result<(), String> {
        self.cmd
            .send(CallCommand::End)
            .map_err(|e| format!("call task gone: {e}"))
    }
}

impl CallHandle {
    pub async fn next_event(&mut self) -> Option<CallEvent> {
        self.events_rx.recv().await
    }

    pub fn mute_audio(&self, enabled: bool) -> Result<(), String> {
        self.control().mute_audio(enabled)
    }

    pub fn mute_video(&self, enabled: bool) -> Result<(), String> {
        self.control().mute_video(enabled)
    }

    pub fn share(&self, enabled: bool) -> Result<(), String> {
        self.control().share(enabled)
    }

    pub fn end(&self) -> Result<(), String> {
        self.control().end()
    }

    pub fn control(&self) -> CallControl {
        CallControl {
            cmd: self.cmd_tx.clone(),
        }
    }

    pub fn shutdown(&self) {
        self.shutdown.store(true, Ordering::Relaxed);
    }
}

pub struct CallController;

impl CallController {
    /// Spawn signaling + media wiring. Returns immediately; engines start
    /// producing once signaling connects (ICE servers first).
    pub fn spawn(
        params: CallParams,
        publish: Box<dyn MediaEngine>,
        subscribe: Box<dyn MediaEngine>,
    ) -> CallHandle {
        let (events_tx, events_rx) = mpsc::unbounded_channel();
        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
        let shutdown = Arc::new(AtomicBool::new(false));

        let task = CallTask {
            params,
            publish,
            subscribe,
            goloom: None,
            events: events_tx,
            cmd_rx,
            pub_rx: None,
            sub_rx: None,
            shutdown: shutdown.clone(),
            state: CallState::Joining,
            signaling_up: false,
            media_connected: false,
            engine_started: false,
            roster: HashMap::new(),
            pc_seq: 0,
            pending_answer_seq: None,
            audio: true,
            video: true,
            sharing: false,
            share_attached: false,
        };
        tokio::spawn(async move { task.run().await });

        CallHandle {
            events_rx,
            cmd_tx,
            shutdown,
        }
    }
}

struct CallTask {
    params: CallParams,
    publish: Box<dyn MediaEngine>,
    subscribe: Box<dyn MediaEngine>,
    goloom: Option<GoloomHandle>,
    events: mpsc::UnboundedSender<CallEvent>,
    cmd_rx: mpsc::UnboundedReceiver<CallCommand>,
    pub_rx: Option<mpsc::UnboundedReceiver<MediaEvent>>,
    sub_rx: Option<mpsc::UnboundedReceiver<MediaEvent>>,
    shutdown: Arc<AtomicBool>,
    state: CallState,
    signaling_up: bool,
    media_connected: bool,
    engine_started: bool,
    roster: HashMap<String, TelemostParticipant>,
    pc_seq: u32,
    pending_answer_seq: Option<u32>,
    audio: bool,
    video: bool,
    sharing: bool,
    share_attached: bool,
}

impl CallTask {
    fn emit(&self, ev: CallEvent) {
        let _ = self.events.send(ev);
    }

    fn set_state(&mut self, s: CallState) {
        if self.state != s {
            self.state = s.clone();
            self.emit(CallEvent::State(s));
        }
    }

    fn emit_roster(&self) {
        let mut list: Vec<TelemostParticipant> = self.roster.values().cloned().collect();
        list.sort_by(|a, b| a.id.cmp(&b.id));
        self.emit(CallEvent::Roster(list));
    }

    async fn run(mut self) {
        self.audio = self.params.send_audio;
        self.video = self.params.send_video;
        self.sharing = self.params.send_sharing;
        self.emit(CallEvent::State(CallState::Joining));

        let goloom = GoloomClient::spawn(self.params.goloom_params());
        self.goloom = Some(goloom);

        let (pub_tx, pub_rx) = mpsc::unbounded_channel();
        let (sub_tx, sub_rx) = mpsc::unbounded_channel();
        self.publish.set_event_sink(pub_tx);
        self.subscribe.set_event_sink(sub_tx);
        self.pub_rx = Some(pub_rx);
        self.sub_rx = Some(sub_rx);
        // NOTE: engines start lazily on the first Connected, so the
        // server's ICE servers are applied before gathering begins.

        loop {
            if self.shutdown.load(Ordering::Relaxed) {
                self.finish(CallState::Ended {
                    reason: "shutdown".into(),
                });
                return;
            }
            let goloom_handle = match self.goloom.as_mut() {
                Some(h) => h,
                None => {
                    self.finish(CallState::Ended {
                        reason: "signaling gone".into(),
                    });
                    return;
                }
            };
            let media_pub = match self.pub_rx.as_mut() {
                Some(rx) => rx,
                None => {
                    self.finish(CallState::Failed {
                        reason: "media channel gone".into(),
                    });
                    return;
                }
            };
            let media_sub = match self.sub_rx.as_mut() {
                Some(rx) => rx,
                None => {
                    self.finish(CallState::Failed {
                        reason: "media channel gone".into(),
                    });
                    return;
                }
            };

            tokio::select! {
                biased;

                cmd = self.cmd_rx.recv() => {
                    match cmd {
                        Some(CallCommand::MuteAudio(enabled)) => {
                            self.audio = enabled;
                            if self.engine_started {
                                let r = self.publish.set_audio_enabled(enabled);
                                Self::engine_result(&self.events, r);
                            }
                            self.send_update_me();
                        }
                        Some(CallCommand::MuteVideo(enabled)) => {
                            self.video = enabled;
                            if self.engine_started {
                                let r = self.publish.set_video_enabled(enabled);
                                Self::engine_result(&self.events, r);
                            }
                            self.send_update_me();
                        }
                        Some(CallCommand::Share(enabled)) => {
                            self.sharing = enabled;
                            if enabled && !self.share_attached {
                                // First enable wires a source: portal picker on
                                // Wayland (user dialog), plain default path
                                // everywhere else. Re-toggles reuse it.
                                #[cfg(feature = "portal")]
                                if crate::api::portal_share::portal_recommended() {
                                    match crate::api::portal_share::pick_screen().await {
                                        Ok(stream) => {
                                            let r = self.publish.attach_share_source(
                                                crate::api::goloom_media::ShareSource::PipeWire {
                                                    fd: stream.fd,
                                                    node: stream.node,
                                                },
                                            );
                                            Self::engine_result(&self.events, r);
                                        }
                                        Err(e) => {
                                            self.emit(CallEvent::Error(format!(
                                                "screen picker: {e} (falling back)"
                                            )));
                                        }
                                    }
                                }
                                self.share_attached = true;
                            }
                            if self.engine_started {
                                let r = self.publish.set_sharing_enabled(enabled);
                                Self::engine_result(&self.events, r);
                            }
                            self.send_update_me();
                        }
                        Some(CallCommand::End) | None => {
                            self.finish(CallState::Ended { reason: "hangup".into() });
                            return;
                        }
                    }
                }

                ev = goloom_handle.next_event() => {
                    match ev {
                        None => {
                            self.finish(CallState::Ended { reason: "signaling closed".into() });
                            return;
                        }
                        Some(e) => {
                            if self.on_goloom(e) {
                                return;
                            }
                        }
                    }
                }

                ev = media_pub.recv() => {
                    match ev {
                        None => {
                            self.finish(CallState::Failed { reason: "publish engine gone".into() });
                            return;
                        }
                        Some(e) => self.on_publish(e),
                    }
                }

                ev = media_sub.recv() => {
                    match ev {
                        None => {
                            self.finish(CallState::Failed { reason: "subscribe engine gone".into() });
                            return;
                        }
                        Some(e) => self.on_subscribe(e),
                    }
                }
            }
        }
    }

    /// Returns `true` when the task must exit.
    fn on_goloom(&mut self, ev: GoloomEvent) -> bool {
        match ev {
            GoloomEvent::Connected {
                ice_servers,
                capabilities: _,
                session_secret: _,
                ping_interval_secs: _,
                ack_timeout_secs: _,
            } => {
                self.signaling_up = true;
                if !self.engine_started {
                    self.engine_started = true;
                    let servers: Vec<IceServer> = ice_servers.iter().map(ice_to_media).collect();
                    let r = self.publish.set_ice_servers(servers.clone());
                    Self::engine_result(&self.events, r);
                    let r = self.subscribe.set_ice_servers(servers);
                    Self::engine_result(&self.events, r);
                    if let Err(e) = self.publish.start() {
                        let reason = format!("publish engine failed to start: {e}");
                        self.shutdown_signaling();
                        self.set_state(CallState::Failed { reason });
                        return true;
                    }
                    if let Err(e) = self.subscribe.start() {
                        let reason = format!("subscribe engine failed to start: {e}");
                        self.shutdown_signaling();
                        self.set_state(CallState::Failed { reason });
                        return true;
                    }
                }
                self.set_state(CallState::Joined);
                if self.media_connected {
                    self.set_state(CallState::InCall);
                }
            }
            GoloomEvent::Disconnected { will_retry, reason } => {
                self.signaling_up = false;
                if will_retry {
                    self.set_state(CallState::Reconnecting);
                } else {
                    self.finish(CallState::Ended { reason });
                    return true;
                }
            }
            GoloomEvent::RosterUpsert(list) => {
                for desc in list {
                    let p = goloom::to_telemost_participant(&desc);
                    self.roster.insert(p.id.clone(), p);
                }
                self.emit_roster();
            }
            GoloomEvent::RosterRemove(ids) => {
                for id in ids {
                    self.roster.remove(&id);
                }
                self.emit_roster();
            }
            GoloomEvent::SubscriberOffer { pc_seq, sdp } => {
                self.pending_answer_seq = Some(pc_seq);
                let r = self.subscribe.handle_remote_offer(&sdp);
                Self::engine_result(&self.events, r);
            }
            GoloomEvent::PublisherAnswer { pc_seq, sdp } => {
                let r = self.publish.handle_remote_answer(&sdp);
                Self::engine_result(&self.events, r);
                let _ = pc_seq;
            }
            GoloomEvent::IceCandidate(c) => {
                let mline = c.sdp_mline_index.unwrap_or(0);
                let target = c.target;
                let engine = if target == goloom::IceTarget::Subscriber as i32 {
                    &mut self.subscribe
                } else {
                    &mut self.publish
                };
                let r = engine.add_remote_ice(mline, &c.candidate);
                Self::engine_result(&self.events, r);
            }
            GoloomEvent::ServerError {
                uid,
                code,
                description,
            } => {
                self.emit(CallEvent::Error(format!(
                    "server rejected {uid} (code {code}): {description}"
                )));
            }
            GoloomEvent::ActiveCodecs { .. }
            | GoloomEvent::AckReceived { .. }
            | GoloomEvent::StateChanged(_)
            | GoloomEvent::ReconnectRequested { .. }
            | GoloomEvent::Slots(_)
            | GoloomEvent::SlotsOffset(_)
            | GoloomEvent::SlotsMeta(_)
            | GoloomEvent::SlotsRules(_)
            | GoloomEvent::MeUpdated(_)
            | GoloomEvent::PinnedRequested(_)
            | GoloomEvent::Quality { .. }
            | GoloomEvent::Vad(_)
            | GoloomEvent::ServerNotification { .. } => {}
        }
        false
    }

    /// Publish-engine events: offers (+ its ICE) go upstream.
    fn on_publish(&mut self, ev: MediaEvent) {
        match ev {
            MediaEvent::LocalOffer { sdp, tracks } => {
                self.pc_seq = self.pc_seq.wrapping_add(1);
                let seq = self.pc_seq;
                let proto_tracks = tracks.iter().map(track_to_proto).collect();
                if let Some(g) = self.goloom.as_ref() {
                    if let Err(e) = g.send_publisher_offer(seq, &sdp, proto_tracks) {
                        self.emit(CallEvent::Error(format!("offer send failed: {e}")));
                    }
                }
            }
            MediaEvent::LocalAnswer { .. } => {
                log::warn!("goloom_call: publisher answered (protocol violation), dropping");
            }
            MediaEvent::LocalIce {
                mline_index,
                candidate,
            } => self.send_ice(IceTarget::Publisher as i32, mline_index, candidate),
            MediaEvent::ConnectionState(s) => self.on_peer_state(s),
            MediaEvent::RemoteFrame(f) => {
                self.emit(CallEvent::RemoteFrame(f));
            }
            MediaEvent::PreviewFrame(f) => {
                self.emit(CallEvent::PreviewFrame(f));
            }
            MediaEvent::Error(e) => {
                self.emit(CallEvent::Error(e));
            }
        }
    }

    /// Subscribe-engine events: answers (+ its ICE) go upstream.
    fn on_subscribe(&mut self, ev: MediaEvent) {
        match ev {
            MediaEvent::LocalOffer { .. } => {
                log::warn!("goloom_call: subscriber offered (protocol violation), dropping");
            }
            MediaEvent::LocalAnswer { sdp } => match self.pending_answer_seq.take() {
                Some(seq) => {
                    if let Some(g) = self.goloom.as_ref() {
                        if let Err(e) = g.send_subscriber_answer(seq, &sdp) {
                            self.emit(CallEvent::Error(format!("answer send failed: {e}")));
                        }
                    }
                }
                None => {
                    log::warn!("goloom_call: unsolicited local answer, dropping");
                }
            },
            MediaEvent::LocalIce {
                mline_index,
                candidate,
            } => self.send_ice(IceTarget::Subscriber as i32, mline_index, candidate),
            MediaEvent::ConnectionState(s) => self.on_peer_state(s),
            MediaEvent::RemoteFrame(f) => {
                self.emit(CallEvent::RemoteFrame(f));
            }
            MediaEvent::PreviewFrame(f) => {
                self.emit(CallEvent::PreviewFrame(f));
            }
            MediaEvent::Error(e) => {
                self.emit(CallEvent::Error(e));
            }
        }
    }

    fn send_ice(&self, target: i32, mline_index: u32, candidate: String) {
        if let Some(g) = self.goloom.as_ref() {
            let cand = goloom::WebrtcIceCandidate {
                pc_seq: self.pc_seq,
                target,
                candidate,
                sdp_mid: None,
                sdp_mline_index: Some(mline_index),
                username_fragment: None,
            };
            if let Err(e) = g.send_ice(cand) {
                self.emit(CallEvent::Error(format!("ice send failed: {e}")));
            }
        }
    }

    fn on_peer_state(&mut self, s: PeerState) {
        match s {
            PeerState::Connected => {
                self.media_connected = true;
                self.emit(CallEvent::Media(PeerState::Connected));
                if self.signaling_up {
                    self.set_state(CallState::InCall);
                }
            }
            PeerState::Failed => {
                self.finish(CallState::Failed {
                    reason: "media connection failed".into(),
                });
            }
            other => {
                self.emit(CallEvent::Media(other));
            }
        }
    }

    fn engine_result(events: &mpsc::UnboundedSender<CallEvent>, r: Result<(), String>) {
        if let Err(e) = r {
            let _ = events.send(CallEvent::Error(e));
        }
    }

    fn send_update_me(&self) {
        if let Some(g) = self.goloom.as_ref() {
            if let Err(e) = g.send_update_me(self.audio, self.video, self.sharing) {
                self.emit(CallEvent::Error(e));
            }
        }
    }

    fn shutdown_signaling(&self) {
        if let Some(g) = self.goloom.as_ref() {
            g.shutdown();
        }
    }

    fn finish(&mut self, state: CallState) {
        self.publish.stop();
        self.subscribe.stop();
        self.shutdown_signaling();
        self.set_state(state);
    }
}

fn ice_to_media(info: &IceServerInfo) -> IceServer {
    IceServer {
        urls: info.urls.clone(),
        username: info.username.clone(),
        credential: info.credential.clone(),
    }
}

fn track_to_proto(t: &MediaTrack) -> goloom::PublisherTrackDescription {
    use super::goloom_media::MediaKind;
    goloom::PublisherTrackDescription {
        transport: t.mid.clone().map(PublisherTrackTransport::TransceiverMid),
        kind: match t.kind {
            MediaKind::Audio => MediaTrackKind::Audio as i32,
            MediaKind::Video => MediaTrackKind::Video as i32,
            MediaKind::Screen => MediaTrackKind::DisplayVideo as i32,
        },
        label: t.label.clone(),
        description: t.description.clone(),
        group_id: None,
        codecs: std::collections::HashMap::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::super::goloom_media::NullMediaEngine;
    use super::*;
    use futures::StreamExt as _;
    use tokio::net::TcpListener;

    fn test_params(ws_url: Option<String>) -> CallParams {
        CallParams {
            room_id: "room-1".to_string(),
            participant_id: "me".to_string(),
            credentials: None,
            display_name: None,
            send_audio: true,
            send_video: true,
            send_sharing: false,
            app_version: "test".to_string(),
            oauth_token: None,
            cookies: None,
            ws_url,
        }
    }

    #[tokio::test]
    async fn spawn_emits_joining_immediately() {
        let (tx, _) = mpsc::unbounded_channel();
        let (tx2, _) = mpsc::unbounded_channel();
        let mut call = CallController::spawn(
            test_params(Some("ws://127.0.0.1:9/join".to_string())),
            Box::new(NullMediaEngine::new(tx)),
            Box::new(NullMediaEngine::answer_only(tx2)),
        );
        let ev = tokio::time::timeout(std::time::Duration::from_secs(2), call.next_event())
            .await
            .expect("joining STARVED")
            .expect("closed");
        assert!(
            matches!(ev, CallEvent::State(CallState::Joining)),
            "first event must be Joining, got {ev:?}"
        );
        call.shutdown();
    }

    type WsStream = tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>;

    async fn recv_proto(
        read: &mut futures::stream::SplitStream<WsStream>,
    ) -> Option<goloom::GoloomMessage> {
        let msg = tokio::time::timeout(std::time::Duration::from_secs(10), read.next())
            .await
            .ok()??;
        match msg.ok()? {
            tokio_tungstenite::tungstenite::Message::Binary(bin) => {
                goloom::decode_message(&bin).ok()
            }
            _ => None,
        }
    }

    async fn send_proto(
        write: &mut futures::stream::SplitSink<WsStream, tokio_tungstenite::tungstenite::Message>,
        msg: &goloom::GoloomMessage,
    ) {
        use futures::SinkExt;
        write
            .send(tokio_tungstenite::tungstenite::Message::Binary(
                goloom::encode_message(msg),
            ))
            .await
            .expect("server send");
    }

    /// Full mesh: offer→answer, remote offer→answer(same pc_seq), ICE, roster.
    #[tokio::test]
    async fn loopback_call_mesh() {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("addr");

        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.expect("accept");
            let ws = tokio_tungstenite::accept_async(stream).await.expect("ws");
            let (mut write, mut read) = ws.split();

            // Hello first.
            let hello = recv_proto(&mut read).await.expect("hello");
            assert!(matches!(hello.kind, Some(goloom::MessageKind::Hello(_))));

            send_proto(
                &mut write,
                &goloom::GoloomMessage {
                    uid: goloom::new_uid(),
                    kind: Some(goloom::MessageKind::ServerHello(goloom::ServerHello {
                        capabilities_answer: None,
                        serving_components: Vec::new(),
                        session_secret: "s".to_string(),
                        sfu_peer_initialization_id: "sfu".to_string(),
                        rtc_configuration: None,
                        log_endpoint: String::new(),
                        ping_pong_configuration: Some(goloom::PingPongConfiguration {
                            ping_interval: 60,
                            ack_timeout: 60,
                        }),
                        telemetry_configuration: None,
                        exclude_from_experiments: false,
                        active_codecs: None,
                    })),
                },
            )
            .await;

            // Engine's LocalOffer → PublisherSdpOffer pc_seq 1, 3 tracks.
            let offer = recv_proto(&mut read).await.expect("offer");
            match offer.kind.expect("offer kind") {
                goloom::MessageKind::PublisherSdpOffer(o) => {
                    assert_eq!(o.pc_seq, 1);
                    assert!(o.sdp.contains("null"));
                    assert_eq!(o.tracks.len(), 3);
                    assert!(o.tracks.iter().any(|t| t.label == "screen"));
                }
                other => panic!("expected PublisherSdpOffer, got {other:?}"),
            }

            // Answer it.
            send_proto(
                &mut write,
                &goloom::GoloomMessage {
                    uid: goloom::new_uid(),
                    kind: Some(goloom::MessageKind::PublisherSdpAnswer(
                        goloom::PublisherSdpAnswer {
                            pc_seq: 1,
                            sdp: "v=0\r\no=srv-ans".to_string(),
                        },
                    )),
                },
            )
            .await;

            // Remote offer pc_seq 7 → answer must carry pc_seq 7.
            send_proto(
                &mut write,
                &goloom::GoloomMessage {
                    uid: goloom::new_uid(),
                    kind: Some(goloom::MessageKind::SubscriberSdpOffer(
                        goloom::SubscriberSdpOffer {
                            pc_seq: 7,
                            sdp: "v=0\r\no=srv-off".to_string(),
                        },
                    )),
                },
            )
            .await;
            let answer = recv_proto(&mut read).await.expect("answer");
            match answer.kind.expect("answer kind") {
                goloom::MessageKind::SubscriberSdpAnswer(a) => {
                    assert_eq!(a.pc_seq, 7);
                    assert!(a.sdp.contains("null-answer"));
                }
                other => panic!("expected SubscriberSdpAnswer, got {other:?}"),
            }

            // ICE + roster pushes (no wire reply expected).
            send_proto(
                &mut write,
                &goloom::GoloomMessage {
                    uid: goloom::new_uid(),
                    kind: Some(goloom::MessageKind::IceCandidate(
                        goloom::WebrtcIceCandidate {
                            pc_seq: 1,
                            target: goloom::IceTarget::Subscriber as i32,
                            candidate: "candidate:9 1 udp 1 10.0.0.1 9 typ host".to_string(),
                            sdp_mid: None,
                            sdp_mline_index: Some(0),
                            username_fragment: None,
                        },
                    )),
                },
            )
            .await;
            let mut attrs = std::collections::HashMap::new();
            attrs.insert("name".to_string(), "Bob".to_string());
            send_proto(
                &mut write,
                &goloom::GoloomMessage {
                    uid: goloom::new_uid(),
                    kind: Some(goloom::MessageKind::UpsertDescription(
                        goloom::UpsertDescription {
                            description: vec![goloom::ParticipantDescription {
                                id: "p9".to_string(),
                                participant_attributes: attrs,
                                send_audio: true,
                                send_video: true,
                                send_sharing: false,
                                hide_from_participants_list: false,
                                disconnected_at: None,
                                network_score: 0,
                                connection_type: 0,
                                ref_participant_id: None,
                            }],
                        },
                    )),
                },
            )
            .await;

            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
            // Client enables sharing → update_me must carry send_sharing.
            let update = recv_proto(&mut read).await.expect("update_me");
            match update.kind.expect("update kind") {
                goloom::MessageKind::UpdateMe(m) => {
                    assert!(m.send_sharing, "sharing flag must be set");
                }
                other => panic!("expected UpdateMe, got {other:?}"),
            }
            use futures::SinkExt;
            write
                .send(tokio_tungstenite::tungstenite::Message::Close(Some(
                    tokio_tungstenite::tungstenite::protocol::CloseFrame {
                        code: 1000u16.into(),
                        reason: "over".into(),
                    },
                )))
                .await
                .ok();
        });

        let (media_tx, _media_rx) = mpsc::unbounded_channel();
        let publish = NullMediaEngine::new(media_tx);
        let publish_log = publish.log.clone();
        let (sub_tx, _sub_rx) = mpsc::unbounded_channel();
        let subscribe = NullMediaEngine::answer_only(sub_tx);
        let subscribe_log = subscribe.log.clone();

        let params = CallParams {
            room_id: "room-1".to_string(),
            participant_id: "me".to_string(),
            credentials: None,
            display_name: None,
            send_audio: true,
            send_video: true,
            send_sharing: false,
            app_version: "test".to_string(),
            oauth_token: None,
            cookies: None,
            ws_url: Some(format!("ws://{addr}/join")),
        };
        let mut call = CallController::spawn(params, Box::new(publish), Box::new(subscribe));

        let mut saw_joined = false;
        let mut saw_incall = false;
        let mut saw_roster = false;
        let mut saw_ended = false;
        let mut share_sent = false;
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(20);
        while tokio::time::Instant::now() < deadline {
            let ev = tokio::time::timeout(std::time::Duration::from_secs(5), call.next_event())
                .await
                .expect("event STARVED")
                .expect("stream closed early");
            match ev {
                CallEvent::State(CallState::Joined) => saw_joined = true,
                CallEvent::State(CallState::InCall) => saw_incall = true,
                CallEvent::Roster(list) => {
                    if list.iter().any(|p| p.name.as_deref() == Some("Bob")) {
                        saw_roster = true;
                        if !share_sent {
                            share_sent = true;
                            call.share(true).expect("share on");
                        }
                    }
                }
                CallEvent::State(CallState::Ended { .. }) => {
                    saw_ended = true;
                    break;
                }
                CallEvent::Error(e) => panic!("unexpected call error: {e}"),
                _ => {}
            }
        }
        assert!(saw_joined && saw_incall && saw_roster && saw_ended);

        let pub_log = publish_log.lock().expect("log").clone();
        let sub_log = subscribe_log.lock().expect("log").clone();
        assert!(pub_log.iter().any(|l| l.starts_with("remote-answer:v=0")));
        assert!(sub_log.iter().any(|l| l.starts_with("remote-offer:v=0")));
        assert!(sub_log
            .iter()
            .any(|l| l == "remote-ice:0:candidate:9 1 udp 1 10.0.0.1 9 typ host"));
        server.await.expect("server task");
    }
}
