#![allow(dead_code)]

use gtk::prelude::*;
use gtk::{Box as GtkBox, Button, Label, Orientation, ProgressBar};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crate::core::voice_player::VoicePlayer;
use crate::models::VoiceMessage;

// Only one voice bubble plays at a time across the whole app.
thread_local! {
    static ACTIVE_VOICE: RefCell<Option<Rc<VoiceMessagePlayer>>> =
        const { RefCell::new(None) };
}

/// VoiceMessagePlayer — UI component for displaying and playing voice messages.
///
/// Features:
/// - Play/Pause button with icon toggle
/// - Progress bar with animated progress
/// - Time label (current / total) formatted as mm:ss
/// - Waveform visualisation rendered via GTK Label markup
/// - Download button
/// - Transcription status indicator (spinner / text / error)
/// - Reply button
/// - Dark mode support via CSS classes
pub struct VoiceMessagePlayer {
    /// Top-level container for the component
    container: GtkBox,
    /// The voice message data
    voice: VoiceMessage,
    /// Whether the message is currently playing
    is_playing: RefCell<bool>,
    /// Current playback progress (0.0–1.0), used by stub animation
    current_progress: RefCell<f64>,
    /// Chat ID for reply callback
    chat_id: RefCell<Option<String>>,
    /// Play/Pause button widget
    play_pause_btn: Button,
    /// Progress bar widget
    progress_bar: ProgressBar,
    /// Time display label (e.g. "0:15 / 0:30")
    time_label: Label,
    /// Waveform visualisation (rendered as markup)
    waveform_label: Label,
    /// Transcription status label
    transcription_label: Label,
    /// Transcription content box (text display or spinner)
    transcription_box: GtkBox,
    /// Download button widget
    download_btn: Button,
    /// Reply button widget
    reply_btn: Button,
    /// On-demand transcription fetch button.
    transcribe_btn: Button,
    /// Transcription fetch in flight.
    transcribe_busy: Rc<Cell<bool>>,
    /// Whether the fetch button is currently offered.
    transcribe_offered: Rc<Cell<bool>>,
    /// Real audio backend (playbin over temp file).
    backend: Rc<RefCell<VoicePlayer>>,
    /// Downloaded bytes, cached for replay without refetch.
    audio_cache: Rc<RefCell<Option<Vec<u8>>>>,
    /// A fetch is in flight (ignore extra taps).
    fetching: Rc<Cell<bool>>,
    /// Media loaded (pause/resume path vs fetch path).
    loaded: Rc<Cell<bool>>,
}

// ── Helper: format seconds as mm:ss ──────────────────────────────────

fn format_time(seconds: f64) -> String {
    let total_secs = seconds as u64;
    let mins = total_secs / 60;
    let secs = total_secs % 60;
    format!("{:02}:{:02}", mins, secs)
}

// ── Helper: generate waveform SVG-like markup for a Label ────────────

fn generate_waveform_markup(waveform: &[f32], width: i32, height: i32) -> String {
    if waveform.is_empty() {
        return String::new();
    }

    let bar_count = waveform.len().min(80); // limit bars for performance
    let gap = 2;
    let bar_width = ((width - (bar_count as i32) * gap) / bar_count as i32).max(1);
    let total_width = bar_count as i32 * (bar_width + gap) - gap;

    let mut svg_parts: Vec<String> = Vec::new();
    svg_parts.push(format!(
        "<svg width=\"{}\" height=\"{}\" xmlns=\"http://www.w3.org/2000/svg\">",
        total_width, height
    ));

    for (i, &amplitude) in waveform.iter().take(bar_count).enumerate() {
        let bar_height = (amplitude * (height as f32)).max(2.0) as i32;
        let x = i as i32 * (bar_width + gap);
        let y = (height - bar_height) / 2;
        svg_parts.push(format!(
            "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"1\" fill=\"#2563eb\"/>",
            x, y, bar_width, bar_height
        ));
    }

    svg_parts.push("</svg>".to_string());
    svg_parts.join("")
}

