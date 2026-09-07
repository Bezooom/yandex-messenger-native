//! Real WebRTC backend on GStreamer `webrtcbin` (feature `gstreamer`).
//!
//! - Local capture: `autoaudio/autovideosrc` (or test sources) → encode →
//!   RTP → `webrtcbin` (`bundle-policy=max-bundle`: one ICE transport, so
//!   gathered candidates serve both directions and routing is by `mline_index`).
//! - Remote: `pad-added` → `decodebin` → convert → auto sinks (or fake sinks
//!   with frame counting for tests).
//! - Signaling (`on-negotiation-needed`, `on-ice-candidate`,
//!   `create-offer/answer`, `set-local/remote-description`,
//!   `add-ice-candidate`) is bridged to [`MediaEvent`]s.
//! - Runs on its own OS thread with a private glib `MainContext`; commands
//!   arrive over `std::mpsc` polled by a glib timeout, events go out over a
//!   tokio channel. No GTK main loop involvement.
//! - Glare (simultaneous offers): polite behavior — an incoming offer rolls
//!   back our unanswered local offer, then is accepted.
//! - Mute: capture pads are probe-blocked (packets stop, m-line stays, no
//!   renegotiation); peers learn the state via `update_me` signaling.
//!
//! Honest limits: only the first STUN + first TURN from the server config are
//! applied (`webrtcbin` convenience properties); late ICE servers (after
//! gathering started) apply to future negotiations only; simulcast,
//! screen-share tracks and data channels are follow-ups.

use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc,
};

use gstreamer as gst;
use gstreamer::prelude::*;
use tokio::sync::mpsc;

use super::goloom_media::{
    EngineRole, IceServer, MediaEngine, MediaEvent, MediaKind, MediaTrack, PeerState,
};

/// Capture source. `Test` needs no hardware (CI / loopback tests).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineSource {
    Auto,
    Test,
}

/// Where decoded remote media goes. `Fake` counts buffers into [`EngineStats`];
/// `Frames` emits RGBA [`MediaEvent::RemoteFrame`]s on the event sink for
/// in-window rendering (no extra plugins needed).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoteSink {
    Auto,
    Fake,
    Frames,
}

#[derive(Debug, Clone)]
pub struct GstEngineConfig {
    pub role: EngineRole,
    pub audio: bool,
    pub video: bool,
    pub source: EngineSource,
    pub audio_sink: RemoteSink,
    pub video_sink: RemoteSink,
    pub app_version: String,
}

impl GstEngineConfig {
    pub fn test() -> Self {
        Self::test_with_role(EngineRole::Publisher)
    }

    pub fn test_with_role(role: EngineRole) -> Self {
        Self {
            role,
            audio: true,
            video: true,
            source: EngineSource::Test,
            audio_sink: RemoteSink::Fake,
            video_sink: RemoteSink::Fake,
            app_version: "test".to_string(),
        }
    }
}

/// Counters proving media actually flows (wired to `fakesink handoff`).
#[derive(Debug, Default)]
pub struct EngineStats {
    pub audio_buffers: AtomicUsize,
    pub video_buffers: AtomicUsize,
}

#[derive(Debug)]
enum Cmd {
    IceServers(Vec<IceServer>),
    RemoteOffer(String),
    RemoteAnswer(String),
    RemoteIce { mline: u32, candidate: String },
    SetEnabled { kind: MediaKind, enabled: bool },
    AttachDefaultShare,
    AttachPipeWire { fd: std::os::fd::OwnedFd, node: u32 },
    Shutdown,
}

pub struct GstMediaEngine {
    tx: Option<mpsc::UnboundedSender<MediaEvent>>,
    config: GstEngineConfig,
    pub stats: Arc<EngineStats>,
    cmd_tx: Option<std::sync::mpsc::Sender<Cmd>>,
    worker: Option<std::thread::JoinHandle<()>>,
}

impl GstMediaEngine {
    pub fn new(tx: mpsc::UnboundedSender<MediaEvent>, config: GstEngineConfig) -> Self {
        Self {
            tx: Some(tx),
            config,
            stats: Arc::new(EngineStats::default()),
            cmd_tx: None,
            worker: None,
        }
    }

    fn send_cmd(&self, cmd: Cmd) -> Result<(), String> {
        self.cmd_tx
            .as_ref()
            .ok_or_else(|| "engine not started".to_string())?
            .send(cmd)
            .map_err(|e| format!("engine gone: {e}"))
    }
}

impl MediaEngine for GstMediaEngine {
    fn set_event_sink(&mut self, tx: mpsc::UnboundedSender<MediaEvent>) {
        self.tx = Some(tx);
    }

    fn start(&mut self) -> Result<(), String> {
        let tx = self.tx.clone().ok_or_else(|| "no event sink".to_string())?;
        if self.worker.is_some() {
            return Err("already started".to_string());
        }
        let (cmd_tx, cmd_rx) = std::sync::mpsc::channel::<Cmd>();
        self.cmd_tx = Some(cmd_tx);
        let config = self.config.clone();
        let stats = self.stats.clone();
        let handle = std::thread::Builder::new()
            .name("goloom-webrtc".to_string())
            .spawn(move || Worker::run(config, stats, tx, cmd_rx))
            .map_err(|e| format!("worker spawn failed: {e}"))?;
        self.worker = Some(handle);
        Ok(())
    }

    fn stop(&mut self) {
        // Best-effort; also safe before start.
        if let Some(ref tx) = self.cmd_tx {
            let _ = tx.send(Cmd::Shutdown);
        }
        if let Some(h) = self.worker.take() {
            let _ = h.join();
        }
        self.cmd_tx = None;
    }

    fn set_ice_servers(&mut self, servers: Vec<IceServer>) -> Result<(), String> {
        self.send_cmd(Cmd::IceServers(servers))
    }

    fn handle_remote_offer(&mut self, sdp: &str) -> Result<(), String> {
        self.send_cmd(Cmd::RemoteOffer(sdp.to_string()))
    }

    fn handle_remote_answer(&mut self, sdp: &str) -> Result<(), String> {
        self.send_cmd(Cmd::RemoteAnswer(sdp.to_string()))
    }

    fn add_remote_ice(&mut self, mline_index: u32, candidate: &str) -> Result<(), String> {
        self.send_cmd(Cmd::RemoteIce {
            mline: mline_index,
            candidate: candidate.to_string(),
        })
    }

    fn set_audio_enabled(&mut self, enabled: bool) -> Result<(), String> {
        self.send_cmd(Cmd::SetEnabled {
            kind: MediaKind::Audio,
            enabled,
        })
    }

    fn set_video_enabled(&mut self, enabled: bool) -> Result<(), String> {
        self.send_cmd(Cmd::SetEnabled {
            kind: MediaKind::Video,
            enabled,
        })
    }

