//! Media engine abstraction for calls.
//!
//! The Goloom protocol splits a call into two directions with separate SDP
//! exchanges (`PublisherSdpOffer/Answer`, `SubscriberSdpOffer/Answer`) and
//! ICE targets — mirroring two server-side PeerConnections. The client
//! mirrors that with **two engines** (see [`EngineRole`]), so glare
//! (simultaneous offers) cannot happen by construction:
//! - the publisher only ever offers, the subscriber only ever answers;
//! - renegotiation from our side is never needed (mute keeps the m-lines).

use tokio::sync::mpsc;

/// Which half of the call an engine handles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineRole {
    /// Local capture → offer → `PublisherSdpOffer`; applies remote answers.
    Publisher,
    /// Applies remote `SubscriberSdpOffer`s → answers; never offers.
    Subscriber,
}

/// Audio or video local track advertised to the server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaTrack {
    /// Transceiver mid once negotiated; `None` on the first offer.
    pub mid: Option<String>,
    pub kind: MediaKind,
    /// Arbitrary label (mirrors `PublisherTrackDescription.label`).
    pub label: String,
    /// Track hints, e.g. `t:{track_id},tr:{transceiver_id},d:{device_id}`.
    pub description: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaKind {
    Audio,
    Video,
    /// Screen share (wire: `DISPLAY_VIDEO`).
    Screen,
}

/// Peer-connection state for the UI. Engines map their native state here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeerState {
    New,
    Connecting,
    Connected,
    Disconnected,
    Failed,
    Closed,
}

/// One decoded remote video frame (RGBA) for UI rendering.
/// Produced by `RemoteSink::Frames`; the UI maps it to a texture
/// (e.g. `gdk::MemoryTexture`), so this type stays toolkit-free.
#[derive(Debug, Clone)]
pub struct VideoFrame {
    pub width: u32,
    pub height: u32,
    /// Bytes per row (`width * 4` when tightly packed).
    pub stride: usize,
    /// RGBA pixels, `stride * height` bytes.
    pub pixels: Vec<u8>,
}

impl VideoFrame {
    pub fn is_valid(&self) -> bool {
        self.width > 0 && self.height > 0 && self.pixels.len() == self.stride * self.height as usize
    }
}

/// Events an engine pushes to the call controller.
#[derive(Debug, Clone)]
pub enum MediaEvent {
    LocalOffer {
        sdp: String,
        tracks: Vec<MediaTrack>,
    },
    LocalAnswer {
        sdp: String,
    },
    LocalIce {
        mline_index: u32,
        candidate: String,
    },
    /// Decoded remote video frame (only with `RemoteSink::Frames`).
    RemoteFrame(VideoFrame),
    /// Local camera preview frame (publisher only).
    PreviewFrame(VideoFrame),
    ConnectionState(PeerState),
    Error(String),
}

/// ICE server in engine terms (converted from signaling by the controller).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IceServer {
    pub urls: Vec<String>,
    pub username: String,
    pub credential: String,
}

/// Screen-share video source for [`MediaEngine::attach_share_source`].
#[derive(Debug)]
pub enum ShareSource {
    /// Engine default (`videotestsrc` for tests, `ximagesrc` otherwise).
    Default,
    /// Portal-picked PipeWire stream. The engine keeps the fd alive for the
    /// call; pass ownership here.
    PipeWire { fd: std::os::fd::OwnedFd, node: u32 },
}

