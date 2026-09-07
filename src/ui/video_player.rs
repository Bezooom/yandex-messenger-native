//! Inline chat video player (picture + play/pause + scrub).
//!
//! GStreamer build: `filesrc ! decodebin` with video to an RGBA appsink
//! (painted on a [`gtk::Picture`]) and audio to the default sink. The UI
//! drives [`VideoPlayer::pump_once`] on a 100ms timer; tests call it
//! directly (no main loop needed). Without `gstreamer` every open fails
//! honestly instead of a dead play button.

use gtk::prelude::*;
use gtk::{Box as GtkBox, Button, Label, Orientation, Picture, Scale};
use std::cell::RefCell;
use std::path::Path;
#[cfg(feature = "gstreamer")]
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

#[cfg_attr(not(feature = "gstreamer"), allow(dead_code))]
fn format_time(secs: f64) -> String {
    let total = secs.max(0.0) as u64;
    format!("{:02}:{:02}", total / 60, total % 60)
}

pub struct VideoPlayer {
    container: GtkBox,
    picture: Picture,
    play_btn: Button,
    scrub: Scale,
    time_label: Label,
    status_label: Label,
    /// change-value handler, blocked around programmatic scrub updates.
    scrub_handler: Rc<RefCell<Option<glib::SignalHandlerId>>>,
    #[cfg(feature = "gstreamer")]
    inner: Rc<RefCell<PlayerInner>>,
    #[cfg(not(feature = "gstreamer"))]
    inner: Rc<RefCell<StubInner>>,
}

#[cfg(feature = "gstreamer")]
struct PlayerInner {
    pipeline: Option<gstreamer::Pipeline>,
    tmp_path: Option<PathBuf>,
    playing: bool,
    eos: bool,
    duration: Option<Duration>,
    fake_audio: bool,
}

#[cfg(not(feature = "gstreamer"))]
struct StubInner {
    _private: (),
}

impl VideoPlayer {
    pub fn new() -> Self {
        let container = GtkBox::new(Orientation::Vertical, 4);
        container.set_css_classes(&["inline-video-player"]);

        let picture = Picture::new();
        picture.set_hexpand(true);
        picture.set_size_request(320, 180);
        picture.set_content_fit(gtk::ContentFit::Cover);
        picture.set_can_shrink(true);
        container.append(&picture);

        let controls = GtkBox::new(Orientation::Horizontal, 8);
        controls.set_halign(gtk::Align::Fill);

        let play_btn = Button::builder()
            .icon_name("media-playback-start-symbolic")
            .css_classes(vec!["btn-icon", "circular"])
            .tooltip_text("Смотреть")
            .build();
        play_btn.set_size_request(36, 36);
        controls.append(&play_btn);

        let scrub = Scale::builder()
            .orientation(Orientation::Horizontal)
            .adjustment(&gtk::Adjustment::new(0.0, 0.0, 1000.0, 1.0, 10.0, 0.0))
            .hexpand(true)
            .draw_value(false)
            .build();
        controls.append(&scrub);

        let time_label = Label::builder()
            .label("00:00 / 00:00")
            .css_classes(vec!["dim-label", "video-time"])
            .build();
        controls.append(&time_label);
        container.append(&controls);

        let status_label = Label::builder()
            .css_classes(vec!["dim-label"])
            .xalign(0.0)
            .visible(false)
            .build();
        container.append(&status_label);

        let this = Self {
            container,
            picture,
            play_btn,
            scrub,
            time_label,
            status_label,
            scrub_handler: Rc::new(RefCell::new(None)),
            #[cfg(feature = "gstreamer")]
            inner: Rc::new(RefCell::new(PlayerInner {
                pipeline: None,
                tmp_path: None,
                playing: false,
                eos: false,
                duration: None,
                fake_audio: false,
            })),
            #[cfg(not(feature = "gstreamer"))]
            inner: Rc::new(RefCell::new(StubInner { _private: () })),
        };
        this.bind_callbacks();
        // Pump on the GTK tick (tests call pump_once directly).
        let pump = this.clone_ref();
        glib::timeout_add_local(Duration::from_millis(100), move || {
            if pump.container.is_visible() {
                pump.pump_once();
            }
            glib::ControlFlow::Continue
        });
        this
    }