    fn set_sharing_enabled(&mut self, enabled: bool) -> Result<(), String> {
        self.send_cmd(Cmd::SetEnabled {
            kind: MediaKind::Screen,
            enabled,
        })
    }

    fn attach_share_source(
        &mut self,
        source: super::goloom_media::ShareSource,
    ) -> Result<(), String> {
        use super::goloom_media::ShareSource as S;
        match source {
            S::Default => self.send_cmd(Cmd::AttachDefaultShare),
            S::PipeWire { fd, node } => self.send_cmd(Cmd::AttachPipeWire { fd, node }),
        }
    }
}

impl Drop for GstMediaEngine {
    fn drop(&mut self) {
        if let Some(ref tx) = self.cmd_tx {
            let _ = tx.send(Cmd::Shutdown);
        }
        if let Some(h) = self.worker.take() {
            let _ = h.join();
        }
    }
}

// ── worker (lives on its own thread) ────────────────────────────────

/// Screen-share source attachment, shared with the command pump.
/// The queue leg is built on first need; the source element attaches after.
#[derive(Default)]
struct ShareAttach {
    element: Option<gst::Element>,
    /// Portal fd guard: kept alive while the pipewire source uses it.
    fd: Option<std::os::fd::OwnedFd>,
}

/// Mute bookkeeping shared between build (fills pads) and the command pump
/// (toggles probes). Same-thread in practice, `Mutex` for the `Send` bound.
#[derive(Default)]
struct PadMute {
    pad: Option<gst::Pad>,
    probe: Option<gst::PadProbeId>,
}

#[derive(Default)]
struct MuteState {
    audio: PadMute,
    video: PadMute,
    share: PadMute,
}

struct Worker {
    config: GstEngineConfig,
    stats: Arc<EngineStats>,
    events: mpsc::UnboundedSender<MediaEvent>,
    pipeline: gst::Pipeline,
    webrtc: gst::Element,
    main_loop: glib::MainLoop,
    mute: Arc<std::sync::Mutex<MuteState>>,
    /// Set once the share leg exists (drives screen track in re-offers).
    share_leg: Arc<AtomicBool>,
    /// appsink receiving remote RGBA frames (if `RemoteSink::Frames`).
    video_appsink: Arc<std::sync::Mutex<Option<gstreamer_app::AppSink>>>,
    /// appsink receiving local camera preview frames (publisher video leg).
    preview_appsink: Arc<std::sync::Mutex<Option<gstreamer_app::AppSink>>>,
    /// Tracks advertised in our offers (fixed at build from real branches).
    offer_tracks: Vec<MediaTrack>,
    /// Attached glib sources; dropping them would detach the timeouts.
    _pump: Option<glib::Source>,
    _housekeeping: Option<glib::Source>,
}

impl Worker {
    fn emit(&self, ev: MediaEvent) {
        let _ = self.events.send(ev);
    }

    fn run(
        config: GstEngineConfig,
        stats: Arc<EngineStats>,
        events: mpsc::UnboundedSender<MediaEvent>,
        cmd_rx: std::sync::mpsc::Receiver<Cmd>,
    ) {
        if let Err(e) = gst::init() {
            let _ = events.send(MediaEvent::Error(format!("gst init: {e}")));
            return;
        }
        let ctx = glib::MainContext::new();
        let main_loop = glib::MainLoop::new(Some(&ctx), false);
        let built = ctx.with_thread_default(|| {
            Self::build(config, stats, events, cmd_rx, main_loop.clone(), &ctx)
        });
        let mut worker = match built {
            Ok(Ok(w)) => w,
            // build() already emitted the error event.
            Ok(Err(())) | Err(_) => return,
        };
        worker.main_loop.run();
        worker.teardown();
    }

    fn build(
        config: GstEngineConfig,
        stats: Arc<EngineStats>,
        events: mpsc::UnboundedSender<MediaEvent>,
        cmd_rx: std::sync::mpsc::Receiver<Cmd>,
        main_loop: glib::MainLoop,
        ctx: &glib::MainContext,
    ) -> Result<Self, ()> {
        if !config.audio && !config.video {
            let _ = events.send(MediaEvent::Error("no media requested".to_string()));
            return Err(());
        }
        let launch = launch_string(&config);
        log::debug!("goloom webrtc pipeline: {launch}");
        // A lone webrtcbin is not a Pipeline, so the subscriber (no local
        // media) is assembled by hand; the publisher uses parse_launch for
        // its `! webrtc.` request-pad links.
        let pipeline = if config.role == EngineRole::Subscriber {
            let pipeline = gst::Pipeline::new();
            let webrtc = gst::ElementFactory::make("webrtcbin")
                .property("name", "webrtc")
                .property(
                    "bundle-policy",
                    gstreamer_webrtc::WebRTCBundlePolicy::MaxBundle,
                )
                .build()
                .map_err(|e| {
                    let _ = events.send(MediaEvent::Error(format!("webrtcbin: {e}")));
                })?;
            pipeline.add(&webrtc).map_err(|e| {
                let _ = events.send(MediaEvent::Error(format!("pipeline add: {e}")));
            })?;
            pipeline
        } else {
            gst::parse::launch(&launch)
                .map_err(|e| {
                    let _ = events.send(MediaEvent::Error(format!("pipeline parse: {e}")));
                })?
                .downcast::<gst::Pipeline>()
                .map_err(|_| {
                    let _ = events.send(MediaEvent::Error("not a pipeline".to_string()));
                })?
        };
        let webrtc = pipeline.by_name("webrtc").ok_or_else(|| {
            let _ = events.send(MediaEvent::Error("no webrtcbin".to_string()));
        })?;

        // Subscriber PCs carry no local media: pre-create RECVONLY
        // transceivers so the server's offers have m-lines to fill.
        // (The publisher gets its transceivers implicitly from linked pads.)
        if config.role == EngineRole::Subscriber {
            if config.audio {
                add_recv_transceiver(&webrtc, &events, AUDIO_RTP_CAPS)?;
            }
            if config.video {
                add_recv_transceiver(&webrtc, &events, VIDEO_RTP_CAPS)?;
            }
        }

        let mute = Arc::new(std::sync::Mutex::new(MuteState {
            audio: PadMute {
                // No preview leg on audio: block at the source.
                pad: src_pad(&pipeline, "audiosrc", "src"),
                probe: None,
            },
            video: PadMute {
                // Post-tee: muting keeps the local preview alive.
                pad: src_pad(&pipeline, "qenc", "sink"),
                probe: None,
            },
            share: PadMute {
                // Queue leg always exists; the source attaches on demand.
                pad: src_pad(&pipeline, "qshare", "sink"),
                probe: None,
            },
        }));
        let share_attach = Arc::new(std::sync::Mutex::new(ShareAttach::default()));
        let share_leg = Arc::new(AtomicBool::new(false));
        // Share starts unblocked here but is blocked on the first offer
        // (blocking pre-PLAYING stalls negotiation); see create_offer.
        let video_appsink = Arc::new(std::sync::Mutex::new(None));
        let preview_appsink = Arc::new(std::sync::Mutex::new(None));
        // Preview appsink is named in the launch string; grab it directly.
        if config.role == EngineRole::Publisher && config.video {
            if let Some(preview) = pipeline
                .by_name("preview")
                .and_then(|e| e.downcast::<gstreamer_app::AppSink>().ok())
            {
                *preview_appsink.lock().expect("preview store") = Some(preview);
            }
        }

        let mut worker = Self {
            config,
            stats,
            events,
            pipeline,
            webrtc,
            main_loop,
            mute,
            share_leg,
            video_appsink,
            preview_appsink,
            offer_tracks: Vec::new(),
            // NOTE: `glib::timeout_add` attaches to the *global default*
            // context, which nobody runs here — attach explicitly instead.
            _pump: None,
            _housekeeping: None,
        };
        // Offer tracks mirror the legs actually built (mids arrive later).
        // Screen joins dynamically once its leg exists (see share_leg).
        if worker.config.role == EngineRole::Publisher {
            if worker.config.audio {
                worker.offer_tracks.push(MediaTrack {
                    mid: None,
                    kind: MediaKind::Audio,
                    label: "audio".to_string(),
                    description: String::new(),
                });
            }
            if worker.config.video {
                worker.offer_tracks.push(MediaTrack {
                    mid: None,
                    kind: MediaKind::Video,
                    label: "video".to_string(),
                    description: String::new(),
                });
            }
        }
        worker.attach_signals();
        let pump = worker.command_pump_source(cmd_rx, share_attach);
        pump.attach(Some(ctx));
        let housekeeping = worker.housekeeping_source();
        housekeeping.attach(Some(ctx));
        worker._pump = Some(pump);
        worker._housekeeping = Some(housekeeping);

        if worker.pipeline.set_state(gst::State::Playing).is_err() {
            worker.emit(MediaEvent::Error("play failed".to_string()));
            return Err(());
        }
        Ok(worker)
    }

