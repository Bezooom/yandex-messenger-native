#![allow(dead_code)]

use gtk::prelude::*;
use gtk::{Align, Box as GtkBox, Button, Entry, Label, Orientation, ScrolledWindow};
use std::cell::RefCell;
use std::sync::Arc;

use crate::api::auth::AuthManager;
use crate::models::Message;

/// ThreadView — displays an in-thread conversation for a message.
///
/// Shows a breadcrumb header, scrollable list of thread messages,
/// a back button to the parent chat, and an input to send new thread replies.
pub struct ThreadView {
    container: GtkBox,
    chat_id: String,
    thread_id: String,
    parent_message: RefCell<Option<Message>>,
    thread_messages: RefCell<Vec<Message>>,
    message_list: GtkBox,
    title_label: Label,
    breadcrumb_label: Label,
    input_entry: Entry,
    send_btn: Button,
    back_btn: Button,
}

impl ThreadView {
    /// Create a new ThreadView for a given chat and thread.
    pub fn new(auth: Arc<AuthManager>, chat_id: String, thread_id: String) -> Self {
        let _ = auth; // future: fetch thread info via auth

        let container = GtkBox::new(Orientation::Vertical, 0);
        container.set_hexpand(true);
        container.set_vexpand(true);
        container.set_css_classes(&["message-area"]);

        // ── Header bar with breadcrumb ──
        let header = Self::create_header(&chat_id, &thread_id);

        // ── Scrolled message area ──
        let scrolled = ScrolledWindow::builder()
            .hexpand(true)
            .vexpand(true)
            .build();

        let message_list = GtkBox::new(Orientation::Vertical, 8);
        message_list.set_margin_start(16);
        message_list.set_margin_end(16);
        message_list.set_margin_top(8);
        message_list.set_margin_bottom(8);

        // Empty state placeholder
        let empty_state = Self::create_thread_empty_state();
        message_list.append(&empty_state);

        scrolled.set_child(Some(&message_list));

        // ── Input area ──
        let (input, input_entry, send_btn) = Self::create_input();

        container.append(&header.0);
        container.append(&scrolled);
        container.append(&input);

        ThreadView {
            container,
            chat_id,
            thread_id,
            parent_message: RefCell::new(None),
            thread_messages: RefCell::new(Vec::new()),
            message_list,
            title_label: header.1,
            breadcrumb_label: header.2,
            input_entry,
            send_btn,
            back_btn: header.3,
        }
    }

    /// Create header with breadcrumb navigation.
    fn create_header(_chat_id: &str, _thread_id: &str) -> (GtkBox, Label, Label, Button) {
        // Title row
        let title_box = GtkBox::new(Orientation::Vertical, 2);
        title_box.set_hexpand(true);

        let title = Label::builder()
            .label("Thread")
            .xalign(0.0)
            .css_classes(vec!["dim-label".to_string()])
            .build();
        title.set_css_classes(&["title-label"]);

        // Breadcrumb: Chat > Thread > Message
        let breadcrumb = Label::builder()
            .label("Chat > Thread > Message")
            .xalign(0.0)
            .use_markup(true)
            .build();
        breadcrumb.set_css_classes(&["breadcrumb"]);

        title_box.append(&title);
        title_box.append(&breadcrumb);

        // Back button
        let back = Button::builder()
            .icon_name("go-previous-symbolic")
            .tooltip_text("Назад к чату")
            .build();
        back.set_css_classes(&["btn-icon"]);

        (title_box, title, breadcrumb, back)
    }

    /// Create thread reply input area.
    fn create_input() -> (GtkBox, Entry, Button) {
        let input = GtkBox::new(Orientation::Horizontal, 4);
        input.set_css_classes(&["message-input"]);
        input.set_margin_start(8);
        input.set_margin_end(8);
        input.set_margin_bottom(8);
        input.set_margin_top(4);

        let attach_btn = Button::builder()
            .icon_name("document-open-symbolic")
            .tooltip_text("Прикрепить файл")
            .build();
        attach_btn.set_css_classes(&["btn-icon"]);

        let entry = Entry::builder()
            .placeholder_text("Ответить в поток...")
            .hexpand(true)
            .build();

        let send = Button::builder()
            .icon_name("mail-send-symbolic")
            .tooltip_text("Отправить в поток")
            .build();
        send.set_css_classes(&["btn-primary"]);

        input.append(&attach_btn);
        input.append(&entry);
        input.append(&send);

        (input, entry, send)
    }

