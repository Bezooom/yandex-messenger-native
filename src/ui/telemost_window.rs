//! Telemost call window: live call UI bound to [`CallController`].
//!
//! - Outgoing: `attach_call(handle, …)` pumps [`CallEvent`]s (state, roster,
//!   remote video frames) into widgets.
//! - Incoming: `show_incoming(peer, meeting)` shows the ringing bar;
//!   `on_accept` fires with the meeting id — the app joins (REST →
//!   [`CallController::spawn`] → `attach_call`). Auto-triggering from push
//!   traffic is a follow-up (needs `dialog_history`/push routing); the hook
//!   point is [`TelemostWindow::show_incoming`].
//! - Remote video renders via `gdk::MemoryTexture` on a [`gtk::Picture`]
//!   (frames arrive as [`CallEvent::RemoteFrame`], no extra plugins needed).
//!   Without the `gstreamer` feature the window still joins signaling-only
//!   (roster works) and says so.

use gtk::prelude::*;
use gtk::{
    gdk, Application, ApplicationWindow, Box, Button, Image, Label, Orientation, Overlay, Picture,
    ScrolledWindow, Stack,
};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;
use std::time::Instant;

use crate::api::goloom_call::{CallControl, CallHandle, CallState};
use crate::api::goloom_media::{PeerState, VideoFrame};
use crate::api::telemost::TelemostClient;
use crate::models::telemost::{PersonalMeeting, TelemostParticipant};

/// Russian labels for call states.
pub fn call_state_label(state: &CallState) -> &'static str {
    match state {
        CallState::Joining => "Подключение…",
        CallState::Joined => "Подключено",
        CallState::InCall => "В звонке",
        CallState::Reconnecting => "Переподключение…",
        CallState::Ended { .. } => "Звонок завершён",
        CallState::Failed { .. } => "Ошибка звонка",
    }
}

#[derive(Clone)]
pub struct TelemostWindow {
    window: ApplicationWindow,
    title_label: Label,
    state_label: Label,
    timer_label: Label,
    video_stack: Stack,
    remote_picture: Picture,
    preview_picture: Picture,
    no_video_label: Label,
    mute_btn: Button,
    cam_btn: Button,
    share_btn: Button,
    end_btn: Button,
    copy_link_btn: Button,
    browser_btn: Button,
    participants_list: Box,
    participant_count_label: Label,
    ring_box: Box,
    ring_label: Label,
    accept_btn: Button,
    decline_btn: Button,
    ring_shown: Rc<Cell<bool>>,
    notice_label: Label,
    client: Arc<TelemostClient>,
    control: Rc<RefCell<Option<CallControl>>>,
    meeting_id: Rc<RefCell<Option<String>>>,
    join_url: Rc<RefCell<Option<String>>>,
    mic_on: Rc<Cell<bool>>,
    cam_on: Rc<Cell<bool>>,
    sharing_on: Rc<Cell<bool>>,
    call_start: Rc<RefCell<Option<Instant>>>,
    last_frame_at: Rc<RefCell<Option<Instant>>>,
    last_preview_at: Rc<RefCell<Option<Instant>>>,
    on_accept: Rc<RefCell<Option<std::boxed::Box<dyn Fn(String)>>>>,
    ended: Rc<Cell<bool>>,
}

