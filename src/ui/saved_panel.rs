#![allow(dead_code)]

use gtk::prelude::*;
use gtk::{
    Box as GtkBox, Button, Entry, FlowBox, FlowBoxChild, Label, Orientation, ScrolledWindow,
};

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use crate::api::auth::AuthManager;
use crate::models::saved_message::{SavedFilter, SavedMessage};
use crate::ui::chat_list::format_message_time;

// ─────────────────────────────────────────────
//  Filter button widget
// ─────────────────────────────────────────────

struct FilterBtn {
    button: Button,
    filter: SavedFilter,
    active: RefCell<bool>,
}

impl FilterBtn {
    fn new(filter: SavedFilter, label: &str, icon: &str) -> Self {
        let button = Button::builder()
            .label(label)
            .icon_name(icon)
            .css_classes(vec!["saved-filter-btn".to_string()])
            .hexpand(false)
            .build();

        Self {
            button,
            filter,
            active: RefCell::new(false),
        }
    }

    fn activate(&self) {
        *self.active.borrow_mut() = true;
        self.button.add_css_class("active");
    }

    fn deactivate(&self) {
        *self.active.borrow_mut() = false;
        self.button.remove_css_class("active");
    }
}

// ─────────────────────────────────────────────
//  SavedPanel — the main widget
// ─────────────────────────────────────────────

pub struct SavedPanel {
    container: GtkBox,
    search_entry: Entry,
    filter_bar: GtkBox,
    message_list: FlowBox,
    filter_btns: Vec<FilterBtn>,
    current_filter: Rc<RefCell<SavedFilter>>,
    messages: Rc<RefCell<Vec<SavedMessage>>>,
    on_message_click: Rc<RefCell<Option<Box<dyn Fn(String, String)>>>>,
    on_message_unsave: Rc<RefCell<Option<Box<dyn Fn(String)>>>>,
    back_callback: Rc<RefCell<Option<Box<dyn Fn()>>>>,
}

impl SavedPanel {
    pub fn new(_auth: Arc<AuthManager>) -> Self {
        Self::apply_css();

        let messages: Rc<RefCell<Vec<SavedMessage>>> = Rc::new(RefCell::new(Vec::new()));
        let current_filter = Rc::new(RefCell::new(SavedFilter::All));
        let back_callback: Rc<RefCell<Option<Box<dyn Fn()>>>> = Rc::new(RefCell::new(None));
        let on_message_click = Rc::new(RefCell::new(None));
        let on_message_unsave = Rc::new(RefCell::new(None));

        // ── Header ──
        let header = GtkBox::new(Orientation::Horizontal, 8);
        header.set_margin_start(8);
        header.set_margin_end(8);
        header.set_margin_top(8);
        header.set_margin_bottom(4);

        let back_btn = Button::builder()
            .icon_name("go-previous-symbolic")
            .css_classes(vec!["action-icon-btn".to_string()])
            .tooltip_text("Назад к чатам")
            .build();

        let title = Label::builder()
            .label("Избранное")
            .css_classes(vec!["title".to_string()])
            .xalign(0.0)
            .build();
        title.set_margin_start(8);

        header.append(&back_btn);
        header.append(&title);

        let back_callback_clone = back_callback.clone();
        back_btn.connect_clicked(move |_| {
            if let Some(cb) = &*back_callback_clone.borrow() {
                cb();
            }
        });

        // ── Search bar ──
        let search_entry = Entry::builder()
            .placeholder_text("Поиск в сохранённых...")
            .css_classes(vec!["search-bar".to_string()])
            .margin_start(8)
            .margin_end(8)
            .margin_top(8)
            .margin_bottom(4)
            .primary_icon_name("edit-clear-symbolic")
            .primary_icon_activatable(true)
            .build();

        let search_entry_inner = search_entry.clone();
        let messages_clone = messages.clone();
        let filter_clone = current_filter.clone();
        search_entry.connect_changed(move |e| {
            let text = e.text().to_string();
            let query = text.clone();
            let msgs = messages_clone.clone();
            let filter = filter_clone.clone();
            let entry = search_entry_inner.clone();
            glib::timeout_add_local_once(std::time::Duration::from_millis(300), move || {
                let filter = *filter.borrow();
                Self::filter_messages(&msgs, &query, &filter);
                entry.set_text(&query);
            });
        });

        // ── Filter bar ──
        let filter_bar = Self::create_filter_bar(current_filter.clone(), messages.clone());

        // ── Message list ──
        let message_list = Self::create_message_list(
            messages.clone(),
            on_message_click.clone(),
            on_message_unsave.clone(),
        );

        // ── Scrolled window ──
        let scrolled = ScrolledWindow::builder()
            .min_content_height(500)
            .hexpand(true)
            .vexpand(true)
            .build();
        scrolled.set_child(Some(&message_list));

        // ── Assemble ──
        let saved_container = GtkBox::new(Orientation::Vertical, 0);
        saved_container.set_css_classes(&["saved-panel"]);
        saved_container.set_size_request(260, -1);
        saved_container.set_hexpand(true);
        saved_container.set_vexpand(true);

        saved_container.append(&header);
        saved_container.append(&search_entry);
        saved_container.append(&filter_bar);
        saved_container.append(&scrolled);

        let container = GtkBox::new(Orientation::Horizontal, 0);
        container.set_size_request(260, -1);
        container.set_hexpand(true);
        container.set_vexpand(true);
        container.append(&saved_container);

        Self {
            container,
            search_entry,
            filter_bar,
            message_list,
            filter_btns: Vec::new(),
            current_filter,
            messages,
            on_message_click,
            on_message_unsave,
            back_callback,
        }
    }

