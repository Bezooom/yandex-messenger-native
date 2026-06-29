#![allow(dead_code)]

  use gtk::prelude::*;
use gtk::glib;
 use gtk::prelude::ListModelExt;
use gtk::{
    Box as GtkBox, Button, Entry, Label, Orientation, Popover,
    ScrolledWindow, Separator, SingleSelection, StringList,
};
use std::sync::{Arc, Mutex};
use gtk::CssProvider;
use libadwaita as adw;

use crate::api::auth::AuthManager;
use crate::models::Chat;
use crate::ui::account_dropdown::AccountDropdown;
use std::sync::OnceLock;
use std::collections::HashMap;

static AVATAR_CACHE: OnceLock<Mutex<HashMap<String, gtk::gdk::Texture>>> = OnceLock::new();
fn get_avatar_cache() -> &'static Mutex<HashMap<String, gtk::gdk::Texture>> {
    AVATAR_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

// ─────────────────────────────────────────────
//  ChatRowData — item stored in the list model
// ─────────────────────────────────────────────

#[derive(Clone, Debug)]
struct ChatRowData {
    pub orig_index: usize,
    pub chat: Chat,
}

// ─────────────────────────────────────────────
//  ChatListModel — model backing the ListView
// ─────────────────────────────────────────────

#[derive(Clone)]
struct ChatListModel {
    chats: Arc<Mutex<Vec<Chat>>>,
    visible: Arc<Mutex<Vec<usize>>>,
    selected: Arc<Mutex<Option<usize>>>,
    chat_ids_list: Arc<gtk::StringList>,
}

impl ChatListModel {
    fn new(chats: Arc<Mutex<Vec<Chat>>>, visible: Arc<Mutex<Vec<usize>>>, chat_ids_list: Arc<gtk::StringList>) -> Self {
        Self {
            chats,
            visible,
            selected: Arc::new(Mutex::new(None)),
            chat_ids_list,
        }
    }

    fn n_items(&self) -> u32 {
        self.visible.lock().unwrap().len() as u32
    }

    fn item(&self, pos: u32) -> Option<ChatRowData> {
        let vis = self.visible.lock().unwrap();
        let orig_idx = *vis.get(pos as usize)?;
        let chats = self.chats.lock().unwrap();
        let chat = chats.get(orig_idx).cloned()?;
        Some(ChatRowData { orig_index: orig_idx, chat })
    }

    fn set_selection(&self, pos: Option<usize>) {
        *self.selected.lock().unwrap() = pos;
    }

    fn filter(&self, query: &str) {
        let chats = self.chats.lock().unwrap();
        let q_lower = query.to_lowercase();
        let mut visible: Vec<usize> = Vec::new();
        for (i, chat) in chats.iter().enumerate() {
            let title = chat.display_name().to_lowercase();
            let last_msg = chat.last_message.as_ref()
                .map(|m| m.preview().to_lowercase())
                .unwrap_or_default();
            if q_lower.is_empty() || title.contains(&q_lower) || last_msg.contains(&q_lower) {
                visible.push(i);
            }
        }
        *self.visible.lock().unwrap() = visible;
    }

/// Update the StringList model to reflect the current visible state.
    /// Called after filter() and set_chats() so the ListView knows how many items to render.
    /// IMPORTANT: We must modify the EXISTING StringList in-place (via splice),
    /// NOT replace it, because the SingleSelection holds a reference to the original object.
    fn update_chat_ids_model(&self) {
        let strings: Vec<String> = {
            let visible = self.visible.lock().unwrap();
            visible.iter()
                .filter_map(|&idx| {
                    let chats = self.chats.lock().unwrap();
                    chats.get(idx).map(|chat| chat.id.clone())
                })
                .collect()
        };
        let refs: Vec<&str> = strings.iter().map(|s| s.as_str()).collect();
        let list = &self.chat_ids_list;
        let old_count = list.n_items();
        list.splice(0, old_count, &refs);
        eprintln!("[CHATLIST] Updated model: {} visible chats (was {})", refs.len(), old_count);
    }
}

 // ─────────────────────────────────────────────
//  ChatListPanel — the main widget
// ─────────────────────────────────────────────

pub struct ChatListPanel {
    container: GtkBox,
    list_view: gtk::ListView,
    selection: SingleSelection,
    model: ChatListModel,
    chat_ids_list: Arc<gtk::StringList>,
    _search_timeout: Option<glib::SourceId>,
    search_entry: Entry,
    logout_callback: Arc<Mutex<Option<Box<dyn Fn()>>>>,
    settings_callback: Arc<Mutex<Option<Box<dyn Fn()>>>>,
    switch_account_callback: Arc<Mutex<Option<Box<dyn Fn(String)>>>>,
    add_account_callback: Arc<Mutex<Option<Box<dyn Fn()>>>>,
    create_group_callback: Arc<Mutex<Option<Box<dyn Fn()>>>>,
    user_avatar: GtkBox,
    avatar_label: Label,
    user_name_label: Label,
}

impl ChatListPanel {
    /// Get the root container widget
    pub fn root(&self) -> &GtkBox {
        &self.container
    }

