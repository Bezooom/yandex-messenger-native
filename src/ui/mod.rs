pub mod account_dropdown;
pub mod auth_dialog;
pub mod bot_panel;
pub mod chat_list;
pub mod chat_view;
pub mod create_group_dialog;
pub mod emoji_picker;
pub mod folder_sidebar;
pub mod global_search;
pub mod group_panel;
pub mod image_viewer;
pub mod message_object;
pub mod notifications;
pub mod poll_creator;
pub mod poll_renderer;
pub mod reaction_panel;
pub mod saved_panel;
pub mod scheduled_panel;
pub mod settings;
pub mod sticker_panel;
pub mod telemost;
pub mod telemost_window;
pub mod thread_view;
pub mod tray;
pub mod video_player;
pub mod voice_message_player;

pub use auth_dialog::AuthDialog;
pub use chat_list::ChatListPanel;
pub use chat_view::ChatView;
pub use telemost_window::TelemostWindow;
pub use thread_view::ThreadView;

/// Run a UI test body on a single dedicated GTK thread.
///
/// Widget code must run on the thread that initialized GTK, but libtest
/// schedules `#[test]` fns on arbitrary worker threads. Posting every UI
/// body to one owned thread keeps the suite deterministic under any
/// `--test-threads` value (panics propagate to the caller).
#[cfg(test)]
pub fn run_gtk_test<R: Send + 'static>(f: impl FnOnce() -> R + Send + 'static) -> R {
    use std::sync::{mpsc, OnceLock};

    type Job = Box<dyn FnOnce() + Send + 'static>;
    static TX: OnceLock<mpsc::Sender<Job>> = OnceLock::new();
    let tx = TX.get_or_init(|| {
        let (tx, rx) = mpsc::channel::<Job>();
        std::thread::Builder::new()
            .name("gtk-test".to_string())
            .spawn(move || {
                let _ = gtk::init();
                for job in rx {
                    job();
                }
            })
            .expect("gtk test thread");
        tx
    });

    let (res_tx, res_rx) = mpsc::channel();
    tx.send(Box::new(move || {
        let out = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
        let _ = res_tx.send(out);
    }))
    .expect("gtk test runner alive");
    match res_rx.recv().expect("gtk test result") {
        Ok(r) => r,
        Err(payload) => std::panic::resume_unwind(payload),
    }
}