    fn create_filter_bar(
        current_filter: Rc<RefCell<SavedFilter>>,
        messages: Rc<RefCell<Vec<SavedMessage>>>,
    ) -> GtkBox {
        let filter_bar = GtkBox::builder()
            .orientation(Orientation::Horizontal)
            .spacing(4)
            .margin_start(8)
            .margin_end(8)
            .margin_top(4)
            .margin_bottom(4)
            .css_classes(vec!["saved-filter-bar".to_string()])
            .build();

        let filters = [
            (SavedFilter::All, "Все"),
            (SavedFilter::Text, "Текст"),
            (SavedFilter::Images, "Карт."),
            (SavedFilter::Links, "Ссылки"),
            (SavedFilter::Files, "Файлы"),
        ];

        for (filter, label) in filters {
            let btn = FilterBtn::new(filter, label, filter.icon());

            let current_filter = current_filter.clone();
            let messages = messages.clone();
            let btn_clone = btn.button.clone();

            btn.button.connect_clicked(move |widget| {
                let _ = widget;
                let current = *current_filter.borrow();
                if current != filter {
                    *current_filter.borrow_mut() = filter;
                    btn_clone.remove_css_class("active");
                    Self::filter_messages(&messages, "", &filter);
                }
                btn_clone.add_css_class("active");
            });

            filter_bar.append(&btn.button);
        }

        filter_bar
    }

    fn create_message_list(
        messages: Rc<RefCell<Vec<SavedMessage>>>,
        on_click: Rc<RefCell<Option<Box<dyn Fn(String, String)>>>>,
        on_unsave: Rc<RefCell<Option<Box<dyn Fn(String)>>>>,
    ) -> FlowBox {
        let message_list = FlowBox::builder()
            .min_children_per_line(1)
            .max_children_per_line(1)
            .selection_mode(gtk::SelectionMode::Single)
            .build();

        // Render initial messages
        Self::render_messages(&message_list, &messages, on_click, on_unsave);

        message_list
    }

    fn render_messages(
        message_list: &FlowBox,
        messages: &Rc<RefCell<Vec<SavedMessage>>>,
        on_click: Rc<RefCell<Option<Box<dyn Fn(String, String)>>>>,
        on_unsave: Rc<RefCell<Option<Box<dyn Fn(String)>>>>,
    ) {
        // Clear
        message_list.remove_all();

        let msgs = messages.borrow();
        if msgs.is_empty() {
            let empty = Label::builder()
                .label("Сохранённые сообщения появятся здесь")
                .css_classes(vec!["dim-label".to_string()])
                .xalign(0.5)
                .yalign(0.5)
                .build();
            empty.set_margin_top(40);
            let empty_box = GtkBox::builder()
                .orientation(Orientation::Vertical)
                .hexpand(true)
                .build();
            empty_box.append(&empty);
            let child = FlowBoxChild::builder().child(&empty_box).build();
            message_list.append(&child);
            return;
        }

        for msg in msgs.iter() {
            let row = Self::create_message_row(msg, on_click.clone(), on_unsave.clone());
            message_list.append(&row);
        }
    }