    fn teardown(&mut self) {
        let _ = self.pipeline.set_state(gst::State::Null);
    }

    fn attach_signals(&self) {
        // Local offers (publisher only — the subscriber never offers).
        let webrtc = self.webrtc.clone();
        let events = self.events.clone();
        let role = self.config.role;
        let tracks = self.offer_tracks.clone();
        let share_leg = self.share_leg.clone();
        let _mute = self.mute.clone();
        self.webrtc
            .connect("on-negotiation-needed", false, move |_| {
                if role == EngineRole::Publisher {
                    create_offer(&webrtc, &events, tracks.clone(), &share_leg);
                }
                None
            });

        // Local ICE.
        let events = self.events.clone();
        self.webrtc
            .connect("on-ice-candidate", false, move |values| {
                let mline: u32 = values.get(1).and_then(|v| v.get().ok()).unwrap_or(0);
                let cand: String = values.get(2).and_then(|v| v.get().ok()).unwrap_or_default();
                if !cand.is_empty() {
                    let _ = events.send(MediaEvent::LocalIce {
                        mline_index: mline,
                        candidate: cand,
                    });
                }
                None
            });

        // Remote media.
        let pipeline = self.pipeline.clone();
        let stats = self.stats.clone();
        let events = self.events.clone();
        let audio_sink = self.config.audio_sink.clone();
        let video_sink = self.config.video_sink.clone();
        let frame_store = self.video_appsink.clone();
        self.webrtc.connect_pad_added(move |_, pad| {
            // webrtcbin also reports its own request *sink* pads (e.g. the
            // share leg we just linked) — only remote *src* pads decode.
            if pad.direction() != gst::PadDirection::Src {
                return;
            }
            if let Err(e) = link_remote(
                &pipeline,
                pad,
                &stats,
                audio_sink,
                video_sink,
                frame_store.clone(),
            ) {
                let _ = events.send(MediaEvent::Error(format!("remote link: {e}")));
            }
        });
    }

    fn command_pump_source(
        &self,
        cmd_rx: std::sync::mpsc::Receiver<Cmd>,
        share_attach: Arc<std::sync::Mutex<ShareAttach>>,
    ) -> glib::Source {
        let main_loop = self.main_loop.clone();
        let pipeline = self.pipeline.clone();
        let webrtc = self.webrtc.clone();
        let events = self.events.clone();
        let mute = self.mute.clone();
        let share_leg = self.share_leg.clone();
        let video_appsink = self.video_appsink.clone();
        let preview_appsink = self.preview_appsink.clone();
        let source = self.config.source;
        glib::timeout_source_new(
            std::time::Duration::from_millis(10),
            None,
            glib::Priority::DEFAULT,
            move || {
                while let Ok(cmd) = cmd_rx.try_recv() {
                    if matches!(cmd, Cmd::Shutdown) {
                        main_loop.quit();
                        return glib::ControlFlow::Break;
                    }
                    handle_cmd(
                        &pipeline,
                        &webrtc,
                        &events,
                        &mute,
                        &share_attach,
                        &share_leg,
                        source,
                        cmd,
                    );
                }
                poll_appsink(&video_appsink, &events, MediaEvent::RemoteFrame);
                poll_appsink(&preview_appsink, &events, MediaEvent::PreviewFrame);
                glib::ControlFlow::Continue
            },
        )
    }

    fn housekeeping_source(&self) -> glib::Source {
        // Bus errors + connection-state polling.
        let pipeline = self.pipeline.clone();
        let webrtc = self.webrtc.clone();
        let events = self.events.clone();
        let last = Arc::new(std::sync::Mutex::new(None::<PeerState>));
        glib::timeout_source_new(
            std::time::Duration::from_secs(2),
            None,
            glib::Priority::DEFAULT,
            move || {
                if let Some(bus) = pipeline.bus() {
                    while let Some(msg) = bus.timed_pop_filtered(
                        gst::ClockTime::ZERO,
                        &[gst::MessageType::Error, gst::MessageType::Eos],
                    ) {
                        match msg.view() {
                            gst::MessageView::Error(e) => {
                                let _ = events.send(MediaEvent::Error(format!(
                                    "pipeline: {} ({:?})",
                                    e.error(),
                                    e.debug()
                                )));
                            }
                            gst::MessageView::Eos(..) => {
                                let _ = events.send(MediaEvent::Error("pipeline EOS".to_string()));
                            }
                            _ => {}
                        }
                    }
                }
                let state: gstreamer_webrtc::WebRTCPeerConnectionState =
                    webrtc.property("connection-state");
                let mapped = map_peer_state(state);
                let mut guard = last.lock().expect("last state");
                if *guard != Some(mapped) {
                    *guard = Some(mapped);
                    let _ = events.send(MediaEvent::ConnectionState(mapped));
                }
                glib::ControlFlow::Continue
            },
        )
    }
}