    pub fn new(auth: Arc<AuthManager>) -> Self {
        Self::apply_css();

        let chats: Arc<Mutex<Vec<Chat>>> = Arc::new(Mutex::new(Vec::new()));
        let visible: Arc<Mutex<Vec<usize>>> = Arc::new(Mutex::new(Vec::new()));

        // Create a StringList as the model for the ListView.
        let chat_ids_list = Arc::new(StringList::new(&[]));
        let model = ChatListModel::new(Arc::clone(&chats), Arc::clone(&visible), Arc::clone(&chat_ids_list));

        let logout_callback = Arc::new(Mutex::new(None));
        let logout_callback_clone = logout_callback.clone();
        let settings_callback = Arc::new(Mutex::new(None));
        let settings_callback_clone = settings_callback.clone();
        let switch_account_callback = Arc::new(Mutex::new(None));
        let switch_account_callback_clone = switch_account_callback.clone();
        let add_account_callback = Arc::new(Mutex::new(None));
        let add_account_callback_clone = add_account_callback.clone();
        let create_group_callback = Arc::new(Mutex::new(None));
        let create_group_callback_clone = create_group_callback.clone();

        // Use StringList with SingleSelection — gtk::StringList implements IsA<gio::ListModel>
        let selection = SingleSelection::new(Some((*chat_ids_list).clone()));

        // ── Search bar ──
        let search_entry = Self::create_search_entry(&model);

         // ── ListView ──
         let factory = Self::create_item_factory(&model, auth.clone());
           let list_view = gtk::ListView::new(Some(selection.clone()), Some(factory));
        list_view.set_accessible_role(gtk::AccessibleRole::List);
        list_view.set_hexpand(true);
        list_view.set_vexpand(true);

        // ── Scrolled window ──
        let scrolled = ScrolledWindow::builder()
            .min_content_height(400)
            .hexpand(false)
            .vexpand(true)
            .build();
        scrolled.set_child(Some(&list_view));

        // ── Header ──
        let (header, user_avatar, avatar_label, user_name_label) = Self::create_header(
            auth.clone(),
            &logout_callback_clone,
            &settings_callback_clone,
            &switch_account_callback_clone,
            &add_account_callback_clone,
            &create_group_callback_clone,
        );

        // ── Keyboard controller ──
        let key_ctrl = Self::create_key_controller(&selection);
        list_view.add_controller(key_ctrl);

        // ── Context menu (right-click) ──
        let gesture = Self::create_context_gesture(&selection, &chats);
        list_view.add_controller(gesture);

        // ── Drag source ──
        let drag_src = gtk::DragSource::new();
        drag_src.set_actions(gtk::gdk::DragAction::MOVE | gtk::gdk::DragAction::COPY);
        drag_src.connect_prepare({
            let sel = selection.clone();
            move |source, _x, _y| {
                let idx = sel.selected();
                let value = glib::Value::from(idx);
                let provider = gtk::gdk::ContentProvider::for_value(&value);
                source.set_content(Some(&provider));
                Some(provider)
            }
        });
        list_view.add_controller(drag_src);

        // ── Drop target ──
        let drop = gtk::DropTarget::new(
            gtk::glib::Type::U32,
            gtk::gdk::DragAction::MOVE | gtk::gdk::DragAction::COPY,
        );
        let model_clone = model.clone();
          let selection_clone = selection.clone();
        drop.connect_drop(move |_, value, _x, _y| {
            if let Ok(src_idx) = value.get::<u32>() {
                // Use selection model to get the current drop position
                let dest_idx = selection_clone.selected();
                log::info!("Reordered chat {} to {}", src_idx, dest_idx);

                let src_idx_usize = src_idx as usize;
                let dest_idx_usize = dest_idx as usize;

                let mut chats = model_clone.chats.lock().unwrap();
                if src_idx_usize < chats.len() {
                    let dest = dest_idx_usize.min(chats.len().saturating_sub(1));
                    let chat = chats.remove(src_idx_usize);
                    chats.insert(dest, chat);

                    let count = chats.len();
                    let all_indices: Vec<usize> = (0..count).collect();
                    *model_clone.visible.lock().unwrap() = all_indices;
                    model_clone.set_selection(None);
                }
                return true;
            }
            false
        });
        list_view.add_controller(drop);

        // ── Assemble ──
         let chat_container = GtkBox::new(Orientation::Vertical, 0);
        chat_container.set_size_request(260, -1);
        chat_container.set_hexpand(true);
        chat_container.set_vexpand(true);

        chat_container.append(&header);
        chat_container.append(&search_entry);
        chat_container.append(&Separator::new(Orientation::Horizontal));
        chat_container.append(&scrolled);

          let container = GtkBox::new(Orientation::Horizontal, 0);
        container.set_size_request(260, -1);
        container.set_accessible_role(gtk::AccessibleRole::List);
        container.set_hexpand(true);
        container.set_vexpand(true);
        container.append(&chat_container);

 
        // ── Search debounce ──
        let search_rc: Arc<Mutex<Option<glib::SourceId>>> = Arc::new(Mutex::new(None));
        let model_clone = model.clone();
        {
            let entry = search_entry.clone();
            let model_ref = model_clone.clone();

            search_entry.connect_changed(move |_| {
                let entry = entry.clone();
                let model = model_ref.clone();
                let token_rc = search_rc.clone();

                if let Some(id) = token_rc.lock().unwrap().take() {
                    id.remove();
                }

                *token_rc.lock().unwrap() = Some(glib::timeout_add_local_once(
                    std::time::Duration::from_millis(300),
                    move || {
                        let q = entry.text().to_string();
                        model.filter(&q);
                        // Update the GtkGioListStore model so the ListView knows about the items.
                        model.update_chat_ids_model();
                    },
                ));
            });
        }

        let panel = Self {
            container,
            list_view,
            selection,
            model,
            chat_ids_list,
            _search_timeout: None,
            search_entry,
            logout_callback,
            settings_callback,
            switch_account_callback,
            add_account_callback,
            create_group_callback,
            user_avatar,
            avatar_label,
            user_name_label,
        };

        panel.refresh_header(&auth);
        panel
    }