    /// Route audio to fakesink (tests).
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn set_fake_audio(&self, _fake: bool) {
        #[cfg(feature = "gstreamer")]
        {
            self.inner.borrow_mut().fake_audio = _fake;
        }
    }

    pub fn container(&self) -> &GtkBox {
        &self.container
    }

    fn bind_callbacks(&self) {
        // Play/pause toggle.
        {
            let this = self.clone_ref();
            self.play_btn.connect_clicked(move |_| {
                this.toggle();
            });
        }
        // Scrub seeks; pump blocks this handler around programmatic sets.
        {
            let this = self.clone_ref();
            let id = self.scrub.connect_change_value(move |_, _, value| {
                this.seek_fraction(value / 1000.0);
                glib::Propagation::Proceed
            });
            *self.scrub_handler.borrow_mut() = Some(id);
        }
    }

    /// Open a remote URL (downloaded with auth, then played).
    pub fn open_url(&self, url: &str) {
        let url = url.to_string();
        let this = self.clone_ref();
        glib::spawn_future_local(async move {
            this.set_status("Загрузка видео…", true);
            match fetch_video_bytes(&url).await {
                Ok(bytes) => {
                    this.set_status("", false);
                    if let Err(e) = this.open_bytes(&bytes, "mp4") {
                        this.set_status(&format!("Не удалось открыть: {e}"), true);
                    }
                }
                Err(e) => {
                    this.set_status(&format!("Не удалось скачать: {e}"), true);
                }
            }
        });
    }

    /// Open in-memory bytes (played from a temp file).
    pub fn open_bytes(&self, bytes: &[u8], suffix: &str) -> Result<(), String> {
        #[cfg(feature = "gstreamer")]
        {
            if bytes.is_empty() {
                return Err("empty video".to_string());
            }
            let mut path = std::env::temp_dir();
            path.push(format!(
                "ym_video_{}_{suffix}",
                uuid::Uuid::new_v4().simple()
            ));
            std::fs::write(&path, bytes).map_err(|e| format!("temp write: {e}"))?;
            self.open_file(&path)?;
            // Track the temp file for cleanup on next open/stop.
            self.inner.borrow_mut().tmp_path = Some(path);
            Ok(())
        }
        #[cfg(not(feature = "gstreamer"))]
        {
            let _ = (bytes, suffix);
            Err("video needs the gstreamer build".to_string())
        }
    }

