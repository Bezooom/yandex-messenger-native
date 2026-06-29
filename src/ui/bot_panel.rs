#![allow(dead_code)]

use gtk::prelude::*;
use gtk::{Box as GtkBox, Button, FlowBox, Label, Orientation, ScrolledWindow};

use std::sync::Arc;

use crate::api::auth::AuthManager;
use crate::models::bot::{BotCommand, BotInfo, BotReplyMarkup, InlineButton, ReplyKeyboard};

/// Панель информации о боте
pub struct BotPanel {
    pub container: GtkBox,
    avatar_box: GtkBox,
    avatar_label: Label,
    username_label: Label,
    description_label: Label,
    badge_box: GtkBox,
    verified_btn: Button,
    command_list: GtkBox,
    command_scrolled: ScrolledWindow,
    inline_button_grid: FlowBox,
    inline_scrolled: ScrolledWindow,
    reply_keyboard_view: GtkBox,
    start_btn: Button,
    on_start: std::sync::Arc<std::sync::Mutex<Option<Box<dyn Fn(String, String) + Send>>>>,
    on_inline_click: std::sync::Arc<std::sync::Mutex<Option<Box<dyn Fn(String, String) + Send>>>>,
    on_command_click: std::sync::Arc<std::sync::Mutex<Option<Box<dyn Fn(String, String) + Send>>>>,
}

impl BotPanel {
    pub fn new(auth: Arc<AuthManager>) -> Self {
        let _ = auth;
        let container = GtkBox::new(Orientation::Vertical, 0);
        container.set_css_classes(&["bot-panel"]);
        container.set_vexpand(true);

        Self {
            container,
            avatar_box: GtkBox::new(Orientation::Horizontal, 0),
            avatar_label: Label::new(Some("B")),
            username_label: Label::new(Some("Бот")),
            description_label: Label::new(Some("Информация о боте")),
            badge_box: GtkBox::new(Orientation::Horizontal, 4),
            verified_btn: Button::builder()
                .icon_name("emblem-default-symbolic")
                .css_classes(vec!["icon-btn".to_string()])
                .build(),
            command_list: GtkBox::new(Orientation::Vertical, 0),
            command_scrolled: ScrolledWindow::new(),
            inline_button_grid: FlowBox::new(),
            inline_scrolled: ScrolledWindow::new(),
            reply_keyboard_view: GtkBox::new(Orientation::Vertical, 4),
            start_btn: Button::with_label("Запустить"),
            on_start: std::sync::Arc::new(std::sync::Mutex::new(None)),
            on_inline_click: std::sync::Arc::new(std::sync::Mutex::new(None)),
            on_command_click: std::sync::Arc::new(std::sync::Mutex::new(None)),
        }
    }

    /// Обновить информацию о боте
    pub fn update(&self, bot: &BotInfo) {
        // Avatar
        let display_name = bot.display_name();
        let initials: String = display_name
            .chars()
            .take(2)
            .map(|c| c.to_ascii_uppercase())
            .collect();
        self.avatar_label.set_label(&initials);
        self.avatar_box.add_css_class("bot-avatar");

        // Username
        if let Some(username) = &bot.username {
            self.username_label.set_label(&format!("@{}", username));
            self.username_label.add_css_class("bot-username");
        } else if let Some(name) = &bot.first_name {
            self.username_label.set_label(name);
        }

        // Description
        if let Some(desc) = &bot.description {
            self.description_label.set_label(desc);
        }

        // Verified badge
        if bot.is_verified {
            self.badge_box.set_visible(true);
            self.verified_btn.add_css_class("verified-badge");
        }

        // Commands
        self.render_commands(&bot.commands);

        // Inline buttons
        self.render_inline_buttons(&vec![]);

        // Reply keyboard
        self.render_reply_keyboard(&ReplyKeyboard::default());
    }

    /// Обновить reply markup
    pub fn update_reply_markup(&self, markup: &BotReplyMarkup) {
        if let Some(reply_kb) = &markup.keyboard {
            self.render_reply_keyboard(reply_kb);
        }
        if !markup.inline_keyboard.is_empty() {
            self.render_inline_buttons(&markup.inline_keyboard);
        }
    }

