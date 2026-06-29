pub fn send_notification(summary: &str, body: &str) {
    eprintln!("[notification] {}: {}", summary, body);
}
