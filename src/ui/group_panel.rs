#![allow(dead_code)]

use gtk::prelude::*;
use gtk::{Button, Label, ListBox, ListBoxRow, Orientation, ScrolledWindow};
use std::cell::RefCell;
use std::sync::Arc;

use crate::api::auth::AuthManager;
use crate::models::Chat;

/// Group/Channel panel - shows group info and member list
pub struct GroupPanel {
    container: gtk::Box,
    chat: RefCell<Option<Chat>>,
    title_label: Label,
    description_label: Label,
    member_count_label: Label,
    member_list: ListBox,
    settings_btn: Button,
    invite_btn: Button,
    leave_btn: Button,
    add_member_btn: Button,
}

impl GroupPanel {
    pub fn new(_auth: Arc<AuthManager>) -> Self {
        let container = gtk::Box::new(Orientation::Vertical, 0);
        container.add_css_class("group-panel");

        // Header with avatar, title, description
        let header = gtk::Box::new(Orientation::Horizontal, 12);
        header.add_css_class("group-header");
        header.set_margin_top(16);
        header.set_margin_bottom(16);
        header.set_margin_start(16);
        header.set_margin_end(16);

        let avatar = gtk::Box::new(Orientation::Horizontal, 0);
        avatar.add_css_class("group-avatar");
        avatar.add_css_class("avatar-gradient-1");
        avatar.set_size_request(64, 64);
        avatar.set_halign(gtk::Align::Start);
        let avatar_label = Label::builder()
            .label("👥")
            .css_classes(vec!["avatar-label".to_string()])
            .build();
        avatar.append(&avatar_label);

        let info_box = gtk::Box::new(Orientation::Vertical, 4);
        info_box.set_hexpand(true);

        let title_label = Label::builder()
            .xalign(0.0)
            .build();
        title_label.add_css_class("group-title");
        title_label.add_css_class("title");

        let description_label = Label::builder()
            .xalign(0.0)
            .wrap(true)
            .build();
        description_label.add_css_class("group-description");
        description_label.add_css_class("dim-label");

        let member_count_label = Label::builder()
            .xalign(0.0)
            .build();
        member_count_label.add_css_class("dim-label");

        info_box.append(&title_label);
        info_box.append(&description_label);
        info_box.append(&member_count_label);

        header.append(&avatar);
        header.append(&info_box);

        // Action buttons
        let action_box = gtk::Box::new(Orientation::Horizontal, 8);
        action_box.set_margin_top(16);
        action_box.set_margin_bottom(16);
        action_box.set_margin_start(16);
        action_box.set_margin_end(16);

        let settings_btn = Button::with_label("Настройки");
        settings_btn.add_css_class("suggested-action");

        let invite_btn = Button::with_label("Пригласить");

        let leave_btn = Button::with_label("Покинуть");
        leave_btn.add_css_class("danger");

        let add_member_btn = Button::with_label("Добавить участника");

        action_box.append(&settings_btn);
        action_box.append(&invite_btn);
        action_box.append(&add_member_btn);
        action_box.append(&leave_btn);

        // Member list
        let scrolled = ScrolledWindow::builder()
            .hexpand(true)
            .vexpand(true)
            .build();

        let member_list = ListBox::new();
        member_list.add_css_class("member-list");
        scrolled.set_child(Some(&member_list));

        let separator = gtk::Separator::new(Orientation::Horizontal);

        container.append(&header);
        container.append(&separator);
        container.append(&action_box);
        container.append(&scrolled);

        Self {
            container,
            chat: RefCell::new(None),
            title_label,
            description_label,
            member_count_label,
            member_list,
            settings_btn,
            invite_btn,
            leave_btn,
            add_member_btn,
        }
    }

    pub fn set_chat(&self, chat: Chat) {
        *self.chat.borrow_mut() = Some(chat.clone());

        self.title_label.set_label(chat.title.as_deref().unwrap_or("Без названия"));

        let chat_type_str = match chat.chat_type {
            crate::models::ChatType::Group => "Группа",
            crate::models::ChatType::Channel => "Канал",
            _ => "Чат",
        };

        let description = chat
            .title
            .as_ref()
            .map(|_| format!("{} • {} участников", chat_type_str, chat.participants.len()))
            .unwrap_or_default();
        self.description_label.set_label(&description);

        self.member_count_label.set_label(&format!("{} участников", chat.participants.len()));

        // Update member list
        self.update_member_list(&chat.participants);
    }

    fn update_member_list(&self, participants: &[crate::models::Participant]) {
        // Clear existing rows
        while let Some(row) = self.member_list.first_child() {
            self.member_list.remove(&row);
        }

        for participant in participants {
            let row = ListBoxRow::new();
            row.add_css_class("group-member-row");

            let row_box = gtk::Box::new(Orientation::Horizontal, 12);
            row_box.set_margin_top(8);
            row_box.set_margin_bottom(8);
            row_box.set_margin_start(8);
            row_box.set_margin_end(8);

            let avatar = gtk::Box::new(Orientation::Horizontal, 0);
            avatar.add_css_class("avatar");
            avatar.set_size_request(36, 36);
            let initials = participant
                .name
                .as_ref()
                .map(|n| {
                    n.chars()
                        .take(2)
                        .map(|c| c.to_ascii_uppercase())
                        .collect::<String>()
                })
                .unwrap_or_else(|| "??".to_string());
            let avatar_label = Label::builder()
                .label(&initials)
                .css_classes(vec!["avatar-label".to_string()])
                .build();
            avatar.append(&avatar_label);

            let info_box = gtk::Box::new(Orientation::Vertical, 2);

            let name_label = Label::builder()
                .xalign(0.0)
                .label(participant.name.as_deref().unwrap_or("Неизвестный"))
                .build();

            let role_label = Label::builder()
                .xalign(0.0)
                .build();
            role_label.add_css_class("group-member-role");
            role_label.add_css_class("dim-label");

            // Determine role (simplified - in real app would come from GroupMember)
            let role_text = if participant.id == "current_user" {
                "Вы"
            } else {
                "Участник"
            };
            role_label.set_label(role_text);

            info_box.append(&name_label);
            info_box.append(&role_label);

            row_box.append(&avatar);
            row_box.append(&info_box);
            row.set_child(Some(&row_box));

            self.member_list.append(&row);
        }
    }

    pub fn container(&self) -> &gtk::Box {
        &self.container
    }

    pub fn connect_settings_clicked<F: Fn() + 'static>(&self, callback: F) {
        self.settings_btn.connect_clicked(move |_| callback());
    }

    pub fn connect_invite_clicked<F: Fn() + 'static>(&self, callback: F) {
        self.invite_btn.connect_clicked(move |_| callback());
    }

    pub fn connect_leave_clicked<F: Fn() + 'static>(&self, callback: F) {
        self.leave_btn.connect_clicked(move |_| callback());
    }

    pub fn connect_add_member_clicked<F: Fn() + 'static>(&self, callback: F) {
        self.add_member_btn.connect_clicked(move |_| callback());
    }
}