    /// Connect logout callback
    pub fn connect_logout<F: Fn() + 'static>(&self, callback: F) {
        *self.logout_callback.lock().unwrap() = Some(Box::new(callback));
    }

    /// Connect settings callback
    pub fn connect_settings<F: Fn() + 'static>(&self, callback: F) {
        *self.settings_callback.lock().unwrap() = Some(Box::new(callback));
    }

    pub fn connect_switch_account<F: Fn(String) + 'static>(&self, callback: F) {
        *self.switch_account_callback.lock().unwrap() = Some(Box::new(callback));
    }

    pub fn connect_add_account<F: Fn() + 'static>(&self, callback: F) {
        *self.add_account_callback.lock().unwrap() = Some(Box::new(callback));
    }

    pub fn connect_create_group<F: Fn() + 'static>(&self, callback: F) {
        *self.create_group_callback.lock().unwrap() = Some(Box::new(callback));
    }

    pub fn refresh_header(&self, auth: &AuthManager) {
        let user_name_label = self.user_name_label.clone();
        let user_avatar = self.user_avatar.clone();
        let auth = auth.clone();
        
        glib::spawn_future_local(async move {
            if let Some(id) = auth.get_current_account_id().await {
                let accounts = auth.list_accounts().await;
                if let Some(acc) = accounts.iter().find(|a| a.id == id) {
                    let label = acc.display_label();
                    let initials: String = label
                        .chars()
                        .take(2)
                        .map(|c| c.to_ascii_uppercase())
                        .collect();
                    user_name_label.set_label(&label);

                    // Clear any existing children of the user_avatar box
                    while let Some(child) = user_avatar.first_child() {
                        user_avatar.remove(&child);
                    }

                    user_avatar.remove_css_class("avatar");
                    for i in 0..8 {
                        user_avatar.remove_css_class(&format!("avatar-gradient-{}", i));
                    }
                    user_avatar.add_css_class("avatar");

                    let hash = hash_color(&id);
                    user_avatar.add_css_class(&format!("avatar-gradient-{}", hash % 8));

                    if let Some(ref avatar_id) = acc.avatar_url {
                        let normalized_id = avatar_id.replace("user_avatar/", "");
                        let avatar_url = format!(
                            "https://avatars.mds.yandex.net/get-{}/islands-middle",
                            normalized_id
                        );
                        let user_avatar_clone = user_avatar.clone();
                        let initials_clone = initials.clone();
                        let hash_clone = hash;
                        glib::spawn_future_local(async move {
                            if let Ok(bytes) = download_avatar_bytes(&avatar_url, None).await {
                                while let Some(child) = user_avatar_clone.first_child() {
                                    user_avatar_clone.remove(&child);
                                }
                                for i in 0..8 {
                                    user_avatar_clone.remove_css_class(&format!("avatar-gradient-{}", i));
                                }
                                let bytes_glib = glib::Bytes::from(&bytes);
                                if let Ok(texture) = gtk::gdk::Texture::from_bytes(&bytes_glib) {
                                    let image = gtk::Image::from_paintable(Some(&texture));
                                    image.add_css_class("avatar-image");
                                    image.set_pixel_size(40);
                                    user_avatar_clone.append(&image);
                                    return;
                                }
                            }
                            // Fallback
                            let label = Label::builder()
                                .label(&initials_clone)
                                .css_classes(vec!["avatar-label".to_string()])
                                .build();
                            user_avatar_clone.append(&label);
                            user_avatar_clone.add_css_class(&format!("avatar-gradient-{}", hash_clone % 8));
                        });
                    } else {
                        let label = Label::builder()
                            .label(&initials)
                            .css_classes(vec!["avatar-label".to_string()])
                            .build();
                        user_avatar.append(&label);
                    }
                    return;
                }
            }
            // Fallback
            while let Some(child) = user_avatar.first_child() {
                user_avatar.remove(&child);
            }
            let label = Label::builder()
                .label("YM")
                .css_classes(vec!["avatar-label".to_string()])
                .build();
            user_avatar.append(&label);
            user_name_label.set_label("Messenger");
            user_avatar.remove_css_class("avatar");
            for i in 0..8 {
                user_avatar.remove_css_class(&format!("avatar-gradient-{}", i));
            }
            user_avatar.add_css_class("avatar");
            user_avatar.add_css_class("avatar-gradient-0");
        });
    }

    // ── Search entry ──

    fn create_search_entry(model: &ChatListModel) -> Entry {
        let entry = Entry::builder()
            .placeholder_text("Поиск чатов...")
            .css_classes(vec!["search-bar".to_string()])
            .margin_start(16) // Token: 16
            .margin_end(16)   // Token: 16
            .margin_top(12)   // Token: 12
            .margin_bottom(12) // Token: 12
            .primary_icon_name("system-search-symbolic")
            .primary_icon_activatable(false)
            .secondary_icon_name("edit-clear-symbolic")
            .secondary_icon_sensitive(false)
            .secondary_icon_activatable(true)
            .build();

        let model_clone = model.clone();
        entry.connect_changed(move |e| {
            let text = e.text().to_string();
            if text.is_empty() {
                e.set_secondary_icon_name(None);
                e.set_secondary_icon_sensitive(false);
            } else {
                e.set_secondary_icon_name(Some("edit-clear-symbolic"));
                e.set_secondary_icon_sensitive(true);
            }
            model_clone.filter(&text);
            model_clone.update_chat_ids_model();
        });

        let model_clone2 = model.clone();
        entry.connect_icon_press(move |e, icon_pos| {
            if icon_pos == gtk::EntryIconPosition::Secondary {
                e.set_text("");
                e.grab_focus();
                model_clone2.filter("");
                model_clone2.update_chat_ids_model();
            }
        });

        // Hide clear icon initially
        entry.set_secondary_icon_name(None);

        entry
    }

    // ── Header ──

    fn create_header(
        auth: Arc<AuthManager>,
        logout_cb: &Arc<Mutex<Option<Box<dyn Fn()>>>>,
        settings_cb: &Arc<Mutex<Option<Box<dyn Fn()>>>>,
        switch_ac_cb: &Arc<Mutex<Option<Box<dyn Fn(String)>>>>,
        add_ac_cb: &Arc<Mutex<Option<Box<dyn Fn()>>>>,
        create_gp_cb: &Arc<Mutex<Option<Box<dyn Fn()>>>>,
    ) -> (GtkBox, GtkBox, Label, Label) {
        let header = GtkBox::new(Orientation::Horizontal, 8);
        header.set_margin_start(16); // Token: 16
        header.set_margin_end(16);   // Token: 16
        header.set_margin_top(16);   // Token: 16
        header.set_margin_bottom(8); // Token: 8

        let user_avatar = GtkBox::new(Orientation::Horizontal, 0);
        user_avatar.add_css_class("avatar");
        user_avatar.add_css_class("avatar-gradient-0");
        user_avatar.set_size_request(40, 40);
        user_avatar.set_valign(gtk::Align::Center);
        user_avatar.set_halign(gtk::Align::Center);
        let avatar_label = Label::builder()
            .label("YM")
            .css_classes(vec!["avatar-label".to_string()])
            .hexpand(true)
            .vexpand(true)
            .halign(gtk::Align::Center)
            .valign(gtk::Align::Center)
            .build();
        user_avatar.append(&avatar_label);

        let user_name = Label::builder()
            .label("Messenger")
            .xalign(0.0)
            .valign(gtk::Align::Center)
            .build();
        user_name.set_margin_start(12); // Token: 12
        user_name.add_css_class("title");

        header.append(&user_avatar);
        header.append(&user_name);

        let spacer = GtkBox::new(Orientation::Horizontal, 0);
        spacer.set_hexpand(true);
        header.append(&spacer);

        // Create Group button
        let create_gp_btn = gtk::Button::builder()
            .icon_name("list-add-symbolic")
            .css_classes(vec!["action-icon-btn".to_string()])
            .tooltip_text("Создать группу или канал")
            .valign(gtk::Align::Center)
            .build();
        create_gp_btn.set_margin_start(8); // Token: 8
        create_gp_btn.set_margin_end(4);
        header.append(&create_gp_btn);
        let create_gp_cb_clone = create_gp_cb.clone();
        create_gp_btn.connect_clicked(move |_btn| {
            if let Some(cb) = &*create_gp_cb_clone.lock().unwrap() {
                cb();
            }
        });

        // Settings button
        let settings_btn = gtk::Button::builder()
            .icon_name("open-menu-symbolic")
            .css_classes(vec!["action-icon-btn".to_string()])
            .valign(gtk::Align::Center)
            .build();
        settings_btn.set_margin_start(4);
        settings_btn.set_margin_end(8); // Token: 8
        header.append(&settings_btn);
        let settings_cb_clone = settings_cb.clone();
        settings_btn.connect_clicked(move |_btn| {
            if let Some(cb) = &*settings_cb_clone.lock().unwrap() {
                cb();
            }
        });

        // Account Switcher Dropdown Popover Click Gesture
        let dropdown = AccountDropdown::new(auth);
        let gesture = gtk::GestureClick::new();
        let dropdown_clone = dropdown.clone();
        let user_avatar_clone = user_avatar.clone();
        gesture.connect_released(move |_, _, _, _| {
            dropdown_clone.popup(&user_avatar_clone);
        });
        user_avatar.add_controller(gesture);

        let switch_ac_cb_clone = switch_ac_cb.clone();
        dropdown.connect_switch(move |id| {
            if let Some(cb) = &*switch_ac_cb_clone.lock().unwrap() {
                cb(id.to_string());
            }
        });

        let add_ac_cb_clone = add_ac_cb.clone();
        dropdown.connect_add_account(move || {
            if let Some(cb) = &*add_ac_cb_clone.lock().unwrap() {
                cb();
            }
        });

        let logout_cb_clone = logout_cb.clone();
        dropdown.connect_logout(move || {
            if let Some(cb) = &*logout_cb_clone.lock().unwrap() {
                cb();
            }
        });

        (header, user_avatar, avatar_label, user_name)
    }

    // ── Item factory ──

     fn create_item_factory(model: &ChatListModel, auth: Arc<AuthManager>) -> gtk::SignalListItemFactory {
         let model_clone = model.clone();
         let factory = gtk::SignalListItemFactory::new();
        let auth_clone = auth.clone();
        factory.connect_setup(move |_factory, list_item| {
            let list_item = list_item.downcast_ref::<gtk::ListItem>().unwrap();
            
            // Row: [Avatar] [Content]
            let row = GtkBox::new(Orientation::Horizontal, 10);
            row.set_css_classes(&["chat-row"]);
            row.set_margin_start(12);
            row.set_margin_end(8);
            row.set_margin_top(3);
            row.set_margin_bottom(3);
            row.set_valign(gtk::Align::Center);

            // Avatar wrapped in Overlay for precise online status dot
            let avatar_overlay = gtk::Overlay::new();
            let avatar = adw::Avatar::builder()
                .size(48)
                .build();
            avatar_overlay.set_child(Some(&avatar));

            let dot = GtkBox::new(Orientation::Horizontal, 0);
            dot.add_css_class("online-dot");
            dot.set_valign(gtk::Align::End);
            dot.set_halign(gtk::Align::End);
            dot.set_margin_bottom(2);
            dot.set_margin_end(2);
            dot.set_size_request(10, 10);
            dot.set_visible(false);
            avatar_overlay.add_overlay(&dot);

            row.append(&avatar_overlay);

            // Content: two rows
            let content = GtkBox::new(Orientation::Vertical, 2);
            content.set_hexpand(true);
            content.set_valign(gtk::Align::Center);

            // Top row: title ... pin ... time
            let top_row = GtkBox::new(Orientation::Horizontal, 4);
            top_row.set_hexpand(true);
            let title = Label::builder()
                .xalign(0.0)
                .ellipsize(gtk::pango::EllipsizeMode::End)
                .css_classes(["chat-title"])
                .hexpand(true)
                .build();
            let pin_indicator = Label::builder()
                .label("📌")
                .css_classes(["pinned-indicator"])
                .valign(gtk::Align::Center)
                .visible(false)
                .build();
            let time = Label::builder()
                .css_classes(["chat-time"])
                .halign(gtk::Align::End)
                .valign(gtk::Align::Center)
                .xalign(1.0)
                .build();
            top_row.append(&title);
            top_row.append(&pin_indicator);
            top_row.append(&time);
            content.append(&top_row);

            // Bottom row: preview ... unread
            let bottom_row = GtkBox::new(Orientation::Horizontal, 4);
            bottom_row.set_hexpand(true);
            let preview = Label::builder()
                .xalign(0.0)
                .ellipsize(gtk::pango::EllipsizeMode::End)
                .css_classes(["chat-preview"])
                .hexpand(true)
                .build();
            let unread = Label::builder()
                .css_classes(["unread-badge"])
                .halign(gtk::Align::End)
                .valign(gtk::Align::Center)
                .xalign(1.0)
                .visible(false)
                .build();
            bottom_row.append(&preview);
            bottom_row.append(&unread);
            content.append(&bottom_row);

            row.append(&content);
            list_item.set_child(Some(&row));
        });

        factory.connect_bind(move |_factory, list_item| {
            let list_item = list_item.downcast_ref::<gtk::ListItem>().unwrap();
            let pos = list_item.position();
            let item = model_clone.item(pos);

            let Some(row_widget) = list_item.child() else { return };
            let Ok(row) = row_widget.downcast::<gtk::Box>() else { return };

            // Collect direct children: [avatar_overlay, content_box]
            let mut children = Vec::new();
            let mut next = row.first_child();
            while let Some(w) = next {
                children.push(w.clone());
                next = w.next_sibling();
            }

            let avatar_widget = children.get(0);
            let content_widget = children.get(1);

            if let (Some(av), Some(chat_row)) = (avatar_widget, item.as_ref()) {
                if let Ok(overlay) = av.clone().downcast::<gtk::Overlay>() {
                    Self::update_avatar_overlay(&overlay, &chat_row.chat, auth_clone.clone());
                }
            }

            let Some(cont) = content_widget else { return };
            let Some(chat_row) = item.as_ref() else { return };

            // top_row children: [title_label, pin_indicator, time_label]
            if let Some(top_row) = cont.first_child() {
                if let Some(title_w) = top_row.first_child() {
                    if let Ok(title) = title_w.clone().downcast::<Label>() {
                        title.set_label(&chat_row.chat.display_name());
                    }
                    if let Some(pin_w) = title_w.next_sibling() {
                        if let Ok(pin) = pin_w.clone().downcast::<Label>() {
                            pin.set_visible(chat_row.chat.pinned);
                        }
                        if let Some(time_w) = pin_w.next_sibling() {
                            if let Ok(time) = time_w.downcast::<Label>() {
                                if let Some(last_msg) = &chat_row.chat.last_message {
                                    time.set_label(&format_message_time(&last_msg.created));
                                } else {
                                    time.set_label("");
                                }
                            }
                        }
                    }
                }

                // bottom_row children: [preview_label, unread_label]
                if let Some(bottom_row) = top_row.next_sibling() {
                    if let Some(preview_w) = bottom_row.first_child() {
                        if let Ok(preview) = preview_w.clone().downcast::<Label>() {
                            preview.set_label(&chat_row.chat.preview_text());
                        }
                        if let Some(unread_w) = preview_w.next_sibling() {
                            if let Ok(unread) = unread_w.downcast::<Label>() {
                                if chat_row.chat.unread_count > 0 {
                                    unread.set_label(&format_unread_count(chat_row.chat.unread_count));
                                    unread.set_visible(true);
                                } else {
                                    unread.set_visible(false);
                                }
                            }
                        }
                    }
                }
            }
        });

        factory
    }

    fn update_avatar_overlay(overlay: &gtk::Overlay, chat: &Chat, auth: Arc<AuthManager>) {
        if let Some(child_widget) = overlay.child() {
            if let Ok(avatar) = child_widget.downcast::<adw::Avatar>() {
                Self::update_avatar(&avatar, chat, auth);
            }
        }

        let is_online = chat.chat_type == crate::models::ChatType::Private
            && chat.participants.iter().any(|p| p.status == Some(crate::models::ParticipantStatus::Online));

        let mut next = overlay.first_child();
        while let Some(w) = next {
            if w.has_css_class("online-dot") {
                w.set_visible(is_online);
                break;
            }
            next = w.next_sibling();
        }
    }

    fn update_avatar(avatar: &adw::Avatar, chat: &Chat, auth: Arc<AuthManager>) {
        let display_name = chat.display_name();
        avatar.set_text(Some(&display_name));

        let avatar_id = chat.avatar_id.clone().unwrap_or_default();
        if !avatar_id.is_empty() {
            let cache = get_avatar_cache();
            let cached_texture = {
                let map = cache.lock().unwrap();
                map.get(&avatar_id).cloned()
            };
            if let Some(texture) = cached_texture {
                avatar.set_custom_image(Some(&texture));
                return;
            }

            // Cache miss: clear custom image to show initials first
            avatar.set_custom_image(None::<&gtk::gdk::Texture>);

            let avatar_clone = avatar.clone();
            let mut is_private_avatar = false;
            let avatar_url = if avatar_id.starts_with("chat_avatar/") {
                is_private_avatar = true;
                format!("https://files.messenger.yandex.ru/{}?size=small", avatar_id)
            } else {
                let normalized_id = avatar_id.replace("user_avatar/", "");
                format!("https://avatars.mds.yandex.net/get-{}/islands-middle", normalized_id)
            };
            
            // Keep track of the current chat ID on the avatar using its name property
            avatar.set_widget_name(&chat.id);
            let expected_chat_id = chat.id.clone();
            let avatar_id_clone = avatar_id.clone();

            glib::spawn_future_local(async move {
                let token_opt = if is_private_avatar {
                    if let Err(_) = auth.get_access_token() {
                        let _ = auth.refresh_if_needed().await;
                    }
                    auth.get_access_token().ok()
                } else {
                    None
                };

                if let Ok(bytes) = download_avatar_bytes(&avatar_url, token_opt.as_deref()).await {
                    let bytes_glib = glib::Bytes::from(&bytes);
                    if let Ok(texture) = gtk::gdk::Texture::from_bytes(&bytes_glib) {
                        // Put in static cache!
                        let cache = get_avatar_cache();
                        cache.lock().unwrap().insert(avatar_id_clone, texture.clone());

                        // Check if the avatar is still bound to the same chat
                        if avatar_clone.widget_name() == expected_chat_id {
                            avatar_clone.set_custom_image(Some(&texture));
                        }
                    }
                }
            });
        } else {
            avatar.set_custom_image(None::<&gtk::gdk::Texture>);
        }
    }

    // ── Keyboard controller ──

    fn create_key_controller(selection: &SingleSelection) -> gtk::EventControllerKey {
        let ctrl = gtk::EventControllerKey::new();
        let sel = selection.clone();

        ctrl.connect_key_pressed(move |_, keyval, _keycode, _state| {
            match keyval {
                gtk::gdk::Key::Down => {
                    let n = sel.n_items();
                    let cur = sel.selected();
                    if cur < n {
                        sel.select_item(cur + 1, false);
                    }
                    glib::Propagation::Stop
                }
                gtk::gdk::Key::Up => {
                    let cur = sel.selected();
                    if cur > 0 {
                        sel.select_item(cur - 1, false);
                    }
                    glib::Propagation::Stop
                }
                 gtk::gdk::Key::Return | gtk::gdk::Key::KP_Enter => {
                                        sel.select_item(sel.selected(), false);
                    glib::Propagation::Stop
                }
                gtk::gdk::Key::space => {
                    log::info!("Space preview: row {}", sel.selected());
                    glib::Propagation::Stop
                }
                _ => glib::Propagation::Proceed,
            }
        });

        ctrl
    }

    // ── Context menu ──

    fn create_context_gesture(selection: &SingleSelection, chats: &Arc<Mutex<Vec<Chat>>>) -> gtk::GestureClick {
        let gesture = gtk::GestureClick::new();
        gesture.set_button(3);
        let sel_clone = selection.clone();
        let chats_clone = Arc::clone(chats);

        gesture.connect_pressed(move |gesture, _, _, _y| {
            let idx = sel_clone.selected();
            if let Some(chat) = chats_clone.lock().unwrap().get(idx as usize).cloned() {
                Self::show_context_menu(chat);
            }
            gesture.set_state(gtk::EventSequenceState::Claimed);
        });

        gesture
    }

    fn show_context_menu(chat: Chat) {
        let menu = GtkBox::new(Orientation::Vertical, 4);
        menu.add_css_class("chat-context-menu");

        let popover = Popover::builder()
            .has_arrow(false)
            .build();

        let actions: Vec<(&str, &str)> = vec![
            ("Отметить как прочитанное", "mark_read"),
            ("Без звука", "mute"),
            ("Закрепить", "pin"),
            ("Архивировать", "archive"),
        ];

        for (label, action_name) in &actions {
            let btn = Button::with_label(label);
            let chat = chat.clone();
            let action = action_name.to_string();
            let pop = popover.clone();
            btn.connect_clicked(move |_| {
                log::info!("Chat action: {} → {}", chat.id, action);
                pop.popdown();
            });
            menu.append(&btn);
        }

        menu.append(&gtk::Separator::new(Orientation::Horizontal));

        let delete = Button::with_label("Удалить чат");
        delete.add_css_class("danger");
        let chat2 = chat.clone();
        let pop2 = popover.clone();
        delete.connect_clicked(move |_| {
            log::info!("Delete chat: {}", chat2.id);
            pop2.popdown();
        });
        menu.append(&delete);

        popover.set_child(Some(&menu));
        popover.set_position(gtk::PositionType::Bottom);
        popover.popup();
    }

    fn apply_css() {
        let provider = CssProvider::new();
        let css = r#"
            .avatar-label {
                font-weight: 700;
                font-size: 14px;
                color: #ffffff;
            }
            .chat-type-icon {
                font-size: 14px;
                color: #6b7280;
                padding: 0 4px;
            }
            .online-dot {
                min-width: 10px;
                min-height: 10px;
                background: #22c55e;
                border: 2px solid #ffffff;
                border-radius: 50%;
                box-shadow: 0 0 0 1px rgba(34, 197, 94, 0.2);
            }
            .away-dot {
                min-width: 10px;
                min-height: 10px;
                background: #f59e0b;
                border: 2px solid #ffffff;
                border-radius: 50%;
            }
            .offline-dot {
                min-width: 10px;
                min-height: 10px;
                background: #9ca3af;
                border: 2px solid #ffffff;
                border-radius: 50%;
            }
            .pinned-indicator {
                font-size: 11px;
                color: #6b7280;
                padding: 0 4px;
            }
            .skeleton {
                background: linear-gradient(90deg, #e8ebef 25%, #f0f2f5 50%, #e8ebef 75%);
                background-size: 200% 100%;
                animation: skeleton-loading 1.5s ease-in-out infinite;
                border-radius: 8px;
            }
            .skeleton-row {
                min-height: 60px;
                margin: 4px 6px;
            }
            .skeleton-title {
                min-height: 14px;
                min-width: 60px;
                margin-top: 8px;
                border-radius: 4px;
            }
            .skeleton-preview {
                min-height: 10px;
                min-width: 80px;
                margin-top: 6px;
                border-radius: 4px;
            }
            @keyframes skeleton-loading {
                0% { background-position: 200% 0; }
                100% { background-position: -200% 0; }
            }

            /* ── ListView row hover ── */
            .chat-row:hover {
                background-color: @bg_hover;
            }
            .chat-row:active {
                background-color: @bg_active;
            }
            .chat-row:selected {
                background-color: @bg_selected;
            }

            /* ── Empty state ── */
            .empty-state {
                padding: 24px;
            }
            .empty-state-icon {
                color: @text_tertiary;
            }

            /* ── Bot chat styles ── */
            .bot-avatar {
                background: linear-gradient(135deg, #6366F1, #8B5CF6);
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

    pub fn load_chats(&mut self, _auth: &Arc<AuthManager>) {
        let chats_result: Result<Vec<Chat>, String> = Ok(Vec::new());
        match chats_result {
            Ok(chats) => self.set_chats(chats),
            Err(_) => self.set_chats(Vec::new()),
        }
    }

    pub fn set_chats(&mut self, mut chats: Vec<Chat>) {
        // Sort chats: pinned first, then by last message timestamp / update time descending
        chats.sort_by(|a, b| {
            if a.pinned && !b.pinned {
                return std::cmp::Ordering::Less;
            }
            if !a.pinned && b.pinned {
                return std::cmp::Ordering::Greater;
            }
            let a_time = a.last_message.as_ref()
                .map(|m| m.created)
                .or(a.updated)
                .or(a.created)
                .unwrap_or_else(|| chrono::DateTime::<chrono::Utc>::MIN_UTC);
            let b_time = b.last_message.as_ref()
                .map(|m| m.created)
                .or(b.updated)
                .or(b.created)
                .unwrap_or_else(|| chrono::DateTime::<chrono::Utc>::MIN_UTC);
            b_time.cmp(&a_time)
        });

        let count = chats.len();
        *self.model.chats.lock().unwrap() = chats;
        let all_indices: Vec<usize> = (0..count).collect();
        *self.model.visible.lock().unwrap() = all_indices;
        self.model.set_selection(None);

        // Update the GtkGioListStore model so the ListView knows about the items.
        self.model.update_chat_ids_model();
    }

    pub fn update_unread(&mut self, chat_id: &str, unread_count: u32) {
        let mut chats = self.model.chats.lock().unwrap();
        if let Some(chat) = chats.iter_mut().find(|c| c.id == chat_id) {
            chat.unread_count = unread_count;
        }
        
        // Re-sort chats
        chats.sort_by(|a, b| {
            if a.pinned && !b.pinned {
                return std::cmp::Ordering::Less;
            }
            if !a.pinned && b.pinned {
                return std::cmp::Ordering::Greater;
            }
            let a_time = a.last_message.as_ref()
                .map(|m| m.created)
                .or(a.updated)
                .or(a.created)
                .unwrap_or_else(|| chrono::DateTime::<chrono::Utc>::MIN_UTC);
            let b_time = b.last_message.as_ref()
                .map(|m| m.created)
                .or(b.updated)
                .or(b.created)
                .unwrap_or_else(|| chrono::DateTime::<chrono::Utc>::MIN_UTC);
            b_time.cmp(&a_time)
        });

        let count = chats.len();
        drop(chats);
        let all_indices: Vec<usize> = (0..count).collect();
        *self.model.visible.lock().unwrap() = all_indices;
        self.model.set_selection(None);
        self.model.update_chat_ids_model();
    }

    pub fn select_chat(&self, chat_id: &str) {
        let chats = self.model.chats.lock().unwrap();
        let visible = self.model.visible.lock().unwrap();
        if let Some(pos) = visible.iter().position(|&orig_idx| {
            if let Some(chat) = chats.get(orig_idx) {
                chat.id == chat_id
            } else {
                false
            }
        }) {
            self.selection.select_item(pos as u32, false);
        }
    }

    pub fn connect_chat_selected<F: Fn(Chat) + 'static>(&self, callback: F) {
        let chats = Arc::clone(&self.model.chats);
        let visible = Arc::clone(&self.model.visible);
        
        self.selection.connect_notify_local(
            Some("selected"),
            move |obj, _| {
                let selection = obj.downcast_ref::<gtk::SingleSelection>().unwrap();
                let idx = selection.selected();
                eprintln!("[CHATLIST] Selection changed to index {}", idx);
                if idx < visible.lock().unwrap().len() as u32 {
                    let orig_idx = visible.lock().unwrap()[idx as usize];
                    if let Some(chat) = chats.lock().unwrap().get(orig_idx).cloned() {
                        eprintln!("[CHATLIST] Selected chat: {} ({})", chat.display_name(), chat.id);
                        callback(chat);
                    }
                }
            },
        );
    }

    pub fn container(&self) -> &GtkBox {
        &self.container
    }

    pub fn chats(&self) -> &Arc<Mutex<Vec<Chat>>> {
        &self.model.chats
    }
}

// ─────────────────────────────────────────────
//  Helpers
// ─────────────────────────────────────────────

pub fn format_timestamp_short(dt: &chrono::DateTime<chrono::Utc>) -> String {
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

pub fn format_message_time(dt: &chrono::DateTime<chrono::Utc>) -> String {
    let now = chrono::Utc::now();
    if now.naive_utc().date() == dt.naive_utc().date() {
        dt.format("%H:%M").to_string()
    } else {
        dt.format("%d.%m %H:%M").to_string()
    }
}

fn format_unread_count(count: u32) -> String {
    if count > 99 {
        "99+".to_string()
    } else {
        count.to_string()
    }
}

 fn hash_color(s: &str) -> usize {
    let mut hash: u64 = 5381;
    for byte in s.bytes() {
        hash = hash.wrapping_shl(5).wrapping_add(hash).wrapping_add(byte as u64);
    }
    hash as usize
}

/// Asynchronously load an avatar image from a URL and set it on the avatar widget.
/// Falls back to the initials display if the image fails to load.
async fn download_avatar_bytes(url: &str, token: Option<&str>) -> Result<bytes::Bytes, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .map_err(|e| format!("Failed to create client: {}", e))?;
    
    let mut req = client.get(url);
    if let Some(t) = token {
        req = req.header("Authorization", format!("OAuth {}", t));
    }
    
    // Load session cookies for private files/avatars if the url is on yandex files domain
    if url.contains("files.messenger.yandex.") {
        if let Some(config_dir) = dirs::config_dir() {
            let session_file = config_dir.join("yandex-messenger-native").join("session.json");
            if session_file.exists() {
                if let Ok(content) = std::fs::read_to_string(&session_file) {
                    if let Ok(data) = serde_json::from_str::<serde_json::Value>(&content) {
                        if let Some(cookies_map) = data.get("cookies").and_then(|v| v.as_object()) {
                            let mut cookie_str = String::new();
                            for (k, v) in cookies_map {
                                if let Some(val_str) = v.as_str() {
                                    if !cookie_str.is_empty() {
                                        cookie_str.push_str("; ");
                                    }
                                    cookie_str.push_str(&format!("{}={}", k, val_str));
                                }
                            }
                            if !cookie_str.is_empty() {
                                req = req.header("Cookie", cookie_str);
                            }
                        }
                    }
                }
            }
        }
    }
    
    let response = req
        .header("User-Agent", "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
        .send()
        .await
        .map_err(|e| format!("Fetch failed: {}", e))?;
    
    if !response.status().is_success() {
        return Err(format!("HTTP error {}", response.status()));
    }
    
    let bytes = response.bytes().await.map_err(|e| format!("Read bytes failed: {}", e))?;
    Ok(bytes)
}