// ── Helper: build CSS for waveform color based on theme ──────────────

fn waveform_color() -> &'static str {
    // Use theme primary color — GTK will resolve via CSS
    "#2563eb"
}

impl VoiceMessagePlayer {
    /// Create a new VoiceMessagePlayer for the given voice message.
    ///
    /// # Arguments
    /// * `voice` — the VoiceMessage to display
    pub fn new(voice: VoiceMessage) -> Self {
        let is_playing = false;
        let current_progress = 0.0_f64;

        // ── Main container ───────────────────────────────────────────
        let container = GtkBox::new(Orientation::Vertical, 6);
        container.set_css_classes(&["voice-player"]);
        container.set_margin_start(8);
        container.set_margin_end(8);
        container.set_margin_top(4);
        container.set_margin_bottom(4);

        // ── Top row: play button + progress + time ───────────────────
        let top_row = GtkBox::new(Orientation::Horizontal, 6);
        top_row.set_hexpand(true);

        // Play/Pause button
        let play_pause_btn = Button::builder()
            .css_classes(vec!["voice-play-btn"])
            .build();
        Self::update_play_icon(&play_pause_btn, is_playing);
        play_pause_btn.set_size_request(36, 36);

        // Progress bar
        let progress_bar = ProgressBar::builder()
            .fraction(current_progress)
            .show_text(false)
            .build();
        progress_bar.set_hexpand(true);
        progress_bar.set_css_classes(&["voice-progress"]);

        // Time label
        let total_time = format_time(voice.duration);
        let time_label = Label::builder()
            .label(&format!("00:00 / {}", total_time))
            .css_classes(vec!["time-label"])
            .xalign(0.0)
            .hexpand(false)
            .build();

        top_row.append(&play_pause_btn);
        top_row.append(&progress_bar);
        top_row.append(&time_label);

        // ── Waveform row ─────────────────────────────────────────────
        let waveform_container = GtkBox::new(Orientation::Horizontal, 0);
        waveform_container.set_css_classes(&["waveform-container"]);
        waveform_container.set_hexpand(true);
        waveform_container.set_vexpand(false);
        waveform_container.set_margin_start(42); // align with play button

        let waveform_label = Label::builder().use_markup(true).xalign(0.0).build();

        let waveform_markup = generate_waveform_markup(&voice.waveform, 240, 32);
        waveform_label.set_markup(&waveform_markup);

        waveform_container.append(&waveform_label);

        // ── Bottom row: download + reply ─────────────────────────────
        let bottom_row = GtkBox::new(Orientation::Horizontal, 8);
        bottom_row.set_hexpand(true);

        // Download button
        let download_btn = Button::builder()
            .icon_name("download-symbolic")
            .css_classes(["btn-icon"])
            .tooltip_text("Скачать")
            .build();
        download_btn.set_size_request(32, 32);

        // Reply button
        let reply_btn = Button::builder()
            .label("↩ Reply")
            .css_classes(["btn-icon"])
            .tooltip_text("Ответить")
            .build();
        reply_btn.set_size_request(-1, 32);

        bottom_row.append(&download_btn);
        bottom_row.append(&reply_btn);

        // ── Transcription box ────────────────────────────────────────
        let transcription_box = GtkBox::new(Orientation::Vertical, 4);
        transcription_box.set_css_classes(&["transcription-box"]);

        let transcription_label = Label::builder()
            .css_classes(vec!["transcription-text"])
            .wrap(true)
            .selectable(true)
            .xalign(0.0)
            .build();

        // On-demand recognition (server-side; fetched once per bubble).
        let transcribe_btn = Button::builder()
            .label("Распознать речь")
            .css_classes(["btn-text", "transcribe-btn"])
            .tooltip_text("Запросить транскрипцию у сервера")
            .build();
        transcribe_btn.set_halign(gtk::Align::Start);

        // One of: spinner (transcribing), text, error, or fetch button.
        // (Box starts visible; every branch below sets both flags.)
        // Visibility is tracked in `transcribe_offered`: GtkWidget::is_visible
        // folds in unmapped ancestors, so it can't drive this logic.
        let offered = Rc::new(Cell::new(false));
        if voice.is_transcribing {
            Self::set_transcription_spinning(&transcription_label);
            transcription_box.set_visible(true);
            transcribe_btn.set_visible(false);
        } else if let Some(text) = voice.transcribed_text.as_deref().filter(|t| !t.is_empty()) {
            transcription_label.set_label(text);
            transcription_box.set_visible(true);
            transcribe_btn.set_visible(false);
        } else if let Some(ref err) = voice.transcribe_error {
            transcription_label.set_label(&format!("Error: {}", err));
            transcription_label.add_css_class("transcription-error");
            transcription_box.set_visible(true);
            transcribe_btn.set_visible(false);
        } else {
            // Nothing yet: offer one-tap fetch instead of an empty box.
            transcription_box.set_visible(true);
            transcribe_btn.set_visible(true);
            offered.set(true);
        }

        transcription_box.append(&transcription_label);
        transcription_box.append(&transcribe_btn);

        // ── Assemble ─────────────────────────────────────────────────
        container.append(&top_row);
        container.append(&waveform_container);
        container.append(&bottom_row);
        container.append(&transcription_box);

        let this = VoiceMessagePlayer {
            container,
            voice,
            is_playing: RefCell::new(is_playing),
            current_progress: RefCell::new(current_progress),
            chat_id: RefCell::new(None),
            play_pause_btn,
            progress_bar,
            time_label,
            waveform_label,
            transcription_label,
            transcription_box,
            download_btn,
            reply_btn,
            transcribe_btn,
            transcribe_busy: Rc::new(Cell::new(false)),
            transcribe_offered: offered.clone(),
            backend: Rc::new(RefCell::new(VoicePlayer::new())),
            audio_cache: Rc::new(RefCell::new(None)),
            fetching: Rc::new(Cell::new(false)),
            loaded: Rc::new(Cell::new(false)),
        };
        this.enable_transcribe_fetch();
        this
    }

