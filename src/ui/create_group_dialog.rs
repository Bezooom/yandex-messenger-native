use gtk::prelude::*;
use gtk::{
    Align, Box as GtkBox, Button, CheckButton, Entry, Label, ListBox, ListBoxRow, Orientation,
    ScrolledWindow, Window,
};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::Arc;

use crate::api::auth::AuthManager;
use crate::models::ContactCandidate;

/// Dialog for creating a new group or channel with real contact selection.
pub struct CreateGroupDialog {
    window: Window,
    title_entry: Entry,
    description_entry: Entry,
    search_entry: Entry,
    member_list: ListBox,
    member_count_label: Label,
    is_public_switch: gtk::Switch,
    chat_type_combo: gtk::DropDown,
    create_btn: Button,
    cancel_btn: Button,
    candidates: RefCell<Vec<ContactCandidate>>,
    selected_members: Rc<RefCell<HashSet<String>>>,
    check_states: Rc<RefCell<HashMap<String, bool>>>,
}

impl CreateGroupDialog {
    pub fn new(_auth: Arc<AuthManager>) -> Self {
        let window = Window::builder()
            .title("Создать группу или канал")
            .modal(true)
            .default_width(480)
            .default_height(640)
            .resizable(true)
            .build();
        window.add_css_class("create-group-dialog");

        let main_box = GtkBox::new(Orientation::Vertical, 12);
        main_box.set_margin_top(16);
        main_box.set_margin_bottom(16);
        main_box.set_margin_start(16);
        main_box.set_margin_end(16);
        main_box.set_hexpand(true);
        main_box.set_vexpand(true);

        // Type
        let type_box = GtkBox::new(Orientation::Horizontal, 8);
        let type_label = Label::builder().label("Тип:").xalign(0.0).build();
        type_label.set_width_chars(10);
        let chat_type_model = gtk::StringList::new(&["Группа", "Канал"]);
        let chat_type_combo = gtk::DropDown::builder()
            .model(&chat_type_model)
            .hexpand(true)
            .build();
        chat_type_combo.set_selected(0);
        type_box.append(&type_label);
        type_box.append(&chat_type_combo);

        // Title
        let title_label = Label::builder().label("Название:").xalign(0.0).build();
        let title_entry = Entry::builder()
            .placeholder_text("Введите название")
            .hexpand(true)
            .build();

        // Description
        let desc_label = Label::builder().label("Описание:").xalign(0.0).build();
        let description_entry = Entry::builder()
            .placeholder_text("Введите описание (необязательно)")
            .hexpand(true)
            .build();

        // Privacy
        let privacy_box = GtkBox::new(Orientation::Horizontal, 8);
        let privacy_label = Label::builder()
            .label("Публичный")
            .xalign(0.0)
            .hexpand(true)
            .build();
        let is_public_switch = gtk::Switch::new();
        is_public_switch.set_valign(Align::Center);
        privacy_box.append(&privacy_label);
        privacy_box.append(&is_public_switch);

        // Members header
        let member_header = GtkBox::new(Orientation::Horizontal, 8);
        let member_label = Label::builder()
            .label("Участники")
            .xalign(0.0)
            .css_classes(["create-group-section-title"])
            .hexpand(true)
            .build();
        let member_count_label = Label::builder()
            .label("0 выбрано")
            .xalign(1.0)
            .css_classes(["dim-label", "create-group-member-count"])
            .build();
        member_header.append(&member_label);
        member_header.append(&member_count_label);

        // Search
        let search_entry = Entry::builder()
            .placeholder_text("Поиск по имени…")
            .hexpand(true)
            .build();
        search_entry.add_css_class("create-group-search");
        search_entry.set_primary_icon_name(Some("system-search-symbolic"));

        // Full-height contact list
        let member_scrolled = ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .min_content_height(300)
            .vexpand(true)
            .hexpand(true)
            .build();
        member_scrolled.add_css_class("create-group-member-scroll");

        let member_list = ListBox::new();
        member_list.add_css_class("member-selection-list");
        member_list.set_selection_mode(gtk::SelectionMode::None);
        member_list.set_show_separators(false);
        member_scrolled.set_child(Some(&member_list));

        let placeholder = Label::builder()
            .label("Загрузка контактов…")
            .css_classes(["dim-label"])
            .margin_top(24)
            .margin_bottom(24)
            .build();
        member_list.set_placeholder(Some(&placeholder));

        // Buttons
        let button_box = GtkBox::new(Orientation::Horizontal, 8);
        button_box.set_halign(Align::End);
        button_box.set_margin_top(4);
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
        main_box.append(&member_header);
        main_box.append(&search_entry);
        main_box.append(&member_scrolled);
        main_box.append(&button_box);
        window.set_child(Some(&main_box));

        let selected_members = Rc::new(RefCell::new(HashSet::new()));
        let check_states = Rc::new(RefCell::new(HashMap::new()));

        let dialog = Self {
            window,
            title_entry,
            description_entry,
            search_entry,
            member_list,
            member_count_label,
            is_public_switch,
            chat_type_combo,
            create_btn,
            cancel_btn,
            candidates: RefCell::new(Vec::new()),
            selected_members,
            check_states,
        };
        dialog.wire_search();
        dialog
    }

