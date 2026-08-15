//! Desktop notifications via notify-rust (XDG / freedesktop).

use std::sync::atomic::{AtomicBool, Ordering};

static NOTIFICATIONS_ENABLED: AtomicBool = AtomicBool::new(true);

/// Global toggle from settings (also checked per-call).
pub fn set_notifications_enabled(enabled: bool) {
    NOTIFICATIONS_ENABLED.store(enabled, Ordering::Relaxed);
}

pub fn notifications_enabled() -> bool {
    NOTIFICATIONS_ENABLED.load(Ordering::Relaxed)
}

/// Show a desktop notification.
///
/// - Respects global enable flag
/// - `chat_id` is stored as the notification ID for replacement/grouping
pub fn send_notification(summary: &str, body: &str) {
    send_notification_for_chat(summary, body, None, false);
}

/// Full variant with mute / chat context.
pub fn send_notification_for_chat(
    summary: &str,
    body: &str,
    chat_id: Option<&str>,
    chat_muted: bool,
) {
    if !notifications_enabled() || chat_muted {
        log::debug!(
            "Notification suppressed (enabled={}, muted={}): {}: {}",
            notifications_enabled(),
            chat_muted,
            summary,
            body
        );
        return;
    }

    let summary = summary.trim();
    let body = body.trim();
    if summary.is_empty() && body.is_empty() {
        return;
    }

    let mut builder = notify_rust::Notification::new();
    builder
        .summary(if summary.is_empty() {
            "Yandex Messenger"
        } else {
            summary
        })
        .body(body)
        .appname("Yandex Messenger")
        .icon("yandex-messenger")
        .timeout(notify_rust::Timeout::Milliseconds(5000));

    if let Some(id) = chat_id {
        // Replace previous toast for the same chat
        let _ = builder.id(stable_id(id));
    }

    if let Err(e) = builder.show() {
        // Fallback so we still see something in logs / headless CI
        eprintln!("[notification] {}: {} (desktop notify failed: {})", summary, body, e);
    }
}

fn stable_id(chat_id: &str) -> u32 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    chat_id.hash(&mut h);
    // Avoid 0 which some servers treat specially
    (h.finish() as u32).max(1)
}