fn src_pad(pipeline: &gst::Pipeline, element: &str, pad: &str) -> Option<gst::Pad> {
    pipeline.by_name(element).and_then(|e| e.static_pad(pad))
}

fn block_pad(pad: &gst::Pad) -> Option<gst::PadProbeId> {
    pad.add_probe(gst::PadProbeType::BLOCK_DOWNSTREAM, |_, _| {
        gst::PadProbeReturn::Ok
    })
}

/// Screen-share source variants for dynamic attach.
enum ShareKind {
    Test,
    XImage,
    PipeWire { fd: std::os::fd::OwnedFd, node: u32 },
}

/// Screen-share RTP caps (own payload type, VP8 like the camera leg).
const SHARE_RTP_CAPS: &str =
    "application/x-rtp,media=video,encoding-name=VP8,payload=98,clock-rate=90000";

/// Build the share leg (queue → encoder → payloader → webrtc sink pad) if
/// missing. Linking the new pad fires on-negotiation-needed, so the m-line
/// is advertised through the normal re-offer path. Returns true when built.
fn ensure_share_leg(
    pipeline: &gst::Pipeline,
    webrtc: &gst::Element,
    mute: &Arc<std::sync::Mutex<MuteState>>,
    share_leg: &Arc<AtomicBool>,
) -> Result<bool, String> {
    use gstreamer::prelude::*;
    if share_leg.load(Ordering::SeqCst) && pipeline.by_name("qshare").is_some() {
        return Ok(false);
    }
    let queue = gstreamer::ElementFactory::make("queue")
        .name("qshare")
        .build()
        .map_err(|e| format!("share queue: {e}"))?;
    let conv = gstreamer::ElementFactory::make("videoconvert")
        .build()
        .map_err(|e| format!("share convert: {e}"))?;
    let enc = gstreamer::ElementFactory::make("vp8enc")
        .property("deadline", 1i64)
        .build()
        .map_err(|e| format!("share enc: {e}"))?;
    let pay = gstreamer::ElementFactory::make("rtpvp8pay")
        .build()
        .map_err(|e| format!("share pay: {e}"))?;
    pipeline
        .add_many([&queue, &conv, &enc, &pay])
        .map_err(|e| format!("share add: {e}"))?;
    gstreamer::Element::link_many([&queue, &conv, &enc, &pay])
        .map_err(|e| format!("share link: {e}"))?;
    for e in [&queue, &conv, &enc, &pay] {
        e.sync_state_with_parent()
            .map_err(|e| format!("share sync: {e}"))?;
    }
    let templ = webrtc
        .pad_template("sink_%u")
        .ok_or_else(|| "no webrtc sink template".to_string())?;
    let caps: gst::Caps = SHARE_RTP_CAPS
        .parse()
        .map_err(|_| "bad share caps".to_string())?;
    let sinkpad = webrtc
        .request_pad(&templ, None, Some(&caps))
        .ok_or_else(|| "webrtc refused share pad".to_string())?;
    pay.static_pad("src")
        .ok_or_else(|| "share pay has no src".to_string())?
        .link(&sinkpad)
        .map_err(|e| format!("share peer link: {e}"))?;
    if let Ok(mut guard) = mute.lock() {
        guard.share.pad = queue.static_pad("sink");
        // Block immediately: the leg is built mid-call (PLAYING), so unlike
        // pre-PLAYING blocks this cannot stall negotiation. Unblocked by
        // the Share(true) that triggered the build.
        if guard.share.probe.is_none() {
            if let Some(pad) = guard.share.pad.clone() {
                guard.share.probe = block_pad(&pad);
            }
        }
    }
    share_leg.store(true, Ordering::SeqCst);
    Ok(true)
}
/// Attach (or replace) the screen-share source feeding the `qshare` queue.
/// The leg must exist first (see [`ensure_share_leg`]).
fn attach_share_source(
    pipeline: &gst::Pipeline,
    share_attach: &Arc<std::sync::Mutex<ShareAttach>>,
    kind: ShareKind,
) -> Result<(), String> {
    use gstreamer::prelude::*;
    let mut guard = share_attach
        .lock()
        .map_err(|e| format!("share lock: {e}"))?;
    // Tear down the previous source, if any.
    if let Some(old) = guard.element.take() {
        if let Some(parent) = old.parent() {
            if let Ok(bin) = parent.downcast::<gst::Bin>() {
                let _ = bin.remove(&old);
            }
        }
        let _ = old.set_state(gst::State::Null);
    }
    guard.fd = None;

    let src: gst::Element = match kind {
        // Built via parse_launch: the pattern enum has no Rust binding,
        // but launch strings accept the nick. Fresh element, no parent.
        ShareKind::Test => {
            gstreamer::parse::launch("videotestsrc is-live=true pattern=ball name=sharesrc")
                .map_err(|e| format!("test share src: {e}"))?
        }
        ShareKind::XImage => gstreamer::ElementFactory::make("ximagesrc")
            .property("show-pointer", true)
            .build()
            .map_err(|e| format!("ximagesrc: {e}"))?,
        ShareKind::PipeWire { fd, node } => {
            use std::os::fd::AsRawFd;
            let raw = fd.as_raw_fd();
            let element = gstreamer::ElementFactory::make("pipewiresrc")
                .property("fd", raw)
                .property("path", node.to_string())
                .build()
                .map_err(|e| format!("pipewiresrc: {e}"))?;
            // Keep ours alive for the call: the element may only borrow it.
            guard.fd = Some(fd);
            element
        }
    };

    let qshare = pipeline
        .by_name("qshare")
        .and_then(|q| q.static_pad("sink"))
        .ok_or_else(|| "no qshare queue (publisher only)".to_string())?;
    pipeline.add(&src).map_err(|e| format!("share add: {e}"))?;
    src.sync_state_with_parent()
        .map_err(|e| format!("share sync: {e}"))?;
    src.static_pad("src")
        .ok_or_else(|| "share source has no src pad".to_string())?
        .link(&qshare)
        .map_err(|e| format!("share link: {e}"))?;
    guard.element = Some(src);
    Ok(())
}

// ── command handling (runs on the worker context) ───────────────────