    fn filter_messages(
        messages: &Rc<RefCell<Vec<SavedMessage>>>,
        query: &str,
        filter: &SavedFilter,
    ) {
        let msgs = messages.borrow();
        let q_lower = query.to_lowercase();

        let filtered: Vec<SavedMessage> = msgs
            .iter()
            .filter(|m| {
                // Apply media type filter
                let media_match = match filter {
                    SavedFilter::All => true,
                    SavedFilter::Text => {
                        m.media_type.as_deref() == Some("text") || m.media_type.is_none()
                    }
                    SavedFilter::Images => m.media_type.as_deref() == Some("image"),
                    SavedFilter::Links => m.media_type.as_deref() == Some("link"),
                    SavedFilter::Files => m.media_type.as_deref() == Some("file"),
                };

                // Apply search query
                let query_match = q_lower.is_empty()
                    || m.preview
                        .as_deref()
                        .unwrap_or("")
                        .to_lowercase()
                        .contains(&q_lower)
                    || m.note
                        .as_deref()
                        .unwrap_or("")
                        .to_lowercase()
                        .contains(&q_lower);

                media_match && query_match
            })
            .cloned()
            .collect();

        let mut store = messages.borrow_mut();
        *store = filtered;
    }

    fn create_message_row(
        msg: &SavedMessage,
        on_click: Rc<RefCell<Option<Box<dyn Fn(String, String)>>>>,
        on_unsave: Rc<RefCell<Option<Box<dyn Fn(String)>>>>,
    ) -> GtkBox {
        let row = GtkBox::builder()
            .orientation(Orientation::Horizontal)
            .spacing(8)
            .css_classes(vec!["saved-message-row".to_string()])
            .margin_start(8)
            .margin_end(8)
            .margin_top(4)
            .margin_bottom(4)
            .build();

        // Media type icon
        let icon_name = match msg.media_type.as_deref() {
            Some("image") => "image-x-generic-symbolic",
            Some("link") => "x-office-document-symbolic",
            Some("file") => "package-x-generic-symbolic",
            _ => "document-open-symbolic",
        };

        let icon = gtk::Image::from_icon_name(icon_name);
        icon.add_css_class("saved-type-icon");

        // Preview text
        let preview_text = msg.preview.as_deref().unwrap_or("Сообщение").to_string();
        let preview = Label::builder()
            .label(&preview_text)
            .hexpand(true)
            .halign(gtk::Align::Start)
            .wrap(true)
            .wrap_mode(gtk::pango::WrapMode::WordChar)
            .max_width_chars(40)
            .build();

        // Note
        let note = if let Some(note) = &msg.note {
            let note_label = Label::builder()
                .label(note)
                .hexpand(true)
                .halign(gtk::Align::Start)
                .wrap(true)
                .css_classes(vec!["saved-note".to_string()])
                .build();
            Some(note_label)
        } else {
            None
        };

        // Date
        let date = format_message_time(&msg.saved_at);
        let date_label = Label::builder()
            .label(&date)
            .css_classes(vec!["saved-date".to_string()])
            .xalign(1.0)
            .build();

        // Unsave button
        let msg_id = msg.message_id.clone();
        let unsave_btn = Rc::new(
            Button::builder()
                .icon_name("object-select-symbolic")
                .css_classes(vec!["icon-btn".to_string()])
                .build(),
        );
        unsave_btn.set_visible(false);
        unsave_btn.set_size_request(24, 24);

        // Hover effect
        let hover = gtk::EventControllerMotion::new();
        let row_clone = row.clone();
        hover.connect_enter(move |_, _, _| {
            row_clone.add_css_class("hovered");
        });
        let row_clone2 = row.clone();
        hover.connect_leave(move |_| {
            row_clone2.remove_css_class("hovered");
        });
        row.add_controller(hover);

        // Click to open
        let msg_id_click = msg.message_id.clone();
        let chat_id = msg.source_chat_id.clone();
        let click = gtk::GestureClick::new();
        let on_click_clone = on_click.clone();
        click.connect_pressed(move |_gesture, _n_press, _x, _y| {
            log::info!(
                "Opening saved message: {} from chat {}",
                msg_id_click,
                chat_id
            );
            if let Some(cb) = &*on_click_clone.borrow() {
                cb(chat_id.clone(), msg_id_click.clone());
            }
        });
        row.add_controller(click);

        // Unsave button hover
        let unsave_btn_hover = unsave_btn.clone();
        let hover2 = gtk::EventControllerMotion::new();
        hover2.connect_enter(move |_, _, _| {
            unsave_btn_hover.set_visible(true);
        });
        row.add_controller(hover2);

        let msg_id_clone = msg_id.clone();
        let unsave_btn_weak = Rc::downgrade(&unsave_btn);
        let on_unsave_clone = on_unsave.clone();
        unsave_btn.connect_clicked(move |_| {
            log::info!("Unsaved message: {}", msg_id_clone);
            if let Some(btn) = unsave_btn_weak.upgrade() {
                btn.set_visible(false);
            }
            if let Some(cb) = &*on_unsave_clone.borrow() {
                cb(msg_id_clone.clone());
            }
        });

        let unsave_btn_ref2 = unsave_btn.as_ref();
        row.append(unsave_btn_ref2);

        row.append(&icon);
        if let Some(note) = note {
            let notes = GtkBox::builder()
                .orientation(Orientation::Vertical)
                .hexpand(true)
                .build();
            notes.append(&note);
            row.append(&notes);
        }
        row.append(&preview);
        row.append(&date_label);

        row
    }