impl TelemostWindow {
    pub fn new(app: &Application, telemost_client: Arc<TelemostClient>) -> Self {
        let window = ApplicationWindow::builder()
            .application(app)
            .title("Телемост")
            .default_width(1100)
            .default_height(760)
            .build();

        let main_box = Box::new(Orientation::Vertical, 0);
        window.set_child(Some(&main_box));

        // ── top bar ──
        let top_bar = Box::new(Orientation::Horizontal, 12);
        top_bar.set_margin_top(12);
        top_bar.set_margin_bottom(8);
        top_bar.set_margin_start(16);
        top_bar.set_margin_end(16);

        let title_label = Label::new(Some("Телемост"));
        title_label.add_css_class("title-2");
        title_label.set_halign(gtk::Align::Start);
        title_label.set_hexpand(true);
        top_bar.append(&title_label);

        let state_label = Label::new(Some("Готов"));
        state_label.add_css_class("dim-label");
        top_bar.append(&state_label);

        let timer_label = Label::new(Some(""));
        timer_label.add_css_class("dim-label");
        timer_label.set_width_chars(5);
        top_bar.append(&timer_label);

        main_box.append(&top_bar);

        // ── ringing bar (hidden until show_incoming) ──
        let ring_box = Box::new(Orientation::Horizontal, 12);
        ring_box.set_margin_start(16);
        ring_box.set_margin_end(16);
        ring_box.set_margin_bottom(8);
        ring_box.add_css_class("card");
        ring_box.set_visible(false);

        let ring_icon = Image::from_icon_name("phone-symbolic");
        ring_box.append(&ring_icon);

        let ring_label = Label::new(Some("Входящий звонок"));
        ring_label.set_hexpand(true);
        ring_label.set_halign(gtk::Align::Start);
        ring_box.append(&ring_label);

        let accept_btn = Button::with_label("Принять");
        accept_btn.add_css_class("suggested-action");
        ring_box.append(&accept_btn);

        let decline_btn = Button::with_label("Отклонить");
        ring_box.append(&decline_btn);

        main_box.append(&ring_box);

        // ── video area ──
        let video_stack = Stack::new();
        video_stack.set_hexpand(true);
        video_stack.set_vexpand(true);

        let remote_picture = Picture::new();
        remote_picture.set_hexpand(true);
        remote_picture.set_vexpand(true);
        remote_picture.set_content_fit(gtk::ContentFit::Cover);
        remote_picture.set_can_shrink(true);
        video_stack.add_named(&remote_picture, Some("video"));

        let empty_box = Box::new(Orientation::Vertical, 0);
        empty_box.set_halign(gtk::Align::Center);
        empty_box.set_valign(gtk::Align::Center);
        empty_box.set_hexpand(true);
        empty_box.set_vexpand(true);
        empty_box.add_css_class("card");
        let empty_icon = Image::from_icon_name("video-display-symbolic");
        empty_box.append(&empty_icon);
        let no_video_label = Label::new(Some("Камера собеседника пока не видна"));
        no_video_label.add_css_class("dim-label");
        no_video_label.set_margin_top(8);
        empty_box.append(&no_video_label);
        video_stack.add_named(&empty_box, Some("empty"));
        video_stack.set_visible_child_name("empty");

        // Local preview (picture-in-picture) over the remote video.
        let overlay = Overlay::new();
        overlay.set_hexpand(true);
        overlay.set_vexpand(true);
        overlay.set_margin_start(16);
        overlay.set_margin_end(16);
        overlay.set_margin_bottom(8);
        overlay.set_child(Some(&video_stack));

        let preview_picture = Picture::new();
        preview_picture.set_size_request(240, 135);
        preview_picture.set_content_fit(gtk::ContentFit::Cover);
        preview_picture.set_can_shrink(true);
        preview_picture.set_halign(gtk::Align::End);
        preview_picture.set_valign(gtk::Align::End);
        preview_picture.set_margin_end(12);
        preview_picture.set_margin_bottom(12);
        preview_picture.add_css_class("card");
        overlay.add_overlay(&preview_picture);

        main_box.append(&overlay);

        // ── notice (media-less builds, errors) ──
        let notice_label = Label::new(None);
        notice_label.add_css_class("dim-label");
        notice_label.set_margin_start(16);
        notice_label.set_margin_end(16);
        notice_label.set_visible(false);
        main_box.append(&notice_label);

        // ── controls ──
        let control_box = Box::new(Orientation::Horizontal, 12);
        control_box.set_margin_start(16);
        control_box.set_margin_end(16);
        control_box.set_margin_bottom(8);
        control_box.set_halign(gtk::Align::Center);

        let mute_btn = Button::new();
        mute_btn.add_css_class("circular");
        mute_btn.set_size_request(48, 48);
        mute_btn.set_tooltip_text(Some("Микрофон вкл/выкл"));
        mute_btn.set_icon_name("audio-input-microphone-symbolic");
        control_box.append(&mute_btn);

        let cam_btn = Button::new();
        cam_btn.add_css_class("circular");
        cam_btn.set_size_request(48, 48);
        cam_btn.set_tooltip_text(Some("Камера вкл/выкл"));
        cam_btn.set_icon_name("camera-video-symbolic");
        control_box.append(&cam_btn);

        let share_btn = Button::new();
        share_btn.add_css_class("circular");
        share_btn.set_size_request(48, 48);
        share_btn.set_tooltip_text(Some("Демонстрация экрана вкл/выкл"));
        share_btn.set_icon_name("view-fullscreen-symbolic");
        control_box.append(&share_btn);

        let copy_link_btn = Button::with_label("Ссылка");
        copy_link_btn.set_tooltip_text(Some("Скопировать ссылку на встречу"));
        control_box.append(&copy_link_btn);

        let browser_btn = Button::with_label("В браузере");
        browser_btn.set_tooltip_text(Some("Открыть встречу в браузере (запасной путь)"));
        control_box.append(&browser_btn);

        let end_btn = Button::with_label("Завершить");
        end_btn.add_css_class("destructive-action");
        end_btn.set_size_request(120, 48);
        control_box.append(&end_btn);

        main_box.append(&control_box);

        // ── participants ──
        let participants_header = Box::new(Orientation::Horizontal, 8);
        participants_header.set_margin_start(16);
        participants_header.set_margin_end(16);
        participants_header.set_margin_bottom(4);
        let participants_title = Label::new(Some("Участники"));
        participants_title.add_css_class("title-3");
        participants_header.append(&participants_title);
        let participant_count_label = Label::new(Some("0"));
        participant_count_label.add_css_class("dim-label");
        participants_header.append(&participant_count_label);
        main_box.append(&participants_header);

        let participants_container = ScrolledWindow::new();
        participants_container.set_vexpand(false);
        participants_container.set_height_request(180);
        participants_container.set_margin_start(16);
        participants_container.set_margin_end(16);
        participants_container.set_margin_bottom(16);
        let participants_list = Box::new(Orientation::Vertical, 4);
        participants_container.set_child(Some(&participants_list));
        main_box.append(&participants_container);

        let this = Self {
            window,
            title_label,
            state_label,
            timer_label,
            video_stack,
            remote_picture,
            preview_picture,
            no_video_label,
            mute_btn,
            cam_btn,
            share_btn,
            end_btn,
            copy_link_btn,
            browser_btn,
            participants_list,
            participant_count_label,
            ring_box,
            ring_label,
            accept_btn,
            decline_btn,
            ring_shown: Rc::new(Cell::new(false)),
            notice_label,
            client: telemost_client,
            control: Rc::new(RefCell::new(None)),
            meeting_id: Rc::new(RefCell::new(None)),
            join_url: Rc::new(RefCell::new(None)),
            mic_on: Rc::new(Cell::new(true)),
            cam_on: Rc::new(Cell::new(true)),
            sharing_on: Rc::new(Cell::new(false)),
            call_start: Rc::new(RefCell::new(None)),
            last_frame_at: Rc::new(RefCell::new(None)),
            last_preview_at: Rc::new(RefCell::new(None)),
            on_accept: Rc::new(RefCell::new(None)),
            ended: Rc::new(Cell::new(false)),
        };
        this.bind_callbacks();
        this.start_timer();
        this
    }