    /// Wire the one-tap transcription fetch (called once at construction).
    fn enable_transcribe_fetch(&self) {
        let this = Rc::new(self.clone_ref());
        let message_id = self.voice.message_id.clone();
        self.transcribe_btn.connect_clicked(move |_| {
            // Single flight per bubble; the button hides while fetching.
            if this.transcribe_busy.get() {
                return;
            }
            this.transcribe_busy.set(true);
            this.transcribe_btn.set_visible(false);
            this.transcribe_offered.set(false);
            this.update_transcription(true, None, None);
            let this = this.clone();
            let message_id = message_id.clone();
            glib::spawn_future_local(async move {
                match fetch_transcription(&message_id).await {
                    Ok(text) => {
                        this.transcribe_busy.set(false);
                        this.update_transcription(false, text, None);
                    }
                    Err(e) => {
                        log::warn!("transcription failed: {e}");
                        this.transcribe_busy.set(false);
                        this.update_transcription(false, None, Some(e));
                        // Offer retry on failure (after update hid the button).
                        this.transcribe_btn.set_label("Повторить");
                        this.transcribe_btn.set_visible(true);
                        this.transcribe_offered.set(true);
                        this.transcription_box.set_visible(true);
                    }
                }
            });
        });
    }

    /// Update the play/pause button icon based on playing state.
    fn update_play_icon(btn: &Button, playing: bool) {
        if playing {
            btn.set_icon_name("media-playback-pause-symbolic");
        } else {
            btn.set_icon_name("media-playback-start-symbolic");
        }
    }