fn handle_cmd(
    pipeline: &gst::Pipeline,
    webrtc: &gst::Element,
    events: &mpsc::UnboundedSender<MediaEvent>,
    mute: &Arc<std::sync::Mutex<MuteState>>,
    share_attach: &Arc<std::sync::Mutex<ShareAttach>>,
    share_leg: &Arc<AtomicBool>,
    source: EngineSource,
    cmd: Cmd,
) {
    match cmd {
        Cmd::Shutdown => {}
        Cmd::IceServers(servers) => apply_ice_servers(webrtc, &servers),
        Cmd::AttachDefaultShare => {
            let kind = match source {
                EngineSource::Test => ShareKind::Test,
                EngineSource::Auto => ShareKind::XImage,
            };
            if let Err(e) = ensure_share_leg(pipeline, webrtc, mute, share_leg)
                .and_then(|_| attach_share_source(pipeline, share_attach, kind))
            {
                let _ = events.send(MediaEvent::Error(format!("share source: {e}")));
            }
        }
        Cmd::AttachPipeWire { fd, node } => {
            if let Err(e) = ensure_share_leg(pipeline, webrtc, mute, share_leg).and_then(|_| {
                attach_share_source(pipeline, share_attach, ShareKind::PipeWire { fd, node })
            }) {
                let _ = events.send(MediaEvent::Error(format!("pipewire share: {e}")));
            }
        }
        Cmd::RemoteOffer(sdp) => {
            // Chained: set-remote is async, the answer must be created in
            // its reply (calling create-answer synchronously fails).
            let w = webrtc.clone();
            let ev = events.clone();
            let create = move || create_answer(&w, &ev);
            if let Err(e) = set_remote_then(
                webrtc,
                gstreamer_webrtc::WebRTCSDPType::Offer,
                &sdp,
                events,
                create,
            ) {
                let _ = events.send(MediaEvent::Error(format!("set remote offer: {e}")));
            }
        }
        Cmd::RemoteAnswer(sdp) => {
            if let Err(e) = set_remote_then(
                webrtc,
                gstreamer_webrtc::WebRTCSDPType::Answer,
                &sdp,
                events,
                || {},
            ) {
                let _ = events.send(MediaEvent::Error(format!("set remote answer: {e}")));
            }
        }
        Cmd::RemoteIce { mline, candidate } => {
            webrtc.emit_by_name::<()>("add-ice-candidate", &[&mline, &candidate]);
        }
        Cmd::SetEnabled { kind, enabled } => {
            // Enabling share with no source yet builds the leg and attaches
            // the default source, so a plain Share(true) just works
            // (portal goes explicit). Linking fires re-negotiation.
            if kind == MediaKind::Screen
                && enabled
                && share_attach
                    .lock()
                    .map(|g| g.element.is_none())
                    .unwrap_or(false)
            {
                let default = match source {
                    EngineSource::Test => ShareKind::Test,
                    EngineSource::Auto => ShareKind::XImage,
                };
                if let Err(e) = ensure_share_leg(pipeline, webrtc, mute, share_leg)
                    .and_then(|_| attach_share_source(pipeline, share_attach, default))
                {
                    let _ = events.send(MediaEvent::Error(format!("share source: {e}")));
                    return;
                }
            }
            set_muted(mute, kind, !enabled, events);
        }
    }
}

/// Mute = probe-block the capture pad (packets stop, m-line stays, no
/// renegotiation). Unmute removes the probe.
fn set_muted(
    mute: &Arc<std::sync::Mutex<MuteState>>,
    kind: MediaKind,
    muted: bool,
    events: &mpsc::UnboundedSender<MediaEvent>,
) {
    let mut guard = match mute.lock() {
        Ok(g) => g,
        Err(e) => {
            let _ = events.send(MediaEvent::Error(format!("mute lock: {e}")));
            return;
        }
    };
    // Single &mut borrow of the selected slot (split borrows don't survive
    // through MutexGuard's DerefMut, hence the PadMute struct).
    let slot = match kind {
        MediaKind::Audio => &mut guard.audio,
        MediaKind::Video => &mut guard.video,
        MediaKind::Screen => &mut guard.share,
    };
    if muted {
        if slot.probe.is_some() {
            return;
        }
        let Some(pad) = slot.pad.clone() else {
            return;
        };
        match pad.add_probe(gst::PadProbeType::BLOCK_DOWNSTREAM, |_, _| {
            gst::PadProbeReturn::Ok
        }) {
            Some(id) => slot.probe = Some(id),
            None => {
                let _ = events.send(MediaEvent::Error("mute probe failed".to_string()));
            }
        }
    } else if let Some(id) = slot.probe.take() {
        if let Some(pad) = slot.pad.clone() {
            pad.remove_probe(id);
        }
    }
}

// ── SDP helpers ──

fn parse_sdp(text: &str) -> Result<gstreamer_sdp::SDPMessage, String> {
    gstreamer_sdp::SDPMessage::parse_buffer(text.as_bytes()).map_err(|e| format!("sdp parse: {e}"))
}

/// Set the remote description; `then` runs after it is applied.
/// `set-remote-description` is asynchronous — anything depending on the new
/// state (notably `create-answer`) must wait for this reply.
fn set_remote_then(
    webrtc: &gst::Element,
    kind: gstreamer_webrtc::WebRTCSDPType,
    sdp_text: &str,
    events: &mpsc::UnboundedSender<MediaEvent>,
    then: impl FnOnce() + Send + 'static,
) -> Result<(), String> {
    let sdp = parse_sdp(sdp_text)?;
    let desc = gstreamer_webrtc::WebRTCSessionDescription::new(kind, sdp);
    let events = events.clone();
    let promise = gst::Promise::with_change_func(move |reply| match reply {
        Ok(_) => then(),
        Err(e) => {
            let _ = events.send(MediaEvent::Error(format!("set-remote failed: {e:?}")));
        }
    });
    webrtc.emit_by_name::<()>("set-remote-description", &[&desc, &promise]);
    Ok(())
}

fn create_offer(
    webrtc: &gst::Element,
    events: &mpsc::UnboundedSender<MediaEvent>,
    tracks: Vec<MediaTrack>,
    share_leg: &Arc<AtomicBool>,
) {
    // Screen joins the offer once its leg exists (built on first share).
    let mut tracks = tracks;
    if share_leg.load(Ordering::SeqCst) && !tracks.iter().any(|t| t.kind == MediaKind::Screen) {
        tracks.push(MediaTrack {
            mid: None,
            kind: MediaKind::Screen,
            label: "screen".to_string(),
            description: String::new(),
        });
    }
    let events = events.clone();
    let webrtc_cb = webrtc.clone();
    let options = gst::Structure::new_empty("options");
    let promise = gst::Promise::with_change_func(move |reply| {
        let s = match reply {
            Ok(Some(s)) => s,
            _ => return,
        };
        let offer = match s.get::<gstreamer_webrtc::WebRTCSessionDescription>("offer") {
            Ok(o) => o,
            Err(e) => {
                let dump = format!("{s:?}");
                let short = dump.chars().take(300).collect::<String>();
                let _ = events.send(MediaEvent::Error(format!(
                    "offer extract: {e} (reply: {short})"
                )));
                return;
            }
        };
        let text = match offer.sdp().as_text() {
            Ok(t) => t,
            Err(e) => {
                let _ = events.send(MediaEvent::Error(format!("offer text: {e}")));
                return;
            }
        };
        let set_promise = gst::Promise::with_change_func(|_| {});
        webrtc_cb.emit_by_name::<()>("set-local-description", &[&offer, &set_promise]);
        let _ = events.send(MediaEvent::LocalOffer { sdp: text, tracks });
    });
    webrtc.emit_by_name::<()>("create-offer", &[&options, &promise]);
}