    /// Отрисовать команды
    fn render_commands(&self, commands: &[BotCommand]) {
        while let Some(child) = self.command_list.first_child() {
            self.command_list.remove(&child);
        }

        for cmd in commands {
            let row = GtkBox::new(Orientation::Horizontal, 8);
            row.add_css_class("bot-command");
            row.set_margin_top(4);
            row.set_margin_bottom(4);

            let cmd_label = Label::builder()
                .label(&format!("/{}", cmd.command))
                .css_classes(vec!["dim-label".to_string()])
                .xalign(0.0)
                .build();

            let desc_label = Label::builder()
                .label(&cmd.description)
                .css_classes(vec!["dim-label".to_string()])
                .xalign(0.0)
                .build();

            row.append(&cmd_label);
            row.append(&desc_label);

            let on_cmd = self.on_command_click.clone();
            let cmd_name = cmd.command.clone();
            let gesture = gtk::GestureClick::new();
            gesture.connect_pressed(move |_, _, _, _| {
                if let Some(cb) = on_cmd.lock().unwrap().as_ref() {
                    cb(cmd_name.clone(), String::new());
                }
            });
            row.add_controller(gesture);

            self.command_list.append(&row);
        }
    }

    /// Отрисовать inline кнопки
    fn render_inline_buttons(&self, buttons: &[Vec<InlineButton>]) {
        while let Some(child) = self.inline_button_grid.first_child() {
            self.inline_button_grid.remove(&child);
        }

        self.inline_button_grid.set_max_children_per_line(2);
        self.inline_button_grid.set_min_children_per_line(1);
        self.inline_button_grid
            .set_orientation(Orientation::Horizontal);
        self.inline_button_grid.set_margin_top(8);
        self.inline_button_grid.set_margin_bottom(8);
        self.inline_button_grid.add_css_class("inline-button-grid");

        for btn_row in buttons {
            for btn in btn_row {
                let button = Button::builder()
                    .label(&btn.text)
                    .css_classes(vec!["inline-button".to_string()])
                    .hexpand(true)
                    .build();

                let on_inline = self.on_inline_click.clone();
                let text = btn.text.clone();
                let callback_data = btn.callback_data.clone();
                let url = btn.url.clone();
                let web_app = btn.web_app.clone();

                button.connect_clicked(move |_| {
                    if let Some(cb) = on_inline.lock().unwrap().as_ref() {
                        if let Some(ref data) = callback_data {
                            cb(text.clone(), data.clone());
                        } else if let Some(ref u) = url {
                            log::info!("Opening URL: {}", u);
                        } else if let Some(ref app) = web_app {
                            log::info!("Opening web app: {}", app);
                        } else {
                            let text_val = text.clone();
                            cb(text_val, String::new());
                        }
                    }
                });

                self.inline_button_grid.append(&button);
            }
        }
    }

    /// Отрисовать reply-клавиатуру
    fn render_reply_keyboard(&self, keyboard: &ReplyKeyboard) {
        while let Some(child) = self.reply_keyboard_view.first_child() {
            self.reply_keyboard_view.remove(&child);
        }

        for row in &keyboard.rows {
            let row_box = GtkBox::new(Orientation::Horizontal, 4);
            row_box.add_css_class("reply-keyboard-row");

            for btn in row {
                let button = Button::builder()
                    .label(&btn.text)
                    .css_classes(vec!["keyboard-button".to_string()])
                    .hexpand(true)
                    .build();

                let text = btn.text.clone();
                let request_contact = btn.request_contact;
                let request_location = btn.request_location;

                button.connect_clicked(move |_| {
                    log::info!(
                        "Keyboard button clicked: {} (contact={}, location={})",
                        text,
                        request_contact,
                        request_location
                    );
                });

                row_box.append(&button);
            }

            self.reply_keyboard_view.append(&row_box);
        }
    }

    pub fn on_start(&self, callback: impl Fn(String, String) + Send + 'static) {
        *self.on_start.lock().unwrap() = Some(Box::new(callback));
    }

    pub fn on_inline_button_click(&self, callback: impl Fn(String, String) + Send + 'static) {
        *self.on_inline_click.lock().unwrap() = Some(Box::new(callback));
    }

    pub fn on_command_click(&self, callback: impl Fn(String, String) + Send + 'static) {
        *self.on_command_click.lock().unwrap() = Some(Box::new(callback));
    }
}

// Update existing closures that use Rc<RefCell>
