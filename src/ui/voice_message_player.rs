#![allow(dead_code)]

use gtk::prelude::*;
use gtk::{Box as GtkBox, Button, Label, Orientation, ProgressBar};
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Instant;

use crate::models::VoiceMessage;

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

        let waveform_markup = generate_waveform_markup(&voice.waveform, 400, 32);
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
        transcription_box.set_visible(voice.is_transcribing || voice.has_transcription());

        let transcription_label = Label::builder()
            .css_classes(vec!["transcription-text"])
            .wrap(true)
            .selectable(true)
            .xalign(0.0)
            .build();

        if voice.is_transcribing {
            Self::set_transcription_spinning(&transcription_label);
        } else if let Some(ref text) = voice.transcribed_text {
            if !text.is_empty() {
                transcription_label.set_label(text);
            }
        } else if let Some(ref err) = voice.transcribe_error {
            transcription_label.set_label(&format!("Error: {}", err));
            transcription_label.add_css_class("transcription-error");
        }

        transcription_box.append(&transcription_label);

        // ── Assemble ─────────────────────────────────────────────────
        container.append(&top_row);
        container.append(&waveform_container);
        container.append(&bottom_row);
        container.append(&transcription_box);

        VoiceMessagePlayer {
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
        }
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

    /// Toggle play/pause state.
    ///
    /// In the stub implementation this only updates the UI. A real
    /// implementation would use GStreamer to start/stop playback.
    pub fn toggle_play(&self) {
        let mut playing = self.is_playing.borrow_mut();
        *playing = !*playing;

        Self::update_play_icon(&self.play_pause_btn, *playing);

        if *playing {
            // Start stub progress animation using std::time::Instant
            let progress = self.current_progress.clone();
            let player = Rc::new(self.clone_ref());
            let start_time = Instant::now();

            glib::timeout_add_local(std::time::Duration::from_millis(50), move || {
                let elapsed = start_time.elapsed().as_secs_f64();
                let duration_secs = player.voice.duration;
                let new_progress = (elapsed / duration_secs).min(1.0);

                if new_progress >= 1.0 {
                    // Reset on completion
                    *progress.borrow_mut() = 0.0;
                    *player.is_playing.borrow_mut() = false;
                    Self::update_play_icon(&player.play_pause_btn, false);
                    return glib::ControlFlow::Break;
                }

                *progress.borrow_mut() = new_progress;
                glib::ControlFlow::Continue
            });
        } else {
            // Pause: keep current progress
        }
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
        self.transcription_box.set_visible(
            is_transcribing || text.as_ref().map_or(false, |t| !t.is_empty()) || error.is_some(),
        );

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
        }
    }
}