    fn apply_css() {
        let provider = gtk::CssProvider::new();
        let css = r#"
            .saved-panel {
                background: @bg_chat;
            }
            .saved-message-row {
                padding: 6px 8px;
                border-radius: 8px;
            }
            .saved-message-row:hover {
                background: @bg_hover;
            }
            .saved-message-row:hover .saved-note {
                color: @text_secondary;
            }
            .saved-type-icon {
                color: @text_tertiary;
            }
            .saved-filter-bar {
                padding: 2px 0;
            }
            .saved-filter-btn {
                font-size: 12px;
                border-radius: 6px;
                padding: 4px 8px;
            }
            .saved-filter-btn.active {
                background: @bg_selected;
                color: @text_primary;
            }
            .saved-filter-btn.active image {
                color: @brand_yellow;
            }
            .saved-note {
                font-size: 11px;
                color: @text_secondary;
                margin-top: 2px;
            }
            .saved-date {
                font-size: 10px;
                color: @text_tertiary;
                margin-left: 8px;
            }
        "#;
        if let Some(display) = gtk::gdk::Display::default() {
            provider.load_from_string(css);
            gtk::style_context_add_provider_for_display(
                &display,
                &provider,
                gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
        }
    }

    /// Set messages from the API
    pub fn set_messages(&self, messages: Vec<SavedMessage>) {
        *self.messages.borrow_mut() = messages;
        Self::render_messages(
            &self.message_list,
            &self.messages,
            self.on_message_click.clone(),
            self.on_message_unsave.clone(),
        );
    }

    /// Add a new saved message
    pub fn add_message(&self, msg: SavedMessage) {
        let mut msgs = self.messages.borrow_mut();
        msgs.push(msg);
        // Re-render with the new message
        Self::render_messages(
            &self.message_list,
            &self.messages,
            self.on_message_click.clone(),
            self.on_message_unsave.clone(),
        );
    }

    /// Get current filter
    pub fn current_filter(&self) -> SavedFilter {
        *self.current_filter.borrow()
    }

    /// Set the filter (called by parent)
    pub fn set_filter(&self, filter: SavedFilter) {
        *self.current_filter.borrow_mut() = filter;
        // Update button states
        for btn in self.filter_btns.iter() {
            if btn.filter == filter {
                btn.activate();
            } else {
                btn.deactivate();
            }
        }
        Self::filter_messages(&self.messages, "", &filter);
        Self::render_messages(
            &self.message_list,
            &self.messages,
            self.on_message_click.clone(),
            self.on_message_unsave.clone(),
        );
    }

    /// Connect callback for clicking a message
    pub fn on_message_click<F: Fn(String, String) + 'static>(&self, callback: F) {
        *self.on_message_click.borrow_mut() = Some(Box::new(callback));
    }

    /// Connect callback for unsave
    pub fn on_message_unsave<F: Fn(String) + 'static>(&self, callback: F) {
        *self.on_message_unsave.borrow_mut() = Some(Box::new(callback));
    }

    pub fn connect_back_clicked<F: Fn() + 'static>(&self, callback: F) {
        *self.back_callback.borrow_mut() = Some(Box::new(callback));
    }

    pub fn container(&self) -> &GtkBox {
        &self.container
    }
}
