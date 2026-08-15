//! System tray (StatusNotifierItem) via ksni.
//!
//! Provides: show window, quit. Unread count updates the tooltip.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::thread;

/// Commands from tray menu → main GTK thread (polled via channel).
#[derive(Debug, Clone)]
pub enum TrayCommand {
    Show,
    Quit,
}

/// Handle kept alive for the lifetime of the app.
pub struct TrayHandle {
    cmd_rx: Option<Receiver<TrayCommand>>,
    unread: Arc<AtomicU32>,
    running: Arc<AtomicBool>,
}

struct MessengerTray {
    tx: Sender<TrayCommand>,
    unread: Arc<AtomicU32>,
}

impl ksni::Tray for MessengerTray {
    fn id(&self) -> String {
        "yandex-messenger-native".into()
    }

    fn category(&self) -> ksni::Category {
        ksni::Category::Communications
    }

    fn icon_name(&self) -> String {
        "yandex-messenger".into()
    }

    fn title(&self) -> String {
        let n = self.unread.load(Ordering::Relaxed);
        if n == 0 {
            "Yandex Messenger".into()
        } else {
            format!("Yandex Messenger ({})", n)
        }
    }

    fn tool_tip(&self) -> ksni::ToolTip {
        let n = self.unread.load(Ordering::Relaxed);
        let title = if n == 0 {
            "Yandex Messenger".to_string()
        } else {
            format!("Непрочитанных: {}", n)
        };
        ksni::ToolTip {
            icon_name: "yandex-messenger".into(),
            icon_pixmap: Vec::new(),
            title,
            description: "Неофициальный Linux-клиент".into(),
        }
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        use ksni::menu::*;
        vec![
            StandardItem {
                label: "Показать".into(),
                activate: Box::new(|tray: &mut Self| {
                    let _ = tray.tx.send(TrayCommand::Show);
                }),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: "Выход".into(),
                activate: Box::new(|tray: &mut Self| {
                    let _ = tray.tx.send(TrayCommand::Quit);
                }),
                ..Default::default()
            }
            .into(),
        ]
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        let _ = self.tx.send(TrayCommand::Show);
    }
}

impl TrayHandle {
    /// Spawn StatusNotifierItem on a background thread. Returns handle with command receiver.
    pub fn init() -> Self {
        let (tx, rx) = mpsc::channel();
        let unread = Arc::new(AtomicU32::new(0));
        let running = Arc::new(AtomicBool::new(true));

        let unread_t = unread.clone();
        let running_t = running.clone();

        thread::Builder::new()
            .name("ym-tray".into())
            .spawn(move || {
                let service = ksni::TrayService::new(MessengerTray {
                    tx,
                    unread: unread_t,
                });
                if running_t.load(Ordering::Relaxed) {
                    if let Err(e) = service.run() {
                        log::warn!("System tray failed (continue without tray): {}", e);
                    }
                }
            })
            .ok();

        Self {
            cmd_rx: Some(rx),
            unread,
            running,
        }
    }

    /// No-op placeholder when tray is disabled.
    pub fn disabled() -> Self {
        Self {
            cmd_rx: None,
            unread: Arc::new(AtomicU32::new(0)),
            running: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn set_unread_count(&self, count: u32) {
        self.unread.store(count, Ordering::Relaxed);
    }

    /// Non-blocking poll of tray menu actions.
    pub fn try_recv(&self) -> Option<TrayCommand> {
        self.cmd_rx.as_ref()?.try_recv().ok()
    }

    pub fn shutdown(&self) {
        self.running.store(false, Ordering::Relaxed);
    }
}