    /// Set transcription label to show a spinning indicator.
    fn set_transcription_spinning(label: &Label) {
        let markup = "<span foreground=\"#ff3b30\">⟳</span> <i>Транскрипция...</i>";
        label.set_markup(markup);
    }

    // ── Public methods ──────────────────────────────────────────────

    /// Toggle play/pause with the real GStreamer backend.
    ///
    /// First tap downloads the audio (cached for replay), then `playbin`
    /// drives `progress_bar`/`time_label` through a 100ms pump. Without the
    /// `gstreamer` feature the backend reports an honest error instead of a
    /// fake animation.
    pub fn toggle_play(&self) {
        let this = Rc::new(self.clone_ref());
        Self::toggle_shared(this);
    }

    fn toggle_shared(this: Rc<Self>) {
        // Pause path.
        if this.backend.borrow().is_playing() {
            this.backend.borrow_mut().pause();
            *this.is_playing.borrow_mut() = false;
            Self::update_play_icon(&this.play_pause_btn, false);
            return;
        }
        // Resume path (media already loaded).
        if this.loaded.get() {
            // Stop any other bubble first.
            Self::claim_active(&this);
            match this.backend.borrow_mut().resume() {
                Ok(()) => {
                    *this.is_playing.borrow_mut() = true;
                    Self::update_play_icon(&this.play_pause_btn, true);
                    Self::start_progress_pump(this.clone());
                }
                Err(e) => {
                    log::warn!("voice resume failed: {e}");
                    this.loaded.set(false);
                }
            }
            return;
        }
        // Fetch path (first tap).
        if this.fetching.get() {
            return;
        }
        this.fetching.set(true);
        this.play_pause_btn.set_sensitive(false);
        let url = this.voice.url.clone();
        glib::spawn_future_local(async move {
            let result = fetch_audio_bytes(&url).await;
            this.fetching.set(false);
            this.play_pause_btn.set_sensitive(true);
            match result {
                Ok(bytes) => {
                    *this.audio_cache.borrow_mut() = Some(bytes.clone());
                    match this.backend.borrow_mut().play_bytes(&bytes, "ogg") {
                        Ok(()) => {
                            this.loaded.set(true);
                            Self::claim_active(&this);
                            *this.is_playing.borrow_mut() = true;
                            Self::update_play_icon(&this.play_pause_btn, true);
                            Self::start_progress_pump(this.clone());
                        }
                        Err(e) => {
                            log::warn!("voice play failed: {e}");
                        }
                    }
                }
                Err(e) => {
                    log::warn!("voice download failed: {e}");
                }
            }
        });
    }

    /// Stop whatever is playing and mark `this` active.
    fn claim_active(this: &Rc<Self>) {
        let prev = ACTIVE_VOICE.replace(Some(this.clone()));
        if let Some(old) = prev {
            if !Rc::ptr_eq(&old, this) {
                old.stop_playback();
            }
        }
    }

    /// Halt playback and reset the button (used when another bubble starts).
    pub fn stop_playback(&self) {
        self.backend.borrow_mut().stop();
        self.loaded.set(false);
        *self.is_playing.borrow_mut() = false;
        Self::update_play_icon(&self.play_pause_btn, false);
        self.set_progress(0.0);
    }

    /// 100ms pump: bus (EOS) + position → progress bar and time label.
    fn start_progress_pump(this: Rc<Self>) {
        let backend = this.backend.clone();
        glib::timeout_add_local(std::time::Duration::from_millis(100), move || {
            let mut player = backend.borrow_mut();
            player.pump();
            if player.eos_reached() {
                drop(player);
                *this.is_playing.borrow_mut() = false;
                this.loaded.set(false);
                Self::update_play_icon(&this.play_pause_btn, false);
                this.set_progress(1.0);
                return glib::ControlFlow::Break;
            }
            let position = player.position().map(|p| p.as_secs_f64()).unwrap_or(0.0);
            let duration = player
                .duration()
                .map(|d| d.as_secs_f64())
                .filter(|d| *d > 0.0)
                .unwrap_or(this.voice.duration.max(0.1));
            drop(player);
            this.set_progress((position / duration).clamp(0.0, 1.0));
            if !*this.is_playing.borrow() {
                return glib::ControlFlow::Break;
            }
            glib::ControlFlow::Continue
        });
    }