fn create_answer(webrtc: &gst::Element, events: &mpsc::UnboundedSender<MediaEvent>) {
    let events = events.clone();
    let webrtc_cb = webrtc.clone();
    let options = gst::Structure::new_empty("options");
    let promise = gst::Promise::with_change_func(move |reply| {
        let s = match reply {
            Ok(Some(s)) => s,
            _ => return,
        };
        let answer = match s.get::<gstreamer_webrtc::WebRTCSessionDescription>("answer") {
            Ok(a) => a,
            Err(e) => {
                let dump = format!("{s:?}");
                let short = dump.chars().take(300).collect::<String>();
                let _ = events.send(MediaEvent::Error(format!(
                    "answer extract: {e} (reply: {short})"
                )));
                return;
            }
        };
        let text = match answer.sdp().as_text() {
            Ok(t) => t,
            Err(e) => {
                let _ = events.send(MediaEvent::Error(format!("answer text: {e}")));
                return;
            }
        };
        let set_promise = gst::Promise::with_change_func(|_| {});
        webrtc_cb.emit_by_name::<()>("set-local-description", &[&answer, &set_promise]);
        let _ = events.send(MediaEvent::LocalAnswer { sdp: text });
    });
    webrtc.emit_by_name::<()>("create-answer", &[&options, &promise]);
}

// ── pipeline ──

/// Codec preferences for subscriber transceivers (must match the offerer).
const AUDIO_RTP_CAPS: &str =
    "application/x-rtp,media=audio,encoding-name=OPUS,payload=96,clock-rate=48000";
const VIDEO_RTP_CAPS: &str =
    "application/x-rtp,media=video,encoding-name=VP8,payload=97,clock-rate=90000";

fn add_recv_transceiver(
    webrtc: &gst::Element,
    events: &mpsc::UnboundedSender<MediaEvent>,
    caps_str: &str,
) -> Result<(), ()> {
    let caps: gst::Caps = caps_str.parse().map_err(|_| {
        let _ = events.send(MediaEvent::Error(format!("bad caps: {caps_str}")));
    })?;
    let transceiver = webrtc.emit_by_name::<Option<gstreamer_webrtc::WebRTCRTPTransceiver>>(
        "add-transceiver",
        &[
            &gstreamer_webrtc::WebRTCRTPTransceiverDirection::Recvonly,
            &caps,
        ],
    );
    if transceiver.is_none() {
        let _ = events.send(MediaEvent::Error(format!(
            "add-transceiver failed for {caps_str}"
        )));
        return Err(());
    }
    Ok(())
}

fn launch_string(config: &GstEngineConfig) -> String {
    // NOTE: linking `! webrtc.` only works when the `nice` plugin is
    // installed (webrtcbin refuses sink pads without ICE elements).
    // Debian/Ubuntu: `apt install gstreamer1.0-nice`.
    let mut parts = vec!["webrtcbin name=webrtc bundle-policy=max-bundle".to_string()];
    if config.role == EngineRole::Subscriber {
        // No local media: transceivers come from `add-transceiver` instead.
        return parts.join(" ");
    }
    let audio_src = match config.source {
        EngineSource::Auto => "autoaudiosrc name=audiosrc",
        EngineSource::Test => "audiotestsrc name=audiosrc is-live=true wave=sine freq=440",
    };
    let video_src = match config.source {
        EngineSource::Auto => "autovideosrc name=videosrc",
        EngineSource::Test => "videotestsrc name=videosrc is-live=true pattern=smpte",
    };
    if config.audio {
        parts.push(format!(
            "{audio_src} ! audioconvert ! audioresample ! opusenc ! rtpopuspay ! application/x-rtp,media=audio,encoding-name=OPUS,payload=96 ! webrtc."
        ));
    }
    if config.video {
        // Camera tee: encoder leg + local preview leg (RGBA appsink).
        parts.push(format!(
            "{video_src} ! tee name=t ! queue name=qenc ! videoconvert ! vp8enc deadline=1 ! rtpvp8pay ! application/x-rtp,media=video,encoding-name=VP8,payload=97 ! webrtc. \
             t. ! queue name=qprev ! videoconvert ! videoscale ! video/x-raw,format=RGBA ! appsink name=preview sync=false max-buffers=2 drop=true emit-signals=false"
        ));
    }
    // NOTE: no share leg here. An idle linked leg suppresses
    // on-negotiation-needed for the whole bin, so the screen branch
    // (queue + encoder + source) is built on first share instead —
    // renegotiation then advertises the new m-line normally.
    parts.join(" ")
}