    /// Open a local file and start playing.
    #[cfg_attr(not(feature = "gstreamer"), allow(dead_code))]
    pub fn open_file(&self, path: &Path) -> Result<(), String> {
        #[cfg(feature = "gstreamer")]
        {
            use gstreamer::prelude::*;
            gstreamer::init().map_err(|e| format!("gst init: {e}"))?;
            self.teardown_locked();
            self.teardown_locked();
            let pipeline = gstreamer::parse::launch(&format!(
                "filesrc location={} ! decodebin name=dec",
                path.display()
            ))
            .map_err(|e| format!("parse: {e}"))?
            .downcast::<gstreamer::Pipeline>()
            .map_err(|_| "not a pipeline".to_string())?;

            let dec = pipeline
                .by_name("dec")
                .ok_or_else(|| "no decodebin".to_string())?;
            let weak_pipe = pipeline.downgrade();
            let fake_audio = self.inner.borrow().fake_audio;
            dec.connect_pad_added(move |_, srcpad| {
                let Some(pipeline) = weak_pipe.upgrade() else {
                    return;
                };
                let caps = match srcpad
                    .current_caps()
                    .filter(|c| !c.is_empty())
                    .or_else(|| Some(srcpad.query_caps(None)))
                {
                    Some(c) if !c.is_empty() => c,
                    _ => return,
                };
                let Some(s) = caps.structure(0) else {
                    return;
                };
                let name = s.name();
                if name.starts_with("audio/") {
                    let conv = gstreamer::ElementFactory::make("audioconvert");
                    let resample = gstreamer::ElementFactory::make("audioresample");
                    let sink = if fake_audio {
                        let built = gstreamer::ElementFactory::make("fakesink").build();
                        if let Ok(ref e) = built {
                            e.set_property("sync", false);
                        }
                        built
                    } else {
                        gstreamer::ElementFactory::make("autoaudiosink").build()
                    };
                    let (Ok(conv), Ok(resample), Ok(sink)) = (conv.build(), resample.build(), sink)
                    else {
                        return;
                    };
                    if pipeline.add_many([&conv, &resample, &sink]).is_err() {
                        return;
                    }
                    for e in [&conv, &resample, &sink] {
                        if e.sync_state_with_parent().is_err() {
                            return;
                        }
                    }
                    if conv.link(&resample).is_err() || resample.link(&sink).is_err() {
                        return;
                    }
                    if let Some(pad) = conv.static_pad("sink") {
                        let _ = srcpad.link(&pad);
                    }
                } else if name.starts_with("video/") {
                    let conv = gstreamer::ElementFactory::make("videoconvert");
                    let scale = gstreamer::ElementFactory::make("videoscale");
                    let filter = gstreamer::ElementFactory::make("capsfilter");
                    let sink = gstreamer::ElementFactory::make("appsink").name("vappsink");
                    let (Ok(conv), Ok(scale), Ok(filter), Ok(sink)) =
                        (conv.build(), scale.build(), filter.build(), sink.build())
                    else {
                        return;
                    };
                    let rgba: gstreamer::Caps = match "video/x-raw,format=RGBA".parse() {
                        Ok(c) => c,
                        Err(_) => return,
                    };
                    filter.set_property("caps", &rgba);
                    sink.set_property("sync", false);
                    sink.set_property("max-buffers", 5u32);
                    sink.set_property("drop", true);
                    sink.set_property("emit-signals", false);
                    if pipeline.add_many([&conv, &scale, &filter, &sink]).is_err() {
                        return;
                    }
                    for e in [&conv, &scale, &filter, &sink] {
                        if e.sync_state_with_parent().is_err() {
                            return;
                        }
                    }
                    if conv.link(&scale).is_err()
                        || scale.link_filtered(&filter, &rgba).is_err()
                        || filter.link(&sink).is_err()
                    {
                        return;
                    }
                    if let Some(pad) = conv.static_pad("sink") {
                        let _ = srcpad.link(&pad);
                    }
                    // Remember the video appsink for frame polling.
                    // (Stored via pipeline name lookup in pump_once.)
                }
            });

            pipeline
                .set_state(gstreamer::State::Playing)
                .map_err(|e| format!("play: {e:?}"))?;
            {
                let mut inner = self.inner.borrow_mut();
                inner.pipeline = Some(pipeline);
                inner.playing = true;
                inner.eos = false;
                inner.duration = None;
            }
            self.play_btn.set_icon_name("media-playback-pause-symbolic");
            Ok(())
        }
        #[cfg(not(feature = "gstreamer"))]
        {
            let _ = path;
            Err("video needs the gstreamer build".to_string())
        }
    }

    fn toggle(&self) {
        #[cfg(feature = "gstreamer")]
        {
            use gstreamer::prelude::*;
            let playing = self.inner.borrow().playing;
            let pipeline = self.inner.borrow().pipeline.clone();
            let Some(pipeline) = pipeline else {
                return;
            };
            if playing {
                let _ = pipeline.set_state(gstreamer::State::Paused);
                self.inner.borrow_mut().playing = false;
                self.play_btn.set_icon_name("media-playback-start-symbolic");
            } else {
                let _ = pipeline.set_state(gstreamer::State::Playing);
                self.inner.borrow_mut().playing = true;
                self.inner.borrow_mut().eos = false;
                self.play_btn.set_icon_name("media-playback-pause-symbolic");
            }
        }
    }