    /// Set the playback progress (0.0–1.0).
    pub fn set_progress(&self, progress: f64) {
        let clamped = progress.clamp(0.0, 1.0);
        self.progress_bar.set_fraction(clamped);

        let elapsed = clamped * self.voice.duration;
        let total_time = format_time(self.voice.duration);
        let elapsed_time = format_time(elapsed);
        self.time_label
            .set_label(&format!("{} / {}", elapsed_time, total_time));
    }

    /// Connect callback for play/pause button clicks.
    ///
    /// The callback receives the voice message ID.
    pub fn on_play_click(&self, callback: impl Fn(String) + 'static) {
        let player = Rc::new(self.clone_ref());
        let message_id = self.voice.message_id.clone();
        self.play_pause_btn.connect_clicked(move |_| {
            player.toggle_play();
            callback(message_id.clone());
        });
    }

    /// Connect callback for download button clicks.
    ///
    /// The callback receives the voice message ID.
    pub fn on_download_click(&self, callback: impl Fn(String) + 'static) {
        let message_id = self.voice.message_id.clone();
        let download_btn = self.download_btn.clone();
        download_btn.connect_clicked(move |_| {
            callback(message_id.clone());
        });
    }

    /// Connect callback for reply button clicks.
    ///
    /// The callback receives `(chat_id, text)` where text is the
    /// transcription of the voice message (if available).
    pub fn on_reply_click(&self, callback: impl Fn(String, String) + 'static) {
        let chat_id = self.chat_id.clone();
        let text = self.voice.transcribed_text.clone().unwrap_or_default();
        self.reply_btn.connect_clicked(move |_| {
            let id = chat_id.borrow().clone().unwrap_or_default();
            callback(id, text.clone());
        });
    }

    /// Update the transcription display.
    ///
    /// Call this when transcription status changes (e.g. after receiving
    /// a WebSocket update).
    pub fn update_transcription(
        &self,
        is_transcribing: bool,
        text: Option<String>,
        error: Option<String>,
    ) {
        // The fetch button only lives while there is nothing to show.
        let has_content =
            is_transcribing || text.as_ref().map_or(false, |t| !t.is_empty()) || error.is_some();
        if has_content {
            self.transcribe_btn.set_visible(false);
            self.transcribe_offered.set(false);
        }
        self.transcription_box
            .set_visible(has_content || self.transcribe_offered.get());

        if is_transcribing {
            Self::set_transcription_spinning(&self.transcription_label);
            self.transcription_label
                .remove_css_class("transcription-error");
        } else if let Some(ref t) = text {
            if !t.is_empty() {
                self.transcription_label.set_label(t);
                self.transcription_label
                    .remove_css_class("transcription-error");
            }
        } else if let Some(ref err) = error {
            self.transcription_label
                .set_label(&format!("Error: {}", err));
            self.transcription_label
                .add_css_class("transcription-error");
        }
    }

    /// Get a reference to the top-level container widget.
    pub fn container(&self) -> &GtkBox {
        &self.container
    }

    /// Set the chat ID for reply callbacks.
    pub fn set_chat_id(&self, chat_id: String) {
        *self.chat_id.borrow_mut() = Some(chat_id);
    }