    fn bind_callbacks(&self) {
        // Mute toggles.
        {
            let control = self.control.clone();
            let mic_on = self.mic_on.clone();
            let btn = self.mute_btn.clone();
            self.mute_btn.connect_clicked(move |_| {
                let next = !mic_on.get();
                mic_on.set(next);
                btn.set_icon_name(if next {
                    "audio-input-microphone-symbolic"
                } else {
                    "microphone-disabled-symbolic"
                });
                if let Some(c) = control.borrow().as_ref() {
                    if let Err(e) = c.mute_audio(next) {
                        log::warn!("mute audio failed: {e}");
                    }
                }
            });
        }
        {
            let control = self.control.clone();
            let cam_on = self.cam_on.clone();
            let btn = self.cam_btn.clone();
            let this_preview = self.preview_picture.clone();
            self.cam_btn.connect_clicked(move |_| {
                let next = !cam_on.get();
                cam_on.set(next);
                btn.set_icon_name(if next {
                    "camera-video-symbolic"
                } else {
                    "camera-disabled-symbolic"
                });
                if !next {
                    // Never show a frozen self-view as live.
                    this_preview.set_paintable(Option::<&gdk::Texture>::None);
                }
                if let Some(c) = control.borrow().as_ref() {
                    if let Err(e) = c.mute_video(next) {
                        log::warn!("mute video failed: {e}");
                    }
                }
            });
        }
        {
            let control = self.control.clone();
            let sharing_on = self.sharing_on.clone();
            let btn = self.share_btn.clone();
            self.share_btn.connect_clicked(move |_| {
                let next = !sharing_on.get();
                sharing_on.set(next);
                btn.set_icon_name(if next {
                    "view-fullscreen-symbolic"
                } else {
                    "view-restore-symbolic"
                });
                if let Some(c) = control.borrow().as_ref() {
                    if let Err(e) = c.share(next) {
                        log::warn!("share failed: {e}");
                    }
                }
            });
        }
        // Hangup: end media + best-effort REST meeting end.
        {
            let this = self.clone();
            self.end_btn.connect_clicked(move |_| {
                this.hangup();
            });
        }
        // Copy invite link.
        {
            let join_url = self.join_url.clone();
            self.copy_link_btn.connect_clicked(move |_| {
                let Some(url) = join_url.borrow().clone() else {
                    return;
                };
                if let Some(display) = gdk::Display::default() {
                    display.clipboard().set_text(&url);
                }
            });
        }
        // Browser fallback (D1-minimum path, always available).
        {
            let join_url = self.join_url.clone();
            self.browser_btn.connect_clicked(move |_| {
                let Some(url) = join_url.borrow().clone() else {
                    return;
                };
                if let Err(e) = std::process::Command::new("xdg-open").arg(&url).spawn() {
                    log::warn!("xdg-open failed: {e}");
                }
            });
        }
        // Ringing accept/decline.
        {
            let this = self.clone();
            self.accept_btn.connect_clicked(move |_| {
                this.accept_current();
            });
        }
        {
            let ring_box = self.ring_box.clone();
            let ring_shown = self.ring_shown.clone();
            self.decline_btn.connect_clicked(move |_| {
                ring_box.set_visible(false);
                ring_shown.set(false);
            });
        }
        // Close request ends the call too.
        {
            let this = self.clone();
            self.window.connect_close_request(move |_| {
                this.hangup();
                gtk::glib::Propagation::Proceed
            });
        }
    }