    fn seek_fraction(&self, fraction: f64) {
        #[cfg(feature = "gstreamer")]
        {
            use gstreamer::prelude::*;
            let (pipeline, duration) = {
                let inner = self.inner.borrow();
                (inner.pipeline.clone(), inner.duration)
            };
            if let (Some(pipeline), Some(total)) = (pipeline, duration) {
                let pos = (fraction.clamp(0.0, 1.0) * total.as_secs_f64()) as u64;
                let _ = pipeline.seek_simple(
                    gstreamer::SeekFlags::FLUSH | gstreamer::SeekFlags::KEY_UNIT,
                    gstreamer::ClockTime::from_seconds(pos),
                );
            }
        }
        #[cfg(not(feature = "gstreamer"))]
        {
            let _ = fraction;
        }
    }

    /// One pump step: bus (EOS), video frames → picture, position → scrub.
    /// Called by the GTK tick and directly by tests.
    pub fn pump_once(&self) {
        #[cfg(feature = "gstreamer")]
        {
            use gstreamer::prelude::*;
            // 1. Bus: EOS / errors.
            let bus = self.inner.borrow().pipeline.clone().and_then(|p| p.bus());
            if let Some(bus) = bus {
                while let Some(msg) = bus.timed_pop_filtered(
                    gstreamer::ClockTime::ZERO,
                    &[gstreamer::MessageType::Eos, gstreamer::MessageType::Error],
                ) {
                    match msg.view() {
                        gstreamer::MessageView::Eos(..) => {
                            let mut inner = self.inner.borrow_mut();
                            inner.eos = true;
                            inner.playing = false;
                            drop(inner);
                            self.play_btn.set_icon_name("media-playback-start-symbolic");
                            self.scrub.set_value(1000.0);
                        }
                        gstreamer::MessageView::Error(e) => {
                            log::warn!("video error: {} ({:?})", e.error(), e.debug());
                            self.set_status(&format!("Ошибка видео: {}", e.error()), true);
                            self.inner.borrow_mut().playing = false;
                        }
                        _ => {}
                    }
                }
            }
            // 2. Video appsink → picture.
            let sink = self.find_video_sink();
            if let Some(sink) = sink {
                for _ in 0..2 {
                    match sink.try_pull_sample(gstreamer::ClockTime::ZERO) {
                        Some(sample) => {
                            if let Some((w, h, stride, pixels)) = video_sample_to_rgba(&sample) {
                                let bytes = glib::Bytes::from(&pixels);
                                let texture = gtk::gdk::MemoryTexture::new(
                                    w as i32,
                                    h as i32,
                                    gtk::gdk::MemoryFormat::R8g8b8a8,
                                    &bytes,
                                    stride,
                                );
                                self.picture.set_paintable(Some(&texture));
                            }
                        }
                        None => break,
                    }
                }
            }
            // 3. Position/duration → scrub + time.
            let (pos, dur) = {
                let inner = self.inner.borrow();
                let pos = inner
                    .pipeline
                    .as_ref()
                    .and_then(|p| p.query_position::<gstreamer::ClockTime>())
                    .map(|t| t.seconds_f64());
                let dur = inner
                    .pipeline
                    .as_ref()
                    .and_then(|p| p.query_duration::<gstreamer::ClockTime>())
                    .map(|t| t.seconds_f64());
                (pos, dur)
            };
            if let Some(total) = dur.filter(|d| *d > 0.0) {
                let mut inner = self.inner.borrow_mut();
                inner.duration = Some(Duration::from_secs_f64(total));
                drop(inner);
                let frac = pos.unwrap_or(0.0) / total;
                // Block the change-value handler around programmatic sets.
                if let Some(id) = self.scrub_handler.borrow().as_ref() {
                    self.scrub.block_signal(id);
                }
                self.scrub
                    .set_value((frac.clamp(0.0, 1.0) * 1000.0).round());
                if let Some(id) = self.scrub_handler.borrow().as_ref() {
                    self.scrub.unblock_signal(id);
                }
                self.time_label.set_text(&format!(
                    "{} / {}",
                    format_time(pos.unwrap_or(0.0)),
                    format_time(total)
                ));
            }
        }
    }

