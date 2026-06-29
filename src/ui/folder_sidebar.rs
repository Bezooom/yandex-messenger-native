#![allow(dead_code)]

use gtk::prelude::*;
use gtk::{Box, Button, Image, Label, ListBox, ListBoxRow, Orientation, ScrolledWindow};
use std::cell::RefCell;
use std::rc::Rc;

use crate::models::ChatFolder;

pub struct FolderSidebar {
    pub container: Box,
    list_box: ListBox,
    folders: Rc<RefCell<Vec<ChatFolder>>>,
}

impl FolderSidebar {
    pub fn new() -> Self {
        let container = Box::new(Orientation::Vertical, 0);
        container.set_css_classes(&["folder-sidebar"]);
        container.set_size_request(64, -1);

        // Header with "All" folder or Add button
        let add_btn = Button::builder()
            .icon_name("list-add-symbolic")
            .css_classes(vec!["folder-add-btn".to_string()])
            .margin_top(8)
            .margin_bottom(8)
            .margin_start(8)
            .margin_end(8)
            .build();
        container.append(&add_btn);

        let scrolled = ScrolledWindow::builder()
            .hexpand(true)
            .vexpand(true)
            .build();

        let list_box = ListBox::new();
        list_box.set_css_classes(&["folder-list"]);
        list_box.set_selection_mode(gtk::SelectionMode::Single);

        scrolled.set_child(Some(&list_box));
        container.append(&scrolled);

        Self {
            container,
            list_box,
            folders: Rc::new(RefCell::new(Vec::new())),
        }
    }

    pub fn set_folders(&self, folders: Vec<ChatFolder>) {
        *self.folders.borrow_mut() = folders;
        self.render();
    }

    fn render(&self) {
        while let Some(child) = self.list_box.first_child() {
            self.list_box.remove(&child);
        }

        // Add "All chats" folder by default
        let all_chats_row = Self::create_folder_row("Все", "user-home-symbolic", 0);
        self.list_box.append(&all_chats_row);

        for folder in self.folders.borrow().iter() {
            let icon = match folder.title.as_str() {
                "Личные" => "user-info-symbolic",
                "Работа" => "briefcase-symbolic",
                "Непрочитанные" => "mail-unread-symbolic",
                "Важное" => "emblem-important-symbolic",
                _ => "folder-symbolic",
            };
            let row = Self::create_folder_row(&folder.title, icon, folder.unread_count);
            self.list_box.append(&row);
        }
    }

    fn create_folder_row(title: &str, icon_name: &str, unread: u32) -> ListBoxRow {
        let row = ListBoxRow::new();
        row.set_css_classes(&["folder-row"]);

        let bx = Box::new(Orientation::Vertical, 4);
        bx.set_css_classes(&["folder-item"]);
        bx.set_margin_top(8);
        bx.set_margin_bottom(8);
        bx.set_margin_start(4);
        bx.set_margin_end(4);
        bx.set_halign(gtk::Align::Center);
        bx.set_valign(gtk::Align::Center);

        let icon = Image::from_icon_name(icon_name);
        // Force larger icon size using CSS class if needed, or stick to standard GTK large
        icon.set_icon_size(gtk::IconSize::Large);
        bx.append(&icon);

        let label = Label::builder()
            .label(title)
            .css_classes(vec!["folder-label".to_string()])
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .justify(gtk::Justification::Center)
            .max_width_chars(10) // Slightly wider for Russian words like "Работа"
            .build();
        bx.append(&label);

        if unread > 0 {
            let badge = Label::builder()
                .label(&unread.to_string())
                .css_classes(vec!["folder-badge".to_string()])
                .build();
            // Optional overlay would be better, but appended below for now
            bx.append(&badge);
        }

        row.set_child(Some(&bx));
        row
    }

    pub fn connect_folder_selected<F: Fn(Option<ChatFolder>) + 'static>(&self, callback: F) {
        let folders = Rc::clone(&self.folders);
        self.list_box.connect_row_selected(move |_, row| {
            if let Some(row) = row {
                let idx = row.index();
                if idx == 0 {
                    // "All chats"
                    callback(None);
                } else {
                    let f = folders.borrow().get(idx as usize - 1).cloned();
                    callback(f);
                }
            }
        });
    }

    pub fn container(&self) -> &Box {
        &self.container
    }
}