    fn start_timer(&self) {
        let window_weak = self.window.downgrade();
        let label_weak = self.timer_label.downgrade();
        let start = self.call_start.clone();
        let ended = self.ended.clone();
        glib::timeout_add_seconds_local(1, move || {
            let (Some(_w), Some(label)) = (window_weak.upgrade(), label_weak.upgrade()) else {
                return glib::ControlFlow::Break;
            };
            if ended.get() {
                return glib::ControlFlow::Continue;
            }
            if let Some(t0) = *start.borrow() {
                let secs = t0.elapsed().as_secs();
                label.set_text(&format!("{:02}:{:02}", secs / 60, secs % 60));
            }
            glib::ControlFlow::Continue
        });
    }

    /// Attach a live call: pumps [`CallEvent`]s into the widgets.
    pub fn attach_call(
        &self,
        mut handle: CallHandle,
        title: &str,
        join_url: Option<String>,
        meeting_id: Option<String>,
    ) {
        *self.control.borrow_mut() = Some(handle.control());
        *self.join_url.borrow_mut() = join_url;
        *self.meeting_id.borrow_mut() = meeting_id;
        *self.call_start.borrow_mut() = Some(Instant::now());
        self.ended.set(false);
        self.title_label.set_text(title);
        self.ring_box.set_visible(false);
        self.ring_shown.set(false);
        self.apply_state(&CallState::Joining);

        let this = self.clone();
        glib::spawn_future_local(async move {
            while let Some(ev) = handle.next_event().await {
                let done = this.apply_call_event(ev);
                if done {
                    break;
                }
            }
        });
    }

    /// Show the incoming-call bar. Accept fires [`TelemostWindow::on_accept`].
    ///
    /// Entry point for the future push trigger (incoming messenger traffic →
    /// meeting → ring). Currently exercised by tests and manual calls.
    #[allow(dead_code)]
    pub fn show_incoming(&self, peer_name: &str, meeting: &PersonalMeeting) {
        *self.meeting_id.borrow_mut() = Some(meeting.meeting_id.clone());
        *self.join_url.borrow_mut() = meeting.join_url.clone();
        self.ring_label
            .set_text(&format!("Входящий звонок: {peer_name}"));
        self.title_label.set_text(&format!("Входящий: {peer_name}"));
        self.state_label.set_text("Входящий звонок…");
        self.ring_box.set_visible(true);
        self.ring_shown.set(true);
    }

    #[allow(dead_code)]
    pub fn on_accept(&self, cb: impl Fn(String) + 'static) {
        *self.on_accept.borrow_mut() = Some(std::boxed::Box::new(cb));
    }

    pub fn set_notice(&self, text: &str) {
        self.notice_label.set_text(text);
        self.notice_label.set_visible(!text.is_empty());
    }