/// WebRTC backend. All methods are non-blocking: results arrive as
/// [`MediaEvent`]s on the sink given to the constructor / `set_event_sink`.
/// Object-safe so the controller can hold `Box<dyn MediaEngine>`.
pub trait MediaEngine: Send {
    fn set_event_sink(&mut self, tx: mpsc::UnboundedSender<MediaEvent>);
    /// Build the pipeline and start producing (emits `LocalOffer`).
    fn start(&mut self) -> Result<(), String>;
    fn stop(&mut self);
    /// ICE servers (STUN/TURN). Called before `start` when known; engines
    /// apply late arrivals best-effort (gathering restart is a follow-up).
    fn set_ice_servers(&mut self, servers: Vec<IceServer>) -> Result<(), String>;
    /// Remote offer (from `SubscriberSdpOffer`) → eventually `LocalAnswer`.
    fn handle_remote_offer(&mut self, sdp: &str) -> Result<(), String>;
    /// Remote answer (to our `PublisherSdpOffer`).
    fn handle_remote_answer(&mut self, sdp: &str) -> Result<(), String>;
    fn add_remote_ice(&mut self, mline_index: u32, candidate: &str) -> Result<(), String>;
    fn set_audio_enabled(&mut self, enabled: bool) -> Result<(), String>;
    fn set_video_enabled(&mut self, enabled: bool) -> Result<(), String>;
    /// Attach (or replace) the screen-share source. Idempotent setup step;
    /// use [`MediaEngine::set_sharing_enabled`] to start/stop the flow.
    fn attach_share_source(&mut self, source: ShareSource) -> Result<(), String>;
    /// Screen share on/off (publisher only; subscriber: noop `Ok`).
    /// Design: the share m-line is negotiated up-front and stays blocked
    /// until enabled — no renegotiation, peers learn via `update_me`.
    fn set_sharing_enabled(&mut self, enabled: bool) -> Result<(), String>;
}

/// Test double: no system dependencies, scripted SDP, full call log.
///
/// - `start()` emits a canned `LocalOffer` (audio + video tracks);
/// - `handle_remote_offer()` emits a canned `LocalAnswer` synchronously;
/// - everything inbound is recorded for assertions.
pub struct NullMediaEngine {
    tx: Option<mpsc::UnboundedSender<MediaEvent>>,
    pub offer_sdp: String,
    pub answer_sdp: String,
    pub log: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
    /// Answer-only engines (subscriber role) stay quiet on `start()`.
    pub offer_on_start: bool,
    started: bool,
}

impl NullMediaEngine {
    pub fn new(tx: mpsc::UnboundedSender<MediaEvent>) -> Self {
        Self {
            tx: Some(tx),
            offer_sdp: "v=0\r\no=null-null 0 0 IN IP4 127.0.0.1\r\ns=null\r\n".to_string(),
            answer_sdp: "v=0\r\no=null-null 0 0 IN IP4 127.0.0.1\r\ns=null-answer\r\n".to_string(),
            log: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
            offer_on_start: true,
            started: false,
        }
    }

    /// Subscriber stand-in: answers offers, never offers first.
    pub fn answer_only(tx: mpsc::UnboundedSender<MediaEvent>) -> Self {
        let mut engine = Self::new(tx);
        engine.offer_on_start = false;
        engine
    }

    fn emit(&self, ev: MediaEvent) {
        if let Some(ref tx) = self.tx {
            let _ = tx.send(ev);
        }
    }

    fn record(&self, entry: String) {
        if let Ok(mut log) = self.log.lock() {
            log.push(entry);
        }
    }

    pub fn recorded(&self) -> Vec<String> {
        self.log.lock().map(|l| l.clone()).unwrap_or_default()
    }
}

impl MediaEngine for NullMediaEngine {
    fn set_event_sink(&mut self, tx: mpsc::UnboundedSender<MediaEvent>) {
        self.tx = Some(tx);
    }

    fn start(&mut self) -> Result<(), String> {
        self.started = true;
        self.record("start".to_string());
        if self.offer_on_start {
            self.emit(MediaEvent::LocalOffer {
                sdp: self.offer_sdp.clone(),
                tracks: vec![
                    MediaTrack {
                        mid: None,
                        kind: MediaKind::Audio,
                        label: "audio".to_string(),
                        description: String::new(),
                    },
                    MediaTrack {
                        mid: None,
                        kind: MediaKind::Video,
                        label: "video".to_string(),
                        description: String::new(),
                    },
                    MediaTrack {
                        mid: None,
                        kind: MediaKind::Screen,
                        label: "screen".to_string(),
                        description: String::new(),
                    },
                ],
            });
        }
        self.emit(MediaEvent::ConnectionState(PeerState::Connecting));
        Ok(())
    }