    fn wire_search(&self) {
        let list = self.member_list.clone();
        self.search_entry.connect_changed(move |entry| {
            let query = entry.text().to_string().to_lowercase();
            let mut child = list.first_child();
            while let Some(row_w) = child {
                let next = row_w.next_sibling();
                if let Ok(row) = row_w.downcast::<ListBoxRow>() {
                    let haystack = row_search_text(&row).to_lowercase();
                    row.set_visible(query.is_empty() || haystack.contains(&query));
                }
                child = next;
            }
        });
    }

    /// Load contacts with real display names and rebuild the list.
    pub fn load_candidates(&self, mut contacts: Vec<ContactCandidate>) {
        contacts.sort_by(|a, b| {
            a.primary_name()
                .to_lowercase()
                .cmp(&b.primary_name().to_lowercase())
        });
        *self.candidates.borrow_mut() = contacts;
        self.rebuild_rows();
    }

    fn rebuild_rows(&self) {
        while let Some(row) = self.member_list.first_child() {
            self.member_list.remove(&row);
        }

        let contacts = self.candidates.borrow().clone();
        if contacts.is_empty() {
            let empty = Label::builder()
                .label("Контакты не найдены")
                .css_classes(["dim-label"])
                .margin_top(24)
                .margin_bottom(24)
                .build();
            self.member_list.set_placeholder(Some(&empty));
            self.update_count_label();
            return;
        }

        let selected = self.selected_members.clone();
        let check_states = self.check_states.clone();
        let count_label = self.member_count_label.clone();

        for contact in contacts {
            let row = ListBoxRow::new();
            row.add_css_class("member-selection-row");
            row.set_activatable(true);

            let row_box = GtkBox::new(Orientation::Horizontal, 10);
            row_box.set_margin_top(6);
            row_box.set_margin_bottom(6);
            row_box.set_margin_start(8);
            row_box.set_margin_end(8);

            // Avatar
            let avatar = GtkBox::new(Orientation::Horizontal, 0);
            avatar.add_css_class("avatar");
            avatar.add_css_class("member-avatar");
            avatar.set_size_request(40, 40);
            avatar.set_halign(Align::Center);
            avatar.set_valign(Align::Center);
            let avatar_label = Label::builder()
                .label(&contact.initials())
                .css_classes(["avatar-label"])
                .halign(Align::Center)
                .valign(Align::Center)
                .build();
            avatar.append(&avatar_label);

            // Names
            let info = GtkBox::new(Orientation::Vertical, 2);
            info.set_hexpand(true);
            info.set_valign(Align::Center);

            let primary = contact.primary_name();
            let name_label = Label::builder()
                .label(&primary)
                .xalign(0.0)
                .ellipsize(gtk::pango::EllipsizeMode::End)
                .max_width_chars(28)
                .width_chars(1)
                .hexpand(true)
                .css_classes(["member-name"])
                .build();

            let secondary = contact.secondary_name();
            let sub_label = Label::builder()
                .label(secondary.as_deref().unwrap_or(""))
                .xalign(0.0)
                .ellipsize(gtk::pango::EllipsizeMode::End)
                .max_width_chars(32)
                .width_chars(1)
                .hexpand(true)
                .css_classes(["dim-label", "member-subtitle"])
                .visible(secondary.is_some())
                .build();

            info.append(&name_label);
            info.append(&sub_label);

            // Checkbox
            let check = CheckButton::new();
            check.set_valign(Align::Center);
            let is_checked = selected.borrow().contains(&contact.guid)
                || check_states
                    .borrow()
                    .get(&contact.guid)
                    .copied()
                    .unwrap_or(false);
            check.set_active(is_checked);

            let guid = contact.guid.clone();
            let selected_t = selected.clone();
            let states_t = check_states.clone();
            let count_t = count_label.clone();
            check.connect_toggled(move |btn| {
                let active = btn.is_active();
                states_t.borrow_mut().insert(guid.clone(), active);
                if active {
                    selected_t.borrow_mut().insert(guid.clone());
                } else {
                    selected_t.borrow_mut().remove(&guid);
                }
                count_t.set_label(&format_selected_count(selected_t.borrow().len()));
            });

            let check_act = check.clone();
            row.connect_activate(move |_| {
                check_act.set_active(!check_act.is_active());
            });

            row_box.append(&avatar);
            row_box.append(&info);
            row_box.append(&check);
            row.set_child(Some(&row_box));
            // Store searchable text on the row via widget name
            row.set_widget_name(&format!("{} {}", primary, secondary.unwrap_or_default()));
            self.member_list.append(&row);
        }

        self.update_count_label();
    }

    fn update_count_label(&self) {
        let n = self.selected_members.borrow().len();
        self.member_count_label.set_label(&format_selected_count(n));
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

    pub fn get_selected_members(&self) -> Vec<String> {
        self.selected_members.borrow().iter().cloned().collect()
    }

    pub fn connect_create_clicked<F: Fn() + 'static>(&self, callback: F) {
        self.create_btn.connect_clicked(move |_| callback());
    }

    pub fn connect_cancel_clicked<F: Fn() + 'static>(&self, callback: F) {
        self.cancel_btn.connect_clicked(move |_| callback());
    }
}

fn row_search_text(row: &ListBoxRow) -> String {
    let name = row.widget_name();
    if !name.is_empty() {
        return name.to_string();
    }
    String::new()
}

fn format_selected_count(n: usize) -> String {
    match n {
        0 => "0 выбрано".to_string(),
        1 => "1 выбран".to_string(),
        _ => format!("{} выбрано", n),
    }
}