    /// Empty state for thread view.
    fn create_thread_empty_state() -> GtkBox {
        let empty = GtkBox::new(Orientation::Vertical, 12);
        empty.set_halign(Align::Center);
        empty.set_valign(Align::Center);
        empty.set_vexpand(true);

        let text = Label::builder()
            .label("В этом потоке пока нет сообщений")
            .xalign(0.5)
            .wrap(true)
            .build();
        text.add_css_class("dim-label");

        empty.append(&text);
        empty
    }

    /// Set the thread messages to render.
    pub fn set_thread_messages(&self, messages: Vec<Message>) {
        *self.thread_messages.borrow_mut() = messages;
        self.render_thread_messages();
    }

    /// Set the parent (root) message that this thread belongs to.
    pub fn set_parent_message(&self, msg: Message) {
        *self.parent_message.borrow_mut() = Some(msg.clone());

        // Update breadcrumb with parent message preview
        if let Some(preview) = &msg.text {
            let truncated: String = preview.chars().take(40).collect();
            let escaped = glib::markup_escape_text(&truncated);
            let breadcrumb_text = format!(
                "<span font_style='italic' color='#9ca3af'>({})</span>",
                escaped.as_str()
            );
            self.breadcrumb_label.set_markup(&breadcrumb_text);
        }
    }

    /// Render all thread messages into the message list.
    fn render_thread_messages(&self) {
        // Clear existing messages
        while let Some(child) = self.message_list.first_child() {
            self.message_list.remove(&child);
        }

        let messages = self.thread_messages.borrow();

        if messages.is_empty() {
            let empty = Self::create_thread_empty_state();
            self.message_list.append(&empty);
            return;
        }

        for msg in messages.iter() {
            let bubble = Self::render_thread_message(msg);
            self.message_list.append(&bubble);
        }
    }

    /// Render a single thread message bubble (simpler version, no sender name).
    fn render_thread_message(msg: &Message) -> GtkBox {
        let msg_box = GtkBox::new(Orientation::Vertical, 2);
        let is_own = msg.sent;

        if is_own {
            msg_box.set_halign(Align::End);
        } else {
            msg_box.set_halign(Align::Start);
        }

        // Thread message bubble
        let bubble = GtkBox::new(Orientation::Vertical, 2);
        let bg_class = if is_own { "accent" } else { "bubble-received" };
        bubble.add_css_class(&bg_class);
        bubble.set_margin_start(4);
        bubble.set_margin_end(4);
        bubble.set_margin_top(2);
        bubble.set_margin_bottom(2);

        if let Some(ref text) = msg.text {
            let label = Label::builder()
                .label(text)
                .wrap(true)
                .max_width_chars(50)
                .xalign(if is_own { 1.0 } else { 0.0 })
                .build();
            bubble.append(&label);
        }

        // Thread timestamp
        let timestamp = Label::builder()
            .label(Self::format_thread_timestamp(&msg.created))
            .css_classes(vec!["dim-label"])
            .xalign(if is_own { 1.0 } else { 0.0 })
            .build();
        timestamp.set_css_classes(&["timestamp"]);

        bubble.append(&timestamp);
        msg_box.append(&bubble);

        msg_box
    }

    /// Format timestamp for thread context (always shows time).
    fn format_thread_timestamp(dt: &chrono::DateTime<chrono::Utc>) -> String {
        let now = chrono::Utc::now();
        let diff = now.signed_duration_since(*dt);

        if diff.num_days() == 0 {
            format!("{}", dt.format("%H:%M"))
        } else if diff.num_days() == 1 {
            format!("Вчера, {}", dt.format("%H:%M"))
        } else if diff.num_days() < 7 {
            format!("{} дн. назад, {}", diff.num_days(), dt.format("%H:%M"))
        } else {
            format!("{}", dt.format("%d.%m.%Y %H:%M"))
        }
    }

    /// Register a callback for sending thread messages.
    /// The callback receives (chat_id, message_text).
    pub fn on_send(&self, callback: impl Fn(String, String) + 'static) {
        let entry = self.input_entry.clone();
        let chat_id = self.chat_id.clone();

        self.send_btn.connect_clicked(move |_| {
            let text = entry.text().to_string();
            if text.trim().is_empty() {
                return;
            }
            callback(chat_id.clone(), text.clone());
            entry.set_text("");
        });
    }

    /// Get the container widget to add to a parent layout.
    pub fn container(&self) -> &GtkBox {
        &self.container
    }

    /// Get the back button (for connecting click handler externally).
    pub fn back_btn(&self) -> &Button {
        &self.back_btn
    }
}
