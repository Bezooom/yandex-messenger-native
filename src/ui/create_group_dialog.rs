use gtk::prelude::*;
use gtk::{Button, Entry, Label, ListBox, ListBoxRow, Orientation, ScrolledWindow, Window};
use std::cell::RefCell;
use std::sync::Arc;

use crate::api::auth::AuthManager;
// use crate::models::Chat; // Temporarily commented - unused

/// Dialog for creating a new group or channel
pub struct CreateGroupDialog {
    window: Window,
    title_entry: Entry,
    description_entry: Entry,
    #[allow(dead_code)]
    member_list: ListBox,
    is_public_switch: gtk::Switch,
    chat_type_combo: gtk::DropDown,
    create_btn: Button,
    cancel_btn: Button,
    #[allow(dead_code)]
    selected_members: RefCell<Vec<String>>,
}

impl CreateGroupDialog {
    pub fn new(_auth: Arc<AuthManager>) -> Self {
        let window = Window::builder()
            .title("Создать группу или канал")
            .modal(true)
            .default_width(400)
            .default_height(500)
            .build();

        let main_box = gtk::Box::new(Orientation::Vertical, 12);
        main_box.set_margin_top(16);
        main_box.set_margin_bottom(16);
        main_box.set_margin_start(16);
        main_box.set_margin_end(16);

        // Chat type selection
        let type_box = gtk::Box::new(Orientation::Horizontal, 8);
        let type_label = Label::builder().label("Тип:").build();

        let chat_type_model = gtk::StringList::new(&["Группа", "Канал"]);
        let chat_type_combo = gtk::DropDown::builder().model(&chat_type_model).build();
        chat_type_combo.set_selected(0);

        type_box.append(&type_label);
        type_box.append(&chat_type_combo);

        // Title entry
        let title_label = Label::builder().label("Название:").xalign(0.0).build();
        let title_entry = Entry::builder()
            .placeholder_text("Введите название")
            .build();

        // Description entry
        let desc_label = Label::builder().label("Описание:").xalign(0.0).build();
        let description_entry = Entry::builder()
            .placeholder_text("Введите описание (необязательно)")
            .build();

        // Privacy toggle
        let privacy_box = gtk::Box::new(Orientation::Horizontal, 8);
        let privacy_label = Label::builder().label("Публичный:").build();
        let is_public_switch = gtk::Switch::new();
        privacy_box.append(&privacy_label);
        privacy_box.append(&is_public_switch);

        // Member selection
        let member_label = Label::builder().label("Участники:").xalign(0.0).build();

        let member_scrolled = ScrolledWindow::builder()
            .min_content_height(150)
            .max_content_height(200)
            .vexpand(true)
            .build();

        let member_list = ListBox::new();
        member_list.add_css_class("member-selection-list");
        member_scrolled.set_child(Some(&member_list));

        // Add some placeholder members (in real app, would load from contacts)
        for i in 1..=5 {
            let row = ListBoxRow::new();
            let row_box = gtk::Box::new(Orientation::Horizontal, 8);

            let check = gtk::CheckButton::new();
            let name_label = Label::builder()
                .label(&format!("Пользователь {}", i))
                .xalign(0.0)
                .hexpand(true)
                .build();

            row_box.append(&check);
            row_box.append(&name_label);
            row.set_child(Some(&row_box));
            member_list.append(&row);
        }

        // Buttons
        let button_box = gtk::Box::new(Orientation::Horizontal, 8);
        button_box.set_halign(gtk::Align::End);

        let cancel_btn = Button::with_label("Отмена");
        let create_btn = Button::with_label("Создать");
        create_btn.add_css_class("suggested-action");

        button_box.append(&cancel_btn);
        button_box.append(&create_btn);

        main_box.append(&type_box);
        main_box.append(&title_label);
        main_box.append(&title_entry);
        main_box.append(&desc_label);
        main_box.append(&description_entry);
        main_box.append(&privacy_box);
        main_box.append(&member_label);
        main_box.append(&member_scrolled);
        main_box.append(&button_box);

        window.set_child(Some(&main_box));

        Self {
            window,
            title_entry,
            description_entry,
            member_list,
            is_public_switch,
            chat_type_combo,
            create_btn,
            cancel_btn,
            selected_members: RefCell::new(Vec::new()),
        }
    }

    pub fn show(&self) {
        self.window.present();
    }

    pub fn hide(&self) {
        self.window.close();
    }

    pub fn set_transient_for<W: IsA<gtk::Window>>(&self, parent: &W) {
        self.window.set_transient_for(Some(parent));
    }

    pub fn get_title(&self) -> String {
        self.title_entry.text().to_string()
    }

    pub fn get_description(&self) -> Option<String> {
        let text = self.description_entry.text().to_string();
        if text.is_empty() {
            None
        } else {
            Some(text)
        }
    }

    pub fn is_public(&self) -> bool {
        self.is_public_switch.is_active()
    }

    pub fn is_channel(&self) -> bool {
        self.chat_type_combo.selected() == 1
    }

    pub fn connect_create_clicked<F: Fn() + 'static>(&self, callback: F) {
        self.create_btn.connect_clicked(move |_| callback());
    }

    pub fn connect_cancel_clicked<F: Fn() + 'static>(&self, callback: F) {
        self.cancel_btn.connect_clicked(move |_| callback());
    }
}