    /// Returns `true` when the pump should stop.
    fn apply_call_event(&self, ev: crate::api::goloom_call::CallEvent) -> bool {
        use crate::api::goloom_call::CallEvent as E;
        match ev {
            E::State(s) => {
                self.apply_state(&s);
                matches!(s, CallState::Ended { .. } | CallState::Failed { .. })
            }
            E::Roster(list) => {
                self.update_roster(&list);
                false
            }
            E::Media(PeerState::Failed) => {
                self.set_notice("Медиа-соединение потеряно");
                false
            }
            E::Media(_) => false,
            E::RemoteFrame(frame) => {
                self.render_frame(&frame);
                false
            }
            E::PreviewFrame(frame) => {
                self.render_preview(&frame);
                false
            }
            E::Error(e) => {
                log::warn!("call event error: {e}");
                self.set_notice(&e);
                false
            }
        }
    }

    pub fn apply_state(&self, state: &CallState) {
        self.state_label.set_text(call_state_label(state));
        if matches!(state, CallState::Ended { .. } | CallState::Failed { .. }) {
            self.ended.set(true);
            if let CallState::Failed { reason } = state {
                self.set_notice(reason);
            }
        }
    }

    pub fn update_roster(&self, participants: &[TelemostParticipant]) {
        while let Some(child) = self.participants_list.first_child() {
            self.participants_list.remove(&child);
        }
        self.participant_count_label
            .set_text(&participants.len().to_string());
        for p in participants {
            self.participants_list.append(&Self::participant_row(p));
        }
    }

    fn participant_row(p: &TelemostParticipant) -> Box {
        let row = Box::new(Orientation::Horizontal, 12);
        row.set_margin_bottom(4);

        let avatar = Box::new(Orientation::Vertical, 0);
        avatar.set_size_request(40, 40);
        avatar.set_valign(gtk::Align::Center);
        avatar.add_css_class("avatar");
        let initials = p
            .name
            .as_deref()
            .unwrap_or("?")
            .chars()
            .take(2)
            .map(|c| c.to_uppercase().to_string())
            .collect::<Vec<_>>()
            .join("");
        let avatar_label = Label::new(Some(&initials));
        avatar_label.add_css_class("avatar-label");
        avatar.append(&avatar_label);
        row.append(&avatar);

        let name_label = Label::new(Some(p.name.as_deref().unwrap_or("Без имени")));
        name_label.set_halign(gtk::Align::Start);
        name_label.set_hexpand(true);
        row.append(&name_label);

        let audio_icon = Image::from_icon_name(if p.audio_enabled.unwrap_or(false) {
            "audio-input-microphone-symbolic"
        } else {
            "microphone-disabled-symbolic"
        });
        audio_icon.add_css_class("dim-label");
        row.append(&audio_icon);

        let video_icon = Image::from_icon_name(if p.video_enabled.unwrap_or(false) {
            "camera-video-symbolic"
        } else {
            "camera-disabled-symbolic"
        });
        video_icon.add_css_class("dim-label");
        row.append(&video_icon);

        row
    }

    /// Render one remote frame, throttled to ~30fps.
    pub fn render_frame(&self, frame: &VideoFrame) {
        if !frame.is_valid() {
            return;
        }
        let now = Instant::now();
        if let Some(last) = *self.last_frame_at.borrow() {
            if now.duration_since(last).as_millis() < 33 {
                return;
            }
        }
        *self.last_frame_at.borrow_mut() = Some(now);
        paint_frame(&self.remote_picture, frame);
        self.video_stack.set_visible_child_name("video");
        self.no_video_label.set_text("Идёт видео");
    }

    /// Render one local preview frame into the PiP overlay.
    pub fn render_preview(&self, frame: &VideoFrame) {
        if !frame.is_valid() || !self.cam_on.get() {
            return;
        }
        let now = Instant::now();
        if let Some(last) = *self.last_preview_at.borrow() {
            if now.duration_since(last).as_millis() < 100 {
                return;
            }
        }
        *self.last_preview_at.borrow_mut() = Some(now);
        paint_frame(&self.preview_picture, frame);
    }

    fn hangup(&self) {
        if let Some(c) = self.control.borrow().as_ref() {
            let _ = c.end();
        }
        *self.control.borrow_mut() = None;
        let client = self.client.clone();
        if let Some(id) = self.meeting_id.borrow().clone() {
            glib::spawn_future_local(async move {
                if let Err(e) = client.end_personal_meeting(&id).await {
                    log::warn!("end_personal_meeting failed: {e}");
                }
            });
        }
        self.apply_state(&CallState::Ended {
            reason: "hangup".to_string(),
        });
    }