    fn stop(&mut self) {
        self.record("stop".to_string());
        self.emit(MediaEvent::ConnectionState(PeerState::Closed));
    }

    fn set_ice_servers(&mut self, servers: Vec<IceServer>) -> Result<(), String> {
        self.record(format!("ice-servers:{}", servers.len()));
        Ok(())
    }

    fn handle_remote_offer(&mut self, sdp: &str) -> Result<(), String> {
        self.record(format!("remote-offer:{}", first_line(sdp)));
        self.emit(MediaEvent::LocalAnswer {
            sdp: self.answer_sdp.clone(),
        });
        self.emit(MediaEvent::ConnectionState(PeerState::Connected));
        Ok(())
    }

    fn handle_remote_answer(&mut self, sdp: &str) -> Result<(), String> {
        self.record(format!("remote-answer:{}", first_line(sdp)));
        self.emit(MediaEvent::ConnectionState(PeerState::Connected));
        Ok(())
    }

    fn add_remote_ice(&mut self, mline_index: u32, candidate: &str) -> Result<(), String> {
        self.record(format!("remote-ice:{mline_index}:{candidate}"));
        Ok(())
    }

    fn set_audio_enabled(&mut self, enabled: bool) -> Result<(), String> {
        self.record(format!("audio:{enabled}"));
        Ok(())
    }

    fn set_video_enabled(&mut self, enabled: bool) -> Result<(), String> {
        self.record(format!("video:{enabled}"));
        Ok(())
    }

    fn set_sharing_enabled(&mut self, enabled: bool) -> Result<(), String> {
        self.record(format!("sharing:{enabled}"));
        Ok(())
    }

    fn attach_share_source(&mut self, source: ShareSource) -> Result<(), String> {
        let name = match source {
            ShareSource::Default => "default",
            ShareSource::PipeWire { node, .. } => {
                self.record(format!("share-pipewire:{node}"));
                return Ok(());
            }
        };
        self.record(format!("share-source:{name}"));
        Ok(())
    }
}

fn first_line(sdp: &str) -> &str {
    sdp.lines().next().unwrap_or("")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn null_engine_contract() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let mut engine = NullMediaEngine::new(tx);
        engine.start().expect("start");
        match rx.try_recv().expect("offer") {
            MediaEvent::LocalOffer { tracks, .. } => assert_eq!(tracks.len(), 3),
            other => panic!("expected offer, got {other:?}"),
        }
        engine
            .handle_remote_offer("v=0\r\no=remote")
            .expect("offer");
        // handle_remote_offer emits LocalAnswer (+ Connected state).
        let mut saw_answer = false;
        for _ in 0..4 {
            match rx.try_recv().expect("answer") {
                MediaEvent::LocalAnswer { .. } => {
                    saw_answer = true;
                    break;
                }
                MediaEvent::ConnectionState(_) => continue,
                other => panic!("expected answer, got {other:?}"),
            }
        }
        assert!(saw_answer, "no LocalAnswer emitted");
        engine
            .add_remote_ice(0, "candidate:1 1 udp 1 127.0.0.1 9 typ host")
            .expect("ice");
        engine.set_sharing_enabled(true).expect("share");
        let log = engine.recorded();
        assert!(log.iter().any(|l| l == "start"));
        assert!(log.iter().any(|l| l.starts_with("remote-offer:")));
        assert!(log.iter().any(|l| l.starts_with("remote-ice:0:")));
        assert!(log.iter().any(|l| l == "sharing:true"));
    }
}