    /// Clone self for use in callbacks (Rc-style pattern).
    fn clone_ref(&self) -> Self {
        VoiceMessagePlayer {
            container: self.container.clone(),
            voice: self.voice.clone(),
            is_playing: self.is_playing.clone(),
            current_progress: self.current_progress.clone(),
            chat_id: self.chat_id.clone(),
            play_pause_btn: self.play_pause_btn.clone(),
            progress_bar: self.progress_bar.clone(),
            time_label: self.time_label.clone(),
            waveform_label: self.waveform_label.clone(),
            transcription_label: self.transcription_label.clone(),
            transcription_box: self.transcription_box.clone(),
            download_btn: self.download_btn.clone(),
            reply_btn: self.reply_btn.clone(),
            transcribe_btn: self.transcribe_btn.clone(),
            transcribe_busy: self.transcribe_busy.clone(),
            transcribe_offered: self.transcribe_offered.clone(),
            backend: self.backend.clone(),
            audio_cache: self.audio_cache.clone(),
            fetching: self.fetching.clone(),
            loaded: self.loaded.clone(),
        }
    }
}

/// Download voice bytes (auth-aware, 6MB cap). Mirrors the attachment
/// fetcher in `chat_view` without depending on it.
async fn fetch_audio_bytes(url: &str) -> Result<Vec<u8>, String> {
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return Err("no voice url".to_string());
    }
    let (oauth, cookie) = crate::ui::chat_view::messenger_auth_for_fetch();
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
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
        return Err("empty audio".to_string());
    }
    if bytes.len() > 6 * 1024 * 1024 {
        return Err("audio too large".to_string());
    }
    Ok(bytes.to_vec())
}

/// Tolerant transcription payload parse (mirrors the API client shapes).
fn parse_transcription_text(json: &serde_json::Value) -> Option<String> {
    if let Some(t) = json.get("text").and_then(|t| t.as_str()) {
        if !t.trim().is_empty() {
            return Some(t.to_string());
        }
    }
    if let Some(msg) = json.get("message") {
        if let Some(t) = msg.get("text").and_then(|t| t.as_str()) {
            if !t.trim().is_empty() {
                return Some(t.to_string());
            }
        }
    }
    None
}

/// Fetch server-side transcription for a voice message id.
async fn fetch_transcription(message_id: &str) -> Result<Option<String>, String> {
    if message_id.trim().is_empty() {
        return Err("no message id".to_string());
    }
    let (oauth, cookie) = crate::ui::chat_view::messenger_auth_for_fetch();
    let url = format!(
        "{}api/get_transcription?messageId={}",
        crate::config::API_BASE_URL,
        urlencoding::encode(message_id)
    );
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| format!("http client: {e}"))?;
    let mut req = client
        .get(&url)
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
    if resp.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }
    if !resp.status().is_success() {
        return Err(format!("fetch HTTP {}", resp.status()));
    }
    let json: serde_json::Value = resp.json().await.map_err(|e| format!("parse: {e}"))?;
    Ok(parse_transcription_text(&json))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transcription_parse_shapes() {
        assert_eq!(
            parse_transcription_text(&serde_json::json!({"text": "привет"})).as_deref(),
            Some("привет")
        );
        assert_eq!(
            parse_transcription_text(&serde_json::json!({"message": {"text": "x"}})).as_deref(),
            Some("x")
        );
        assert!(parse_transcription_text(&serde_json::json!({"text": "  "})).is_none());
        assert!(parse_transcription_text(&serde_json::json!({})).is_none());
    }

    #[test]
    fn transcribe_button_lifecycle() {
        crate::ui::run_gtk_test(|| {
            // Fresh bubble without transcription offers one-tap fetch.
            let voice = VoiceMessage::new(
                "m1".into(),
                "https://example.invalid/v.ogg".into(),
                3.0,
                vec![],
            );
            let player = VoiceMessagePlayer::new(voice);
            assert!(player.transcribe_offered.get());
            // Result hides the button; error re-offers retry.
            player.update_transcription(false, Some("hi".into()), None);
            assert!(!player.transcribe_offered.get());
            player.update_transcription(false, None, Some("nope".into()));
            // update() itself hides; the fetch flow re-shows explicitly.
            assert!(!player.transcribe_offered.get());
        });
    }
}