    pub fn show(&self) {
        self.window.present();
    }

    /// Kept for API symmetry with [`TelemostWindow::show`] (tray flows).
    #[allow(dead_code)]
    pub fn hide(&self) {
        self.window.set_visible(false);
    }

    // ── test hooks ──

    #[cfg(test)]
    pub fn test_state_text(&self) -> String {
        self.state_label.text().to_string()
    }

    #[cfg(test)]
    pub fn test_roster_count(&self) -> u32 {
        self.participant_count_label
            .text()
            .parse()
            .unwrap_or(u32::MAX)
    }

    #[cfg(test)]
    pub fn test_ring_visible(&self) -> bool {
        self.ring_shown.get()
    }

    /// Shared by the accept button and tests.
    fn accept_current(&self) {
        let id = self.meeting_id.borrow().clone().unwrap_or_default();
        self.ring_box.set_visible(false);
        self.ring_shown.set(false);
        if let Some(cb) = self.on_accept.borrow().as_ref() {
            cb(id);
        }
    }
}

/// Map an RGBA frame onto a picture widget.
fn paint_frame(picture: &Picture, frame: &VideoFrame) {
    let bytes = glib::Bytes::from(&frame.pixels);
    let texture = gdk::MemoryTexture::new(
        frame.width as i32,
        frame.height as i32,
        gdk::MemoryFormat::R8g8b8a8,
        &bytes,
        frame.stride,
    );
    picture.set_paintable(Some(&texture));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::auth::AuthManager;

    fn test_window() -> TelemostWindow {
        let app = Application::builder()
            .application_id("org.test.telemost-window")
            .build();
        let auth = Arc::new(AuthManager::new().expect("auth manager"));
        TelemostWindow::new(&app, Arc::new(TelemostClient::new(auth)))
    }

    fn sample_meeting() -> PersonalMeeting {
        PersonalMeeting {
            meeting_id: "m-1".to_string(),
            join_url: Some("https://telemost.yandex.ru/j/m-1".to_string()),
            title: None,
            extra: serde_json::Value::Null,
        }
    }

    fn sample_participant(id: &str, name: &str) -> TelemostParticipant {
        TelemostParticipant {
            id: id.to_string(),
            name: Some(name.to_string()),
            avatar_id: None,
            role: crate::models::telemost::ParticipantRole::Participant,
            audio_enabled: Some(true),
            video_enabled: Some(false),
            screen_share: Some(false),
            joined_at: None,
        }
    }

    /// Single workflow test: everything runs on the dedicated GTK thread
    /// (see [`crate::ui::run_gtk_test`]) — parallel `#[test]` fns must never
    /// touch GTK directly.
    #[test]
    fn test_telemost_window_workflow() {
        crate::ui::run_gtk_test(|| {
            // 1. Incoming → accept carries the meeting id.
            let win = test_window();
            assert!(!win.test_ring_visible());

            let accepted = Rc::new(RefCell::new(String::new()));
            let accepted_cb = accepted.clone();
            win.on_accept(move |id| {
                *accepted_cb.borrow_mut() = id;
            });
            win.show_incoming("Alice", &sample_meeting());
            assert!(win.test_ring_visible());

            win.accept_current();
            assert_eq!(accepted.borrow().as_str(), "m-1");
            assert!(!win.test_ring_visible());

            // 2. Roster + state labels.
            win.update_roster(&[
                sample_participant("p1", "Alice"),
                sample_participant("p2", "Bob"),
            ]);
            assert_eq!(win.test_roster_count(), 2);

            win.apply_state(&CallState::Joining);
            assert_eq!(win.test_state_text(), "Подключение…");
            win.apply_state(&CallState::InCall);
            assert_eq!(win.test_state_text(), "В звонке");
            win.apply_state(&CallState::Ended {
                reason: "done".to_string(),
            });
            assert_eq!(win.test_state_text(), "Звонок завершён");

            win.update_roster(&[]);
            assert_eq!(win.test_roster_count(), 0);

            // 3. Remote frame renders into the picture.
            let frame = VideoFrame {
                width: 2,
                height: 2,
                stride: 8,
                pixels: vec![
                    255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 0, 255,
                ],
            };
            assert!(frame.is_valid());
            win.render_frame(&frame);
            assert!(win.remote_picture.paintable().is_some());

            // Local preview renders into the PiP overlay.
            win.render_preview(&frame);
            assert!(win.preview_picture.paintable().is_some());
        });
    }
}