    fn set_status(&self, text: &str, visible: bool) {
        self.status_label.set_text(text);
        self.status_label.set_visible(visible);
    }

    #[cfg_attr(not(feature = "gstreamer"), allow(dead_code))]
    fn teardown_locked(&self) {
        #[cfg(feature = "gstreamer")]
        {
            use gstreamer::prelude::*;
            let mut inner = self.inner.borrow_mut();
            if let Some(p) = inner.pipeline.take() {
                let _ = p.set_state(gstreamer::State::Null);
            }
            if let Some(path) = inner.tmp_path.take() {
                let _ = std::fs::remove_file(path);
            }
            inner.playing = false;
            inner.eos = false;
            inner.duration = None;
        }
    }

    fn clone_ref(&self) -> Self {
        Self {
            container: self.container.clone(),
            picture: self.picture.clone(),
            play_btn: self.play_btn.clone(),
            scrub: self.scrub.clone(),
            time_label: self.time_label.clone(),
            status_label: self.status_label.clone(),
            scrub_handler: self.scrub_handler.clone(),
            inner: self.inner.clone(),
        }
    }

    // ── test hooks ──

    #[cfg(test)]
    pub fn test_has_picture(&self) -> bool {
        self.picture.paintable().is_some()
    }

    #[cfg(test)]
    pub fn test_position_secs(&self) -> f64 {
        #[cfg(feature = "gstreamer")]
        {
            self.inner
                .borrow()
                .pipeline
                .as_ref()
                .and_then(|p| {
                    use gstreamer::prelude::*;
                    p.query_position::<gstreamer::ClockTime>()
                })
                .map(|t| t.seconds_f64())
                .unwrap_or(0.0)
        }
        #[cfg(not(feature = "gstreamer"))]
        {
            0.0
        }
    }
}

#[cfg(feature = "gstreamer")]
impl VideoPlayer {
    /// Locate the video appsink created in the decodebin pad handler.
    /// Appsinks are named `vappsink` at build time.
    fn find_video_sink(&self) -> Option<gstreamer_app::AppSink> {
        let pipeline = self.inner.borrow().pipeline.clone()?;
        // The pad handler names the video appsink deterministically.
        use gstreamer::prelude::*;
        pipeline
            .by_name("vappsink")
            .and_then(|e| e.downcast::<gstreamer_app::AppSink>().ok())
    }
}

/// Decode one RGBA sample into raw parts.
#[cfg(feature = "gstreamer")]
fn video_sample_to_rgba(sample: &gstreamer::Sample) -> Option<(u32, u32, usize, Vec<u8>)> {
    let caps = sample.caps()?;
    let info = gstreamer_video::VideoInfo::from_caps(caps).ok()?;
    if info.format() != gstreamer_video::VideoFormat::Rgba {
        return None;
    }
    let stride = *info.stride().first()? as usize;
    let pixels = sample.buffer()?.map_readable().ok()?.as_slice().to_vec();
    if pixels.len() != stride * info.height() as usize {
        return None;
    }
    Some((info.width(), info.height(), stride, pixels))
}