fn link_remote(
    pipeline: &gst::Pipeline,
    pad: &gst::Pad,
    stats: &Arc<EngineStats>,
    audio_sink: RemoteSink,
    video_sink: RemoteSink,
    frame_store: Arc<std::sync::Mutex<Option<gstreamer_app::AppSink>>>,
) -> Result<(), String> {
    // decodebin autoplugs depay + decode from the registry.
    let decode = gst::ElementFactory::make("decodebin")
        .build()
        .map_err(|e| format!("decodebin: {e}"))?;
    let queue = gst::ElementFactory::make("queue")
        .build()
        .map_err(|e| format!("queue: {e}"))?;
    pipeline
        .add_many([&queue, &decode])
        .map_err(|e| format!("add: {e}"))?;
    queue.sync_state_with_parent().map_err(|e| format!("{e}"))?;
    decode
        .sync_state_with_parent()
        .map_err(|e| format!("{e}"))?;
    queue.link(&decode).map_err(|e| format!("link: {e}"))?;
    pad.link(&queue.static_pad("sink").expect("queue sink"))
        .map_err(|e| format!("pad link: {e}"))?;

    let stats = stats.clone();
    let pipeline_weak = pipeline.downgrade();
    decode.connect_pad_added(move |_, srcpad| {
        let Some(pipeline) = pipeline_weak.upgrade() else {
            return;
        };
        let caps = srcpad
            .current_caps()
            .filter(|c| !c.is_empty())
            .unwrap_or_else(|| srcpad.query_caps(None));
        if caps.is_empty() {
            return;
        }
        let Some(s) = caps.structure(0) else {
            return;
        };
        let name = s.name();
        let is_audio = name.starts_with("audio/");
        if !is_audio && !name.starts_with("video/") {
            return;
        }
        let fake = matches!(audio_sink, RemoteSink::Fake) && is_audio
            || matches!(video_sink, RemoteSink::Fake) && !is_audio;
        let want_frames = !is_audio && matches!(video_sink, RemoteSink::Frames);
        let stats = stats.clone();
        let frame_store = frame_store.clone();
        // Build convert + sink; bail out silently on any error (logged upstream).
        let convert = gst::ElementFactory::make(if is_audio {
            "audioconvert"
        } else {
            "videoconvert"
        });
        let sink: gst::Element = if fake {
            let fakesink = match gst::ElementFactory::make("fakesink").build() {
                Ok(e) => e,
                Err(_) => return,
            };
            fakesink.set_property("sync", false);
            fakesink.set_property("signal-handoffs", true);
            fakesink.connect("handoff", false, move |_| {
                if is_audio {
                    stats.audio_buffers.fetch_add(1, Ordering::Relaxed);
                } else {
                    stats.video_buffers.fetch_add(1, Ordering::Relaxed);
                }
                None
            });
            fakesink
        } else if is_audio {
            match gst::ElementFactory::make("autoaudiosink").build() {
                Ok(e) => e,
                Err(_) => return,
            }
        } else if want_frames {
            // In-window rendering path: RGBA frames to an appsink polled by
            // the command pump (no extra plugins needed).
            let scale = match gst::ElementFactory::make("videoscale").build() {
                Ok(e) => e,
                Err(_) => return,
            };
            let filter = match gst::ElementFactory::make("capsfilter").build() {
                Ok(e) => e,
                Err(_) => return,
            };
            let rgba: gst::Caps = match "video/x-raw,format=RGBA".parse() {
                Ok(c) => c,
                Err(_) => return,
            };
            filter.set_property("caps", &rgba);
            let appsink_elem = match gst::ElementFactory::make("appsink").build() {
                Ok(e) => e,
                Err(_) => return,
            };
            appsink_elem.set_property("sync", false);
            appsink_elem.set_property("max-buffers", 3u32);
            appsink_elem.set_property("drop", true);
            appsink_elem.set_property("emit-signals", false);
            let convert = match convert.build() {
                Ok(e) => e,
                Err(_) => return,
            };
            if pipeline
                .add_many([&convert, &scale, &filter, &appsink_elem])
                .is_err()
            {
                return;
            }
            for e in [&convert, &scale, &filter, &appsink_elem] {
                if e.sync_state_with_parent().is_err() {
                    return;
                }
            }
            if convert.link(&scale).is_err()
                || scale.link_filtered(&filter, &rgba).is_err()
                || filter.link(&appsink_elem).is_err()
            {
                return;
            }
            if let Some(sinkpad) = convert.static_pad("sink") {
                let _ = srcpad.link(&sinkpad);
            }
            match appsink_elem.downcast::<gstreamer_app::AppSink>() {
                Ok(sink) => {
                    *frame_store.lock().expect("frame store") = Some(sink);
                }
                Err(_) => {
                    // decode chain works; frames just won't reach the UI.
                }
            }
            return;
        } else {
            match gst::ElementFactory::make("autovideosink").build() {
                Ok(e) => e,
                Err(_) => return,
            }
        };
        let convert = match convert.build() {
            Ok(e) => e,
            Err(_) => return,
        };
        if pipeline.add_many([&convert, &sink]).is_err() {
            return;
        }
        if convert.sync_state_with_parent().is_err() || sink.sync_state_with_parent().is_err() {
            return;
        }
        if convert.link(&sink).is_err() {
            return;
        }
        if let Some(sinkpad) = convert.static_pad("sink") {
            let _ = srcpad.link(&sinkpad);
        }
    });
    Ok(())
}

fn apply_ice_servers(webrtc: &gst::Element, servers: &[IceServer]) {
    // First STUN via the convenience property; EVERY TURN via the action
    // signal (the property holds only one — time-limited credentials included
    // verbatim, the server mints them).
    let mut stun_set = false;
    for s in servers {
        for url in &s.urls {
            if url.starts_with("stun:") && !stun_set {
                webrtc.set_property("stun-server", url);
                stun_set = true;
            } else if url.starts_with("turn:") || url.starts_with("turns:") {
                let _ok: bool = webrtc.emit_by_name("add-turn-server", &[&url.as_str()]);
            }
        }
    }
}

/// Drain decoded RGBA frames from an appsink (if any).
fn poll_appsink(
    store: &Arc<std::sync::Mutex<Option<gstreamer_app::AppSink>>>,
    events: &mpsc::UnboundedSender<MediaEvent>,
    wrap: fn(super::goloom_media::VideoFrame) -> MediaEvent,
) {
    let guard = match store.lock() {
        Ok(g) => g,
        Err(_) => return,
    };
    let Some(sink) = guard.as_ref() else {
        return;
    };
    // At most 2 per 10ms tick; max-buffers+drop bounds the latency anyway.
    for _ in 0..2 {
        match sink.try_pull_sample(gst::ClockTime::ZERO) {
            Some(sample) => {
                if let Some(frame) = sample_to_frame(&sample) {
                    let _ = events.send(wrap(frame));
                }
            }
            None => break,
        }
    }
}

fn sample_to_frame(sample: &gst::Sample) -> Option<super::goloom_media::VideoFrame> {
    let caps = sample.caps()?;
    let info = gstreamer_video::VideoInfo::from_caps(caps).ok()?;
    if info.format() != gstreamer_video::VideoFormat::Rgba {
        return None;
    }
    let stride = *info.stride().first()? as usize;
    let frame = super::goloom_media::VideoFrame {
        width: info.width(),
        height: info.height(),
        stride,
        pixels: sample.buffer()?.map_readable().ok()?.as_slice().to_vec(),
    };
    frame.is_valid().then_some(frame)
}

fn map_peer_state(s: gstreamer_webrtc::WebRTCPeerConnectionState) -> PeerState {
    use gstreamer_webrtc::WebRTCPeerConnectionState as S;
    match s {
        S::New => PeerState::New,
        S::Connecting => PeerState::Connecting,
        S::Connected => PeerState::Connected,
        S::Disconnected => PeerState::Disconnected,
        S::Failed => PeerState::Failed,
        S::Closed => PeerState::Closed,
        _ => PeerState::New,
    }
}

#[cfg(test)]
mod tests {
    use super::super::goloom_media::MediaEngine;
    use super::*;

