#![allow(dead_code)]

use gtk::prelude::*;
use gtk::{
    Entry, Label, ListView, Orientation, ScrolledWindow,
    SingleSelection,
};
use std::cell::RefCell;
use std::rc::Rc;

use crate::models::{Chat, Message};

/// Result of a global search
#[derive(Clone, Debug)]
pub struct SearchResult {
    pub chat_id: String,
    pub chat_name: String,
    pub message_text: String,
    pub message_time: String,
    pub match_position: usize,
}

/// Global search panel overlay
pub struct GlobalSearch {
    pub container: gtk::Overlay,
    search_entry: Entry,
    results: Rc<RefCell<Vec<SearchResult>>>,
    list_view: ListView,
    selection: SingleSelection,
    /// Callback to open a chat from search results
    pub on_select: Rc<RefCell<Option<Rc<dyn Fn(String) + 'static>>>>,
}

impl GlobalSearch {
    pub fn new(chats: Rc<RefCell<Vec<Chat>>>, messages: Rc<RefCell<Vec<Message>>>) -> Self {
        let overlay = gtk::Overlay::new();
        overlay.set_halign(gtk::Align::Center);
        overlay.set_valign(gtk::Align::Start);
        overlay.set_margin_top(16);
        overlay.set_margin_start(16);
        overlay.set_margin_end(16);

        // Backdrop
        let backdrop = gtk::Box::new(Orientation::Vertical, 0);
        backdrop.set_css_classes(&["search-backdrop"]);
        backdrop.set_size_request(-1, 400);
        backdrop.set_halign(gtk::Align::Fill);
        backdrop.set_valign(gtk::Align::Start);
        backdrop.set_vexpand(true);
        backdrop.set_hexpand(true);

        // Search panel
        let panel = gtk::Box::new(Orientation::Vertical, 0);
        panel.set_css_classes(&["search-panel"]);
        panel.set_size_request(-1, 400);
        panel.set_margin_start(48);
        panel.set_margin_end(48);
        panel.set_margin_top(32);
        panel.set_margin_bottom(32);

        // Entry
        let search_entry = Entry::builder()
            .placeholder_text("Поиск сообщений... (Esc для закрытия)")
            .css_classes(vec!["search-bar".to_string()])
            .hexpand(true)
            .build();
        search_entry.set_margin_start(16);
        search_entry.set_margin_end(16);
        search_entry.set_margin_top(16);
        search_entry.set_margin_bottom(16);

        // Results list
        let model = Rc::new(RefCell::new(Vec::<SearchResult>::new()));
        let selection = SingleSelection::default();
        let factory = gtk::SignalListItemFactory::new();

        factory.connect_setup(move |_, obj| {
            let list_item = obj.downcast_ref::<gtk::ListItem>().unwrap();
            let label = Label::builder().build();
            list_item.set_child(Some(&label));
        });

        let model_clone2 = model.clone();
        factory.connect_bind(move |_, obj| {
            let list_item = obj.downcast_ref::<gtk::ListItem>().unwrap();
            let label = list_item.child().and_downcast::<Label>().unwrap();
            let position = list_item.position();
            let results = model_clone2.borrow();
            if let Some(result) = results.get(position as usize) {
                label.set_label(&format!(
                    "{}  {}\n{}",
                    result.chat_name, result.message_time, result.message_text
                ));
            }
        });

        let list_view = ListView::new(Some(selection.clone()), Some(factory));
        list_view.set_vexpand(true);
        list_view.set_hexpand(true);

        panel.append(&search_entry);
        panel.append(&ScrolledWindow::builder().child(&list_view).build());

        overlay.add_overlay(&panel);

        // Search debounce
        let model_clone = model.clone();
        let chats_clone = chats.clone();
        let messages_clone = messages.clone();
        search_entry.connect_changed(move |entry| {
            let query = entry.text().to_lowercase();
            let mut results: Vec<SearchResult> = Vec::new();

            if query.is_empty() {
                *model_clone.borrow_mut() = results;
                return;
            }

            let chats = chats_clone.borrow();
            let messages = messages_clone.borrow();

            for msg in messages.iter() {
                let text = msg.text.as_deref().unwrap_or("");
                if text.to_lowercase().contains(&query) {
                    let chat = chats.iter().find(|c| c.id == msg.chat_id);
                    let chat_name = chat
                        .map(|c| c.display_name())
                        .unwrap_or("Неизвестный чат".to_string());
                    let time = format_timestamp(&msg.created);

                    results.push(SearchResult {
                        chat_id: msg.chat_id.clone(),
                        chat_name: chat_name.to_string(),
                        message_text: text.to_string(),
                        message_time: time,
                        match_position: text.to_lowercase().find(&query).unwrap_or(0),
                    });
                }
            }

            // Sort by most recent
            results.sort_by(|a, b| {
                let chat_a = chats.iter().find(|c| c.id == a.chat_id);
                let chat_b = chats.iter().find(|c| c.id == b.chat_id);
                let time_a = chat_a.map(|c| c.last_message.as_ref().map(|m| m.created).unwrap_or_default());
                let time_b = chat_b.map(|c| c.last_message.as_ref().map(|m| m.created).unwrap_or_default());
                time_b.cmp(&time_a)
            });

            *model_clone.borrow_mut() = results;
        });

        // Select result
        let on_select: Rc<RefCell<Option<Rc<dyn Fn(String) + 'static>>>> = Rc::new(RefCell::new(None));
        let on_select_inner = on_select.clone();
        let selection_clone = selection.clone();
        let model_clone3 = model.clone();
        selection.connect_notify_local(Some("notify::selected-item"), move |_, _| {
            let idx = selection_clone.selected();
            let results = model_clone3.borrow();
            if idx < results.len() as u32 {
                let result = results[idx as usize].clone();
                if let Some(cb) = on_select_inner.borrow().as_ref() {
                    cb(result.chat_id);
                }
            }
        });

        Self {
            container: overlay,
            search_entry,
            results: model,
            list_view,
            selection,
            on_select,
        }
    }

    pub fn show(&self) {
        self.container.set_visible(true);
        self.search_entry.grab_focus();
        self.search_entry.set_text("");
    }

    pub fn hide(&self) {
        self.container.set_visible(false);
    }

    pub fn on_select<F: Fn(String) + 'static>(&self, callback: F) {
        *self.on_select.borrow_mut() = Some(Rc::new(callback));
    }
}

fn format_timestamp(dt: &chrono::DateTime<chrono::Utc>) -> String {
    let now = chrono::Utc::now();
    let diff = now.signed_duration_since(*dt);

    if diff.num_minutes() < 1 {
        "Только что".to_string()
    } else if diff.num_hours() < 24 {
        dt.format("%H:%M").to_string()
    } else if diff.num_days() == 1 {
        "Вчера".to_string()
    } else {
        dt.format("%d.%m").to_string()
    }
}