/// Download video bytes (auth-aware, 50MB cap).
async fn fetch_video_bytes(url: &str) -> Result<Vec<u8>, String> {
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return Err("no video url".to_string());
    }
    let (oauth, cookie) = crate::ui::chat_view::messenger_auth_for_fetch();
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|e| format!("http client: {e}"))?;
    let mut req = client
        .get(url)
        .header(
            "User-Agent",
            "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
        )
        .header("Origin", "https://yandex.ru")
        .header("Referer", "https://yandex.ru/chat");
    if let Some(a) = oauth {
        req = req.header("Authorization", a);
    }
    if let Some(c) = cookie {
        req = req.header("Cookie", c);
    }
    let resp = req.send().await.map_err(|e| format!("fetch: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("fetch HTTP {}", resp.status()));
    }
    let bytes = resp.bytes().await.map_err(|e| format!("body: {e}"))?;
    if bytes.is_empty() {
        return Err("empty video".to_string());
    }
    if bytes.len() > 50 * 1024 * 1024 {
        return Err("video too large".to_string());
    }
    Ok(bytes.to_vec())
}

#[cfg(all(test, feature = "gstreamer"))]
mod tests {
    use super::*;

    /// Produce a ~1s A/V fixture (testsrc → vp8/vorbis → webmmux → file).
    fn make_fixture(path: &Path) {
        use gstreamer::prelude::*;
        gstreamer::init().expect("init");
        let launch = format!(
            "videotestsrc is-live=true pattern=smpte num-buffers=30 ! videoconvert ! vp8enc deadline=1 ! queue ! mux. \
             audiotestsrc is-live=true wave=sine num-buffers=30 ! audioconvert ! vorbisenc ! queue ! mux. \
             webmmux name=mux ! filesink location={}",
            path.display()
        );
        let pipeline = gstreamer::parse::launch(&launch)
            .expect("fixture parse")
            .downcast::<gstreamer::Pipeline>()
            .expect("pipeline");
        pipeline.set_state(gstreamer::State::Playing).expect("play");
        let bus = pipeline.bus().expect("bus");
        let msg = bus
            .timed_pop_filtered(
                gstreamer::ClockTime::from_seconds(20),
                &[gstreamer::MessageType::Eos, gstreamer::MessageType::Error],
            )
            .expect("eos");
        assert!(matches!(msg.view(), gstreamer::MessageView::Eos(..)));
        pipeline.set_state(gstreamer::State::Null).expect("null");
    }

    #[test]
    fn inline_playback_paints_frames() {
        crate::ui::run_gtk_test(|| {
            let dir = std::env::temp_dir().join(format!("ym_vptest_{}", uuid::Uuid::new_v4()));
            std::fs::create_dir_all(&dir).expect("tmpdir");
            let path = dir.join("fixture.webm");
            make_fixture(&path);
            assert!(std::fs::metadata(&path).expect("stat").len() > 0);

            let player = VideoPlayer::new();
            player.set_fake_audio(true);
            player.open_file(&path).expect("open");

            // Drive the pump manually (no main loop in tests).
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
            let mut saw_frame = false;
            let mut saw_position = false;
            while std::time::Instant::now() < deadline {
                player.pump_once();
                if player.test_has_picture() {
                    saw_frame = true;
                }
                if player.test_position_secs() > 0.0 {
                    saw_position = true;
                }
                if saw_frame && saw_position {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            assert!(saw_frame, "no video frame painted");
            assert!(saw_position, "position never advanced");
            let _ = std::fs::remove_dir_all(dir);
        });
    }

    #[test]
    fn stub_paths_reject_gracefully() {
        crate::ui::run_gtk_test(|| {
            let player = VideoPlayer::new();
            // Empty input is rejected synchronously on every build.
            assert!(player.open_bytes(&[], "mp4").is_err());
            #[cfg(not(feature = "gstreamer"))]
            {
                // Missing files fail fast without the backend…
                assert!(player
                    .open_file(Path::new("/nonexistent/ym_video.webm"))
                    .is_err());
            }
        });
    }
}