    /// Full-mesh call between two peers, mirroring the Goloom two-PC design:
    /// `A_pub` offers → `B_sub` answers, `B_pub` offers → `A_sub` answers,
    /// ICE trickles per direction. Subscribe sides render RGBA `Frames`
    /// (the UI video path); publish sides count into `Fake` sinks.
    #[tokio::test]
    async fn p2p_offer_answer_ice_and_media_flow() {
        struct Side {
            publish: GstMediaEngine,
            subscribe: GstMediaEngine,
            pub_rx: mpsc::UnboundedReceiver<MediaEvent>,
            sub_rx: mpsc::UnboundedReceiver<MediaEvent>,
            connected: bool,
            frames: u32,
        }
        impl Side {
            fn new() -> Self {
                let (pub_tx, pub_rx) = mpsc::unbounded_channel();
                let (sub_tx, sub_rx) = mpsc::unbounded_channel();
                let pub_cfg = GstEngineConfig::test_with_role(EngineRole::Publisher);
                let mut sub_cfg = GstEngineConfig::test_with_role(EngineRole::Subscriber);
                sub_cfg.video_sink = RemoteSink::Frames;
                let mut publish = GstMediaEngine::new(pub_tx, pub_cfg);
                let mut subscribe = GstMediaEngine::new(sub_tx, sub_cfg);
                publish.start().expect("publish start");
                subscribe.start().expect("subscribe start");
                Self {
                    publish,
                    subscribe,
                    pub_rx,
                    sub_rx,
                    connected: false,
                    frames: 0,
                }
            }
            fn stop(&mut self) {
                self.publish.stop();
                self.subscribe.stop();
            }
        }

        let mut a = Side::new();
        let mut b = Side::new();
        let mut errors: Vec<String> = Vec::new();
        let mut offers_answered = 0u32;
        let mut share_enabled = false;
        let mut screen_offer_seen = false;
        let screen_kind = super::super::goloom_media::MediaKind::Screen;
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(60);
        while tokio::time::Instant::now() < deadline {
            let base_done =
                a.connected && b.connected && offers_answered >= 2 && a.frames > 0 && b.frames > 0;
            if base_done && !share_enabled {
                // Dynamic share: building the leg must re-offer with screen.
                a.publish.set_sharing_enabled(true).expect("share on");
                share_enabled = true;
            }
            if base_done && share_enabled && screen_offer_seen && offers_answered >= 3 {
                break;
            }
            // Pumps one event; returns true when both directions still need work.
            {
                let got = tokio::select! {
                    ev = a.pub_rx.recv() => ev.map(|e| (true, e)),
                    ev = a.sub_rx.recv() => ev.map(|e| (false, e)),
                    _ = tokio::time::sleep(std::time::Duration::from_millis(200)) => None,
                };
                match got {
                    None => {}
                    Some((is_pub, ev)) => match ev {
                        MediaEvent::LocalOffer { sdp, tracks } => {
                            assert!(is_pub, "A subscriber must never offer");
                            if tracks.iter().any(|t| t.kind == screen_kind) {
                                screen_offer_seen = true;
                            }
                            b.subscribe
                                .handle_remote_offer(&sdp)
                                .expect("B takes offer");
                        }
                        MediaEvent::LocalAnswer { sdp } => {
                            assert!(!is_pub, "A publisher must never answer");
                            offers_answered += 1;
                            b.publish
                                .handle_remote_answer(&sdp)
                                .expect("B takes answer");
                        }
                        MediaEvent::LocalIce {
                            mline_index,
                            candidate,
                        } => {
                            // A publishes → B subscribes; A subscribes ← B publishes.
                            if is_pub {
                                b.subscribe
                                    .add_remote_ice(mline_index, &candidate)
                                    .expect("B takes ice");
                            } else {
                                b.publish
                                    .add_remote_ice(mline_index, &candidate)
                                    .expect("B takes ice");
                            }
                        }
                        MediaEvent::ConnectionState(PeerState::Connected) => {
                            a.connected = true;
                        }
                        MediaEvent::RemoteFrame(f) => {
                            assert!(!is_pub, "A publisher never renders");
                            assert!(f.is_valid(), "invalid frame from A_sub");
                            a.frames += 1;
                        }
                        MediaEvent::ConnectionState(PeerState::Failed) => {
                            errors.push("A failed".to_string())
                        }
                        MediaEvent::Error(e) => errors.push(format!("A: {e}")),
                        _ => {}
                    },
                }
            }
            {
                let got = tokio::select! {
                    ev = b.pub_rx.recv() => ev.map(|e| (true, e)),
                    ev = b.sub_rx.recv() => ev.map(|e| (false, e)),
                    _ = tokio::time::sleep(std::time::Duration::from_millis(200)) => None,
                };
                match got {
                    None => {}
                    Some((is_pub, ev)) => match ev {
                        MediaEvent::LocalOffer { sdp, .. } => {
                            assert!(is_pub, "B subscriber must never offer");
                            a.subscribe
                                .handle_remote_offer(&sdp)
                                .expect("A takes offer");
                        }
                        MediaEvent::LocalAnswer { sdp } => {
                            assert!(!is_pub, "B publisher must never answer");
                            offers_answered += 1;
                            a.publish
                                .handle_remote_answer(&sdp)
                                .expect("A takes answer");
                        }
                        MediaEvent::LocalIce {
                            mline_index,
                            candidate,
                        } => {
                            if is_pub {
                                a.subscribe
                                    .add_remote_ice(mline_index, &candidate)
                                    .expect("A takes ice");
                            } else {
                                a.publish
                                    .add_remote_ice(mline_index, &candidate)
                                    .expect("A takes ice");
                            }
                        }
                        MediaEvent::ConnectionState(PeerState::Connected) => {
                            b.connected = true;
                        }
                        MediaEvent::RemoteFrame(f) => {
                            assert!(!is_pub, "B publisher never renders");
                            assert!(f.is_valid(), "invalid frame from B_sub");
                            b.frames += 1;
                        }
                        MediaEvent::ConnectionState(PeerState::Failed) => {
                            errors.push("B failed".to_string())
                        }
                        MediaEvent::Error(e) => errors.push(format!("B: {e}")),
                        _ => {}
                    },
                }
            }
        }

        a.stop();
        b.stop();

        eprintln!(
            "p2p done: a={} b={} answered={offers_answered}",
            a.connected, b.connected
        );
        eprintln!("p2p frames: a_sub={} b_sub={}", a.frames, b.frames);
        eprintln!("p2p errors: {errors:?}");
        assert!(offers_answered >= 3, "share re-offer never answered");
        assert!(screen_offer_seen, "no re-offer carried a screen track");
        assert!(a.connected, "A never connected");
        assert!(b.connected, "B never connected");
        assert!(a.frames > 0, "A got no remote video frames");
        assert!(b.frames > 0, "B got no remote video frames");
        assert!(errors.is_empty(), "engine errors: {errors:?}");
    }
}
