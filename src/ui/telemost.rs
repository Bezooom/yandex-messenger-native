use std::sync::Arc;

use crate::api::goloom_call::CallHandle;
use crate::api::goloom_media::VideoFrame;
use crate::api::telemost::TelemostClient;
use crate::models::telemost::{PersonalMeeting, TelemostParticipant};
use crate::ui::telemost_window::TelemostWindow as NativeTelemostWindow;

/// Telemost video calling window.
///
/// This is a thin compatibility wrapper around the native GTK4
/// TelemostWindow implementation. It holds a shared TelemostClient
/// and exposes the same `show()` / `hide()` API used by the rest
/// of the messenger UI, plus the live-call bindings.
pub struct TelemostWindow {
    inner: NativeTelemostWindow,
}

impl TelemostWindow {
    pub fn new(app: &gtk::Application, telemost_client: Arc<TelemostClient>) -> Self {
        Self {
            inner: NativeTelemostWindow::new(app, telemost_client),
        }
    }

    pub fn show(&self) {
        self.inner.show();
    }

    pub fn hide(&self) {
        self.inner.hide();
    }

    pub fn attach_call(
        &self,
        handle: CallHandle,
        title: &str,
        join_url: Option<String>,
        meeting_id: Option<String>,
    ) {
        self.inner.attach_call(handle, title, join_url, meeting_id);
    }

    pub fn show_incoming(&self, peer_name: &str, meeting: &PersonalMeeting) {
        self.inner.show_incoming(peer_name, meeting);
    }

    pub fn on_accept(&self, cb: impl Fn(String) + 'static) {
        self.inner.on_accept(cb);
    }

    pub fn set_notice(&self, text: &str) {
        self.inner.set_notice(text);
    }

    pub fn update_roster(&self, participants: &[TelemostParticipant]) {
        self.inner.update_roster(participants);
    }

    pub fn render_frame(&self, frame: &VideoFrame) {
        self.inner.render_frame(frame);
    }
}
