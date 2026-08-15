use std::sync::Arc;

use crate::api::telemost::TelemostClient;
use crate::ui::telemost_window::TelemostWindow as NativeTelemostWindow;

/// Telemost video calling window.
///
/// This is a thin compatibility wrapper around the native GTK4
/// TelemostWindow implementation. It holds a shared TelemostClient
/// and exposes the same `show()` / `hide()` API used by the rest
/// of the messenger UI.
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
}
