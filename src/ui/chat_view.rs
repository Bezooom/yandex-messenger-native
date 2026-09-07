#![allow(dead_code)]

use gtk::prelude::*;
use gtk::{
    Align, Box as GtkBox, Button, Calendar, Entry, Label, Orientation, Popover, ScrolledWindow,
    TextView,
};

use std::boxed::Box as StdBox;
use std::sync::{Arc, Mutex};

use crate::api::auth::AuthManager;
use crate::core::voice_recorder::VoiceRecorder;
use crate::models::scheduled_message::MessageSchedule;
use crate::models::{Chat, ExtendedReactionsConfig, Message, MessageType, Poll, Reaction};
use crate::ui::bot_panel::BotPanel;
use crate::ui::emoji_picker::EmojiPicker;
use crate::ui::reaction_panel::ReactionPanel;

/// Which composer popover should stay open when switching tools.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ComposerPopover {
    Emoji,
    Sticker,
    Attach,
    Poll,
    Schedule,
}
use crate::ui::poll_creator::PollCreator;
use crate::ui::poll_renderer::PollRenderer;
use crate::ui::scheduled_panel::{ScheduledPanel, SendAtPopover};
use crate::ui::sticker_panel::StickerPanel;
use serde_json;

/// Chat view — displays messages for a selected chat
pub struct ChatView {
    container: GtkBox,
    scrolled: ScrolledWindow,
    chat: Mutex<Option<Chat>>,
    messages: Mutex<Vec<Message>>,
    messages_store: gtk::gio::ListStore,
    message_list_view: gtk::ListView,
    /// Stack: list | empty | skeleton | welcome
    content_stack: gtk::Stack,
    empty_messages_box: GtkBox,
    welcome_box: GtkBox,
    skeleton_messages: GtkBox,
    title_label: Label,
    status_label: Label,
    search_btn: Button,
    search_entry: Entry,
    search_query: Mutex<String>,
    /// Whole composer bar (must be shown/hidden as a unit)
    input_area: GtkBox,
    input_entry: TextView,
    message_rows: Mutex<std::collections::HashMap<(String, bool), gtk::Box>>,
    send_btn: Button,
    voice_btn: Button,
    attach_btn: Button,
    call_btn: Button,
    menu_btn: Button,
    /// Shared AuthManager — used for user_id lookups and avatar URLs
    auth: Arc<AuthManager>,
    /// (chat_id, text, reply_to_msg_id, edit_msg_id)
    on_send: Arc<Mutex<Option<StdBox<dyn Fn(String, String, Option<String>, Option<String>)>>>>,
    on_attach: Arc<Mutex<Option<StdBox<dyn Fn(String, Vec<u8>, String)>>>>,
    on_call: Arc<Mutex<Option<StdBox<dyn Fn(String)>>>>,
    on_thread_open: Arc<Mutex<Option<StdBox<dyn Fn(String, String)>>>>,
    on_voice_send: Arc<Mutex<Option<StdBox<dyn Fn(String, Vec<u8>, f64, Vec<f32>)>>>>,
    on_translate: Arc<Mutex<Option<StdBox<dyn Fn(String, String)>>>>,
    on_image_open: Arc<Mutex<Option<StdBox<dyn Fn(String, String, Vec<(String, String)>)>>>>,
    /// (file_id, url, filename, open_after)
    on_file_download: Arc<Mutex<Option<StdBox<dyn Fn(String, String, String, bool)>>>>,
    on_typing: Arc<Mutex<Option<StdBox<dyn Fn(String)>>>>,
    last_typing_time: Arc<Mutex<i64>>,
    current_thread_view: Mutex<Option<Arc<crate::ui::ThreadView>>>,
    reaction_popover: Mutex<Option<Popover>>,
    reactions_config: Mutex<Option<ExtendedReactionsConfig>>,
    on_reaction_toggle: Arc<Mutex<Option<StdBox<dyn Fn(String, String, bool)>>>>,
    reply_preview_box: GtkBox,
    reply_preview_label: Label,
    reply_preview_close_btn: Button,
    reply_to_msg_id: Arc<Mutex<Option<String>>>,
    edit_msg_id: Arc<Mutex<Option<String>>>,
    /// Button to open poll creator
    poll_btn: Button,
    /// Poll creator popover/overlay
    poll_popover: Mutex<Option<Popover>>,
    /// Active poll renderers (poll_id -> PollRenderer)
    poll_renderers: Mutex<Vec<(String, Arc<PollRenderer>)>>,
    // Emoji picker state
    emoji_btn: Button,
    emoji_popover: Mutex<Option<Popover>>,
    // Sticker panel state
    sticker_btn: Button,
    sticker_panel: Mutex<Option<StickerPanel>>,
    sticker_packs: Mutex<Vec<crate::models::StickerPack>>,
    /// Reused attach menu popover (avoid creating nested shells each click)
    attach_menu_popover: Mutex<Option<Popover>>,
    // Voice recording state
    voice_recorder: Mutex<Option<Arc<VoiceRecorder>>>,
    voice_recording: Mutex<bool>,
    /// Timer handle for long-press detection (500ms threshold)
    long_press_timer: Mutex<Option<glib::SourceId>>,
    /// Whether the button is currently in recording mode
    in_recording_mode: Mutex<bool>,
    /// Callback for saving a message to favorites
    on_save: Arc<Mutex<Option<Box<dyn Fn(String, String, Option<String>) + Send>>>>,
    /// Server-side delete after the undo window: (chat_id, message_id)
    on_delete: Arc<Mutex<Option<StdBox<dyn Fn(String, String)>>>>,
    undo_bar: GtkBox,
    undo_label: Label,
    undo_btn: Button,
    pending_delete_msg_id: Arc<Mutex<Option<String>>>,
    pending_delete_row: Arc<Mutex<Option<GtkBox>>>,
    pinned_box: GtkBox,
    pinned_label: Label,
    pinned_message_id: Arc<Mutex<Option<String>>>,
    /// Bot panel (shown when a bot chat is selected)
    bot_panel: Mutex<Option<BotPanel>>,
    /// Bot info for current chat
    bot_info: Mutex<Option<crate::models::BotInfo>>,
    /// Callback for inline button clicks
    on_inline_click: std::sync::Arc<std::sync::Mutex<Option<Box<dyn Fn(String, String) + Send>>>>,
    /// Callback for keyboard button clicks
    on_keyboard_click: std::sync::Arc<std::sync::Mutex<Option<Box<dyn Fn(String) + Send>>>>,
    /// Callback for command button clicks
    on_command_click: std::sync::Arc<std::sync::Mutex<Option<Box<dyn Fn(String, String) + Send>>>>,
    /// Reply keyboard area
    keyboard_area: Mutex<Option<GtkBox>>,
    /// Scheduled messages for current chat
    scheduled_messages: Mutex<Vec<crate::models::ScheduledMessage>>,
    /// Send-at popover
    send_at_popover: Mutex<Option<SendAtPopover>>,
    /// Whether send-at popover is open
    popover_open: Mutex<bool>,
    /// Button for scheduling (clock icon)
    schedule_btn: Button,
    /// Callback for scheduling a message
    on_schedule: Arc<Mutex<Option<StdBox<dyn Fn(String, String, chrono::DateTime<chrono::Utc>)>>>>,
    /// Callback for canceling a scheduled message
    on_cancel_schedule: Arc<Mutex<Option<StdBox<dyn Fn(String, String)>>>>,
    /// Load older history: (chat_id, oldest_message_id, server cursor)
    on_load_older: Arc<Mutex<Option<StdBox<dyn Fn(String, String, Option<String>)>>>>,
    /// Prevent concurrent pagination requests
    loading_older: Mutex<bool>,
    /// Whether more history may exist above
    has_more_history: Mutex<bool>,
    /// Server pagination cursor for the next older page (None = use oldest id)
    pagination_cursor: Mutex<Option<String>>,
    /// Top bar: spinner while loading older messages
    pagination_bar: GtkBox,
    /// Stick viewport to newest messages after open / append (async layout-safe).
    stick_to_bottom: Mutex<bool>,
}

impl ChatView {
    pub fn new(auth: Arc<AuthManager>) -> Arc<Self> {
        let container = GtkBox::new(Orientation::Vertical, 0);
        container.set_hexpand(true);
        container.set_vexpand(true);
        container.add_css_class("chat-view");

        // Header bar
        let (header, title_label, status_label, search_btn, call_btn, menu_btn) =
            Self::create_header();
        header.set_hexpand(true);
        header.set_vexpand(false);

        // Search bar (initially hidden)
        let search_entry = gtk::Entry::builder()
            .placeholder_text("Поиск по сообщениям...")
            .visible(false)
            .build();
        search_entry.set_margin_start(8);
        search_entry.set_margin_end(8);
        search_entry.set_margin_top(4);
        search_entry.set_margin_bottom(4);

        // Messages area — must NOT publish natural width to paned (causes sidebar squeeze)
        let scrolled = ScrolledWindow::builder()
            .hexpand(true)
            .vexpand(true)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .propagate_natural_width(false)
            .propagate_natural_height(false)
            .build();
        scrolled.add_css_class("chat-scrolled-window");
        scrolled.set_kinetic_scrolling(true);
        scrolled.set_overlay_scrolling(true);
        scrolled.set_min_content_width(0);

        let messages_store = gtk::gio::ListStore::new::<crate::ui::message_object::MessageObject>();
        let selection = gtk::NoSelection::new(Some(messages_store.clone()));
        let message_list_view = gtk::ListView::new(Some(selection), None::<gtk::ListItemFactory>);
        message_list_view.set_css_classes(&["messages-list"]);

        // TG history padding: moderate side margins, space above composer
        message_list_view.set_margin_start(12);
        message_list_view.set_margin_end(12);
        message_list_view.set_margin_top(6);
        message_list_view.set_margin_bottom(10);
        message_list_view.set_hexpand(true);
        message_list_view.set_vexpand(true);
        message_list_view.set_halign(gtk::Align::Fill);

        scrolled.set_child(Some(&message_list_view));

        // Welcome (no chat selected)
        let welcome_box = GtkBox::new(Orientation::Vertical, 12);
        welcome_box.add_css_class("empty-chat");
        welcome_box.add_css_class("welcome-state");
        welcome_box.set_vexpand(true);
        welcome_box.set_hexpand(true);
        welcome_box.set_valign(Align::Center);
        welcome_box.set_halign(Align::Center);
        let welcome_logo = Label::builder()
            .label("Я")
            .css_classes(vec!["empty-chat-logo".to_string(), "pop-in".to_string()])
            .build();
        let welcome_title = Label::builder()
            .label("Yandex Messenger")
            .css_classes(vec!["empty-chat-title".to_string()])
            .build();
        let welcome_sub = Label::builder()
            .label("Выберите чат слева, чтобы начать переписку")
            .justify(gtk::Justification::Center)
            .wrap(true)
            .css_classes(vec!["empty-chat-subtitle".to_string()])
            .build();
        welcome_box.append(&welcome_logo);
        welcome_box.append(&welcome_title);
        welcome_box.append(&welcome_sub);

        // Empty conversation
        let empty_messages_box = GtkBox::new(Orientation::Vertical, 10);
        empty_messages_box.add_css_class("empty-chat");
        empty_messages_box.add_css_class("empty-conversation");
        empty_messages_box.set_vexpand(true);
        empty_messages_box.set_hexpand(true);
        empty_messages_box.set_valign(Align::Center);
        empty_messages_box.set_halign(Align::Center);
        let empty_icon = gtk::Image::from_icon_name("mail-send-symbolic");
        empty_icon.set_pixel_size(48);
        empty_icon.add_css_class("empty-chat-icon");
        empty_icon.add_css_class("pop-in");
        let empty_title = Label::builder()
            .label("Пока нет сообщений")
            .css_classes(vec!["empty-chat-title".to_string()])
            .build();
        let empty_sub = Label::builder()
            .label("Напишите первое сообщение —\nистория появится здесь")
            .justify(gtk::Justification::Center)
            .wrap(true)
            .css_classes(vec!["empty-chat-subtitle".to_string()])
            .build();
        empty_messages_box.append(&empty_icon);
        empty_messages_box.append(&empty_title);
        empty_messages_box.append(&empty_sub);

        // Message skeleton
        let skeleton_messages = GtkBox::new(Orientation::Vertical, 10);
        skeleton_messages.add_css_class("skeleton-messages");
        skeleton_messages.set_vexpand(true);
        skeleton_messages.set_margin_start(24);
        skeleton_messages.set_margin_end(24);
        skeleton_messages.set_margin_top(24);
        for i in 0..6 {
            let row = GtkBox::new(Orientation::Horizontal, 0);
            row.set_hexpand(true);
            let bubble = gtk::Box::new(Orientation::Vertical, 6);
            bubble.add_css_class("skeleton");
            bubble.add_css_class("skeleton-bubble");
            if i % 2 == 0 {
                bubble.set_halign(Align::Start);
                bubble.set_size_request(180 + (i * 20) as i32, 48);
                row.set_margin_end(80);
            } else {
                bubble.set_halign(Align::End);
                bubble.set_size_request(140 + (i * 15) as i32, 40);
                row.set_margin_start(80);
            }
            row.append(&bubble);
            skeleton_messages.append(&row);
        }

        let content_stack = gtk::Stack::builder()
            .transition_type(gtk::StackTransitionType::Crossfade)
            .transition_duration(200)
            .vexpand(true)
            .hexpand(true)
            .build();
        // Critical: stack must shrink so composer is not pushed off the window
        content_stack.set_vexpand(true);
        content_stack.set_hexpand(true);
        content_stack.set_size_request(-1, 80);
        content_stack.add_named(&scrolled, Some("list"));
        content_stack.add_named(&welcome_box, Some("welcome"));
        content_stack.add_named(&empty_messages_box, Some("empty"));
        content_stack.add_named(&skeleton_messages, Some("skeleton"));
        content_stack.set_visible_child_name("welcome");
        // Scrolled history absorbs remaining height
        scrolled.set_vexpand(true);
        scrolled.set_propagate_natural_height(false);

        // Input area
        let (
            input,
            input_entry,
            send_btn,
            voice_btn,
            attach_btn,
            emoji_btn,
            sticker_btn,
            poll_btn,
            schedule_btn,
        ) = Self::create_input();

        let pinned_message_id = Arc::new(Mutex::new(None));
        let pinned_box = GtkBox::new(Orientation::Horizontal, 8);
        pinned_box.set_visible(false);
        pinned_box.set_hexpand(false);
        pinned_box.set_vexpand(false);
        pinned_box.add_css_class("pinned-message-bar");
        pinned_box.set_margin_start(8);
        pinned_box.set_margin_end(8);
        pinned_box.set_margin_top(4);
        pinned_box.set_margin_bottom(4);

        let pin_icon = gtk::Image::from_icon_name("view-pin-symbolic");
        let pinned_label = Label::builder()
            .halign(Align::Start)
            .hexpand(true)
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .build();
        let unpin_btn = Button::builder()
            .icon_name("window-close-symbolic")
            .css_classes(vec!["icon-btn".to_string()])
            .build();

        pinned_box.append(&pin_icon);
        pinned_box.append(&pinned_label);
        pinned_box.append(&unpin_btn);

        let pb_clone = pinned_box.clone();
        let pmid_clone = pinned_message_id.clone();
        unpin_btn.connect_clicked(move |_| {
            pb_clone.set_visible(false);
            *pmid_clone.lock().unwrap() = None;
        });

        let reply_to_msg_id = Arc::new(Mutex::new(None));
        let edit_msg_id = Arc::new(Mutex::new(None));

        let reply_preview_box = GtkBox::new(Orientation::Horizontal, 8);
        reply_preview_box.set_visible(false);
        reply_preview_box.set_hexpand(false);
        reply_preview_box.set_vexpand(false);
        reply_preview_box.add_css_class("reply-preview-box");
        let reply_preview_label = Label::builder()
            .halign(Align::Start)
            .hexpand(true)
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .build();
        let reply_preview_close_btn = Button::builder()
            .icon_name("window-close-symbolic")
            .css_classes(vec!["icon-btn".to_string()])
            .build();
        reply_preview_box.append(&reply_preview_label);
        reply_preview_box.append(&reply_preview_close_btn);

        let close_box = reply_preview_box.clone();
        let close_input = input_entry.clone();
        let close_reply_id = reply_to_msg_id.clone();
        let close_edit_id = edit_msg_id.clone();
        reply_preview_close_btn.connect_clicked(move |_| {
            close_box.set_visible(false);
            *close_reply_id.lock().unwrap() = None;
            if close_edit_id.lock().unwrap().is_some() {
                close_input.buffer().set_text("");
            }
            *close_edit_id.lock().unwrap() = None;
        });

        let undo_bar = GtkBox::new(Orientation::Horizontal, 8);
        undo_bar.set_hexpand(false);
        undo_bar.set_vexpand(false);
        undo_bar.set_visible(false);
        undo_bar.set_halign(Align::Center);
        undo_bar.set_margin_bottom(8);
        undo_bar.add_css_class("undo-bar");
        let undo_label = Label::new(Some("Сообщение удалено"));
        let undo_btn = Button::with_label("Отменить");
        undo_bar.append(&undo_label);
        undo_bar.append(&undo_btn);

        let pending_delete_msg_id: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
        let pending_delete_row: Arc<Mutex<Option<GtkBox>>> = Arc::new(Mutex::new(None));

        let undo_bar_clone = undo_bar.clone();
        let pending_msg_clone = pending_delete_msg_id.clone();
        let pending_row_clone = pending_delete_row.clone();

        undo_btn.connect_clicked(move |_| {
            undo_bar_clone.set_visible(false);
            if let Some(row) = pending_row_clone.lock().unwrap().as_ref() {
                row.set_visible(true);
            }
            *pending_msg_clone.lock().unwrap() = None;
            *pending_row_clone.lock().unwrap() = None;
        });

        // Pagination loading indicator (above message stack)
        let pagination_bar = GtkBox::new(Orientation::Horizontal, 8);
        pagination_bar.add_css_class("pagination-bar");
        pagination_bar.set_halign(Align::Center);
        pagination_bar.set_hexpand(true);
        pagination_bar.set_visible(false);
        pagination_bar.set_margin_top(6);
        pagination_bar.set_margin_bottom(2);
        let pag_spinner = gtk::Spinner::new();
        pag_spinner.set_spinning(true);
        pag_spinner.add_css_class("pagination-spinner");
        let pag_label = Label::builder()
            .label("Загрузка истории…")
            .css_classes(vec!["pagination-label".to_string()])
            .build();
        pagination_bar.append(&pag_spinner);
        pagination_bar.append(&pag_label);

        container.append(&header);
        container.append(&search_entry);
        container.append(&pinned_box);
        container.append(&pagination_bar);
        container.append(&content_stack);
        container.append(&reply_preview_box);
        container.append(&undo_bar);
        container.append(&input);

        // Hide composer on welcome
        input.set_visible(false);

        let view = Self {
            container,
            scrolled: scrolled.clone(),
            chat: Mutex::new(None),
            messages: Mutex::new(Vec::new()),
            messages_store,
            message_list_view,
            content_stack,
            empty_messages_box,
            welcome_box,
            skeleton_messages,
            title_label,
            status_label,
            search_btn,
            search_entry,
            search_query: Mutex::new(String::new()),
            input_area: input.clone(),
            input_entry,
            message_rows: Mutex::new(std::collections::HashMap::new()),
            send_btn,
            voice_btn,
            attach_btn,
            call_btn,
            menu_btn,
            auth,
            on_send: Arc::new(Mutex::new(None)),
            on_attach: Arc::new(Mutex::new(None)),
            on_call: Arc::new(Mutex::new(None)),
            on_thread_open: Arc::new(Mutex::new(None)),
            on_voice_send: Arc::new(Mutex::new(None)),
            on_translate: Arc::new(Mutex::new(None)),
            on_image_open: Arc::new(Mutex::new(None)),
            on_file_download: Arc::new(Mutex::new(None)),
            on_typing: Arc::new(Mutex::new(None)),
            last_typing_time: Arc::new(Mutex::new(0)),
            current_thread_view: Mutex::new(None),
            reaction_popover: Mutex::new(None),
            reactions_config: Mutex::new(None),
            on_reaction_toggle: Arc::new(Mutex::new(None)),
            reply_preview_box: reply_preview_box.clone(),
            reply_preview_label: reply_preview_label.clone(),
            reply_preview_close_btn: reply_preview_close_btn.clone(),
            reply_to_msg_id: reply_to_msg_id.clone(),
            edit_msg_id: edit_msg_id.clone(),
            poll_btn,
            poll_popover: Mutex::new(None),
            poll_renderers: Mutex::new(Vec::new()),
            emoji_btn,
            emoji_popover: Mutex::new(None),
            sticker_btn,
            sticker_panel: Mutex::new(None),
            sticker_packs: Mutex::new(Vec::new()),
            attach_menu_popover: Mutex::new(None),
            voice_recorder: Mutex::new(None),
            voice_recording: Mutex::new(false),
            long_press_timer: Mutex::new(None),
            in_recording_mode: Mutex::new(false),
            undo_bar,
            undo_label,
            undo_btn,
            pending_delete_msg_id,
            pending_delete_row,
            pinned_box,
            pinned_label,
            pinned_message_id,
            on_save: Arc::new(Mutex::new(None)),
            on_delete: Arc::new(Mutex::new(None)),
            bot_panel: Mutex::new(None),
            bot_info: Mutex::new(None),
            scheduled_messages: Mutex::new(Vec::new()),
            send_at_popover: Mutex::new(None),
            popover_open: Mutex::new(false),
            schedule_btn,
            on_schedule: Arc::new(Mutex::new(None)),
            on_cancel_schedule: Arc::new(Mutex::new(None)),
            on_inline_click: std::sync::Arc::new(std::sync::Mutex::new(None)),
            on_keyboard_click: std::sync::Arc::new(std::sync::Mutex::new(None)),
            on_command_click: std::sync::Arc::new(std::sync::Mutex::new(None)),
            keyboard_area: Mutex::new(None),
            on_load_older: Arc::new(Mutex::new(None)),
            loading_older: Mutex::new(false),
            has_more_history: Mutex::new(true),
            pagination_cursor: Mutex::new(None),
            pagination_bar,
            stick_to_bottom: Mutex::new(false),
        };

        let this = Arc::new(view);
        this.setup_history_scroll();
        this.setup_stick_to_bottom();
        this.setup_file_drop_and_paste();
        this
    }

    /// Инициализирует poll creator popover (вызывать после создания ChatView)
    /// Этот метод должен вызываться когда ChatView находится внутри Arc, например:
    /// `Arc::new(ChatView::new()).init_poll_creator()`

    pub fn init_callbacks(self: &Arc<Self>) {
        let cv_send = self.clone();
        self.send_btn.connect_clicked(move |_| {
            cv_send.handle_send();
        });

        let cv_schedule = self.clone();
        self.schedule_btn.connect_clicked(move |_| {
            cv_schedule.show_send_at_popover();
        });

        // Capture phase so TextView does not eat Enter / Ctrl+V first
        let key_controller = gtk::EventControllerKey::new();
        key_controller.set_propagation_phase(gtk::PropagationPhase::Capture);
        let cv_key = self.clone();

        key_controller.connect_key_pressed(move |_controller, keyval, _keycode, state| {
            let is_shift = state.contains(gtk::gdk::ModifierType::SHIFT_MASK);
            let is_ctrl = state.contains(gtk::gdk::ModifierType::CONTROL_MASK)
                || state.contains(gtk::gdk::ModifierType::CONTROL_MASK);
            // Some layouts report Super+V; primary modifier mask covers Ctrl on most desktops
            let is_primary = state.contains(gtk::gdk::ModifierType::CONTROL_MASK);

            if (keyval == gtk::gdk::Key::Return || keyval == gtk::gdk::Key::KP_Enter) && !is_shift {
                cv_key.handle_send();
                return glib::Propagation::Stop;
            }
            if is_primary && (keyval == gtk::gdk::Key::v || keyval == gtk::gdk::Key::V) {
                // Prefer image paste; if no image, let TextView handle text paste
                if cv_key.try_paste_clipboard_image() {
                    return glib::Propagation::Stop;
                }
                return glib::Propagation::Proceed;
            }
            if keyval == gtk::gdk::Key::Up {
                let is_empty = {
                    let buffer = cv_key.input_entry.buffer();
                    let (start, end) = buffer.bounds();
                    buffer.text(&start, &end, false).trim().is_empty()
                };
                if is_empty {
                    cv_key.edit_last_message();
                    return glib::Propagation::Stop;
                }
            }
            let _ = is_ctrl;
            glib::Propagation::Proceed
        });
        self.input_entry.add_controller(key_controller);

        let cv_attach = self.clone();
        self.attach_btn.connect_clicked(move |_| {
            cv_attach.show_attach_menu();
        });

        let cv_menu = self.clone();
        self.menu_btn.connect_clicked(move |_| {
            cv_menu.show_header_menu();
        });

        let search_entry_clone = self.search_entry.clone();
        self.search_btn.connect_clicked(move |_| {
            let visible = gtk::prelude::WidgetExt::is_visible(&search_entry_clone);
            search_entry_clone.set_visible(!visible);
            if !visible {
                search_entry_clone.grab_focus();
            } else {
                search_entry_clone.set_text("");
            }
        });

        let cv_search = self.clone();
        self.search_entry.connect_changed(move |entry| {
            let query = entry.text().to_string();
            *cv_search.search_query.lock().unwrap() = query;
            cv_search.render_messages();
        });

        let cv_typing = self.clone();
        let send_btn_clone = self.send_btn.clone();
        let voice_btn_clone = self.voice_btn.clone();

        // Initial state
        send_btn_clone.set_visible(false);
        voice_btn_clone.set_visible(crate::config::ym_enable_voice());

        self.input_entry.buffer().connect_changed(move |buffer| {
            let (start, end) = buffer.bounds();
            let text = buffer.text(&start, &end, false);
            let is_empty = text.trim().is_empty();

            send_btn_clone.set_visible(!is_empty);
            voice_btn_clone.set_visible(is_empty && crate::config::ym_enable_voice());

            let now = chrono::Utc::now().timestamp_millis();
            let mut last_time = cv_typing.last_typing_time.lock().unwrap();
            if now - *last_time > 3000 {
                *last_time = now;
                if let Some(ref cb) = cv_typing.on_typing.lock().unwrap().as_ref() {
                    if let Some(chat) = cv_typing.chat.lock().unwrap().as_ref() {
                        cb(chat.id.clone());
                    }
                }
            }
        });

        let cv_call = self.clone();
        self.call_btn.connect_clicked(move |_| {
            if let Some(chat) = cv_call.chat.lock().unwrap().as_ref() {
                if let Some(ref cb) = *cv_call.on_call.lock().unwrap() {
                    cb(chat.id.clone());
                }
            }
        });

        // Voice recording logic
        let cv_voice = self.clone();
        let voice_btn_clone2 = self.voice_btn.clone();
        self.voice_btn.connect_clicked(move |_| {
            let mut recording = cv_voice.voice_recording.lock().unwrap();
            let mut recorder = cv_voice.voice_recorder.lock().unwrap();

            if recorder.is_none() {
                *recorder = Some(Arc::new(VoiceRecorder::new()));
            }

            if !*recording {
                // Start recording
                if let Some(rec) = recorder.as_ref() {
                    match rec.start() {
                        Ok(_) => {
                            *recording = true;
                            voice_btn_clone2.set_icon_name("media-record-symbolic");
                            voice_btn_clone2.add_css_class("recording-active");
                            log::info!("Started voice recording");

                            // Drain encoder + waveform tap like a meter
                            // (works with a real mic; silent without gstreamer).
                            let rec_clone = rec.clone();
                            glib::timeout_add_local(
                                std::time::Duration::from_millis(100),
                                move || {
                                    if rec_clone.is_recording() {
                                        rec_clone.pump();
                                        glib::ControlFlow::Continue
                                    } else {
                                        glib::ControlFlow::Break
                                    }
                                },
                            );
                        }
                        Err(e) => {
                            log::error!("Failed to start recording: {}", e);
                            cv_voice.show_error(&format!("Ошибка записи: {}", e));
                        }
                    }
                }
            } else {
                // Stop recording and send
                if let Some(rec) = recorder.as_ref() {
                    match rec.stop() {
                        Ok(data) => {
                            *recording = false;
                            voice_btn_clone2.set_icon_name("audio-input-microphone-symbolic");
                            voice_btn_clone2.remove_css_class("recording-active");

                            let duration = rec.duration();
                            let waveform = rec.waveform();

                            if let Some(chat) = cv_voice.chat.lock().unwrap().as_ref() {
                                if let Some(cb) = cv_voice.on_voice_send.lock().unwrap().as_ref() {
                                    cb(chat.id.clone(), data, duration, waveform);
                                }
                            }
                        }
                        Err(e) => {
                            log::error!("Failed to stop recording: {}", e);
                            cv_voice.show_error(&format!("Ошибка записи: {}", e));
                        }
                    }
                }
            }
        });
    }

    pub fn init_poll_creator(self: &Arc<Self>) {
        // Skip if already initialized
        if self.poll_popover.lock().unwrap().is_some() {
            return;
        }

        let poll_creator = PollCreator::new();
        let popover = Popover::builder().has_arrow(false).autohide(true).build();
        popover.add_css_class("poll-creator-popover");
        popover.set_child(Some(poll_creator.container()));
        popover.set_parent(&self.attach_btn);
        popover.set_position(gtk::PositionType::Top);

        // Обработка создания опроса — захватываем только необходимые данные
        let on_send = self.on_send.clone();
        let chat_id_opt = {
            let chat = self.chat.lock().unwrap();
            chat.as_ref().map(|c| c.id.clone())
        };
        let pop_clone1 = popover.clone();
        poll_creator.on_submit(move |poll| {
            log::info!(
                "Poll created: {} with {} answers",
                poll.question,
                poll.answers.len()
            );
            if let Some(chat_id) = &chat_id_opt {
                if let Some(ref cb) = on_send.lock().unwrap().as_ref() {
                    if let Ok(json) = serde_json::to_string(&poll) {
                        cb(chat_id.clone(), json, None, None);
                    }
                }
            }
            pop_clone1.popdown();
        });

        let pop_clone2 = popover.clone();
        poll_creator.on_cancel(move || {
            pop_clone2.popdown();
        });

        *self.poll_popover.lock().unwrap() = Some(popover.clone());

        let poll_btn = self.poll_btn.clone();
        let _poll_btn_clone = poll_btn.clone();
        let cv_poll = self.clone();
        poll_btn.connect_clicked(move |_| {
            if let Some(pop) = cv_poll.poll_popover.lock().unwrap().as_ref() {
                pop.popup();
            }
        });
    }

    /// Рендерит сообщение-опрос
    fn render_poll_message(&self, msg: &Message, poll: &Poll, parent_box: &GtkBox) {
        let renderer = Arc::new(PollRenderer::new(poll.clone(), msg.chat_id.clone()));
        let container = renderer.container().clone();

        let on_send = self.on_send.clone();
        let chat_id_opt = self.chat.lock().unwrap().as_ref().map(|c| c.id.clone());
        let poll_id = poll.poll_id.clone();
        renderer.on_vote(move |_poll_id, answer_ids| {
            log::info!("Vote on poll {}: {:?}", poll_id, answer_ids);
            if let (Some(cb), Some(chat_id)) =
                (on_send.lock().unwrap().as_ref(), chat_id_opt.as_ref())
            {
                let vote_data = serde_json::json!({
                    "poll_id": poll_id,
                    "answer_ids": answer_ids,
                });
                if let Ok(json) = serde_json::to_string(&vote_data) {
                    cb(chat_id.clone(), json, None, None);
                }
            }
        });

        self.poll_renderers
            .lock()
            .unwrap()
            .push((msg.id.clone(), renderer.clone()));

        parent_box.append(&container);
    }

    /// Добавляет сообщение в чат
    pub fn add_message(&self, msg: Message) {
        if let Some(current_chat_id) = self.current_chat_id() {
            if msg.chat_id != current_chat_id {
                return;
            }
        }

        let mut messages = self.messages.lock().unwrap();
        if let Some(existing) = messages
            .iter_mut()
            .find(|m| m.id == msg.id || (m.message_id.is_some() && m.message_id == msg.message_id))
        {
            let mut need_rerender = false;
            if !msg.reactions.is_empty() {
                existing.reactions = msg.reactions.clone();
                need_rerender = true;
            }
            // Edited text arriving under the same payload id (edit echo).
            if let Some(ref t) = msg.text {
                if !t.trim().is_empty()
                    && existing.text.as_ref() != Some(t)
                    && existing.type_ == msg.type_
                {
                    existing.text = Some(t.clone());
                    existing.edited = true;
                    existing.edited_at = Some(chrono::Utc::now());
                    need_rerender = true;
                }
            }
            if msg.read && !existing.read {
                existing.read = true;
                existing.delivered = true;
                need_rerender = true;
            }
            if msg.delivered && !existing.delivered {
                existing.delivered = true;
                need_rerender = true;
            }
            if msg.sent && !existing.sent {
                existing.sent = true;
                need_rerender = true;
            }
            if need_rerender {
                let snapshot = messages.clone();
                drop(messages);
                self.message_rows.lock().unwrap().clear();
                *self.messages.lock().unwrap() = snapshot;
                self.render_messages();
            }
            return;
        }

        messages.push(msg.clone());
        drop(messages);
        // Leaving empty conversation → show list
        if self.content_stack.visible_child_name().as_deref() == Some("empty")
            || self.content_stack.visible_child_name().as_deref() == Some("skeleton")
            || self.content_stack.visible_child_name().as_deref() == Some("welcome")
        {
            self.show_messages_list();
        }
        let obj = crate::ui::message_object::MessageObject::new(msg);
        self.messages_store.append(&obj);
        // New message → stick to bottom (latest messages visible)
        self.scroll_to_latest();
    }

    /// Update delivery/read ticks for one or many messages and re-render.
    pub fn apply_status_updates(&self, updates: &[(String, bool, bool)]) {
        if updates.is_empty() {
            return;
        }
        let mut messages = self.messages.lock().unwrap();
        let mut dirty = false;
        for (mid, delivered, read) in updates {
            for msg in messages.iter_mut() {
                let match_id = msg.id == *mid
                    || msg.message_id.as_deref() == Some(mid.as_str())
                    || msg.id.ends_with(&format!("_{}", mid));
                if !match_id {
                    continue;
                }
                if *delivered && !msg.delivered {
                    msg.delivered = true;
                    dirty = true;
                }
                if *read && !msg.read {
                    msg.read = true;
                    msg.delivered = true;
                    dirty = true;
                }
            }
        }
        if dirty {
            let snapshot = messages.clone();
            drop(messages);
            self.message_rows.lock().unwrap().clear();
            *self.messages.lock().unwrap() = snapshot;
            self.render_messages();
        }
    }

    /// Mark all currently shown outgoing messages as read (peer read the chat).
    pub fn mark_all_outgoing_read(&self) {
        let mut messages = self.messages.lock().unwrap();
        let mut dirty = false;
        for msg in messages.iter_mut() {
            if (msg.sent || msg.delivered) && !msg.read {
                msg.read = true;
                msg.delivered = true;
                dirty = true;
            }
        }
        if dirty {
            let snapshot = messages.clone();
            drop(messages);
            self.message_rows.lock().unwrap().clear();
            *self.messages.lock().unwrap() = snapshot;
            self.render_messages();
        }
    }

    pub fn set_reactions_config(&self, config: ExtendedReactionsConfig) {
        *self.reactions_config.lock().unwrap() = Some(config);
    }

    pub fn on_reaction_toggle(&self, callback: impl Fn(String, String, bool) + 'static) {
        *self.on_reaction_toggle.lock().unwrap() = Some(StdBox::new(callback));
    }

    pub fn update_message_reactions(&self, message_id: &str, reactions: Vec<Reaction>) {
        let mut messages = self.messages.lock().unwrap();
        let mut updated_id = None;
        for msg in messages.iter_mut() {
            if msg.id == message_id || msg.message_id.as_deref() == Some(message_id) {
                msg.reactions = reactions.clone();
                updated_id = Some(msg.id.clone());
                break;
            }
        }

        let Some(row_id) = updated_id else {
            return;
        };
        drop(messages);
        self.refresh_message_list_item(&row_id);
    }

    pub fn toggle_reaction_local(&self, message_id: &str, emoji: &str, add: bool) {
        let mut messages = self.messages.lock().unwrap();
        let mut updated_id = None;
        for msg in messages.iter_mut() {
            if msg.id != message_id && msg.message_id.as_deref() != Some(message_id) {
                continue;
            }

            if add {
                if let Some(reaction) = msg.reactions.iter_mut().find(|r| r.emoji == emoji) {
                    reaction.count = reaction.count.saturating_add(1);
                    reaction.selected = true;
                } else {
                    msg.reactions.push(Reaction {
                        emoji: emoji.to_string(),
                        count: 1,
                        selected: true,
                        user_ids: vec![],
                        is_extended: false,
                    });
                }
            } else if let Some(index) = msg.reactions.iter().position(|r| r.emoji == emoji) {
                let reaction = &mut msg.reactions[index];
                if reaction.count > 1 {
                    reaction.count -= 1;
                } else {
                    msg.reactions.remove(index);
                }
            }

            updated_id = Some(msg.id.clone());
            break;
        }

        let Some(row_id) = updated_id else {
            return;
        };
        drop(messages);
        self.refresh_message_list_item(&row_id);
    }

    fn refresh_message_list_item(&self, message_id: &str) {
        let messages = self.messages.lock().unwrap();
        let Some(position) = messages.iter().position(|m| m.id == message_id) else {
            return;
        };
        let msg = messages[position].clone();
        drop(messages);

        self.message_rows
            .lock()
            .unwrap()
            .retain(|(id, _), _| id != message_id);

        let obj = crate::ui::message_object::MessageObject::new(msg);
        self.messages_store.remove(position as u32);
        self.messages_store.insert(position as u32, &obj);
    }

    fn show_reaction_picker(self: &Arc<Self>, msg: &Message, target: &impl IsA<gtk::Widget>) {
        let message_id = msg.message_id.clone().unwrap_or_else(|| msg.id.clone());
        let panel = ReactionPanel::new(message_id.clone());
        panel.set_reactions(msg.reactions.clone());
        if let Some(config) = self.reactions_config.lock().unwrap().clone() {
            panel.set_config(config);
        }

        let view_add = self.clone();
        let view_remove = self.clone();
        panel.on_reaction_click(move |msg_id, emoji| {
            if let Some(cb) = view_add.on_reaction_toggle.lock().unwrap().as_ref() {
                cb(msg_id.clone(), emoji.clone(), true);
            }
            view_add.toggle_reaction_local(&msg_id, &emoji, true);
        });
        panel.on_remove_reaction(move |msg_id, emoji| {
            if let Some(cb) = view_remove.on_reaction_toggle.lock().unwrap().as_ref() {
                cb(msg_id.clone(), emoji.clone(), false);
            }
            view_remove.toggle_reaction_local(&msg_id, &emoji, false);
        });

        panel.show(target);
    }

    fn create_reaction_chips_row(
        self: &Arc<Self>,
        msg: &Message,
        is_sent: bool,
        picker_target: &impl IsA<gtk::Widget>,
    ) -> GtkBox {
        let row = GtkBox::new(Orientation::Horizontal, 4);
        row.add_css_class("message-reactions");
        row.set_halign(if is_sent { Align::End } else { Align::Start });

        let message_id = msg.message_id.clone().unwrap_or_else(|| msg.id.clone());

        for (i, reaction) in msg.reactions.iter().enumerate() {
            let label = if reaction.count > 1 {
                format!("{} {}", reaction.emoji, reaction.count)
            } else {
                reaction.emoji.clone()
            };
            let mut classes = vec!["reaction-chip".to_string(), "reaction-pop".to_string()];
            if reaction.selected {
                classes.push("selected".to_string());
            }
            // Stagger pop-in slightly
            classes.push(format!("reaction-delay-{}", i.min(5)));

            let chip = Button::builder().label(&label).css_classes(classes).build();
            let emoji = reaction.emoji.clone();
            let selected = reaction.selected;
            let msg_id = message_id.clone();
            let view = self.clone();
            chip.connect_clicked(move |_| {
                if let Some(cb) = view.on_reaction_toggle.lock().unwrap().as_ref() {
                    cb(msg_id.clone(), emoji.clone(), !selected);
                }
                view.toggle_reaction_local(&msg_id, &emoji, !selected);
            });
            row.append(&chip);
        }

        let add_btn = Button::builder()
            .label("+")
            .css_classes(vec![
                "reaction-chip".to_string(),
                "add".to_string(),
                "reaction-pop".to_string(),
            ])
            .build();
        let msg_clone = msg.clone();
        let view = self.clone();
        let target_widget = picker_target.clone();
        add_btn.connect_clicked(move |_| {
            view.show_reaction_picker(&msg_clone, &target_widget);
        });
        row.append(&add_btn);

        row
    }

    /// Обновляет опрос по ID (например, после получения обновления от сервера)
    pub fn update_poll(&self, poll_id: &str, poll: Poll) {
        for (_, renderer) in self.poll_renderers.lock().unwrap().iter_mut() {
            if renderer.poll_id() == poll_id {
                renderer.update_poll(poll);
                return;
            }
        }
    }

    fn apply_entities(text: &str, entities: &[crate::models::MessageEntity]) -> String {
        let mut result = text.to_string();
        for entity in entities {
            match entity.r#type.as_str() {
                "bold" => {
                    let start = entity.offset.min(result.len());
                    let end = (entity.offset + entity.length).min(result.len());
                    if start < end {
                        let before = &result[..start];
                        let body = &result[start..end];
                        let after = &result[end..];
                        result = format!("{}**{}**{}", before, body, after);
                    }
                }
                "link" => {
                    if let Some(url) = &entity.url {
                        let start = entity.offset.min(result.len());
                        let end = (entity.offset + entity.length).min(result.len());
                        if start < end {
                            let before = &result[..start];
                            let link_text = &result[start..end];
                            let after = &result[end..];
                            result = format!("{}[{}]({}){}", before, link_text, url, after);
                        }
                    }
                }
                _ => {}
            }
        }
        result
    }

    fn format_timestamp(dt: &chrono::DateTime<chrono::Utc>) -> String {
        let now = chrono::Utc::now();
        // Guard bad epochs (1970 / far future) from broken unit conversion
        let year: i32 = dt.format("%Y").to_string().parse().unwrap_or(0);
        if !(2000..=2100).contains(&year) {
            return format!("{}", now.with_timezone(&chrono::Local).format("%H:%M"));
        }
        let diff = now.signed_duration_since(*dt);

        if diff.num_seconds().abs() < 86400 && diff.num_days() == 0 {
            format!("{}", dt.with_timezone(&chrono::Local).format("%H:%M"))
        } else if diff.num_days() == 1 || (diff.num_hours() >= 24 && diff.num_hours() < 48) {
            "Вчера".to_string()
        } else if (0..7).contains(&diff.num_days()) {
            format!("{} дн. назад", diff.num_days())
        } else {
            format!("{}", dt.with_timezone(&chrono::Local).format("%d.%m.%Y"))
        }
    }

    fn format_duration(seconds: u64) -> String {
        let mins = seconds / 60;
        let secs = seconds % 60;
        if mins > 0 {
            format!("{:02}:{:02}", mins, secs)
        } else {
            format!("0:{:02}", secs)
        }
    }

    fn handle_send(&self) {
        log::info!("handle_send called");
        let chat_id = match self.current_chat_id() {
            Some(id) => id,
            None => {
                log::warn!("handle_send: no chat selected");
                self.show_error("Сначала выберите чат");
                return;
            }
        };

        let text = {
            let buffer = self.input_entry.buffer();
            let (start, end) = buffer.bounds();
            buffer.text(&start, &end, false).to_string()
        };
        let text = text.trim().to_string();
        if text.is_empty() {
            return;
        }

        let reply_id = self.reply_to_msg_id.lock().unwrap().clone();
        let edit_id = self.edit_msg_id.lock().unwrap().clone();

        log::info!(
            "Sending to {}: {} chars (reply={:?}, edit={:?})",
            chat_id,
            text.len(),
            reply_id,
            edit_id
        );

        let has_cb = self.on_send.lock().unwrap().is_some();
        if !has_cb {
            log::error!("handle_send: on_send callback is not bound");
            self.show_error("Отправка не инициализирована");
            return;
        }

        // Clear UI first so Enter cannot double-send
        self.input_entry.buffer().set_text("");
        self.reply_preview_box.set_visible(false);
        *self.reply_to_msg_id.lock().unwrap() = None;
        *self.edit_msg_id.lock().unwrap() = None;
        self.send_btn.set_visible(false);
        self.voice_btn.set_visible(crate::config::ym_enable_voice());

        if let Some(cb) = self.on_send.lock().unwrap().as_ref() {
            cb(chat_id, text, reply_id, edit_id);
        }
    }

    fn handle_attach(&self) {
        let chat_id = {
            let chat_ref = self.chat.lock().unwrap();
            let Some(chat) = chat_ref.as_ref() else {
                return;
            };
            chat.id.clone()
        };

        let on_attach = self.on_attach.clone();

        glib::spawn_future_local(async move {
            let file_dialog = gtk::FileDialog::new();
            file_dialog.set_title("Select image to attach");

            if let Ok(file) = file_dialog.open_future(None::<&gtk::Window>).await {
                let path = file.path().expect("Valid path");
                let filename = path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();

                // Read the file and compress if it's an image
                let bytes = if filename.to_lowercase().ends_with(".jpg")
                    || filename.to_lowercase().ends_with(".png")
                    || filename.to_lowercase().ends_with(".jpeg")
                {
                    log::info!("Compressing image: {}", filename);
                    if let Ok(img) = image::open(&path) {
                        // Resize image to max 1920x1080 to compress
                        let resized = img.resize(1920, 1080, image::imageops::FilterType::Lanczos3);
                        let mut cursor = std::io::Cursor::new(Vec::new());
                        if let Ok(_) = resized.write_to(&mut cursor, image::ImageFormat::Jpeg) {
                            cursor.into_inner()
                        } else {
                            std::fs::read(&path).unwrap_or_default()
                        }
                    } else {
                        std::fs::read(&path).unwrap_or_default()
                    }
                } else {
                    std::fs::read(&path).unwrap_or_default()
                };

                if let Some(cb) = on_attach.lock().unwrap().as_ref() {
                    cb(chat_id, bytes, filename);
                }
            }
        });
    }

    fn show_attach_menu(self: &Arc<Self>) {
        // Reuse a single popover — creating a new one every click left orphan shells
        // and looked like a "window inside a window".
        {
            let guard = self.attach_menu_popover.lock().unwrap();
            if let Some(pop) = guard.as_ref() {
                if pop.is_visible() {
                    pop.popdown();
                } else {
                    pop.popup();
                }
                return;
            }
        }

        let popover = Popover::builder().has_arrow(false).autohide(true).build();
        popover.set_parent(&self.attach_btn);
        popover.set_position(gtk::PositionType::Top);
        popover.add_css_class("attach-menu-popover");

        let vbox = gtk::Box::new(Orientation::Vertical, 2);
        vbox.set_margin_start(4);
        vbox.set_margin_end(4);
        vbox.set_margin_top(4);
        vbox.set_margin_bottom(4);
        vbox.add_css_class("attach-menu");

        let btn_file = menu_row_button("mail-attachment-symbolic", "Отправить файл");
        let this_file = self.clone();
        let pop_clone = popover.clone();
        btn_file.connect_clicked(move |_| {
            pop_clone.popdown();
            this_file.handle_attach();
        });

        let btn_sticker = menu_row_button("face-smile-symbolic", "Стикеры");
        let this_sticker = self.clone();
        let pop_st = popover.clone();
        btn_sticker.connect_clicked(move |_| {
            pop_st.popdown();
            this_sticker.sticker_btn.emit_clicked();
        });

        let btn_poll = menu_row_button("view-list-symbolic", "Создать опрос");
        let this_poll = self.clone();
        let pop_clone2 = popover.clone();
        btn_poll.connect_clicked(move |_| {
            pop_clone2.popdown();
            if let Some(pop) = this_poll.poll_popover.lock().unwrap().as_ref() {
                if pop.is_visible() {
                    pop.popdown();
                } else {
                    pop.popup();
                }
            }
        });

        let btn_sched = menu_row_button("preferences-system-time-symbolic", "Запланировать");
        let this_sched = self.clone();
        let pop_clone3 = popover.clone();
        btn_sched.connect_clicked(move |_| {
            pop_clone3.popdown();
            this_sched.show_send_at_popover();
        });

        vbox.append(&btn_file);
        vbox.append(&btn_sticker);
        vbox.append(&btn_poll);
        vbox.append(&btn_sched);

        popover.set_child(Some(&vbox));
        *self.attach_menu_popover.lock().unwrap() = Some(popover.clone());
        popover.popup();
    }

    fn show_header_menu(self: &Arc<Self>) {
        let popover = Popover::builder().has_arrow(false).autohide(true).build();
        popover.set_parent(&self.menu_btn);
        popover.set_position(gtk::PositionType::Bottom);
        popover.add_css_class("header-menu-popover");

        let menu_vbox = gtk::Box::new(Orientation::Vertical, 2);
        menu_vbox.set_margin_start(4);
        menu_vbox.set_margin_end(4);
        menu_vbox.set_margin_top(4);
        menu_vbox.set_margin_bottom(4);

        let btn_info = menu_row_button("dialog-information-symbolic", "Информация");
        let cv_info = self.clone();
        let menu_pop_clone = popover.clone();
        btn_info.connect_clicked(move |_| {
            menu_pop_clone.popdown();
            if let Some(chat) = cv_info.chat.lock().unwrap().as_ref() {
                log::info!("Chat Info requested for chat: {}", chat.id);
                #[allow(deprecated)]
                let dialog = gtk::MessageDialog::new(
                    None::<&gtk::Window>,
                    gtk::DialogFlags::MODAL,
                    gtk::MessageType::Info,
                    gtk::ButtonsType::Ok,
                    &format!(
                        "Название: {}\nID: {}\nТип: {:?}",
                        chat.display_name(),
                        chat.id,
                        chat.chat_type
                    ),
                );
                dialog.set_title(Some("Информация о чате"));
                dialog.present();
            }
        });

        let btn_mute = Button::builder()
            .label("🔔 Уведомления")
            .css_classes(vec!["flat".to_string(), "attach-menu-btn".to_string()])
            .halign(gtk::Align::Start)
            .build();
        let menu_pop_clone2 = popover.clone();
        btn_mute.connect_clicked(move |_| {
            menu_pop_clone2.popdown();
            log::info!("Toggle mute notifications clicked");
        });

        menu_vbox.append(&btn_info);
        menu_vbox.append(&btn_mute);
        popover.set_child(Some(&menu_vbox));
        popover.popup();
    }

    pub fn set_typing(&self, user: &str) {
        if self.chat.lock().unwrap().is_some() {
            let escaped_user = glib::markup_escape_text(user);
            self.status_label.set_use_markup(true);
            self.status_label.set_markup(&format!(
                "<span class='status-label'>{} печатает</span> <span class='typing-dots typing-dot-1'>●</span><span class='typing-dots typing-dot-2'>●</span><span class='typing-dots typing-dot-3'>●</span>",
                escaped_user
            ));
        }
    }

    pub fn set_online(&self) {
        if let Some(_chat) = self.chat.lock().unwrap().as_ref() {
            self.status_label.set_use_markup(false);
            self.status_label.set_label("В сети");
        }
    }

    pub fn set_status_text(&self, text: &str) {
        self.status_label.set_use_markup(false);
        self.status_label.set_label(text);
    }

    fn edit_last_message(&self) {
        let current_user_id = self.auth.user_id();
        let Some(ref uid) = current_user_id else {
            return;
        };

        let messages = self.messages.lock().unwrap();
        // Ищем последнее сообщение текущего пользователя
        if let Some(msg) = messages
            .iter()
            .rev()
            .find(|m| m.from_id == *uid && m.text.is_some())
        {
            let msg_text = msg.text.clone().unwrap_or_default();

            self.reply_preview_label
                .set_label(&format!("Редактирование: {}", msg_text));
            self.reply_preview_box.set_visible(true);
            *self.edit_msg_id.lock().unwrap() = Some(msg.id.clone());
            *self.reply_to_msg_id.lock().unwrap() = None;

            let buffer = self.input_entry.buffer();
            buffer.set_text(&msg_text);
            self.input_entry.grab_focus();
            let end_iter = buffer.end_iter();
            buffer.place_cursor(&end_iter);
        }
    }

    pub fn container(&self) -> &GtkBox {
        &self.container
    }

    /// Инициализирует emoji picker popover
    pub fn init_emoji_picker(self: &Arc<Self>) {
        if self.emoji_popover.lock().unwrap().is_some() {
            return;
        }

        let emoji_picker = EmojiPicker::new();
        let popover = Popover::builder().has_arrow(false).autohide(true).build();
        popover.add_css_class("emoji-picker-popover");
        popover.set_child(Some(emoji_picker.container()));
        popover.set_parent(&self.emoji_btn);
        popover.set_position(gtk::PositionType::Top);

        let input_entry_clone = self.input_entry.clone();
        // Keep popover open while picking multiple emojis (YM-style)
        emoji_picker.on_select(move |emoji| {
            let buffer = input_entry_clone.buffer();
            let (start, end) = buffer.bounds();
            let current_text = buffer.text(&start, &end, false).to_string();
            buffer.set_text(&format!("{}{}", current_text, emoji));
            input_entry_clone.grab_focus();
            let end_iter = buffer.end_iter();
            buffer.place_cursor(&end_iter);
        });

        *self.emoji_popover.lock().unwrap() = Some(popover.clone());

        let emoji_btn = self.emoji_btn.clone();
        let cv_emoji = self.clone();
        emoji_btn.connect_clicked(move |_| {
            // Close sibling popovers so shells don't stack
            cv_emoji.close_composer_popovers(ComposerPopover::Emoji);
            if let Some(pop) = cv_emoji.emoji_popover.lock().unwrap().as_ref() {
                if pop.is_visible() {
                    pop.popdown();
                } else {
                    pop.popup();
                }
            }
        });
    }

    pub fn show_error(&self, err_msg: &str) {
        self.show_toast(&format!("Ошибка: {}", err_msg));
    }

    /// Transient status bar (reuse undo bar chrome).
    pub fn show_toast(&self, msg: &str) {
        self.undo_label.set_text(msg);
        self.undo_bar.set_visible(true);

        let undo_bar = self.undo_bar.clone();
        glib::timeout_add_seconds_local(4, move || {
            undo_bar.set_visible(false);
            glib::ControlFlow::Break
        });
    }

    /// Инициализирует sticker panel popover (вызывать после создания ChatView)
    pub fn init_sticker_panel(self: &Arc<Self>) {
        if self.sticker_panel.lock().unwrap().is_some() {
            return;
        }

        let sticker_panel = StickerPanel::new(vec![]);
        let popover = Popover::builder().has_arrow(false).autohide(true).build();
        popover.add_css_class("sticker-panel-popover");
        popover.set_child(Some(sticker_panel.container()));
        // Anchor on visible emoji btn (sticker icon is hidden to free composer width)
        popover.set_parent(&self.emoji_btn);
        popover.set_position(gtk::PositionType::Top);

        let cv_select = self.clone();
        let popover_clone = popover.clone();
        sticker_panel.on_select(move |sticker_id, pack_id| {
            let chat_id_opt = {
                let chat = cv_select.chat.lock().unwrap();
                chat.as_ref().map(|c| c.id.clone())
            };
            if let Some(chat_id) = &chat_id_opt {
                if let Some(ref cb) = cv_select.on_send.lock().unwrap().as_ref() {
                    let sticker = serde_json::json!({
                        "sticker_id": sticker_id,
                        "pack_id": pack_id,
                    });
                    if let Ok(json) = serde_json::to_string(&sticker) {
                        cb(chat_id.clone(), json, None, None);
                    }
                }
            }
            popover_clone.popdown();
        });

        *sticker_panel.popover.borrow_mut() = Some(popover.clone());
        *self.sticker_panel.lock().unwrap() = Some(sticker_panel);

        let sticker_btn = self.sticker_btn.clone();
        let cv_sticker = self.clone();
        sticker_btn.connect_clicked(move |_| {
            cv_sticker.close_composer_popovers(ComposerPopover::Sticker);
            if let Some(panel) = cv_sticker.sticker_panel.lock().unwrap().as_ref() {
                if let Some(pop) = panel.popover.borrow().as_ref() {
                    if pop.is_visible() {
                        pop.popdown();
                    } else {
                        pop.popup();
                    }
                }
            }
        });
    }

    /// Close other composer popovers so only one shell is visible.
    fn close_composer_popovers(&self, keep: ComposerPopover) {
        if !matches!(keep, ComposerPopover::Emoji) {
            if let Some(pop) = self.emoji_popover.lock().unwrap().as_ref() {
                pop.popdown();
            }
        }
        if !matches!(keep, ComposerPopover::Sticker) {
            if let Some(panel) = self.sticker_panel.lock().unwrap().as_ref() {
                if let Some(pop) = panel.popover.borrow().as_ref() {
                    pop.popdown();
                }
            }
        }
        if !matches!(keep, ComposerPopover::Attach) {
            if let Some(pop) = self.attach_menu_popover.lock().unwrap().as_ref() {
                pop.popdown();
            }
        }
        if !matches!(keep, ComposerPopover::Poll) {
            if let Some(pop) = self.poll_popover.lock().unwrap().as_ref() {
                pop.popdown();
            }
        }
        if !matches!(keep, ComposerPopover::Schedule) {
            if let Some(pop) = self.send_at_popover.lock().unwrap().as_ref() {
                pop.popover().popdown();
                *self.popover_open.lock().unwrap() = false;
            }
        }
    }

    /// Обновляет список пакетов стикеров
    pub fn update_sticker_packs(&self, packs: Vec<crate::models::StickerPack>) {
        *self.sticker_packs.lock().unwrap() = packs.clone();
        if let Some(panel) = self.sticker_panel.lock().unwrap().as_ref() {
            panel.update_packs(packs);
        }
    }

    /// Регистрирует callback при выборе стикера
    pub fn on_sticker_select(&self, callback: impl Fn(String, String) + 'static) {
        // Примечание: on_select уже обрабатывается внутри init_sticker_panel.
        // Если нужно добавить дополнительное действие, используйте этот метод для
        // подписки на события после инициализации.
        let sticker_btn = self.sticker_btn.clone();
        sticker_btn.connect_clicked(move |_| {
            callback("".to_string(), "".to_string());
        });
    }

    /// Регистрирует callback для перевода сообщения
    pub fn on_translate(&self, callback: impl Fn(String, String) + 'static) {
        *self.on_translate.lock().unwrap() = Some(StdBox::new(callback));
    }

    /// Показывает ImageViewer для изображения
    pub fn show_image(&self, url: &str, filename: &str) {
        let mut all_images = Vec::new();
        for msg in self.messages.lock().unwrap().iter() {
            for m in &msg.media {
                if m.type_ == crate::models::MediaType::Image {
                    all_images.push((m.url.clone(), "image.jpg".to_string()));
                }
            }
        }

        if let Some(cb) = self.on_image_open.lock().unwrap().as_ref() {
            cb(url.to_string(), filename.to_string(), all_images);
        }
    }

    pub fn on_image_open(
        &self,
        callback: impl Fn(String, String, Vec<(String, String)>) + 'static,
    ) {
        *self.on_image_open.lock().unwrap() = Some(StdBox::new(callback));
    }

    /// Callback for file download/open: (file_id, url, filename, open_after).
    pub fn on_file_download(&self, callback: impl Fn(String, String, String, bool) + 'static) {
        *self.on_file_download.lock().unwrap() = Some(StdBox::new(callback));
    }

    pub fn on_typing(&self, callback: impl Fn(String) + 'static) {
        *self.on_typing.lock().unwrap() = Some(StdBox::new(callback));
    }
    /// Register callback for saving a message to favorites
    pub fn on_save(&self, callback: impl Fn(String, String, Option<String>) + Send + 'static) {
        *self.on_save.lock().unwrap() = Some(StdBox::new(callback));
    }

    /// Register server-side delete callback (chat_id, message_id).
    /// Fired when the undo window expires without undo.
    pub fn on_delete(&self, callback: impl Fn(String, String) + 'static) {
        *self.on_delete.lock().unwrap() = Some(StdBox::new(callback));
    }

    /// Remove a message from the list (after confirmed server delete).
    /// Splices just that row — no scroll jump.
    pub fn remove_message(&self, message_id: &str) {
        {
            let mut messages = self.messages.lock().unwrap();
            let before = messages.len();
            messages.retain(|m| {
                m.id != message_id && m.message_id.as_deref() != Some(message_id)
            });
            if messages.len() == before {
                return;
            }
        }
        let store = &self.messages_store;
        let n = store.n_items();
        let mut pos = None;
        for i in 0..n {
            if let Some(obj) = store
                .item(i)
                .and_then(|o| o.downcast::<crate::ui::message_object::MessageObject>().ok())
            {
                let m = obj.message();
                if m.id == message_id || m.message_id.as_deref() == Some(message_id) {
                    pos = Some(i);
                    break;
                }
            }
        }
        if let Some(p) = pos {
            store.remove(p);
        }
        self.message_rows.lock().unwrap().clear();
        self.undo_bar.set_visible(false);
        *self.pending_delete_msg_id.lock().unwrap() = None;
        *self.pending_delete_row.lock().unwrap() = None;
    }

    /// Restore a pending-delete row (server delete failed).
    pub fn cancel_pending_delete(&self) {
        self.undo_bar.set_visible(false);
        if let Some(row) = self.pending_delete_row.lock().unwrap().as_ref() {
            row.set_visible(true);
        }
        *self.pending_delete_msg_id.lock().unwrap() = None;
        *self.pending_delete_row.lock().unwrap() = None;
    }

    /// Callback for inline button clicks
    pub fn on_inline_button_click(&self, callback: impl Fn(String, String) + Send + 'static) {
        *self.on_inline_click.lock().unwrap() = Some(StdBox::new(callback));
    }

    /// Callback for keyboard button clicks
    pub fn on_keyboard_button_click(&self, callback: impl Fn(String) + Send + 'static) {
        *self.on_keyboard_click.lock().unwrap() = Some(StdBox::new(callback));
    }
    pub fn on_voice_send(&self, callback: impl Fn(String, Vec<u8>, f64, Vec<f32>) + 'static) {
        *self.on_voice_send.lock().unwrap() = Some(StdBox::new(callback));
    }

    pub fn set_chat(&self, chat: Chat) {
        let previous_chat_id = self.current_chat_id();
        let is_new_chat = previous_chat_id.as_deref() != Some(chat.id.as_str());

        *self.chat.lock().unwrap() = Some(chat.clone());
        self.title_label
            .set_label(chat.title.as_deref().unwrap_or("Чат"));

        if is_new_chat {
            self.messages_store.remove_all();
            self.messages.lock().unwrap().clear();
            self.message_rows.lock().unwrap().clear();
            *self.has_more_history.lock().unwrap() = true;
            *self.loading_older.lock().unwrap() = false;
            *self.pagination_cursor.lock().unwrap() = None;
            // Next history load must land on the newest messages
            *self.stick_to_bottom.lock().unwrap() = true;
        }

        // Update status based on chat type
        let status_text = match chat.chat_type {
            crate::models::ChatType::Group => {
                format!("Группа • {} участников", chat.participants.len())
            }
            crate::models::ChatType::Channel => {
                format!("Канал • {} подписчиков", chat.participants.len())
            }
            crate::models::ChatType::Bot => "Бот".to_string(),
            crate::models::ChatType::Private => "В сети".to_string(),
            crate::models::ChatType::Unknown => "".to_string(),
        };
        self.status_label.set_label(&status_text);

        // Show the call button when a chat is selected (except for channels)
        self.call_btn.set_visible(
            chat.chat_type != crate::models::ChatType::Channel
                && crate::config::ym_enable_telemost_ui(),
        );

        // Show composer bar (TG layout: attach + full-width text + emoji + send/voice)
        self.input_area.set_visible(true);
        self.input_entry.set_visible(true);
        self.attach_btn.set_visible(true);
        self.emoji_btn.set_visible(true);
        // sticker/poll/schedule stay hidden anchors — opened from attach menu

        let is_empty = {
            let buffer = self.input_entry.buffer();
            let (start, end) = buffer.bounds();
            buffer.text(&start, &end, false).trim().is_empty()
        };
        self.send_btn.set_visible(!is_empty);
        self.voice_btn
            .set_visible(is_empty && crate::config::ym_enable_voice());

        // Loading skeleton until messages arrive
        if is_new_chat {
            self.show_messages_skeleton();
        }

        // Check if this is a bot chat and show BotPanel
        self.handle_bot_chat(&chat);
    }

    pub fn show_messages_skeleton(&self) {
        self.content_stack.set_visible_child_name("skeleton");
    }

    pub fn show_messages_list(&self) {
        self.content_stack.set_visible_child_name("list");
    }

    pub fn show_empty_conversation(&self) {
        self.content_stack.set_visible_child_name("empty");
    }

    pub fn show_welcome(&self) {
        self.content_stack.set_visible_child_name("welcome");
    }

    /// Hide chat view and show welcome empty state
    pub fn set_empty(&self) {
        *self.chat.lock().unwrap() = None;
        self.title_label.set_label("Messenger");
        self.status_label.set_label("");
        // Hide composer bar
        self.input_area.set_visible(false);
        self.input_entry.set_visible(false);
        self.send_btn.set_visible(false);
        self.attach_btn.set_visible(false);
        self.call_btn.set_visible(false);
        self.emoji_btn.set_visible(false);
        self.sticker_btn.set_visible(false);
        self.poll_btn.set_visible(false);
        self.schedule_btn.set_visible(false);

        // Clear message list + welcome
        self.messages_store.remove_all();
        self.messages.lock().unwrap().clear();
        self.show_welcome();

        // Hide bot panel
        if let Some(_bot_p) = self.bot_panel.lock().unwrap().take() {
            // Remove bot panel from container
            if let Some(parent) = self.container.parent() {
                let mut child = parent.first_child();
                while let Some(current) = child {
                    child = current.next_sibling();
                    if current
                        .parent()
                        .map(|p| {
                            p.downcast::<gtk::Box>()
                                .map(|box_| {
                                    box_.remove(&current);
                                    true
                                })
                                .unwrap_or(false)
                        })
                        .unwrap_or(false)
                    {
                        break;
                    }
                }
            }
        }
        self.bot_info.lock().unwrap().take();
    }

    /// Handle bot chat display
    fn handle_bot_chat(&self, chat: &Chat) {
        // Determine if this is a bot chat
        let is_bot = chat.chat_type == crate::models::ChatType::Bot
            || chat
                .title
                .as_ref()
                .map(|t| t.contains("Бот"))
                .unwrap_or(false);

        if is_bot {
            if let Some(parent) = self.container.parent() {
                // Create bot panel if not exists — use the same AuthManager instance
                if self.bot_panel.lock().unwrap().is_none() {
                    let bot_p = BotPanel::new(self.auth.clone());

                    let on_inline = std::sync::Arc::downgrade(&self.on_inline_click);
                    let chat_id_clone = chat.clone();
                    bot_p.on_inline_button_click(move |_text, data| {
                        if let Some(weak_cell) = on_inline.upgrade() {
                            if let Some(cb) = weak_cell.lock().unwrap().as_ref() {
                                let cid = chat_id_clone.id.clone();
                                cb(cid, data);
                            }
                        }
                    });

                    let on_cmd = std::sync::Arc::downgrade(&self.on_command_click);
                    let chat_id_clone2 = chat.clone();
                    bot_p.on_command_click(move |cmd, params| {
                        if let Some(weak_cell) = on_cmd.upgrade() {
                            if let Some(cb) = weak_cell.lock().unwrap().as_ref() {
                                let cid = chat_id_clone2.id.clone();
                                cb(cid, format!("{}:{}", cmd, params));
                            }
                        }
                    });

                    *self.bot_panel.lock().unwrap() = Some(bot_p);
                }

                // Show bot panel
                if let Some(bot_p) = self.bot_panel.lock().unwrap().as_ref() {
                    let bot_container = bot_p.container.clone();
                    // Add the bot panel to parent
                    if let Some(parent_box) = parent.downcast_ref::<gtk::Box>() {
                        parent_box.reorder_child_after(&bot_container, None::<&gtk::Widget>);
                    } else {
                        let _ = bot_container.set_parent(&parent);
                    }
                    bot_container.set_visible(true);
                }
            }
        }
    }

    /// Update bot info for current chat
    pub fn set_bot_info(&self, bot_info: crate::models::BotInfo) {
        *self.bot_info.lock().unwrap() = Some(bot_info);
        if let Some(panel) = self.bot_panel.lock().unwrap().as_ref() {
            panel.update(&self.bot_info.lock().unwrap().as_ref().unwrap());
        }
    }

    /// Update reply markup in bot panel
    pub fn update_reply_markup(&self, markup: crate::models::BotReplyMarkup) {
        if let Some(panel) = self.bot_panel.lock().unwrap().as_ref() {
            panel.update_reply_markup(&markup);
        }
    }

    pub fn set_messages(&self, messages: Vec<Message>) {
        let current_chat_id = self.current_chat_id();
        if let Some(chat_id) = current_chat_id.as_ref() {
            if let Some(first) = messages.first() {
                if first.chat_id != *chat_id {
                    return;
                }
            }
        }

        let should_render = {
            let current = self.messages.lock().unwrap();
            !crate::models::messages_equivalent(&current, &messages)
        };
        // Even if content equivalent, still force scroll to latest when opening chat
        if !should_render {
            if !messages.is_empty() {
                self.show_messages_list();
                self.scroll_to_latest();
            }
            return;
        }

        self.message_rows.lock().unwrap().clear();
        // Fresh page invalidates the older-pages cursor; assume more
        // history if we got a full first page (initial page size = 100).
        *self.pagination_cursor.lock().unwrap() = None;
        *self.has_more_history.lock().unwrap() = messages.len() >= 100;
        *self.loading_older.lock().unwrap() = false;
        *self.messages.lock().unwrap() = messages.clone();
        if messages.is_empty() {
            self.show_empty_conversation();
        } else {
            self.show_messages_list();
            self.render_messages(); // ends with scroll_to_latest
        }
    }

    pub fn set_pagination_loading(&self, loading: bool) {
        self.pagination_bar.set_visible(loading);
        // Keep spinner spinning while visible
        if let Some(child) = self.pagination_bar.first_child() {
            if let Ok(spinner) = child.downcast::<gtk::Spinner>() {
                spinner.set_spinning(loading);
            }
        }
        if !loading {
            *self.loading_older.lock().unwrap() = false;
        }
    }

    /// Prepend older messages (pagination) while trying to keep scroll position.
    /// `next_cursor` is the server cursor for the following older page;
    /// `None` means the server reported the end of history.
    pub fn prepend_messages(&self, older: Vec<Message>, next_cursor: Option<String>) {
        // Pagination is upward history — do not force bottom
        *self.stick_to_bottom.lock().unwrap() = false;
        *self.pagination_cursor.lock().unwrap() = next_cursor.clone();
        if older.is_empty() {
            // Empty page ends history only when the server also gives no cursor.
            if next_cursor.is_none() {
                *self.has_more_history.lock().unwrap() = false;
            }
            self.set_pagination_loading(false);
            return;
        }

        let adj = self.scrolled.vadjustment();
        let old_upper = adj.upper();
        let old_value = adj.value();

        {
            let mut messages = self.messages.lock().unwrap();
            let existing_ids: std::collections::HashSet<String> =
                messages.iter().map(|m| m.id.clone()).collect();
            let mut filtered: Vec<Message> = older
                .into_iter()
                .filter(|m| !existing_ids.contains(&m.id))
                .collect();
            if filtered.is_empty() {
                // All fetched messages are duplicates — keep paging only if
                // the server explicitly gave us a cursor for the next page.
                if next_cursor.is_none() {
                    *self.has_more_history.lock().unwrap() = false;
                }
                self.set_pagination_loading(false);
                return;
            }
            // More history exists when the server hands us a cursor,
            // or when we got a full page (page size = 100).
            *self.has_more_history.lock().unwrap() =
                next_cursor.is_some() || filtered.len() >= 100;
            filtered.append(&mut *messages);
            filtered.sort_by(|a, b| a.created.cmp(&b.created));
            let mut seen = std::collections::HashSet::new();
            filtered.retain(|m| seen.insert(m.id.clone()));
            *messages = filtered;
        }

        self.message_rows.lock().unwrap().clear();
        self.render_messages();

        // Restore relative scroll after content height grows
        let scrolled = self.scrolled.clone();
        glib::timeout_add_local_once(std::time::Duration::from_millis(50), move || {
            let adj = scrolled.vadjustment();
            let new_upper = adj.upper();
            let delta = new_upper - old_upper;
            if delta > 0.0 {
                adj.set_value(old_value + delta);
            }
        });
        self.set_pagination_loading(false);
    }

    pub fn on_load_older(&self, callback: impl Fn(String, String, Option<String>) + 'static) {
        *self.on_load_older.lock().unwrap() = Some(StdBox::new(callback));
    }

    fn build_file_attachment_row(&self, media: &crate::models::MediaAttachment) -> GtkBox {
        let row = GtkBox::new(Orientation::Horizontal, 8);
        row.add_css_class("file-attachment");
        row.set_margin_top(4);
        row.set_margin_bottom(4);
        row.set_hexpand(true);

        let icon = gtk::Image::from_icon_name("x-office-document-symbolic");
        icon.set_pixel_size(28);
        row.append(&icon);

        let info = GtkBox::new(Orientation::Vertical, 2);
        info.set_hexpand(true);
        let name = media
            .filename
            .clone()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| {
                if media.url.is_empty() {
                    "Файл".to_string()
                } else {
                    media.url.rsplit('/').next().unwrap_or("file").to_string()
                }
            });
        let name_label = Label::builder()
            .label(&name)
            .xalign(0.0)
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .css_classes(vec!["file-attachment-name".to_string()])
            .build();
        info.append(&name_label);
        if let Some(sz) = media.size {
            let size_label = Label::builder()
                .label(&format_file_size(sz))
                .xalign(0.0)
                .css_classes(vec!["dim-label".to_string()])
                .build();
            info.append(&size_label);
        }
        row.append(&info);

        let file_id = media.id.clone();
        let url = media.url.clone();
        let filename = name.clone();
        let on_dl = self.on_file_download.clone();

        let btn_save = Button::builder()
            .label("Скачать")
            .css_classes(vec!["flat".to_string(), "file-action-btn".to_string()])
            .tooltip_text("Сохранить в Загрузки")
            .build();
        {
            let on_dl = on_dl.clone();
            let file_id = file_id.clone();
            let url = url.clone();
            let filename = filename.clone();
            btn_save.connect_clicked(move |_| {
                if let Some(ref cb) = *on_dl.lock().unwrap() {
                    cb(file_id.clone(), url.clone(), filename.clone(), false);
                }
            });
        }
        row.append(&btn_save);

        let btn_open = Button::builder()
            .label("Открыть")
            .css_classes(vec!["flat".to_string(), "file-action-btn".to_string()])
            .tooltip_text("Скачать и открыть")
            .build();
        {
            let on_dl = on_dl.clone();
            btn_open.connect_clicked(move |_| {
                if let Some(ref cb) = *on_dl.lock().unwrap() {
                    cb(file_id.clone(), url.clone(), filename.clone(), true);
                }
            });
        }
        row.append(&btn_open);

        row
    }

    fn setup_history_scroll(self: &Arc<Self>) {
        let this = self.clone();
        let adj = self.scrolled.vadjustment();
        adj.connect_value_changed(move |adj| {
            // While pinning to latest (chat open), ignore false "top" positions
            if *this.stick_to_bottom.lock().unwrap() {
                return;
            }
            // Near top of history → load older
            if adj.value() > 80.0 {
                return;
            }
            if *this.loading_older.lock().unwrap() {
                return;
            }
            if !*this.has_more_history.lock().unwrap() {
                return;
            }
            let chat_id = match this.current_chat_id() {
                Some(id) => id,
                None => return,
            };
            let oldest_id = {
                let msgs = this.messages.lock().unwrap();
                msgs.first().map(|m| m.id.clone())
            };
            let Some(oldest_id) = oldest_id else {
                return;
            };
            let cursor = this.pagination_cursor.lock().unwrap().clone();
            *this.loading_older.lock().unwrap() = true;
            this.set_pagination_loading(true);
            if let Some(ref cb) = *this.on_load_older.lock().unwrap() {
                cb(chat_id, oldest_id, cursor);
            } else {
                this.set_pagination_loading(false);
            }
        });
    }

    /// Drag-and-drop files + paste images into the chat.
    fn setup_file_drop_and_paste(self: &Arc<Self>) {
        // DnD target on the whole chat container
        let drop_target =
            gtk::DropTarget::new(gio::File::static_type(), gtk::gdk::DragAction::COPY);
        let this = self.clone();
        drop_target.connect_drop(move |_target, value, _x, _y| {
            let Ok(file) = value.get::<gio::File>() else {
                return false;
            };
            let Some(path) = file.path() else {
                return false;
            };
            let chat_id = match this.current_chat_id() {
                Some(id) => id,
                None => return false,
            };
            let filename = path
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "file.bin".into());
            let bytes = std::fs::read(&path).unwrap_or_default();
            if bytes.is_empty() {
                return false;
            }
            if let Some(ref cb) = *this.on_attach.lock().unwrap() {
                cb(chat_id, bytes, filename);
                return true;
            }
            false
        });
        self.container.add_controller(drop_target);

        // Also accept image paste on the whole chat surface (not only TextView)
        let key = gtk::EventControllerKey::new();
        key.set_propagation_phase(gtk::PropagationPhase::Capture);
        let this = self.clone();
        key.connect_key_pressed(move |_c, keyval, _code, state| {
            let ctrl = state.contains(gtk::gdk::ModifierType::CONTROL_MASK);
            if !ctrl || (keyval != gtk::gdk::Key::v && keyval != gtk::gdk::Key::V) {
                return glib::Propagation::Proceed;
            }
            if this.try_paste_clipboard_image() {
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
        self.container.add_controller(key);
    }

    /// Try to paste an image/screenshot from the system clipboard.
    /// Returns true if an image paste was initiated (caller should stop key propagation).
    fn try_paste_clipboard_image(self: &Arc<Self>) -> bool {
        if self.current_chat_id().is_none() {
            self.show_error("Сначала выберите чат");
            return true;
        }
        let Some(display) = gtk::gdk::Display::default() else {
            return false;
        };
        let clipboard = display.clipboard();
        let this = self.clone();
        log::info!("Ctrl+V: attempting clipboard image paste");

        // 1) Texture path (works when compositor exposes gdk.Texture)
        clipboard.read_texture_async(gio::Cancellable::NONE, {
            let this = this.clone();
            move |result| {
                match result {
                    Ok(Some(texture)) => {
                        // save_to_png on huge screenshots is expensive — defer one idle tick
                        // so the key event finishes first (reduces "UI freeze" feeling).
                        let this2 = this.clone();
                        glib::idle_add_local_once(move || {
                            let w = texture.width();
                            let h = texture.height();
                            log::info!("clipboard texture {}x{}, encoding PNG…", w, h);
                            // Cap enormous textures — avoid multi‑100MB PNG + process kill
                            if w > 8192 || h > 8192 {
                                log::warn!(
                                    "clipboard texture too large ({}x{}), trying MIME fallback",
                                    w,
                                    h
                                );
                                this2.paste_clipboard_image_mime();
                                return;
                            }
                            let bytes = texture.save_to_png_bytes().to_vec();
                            if !bytes.is_empty() && bytes.len() < 40 * 1024 * 1024 {
                                this2.deliver_pasted_image(bytes, "screenshot.png");
                            } else if bytes.len() >= 40 * 1024 * 1024 {
                                log::warn!("clipboard PNG {} bytes — MIME fallback", bytes.len());
                                this2.paste_clipboard_image_mime();
                            } else {
                                this2.paste_clipboard_image_mime();
                            }
                        });
                        return;
                    }
                    Ok(None) => log::debug!("clipboard: no texture"),
                    Err(e) => log::debug!("clipboard texture: {}", e),
                }
                // 2) Fallback: raw image MIME (common for GNOME/KDE screenshots)
                this.paste_clipboard_image_mime();
            }
        });
        true
    }

    fn paste_clipboard_image_mime(self: &Arc<Self>) {
        let Some(display) = gtk::gdk::Display::default() else {
            return;
        };
        let clipboard = display.clipboard();
        let this = self.clone();
        let mimes = [
            "image/png",
            "image/jpeg",
            "image/jpg",
            "image/bmp",
            "image/tiff",
            "image/webp",
            "image/x-png",
        ];
        clipboard.read_async(
            &mimes,
            glib::Priority::DEFAULT,
            gio::Cancellable::NONE,
            move |result| match result {
                Ok((stream, mime)) => {
                    let mime = mime.to_string();
                    let this = this.clone();
                    glib::spawn_future_local(async move {
                        match read_gio_stream_bytes(stream).await {
                            Ok(bytes) if !bytes.is_empty() => {
                                let name = if mime.contains("jpeg") || mime.contains("jpg") {
                                    "screenshot.jpg"
                                } else if mime.contains("webp") {
                                    "screenshot.webp"
                                } else {
                                    "screenshot.png"
                                };
                                this.deliver_pasted_image(bytes, name);
                            }
                            Ok(_) => log::debug!("clipboard image empty body"),
                            Err(e) => log::warn!("clipboard image stream: {}", e),
                        }
                    });
                }
                Err(e) => {
                    log::debug!("clipboard read_async image mime failed: {}", e);
                    // 3) file:// URI list (some tools put path on clipboard)
                    this.paste_clipboard_uri_file();
                    // Soft feedback if nothing works shortly
                    let toast = this.clone();
                    glib::timeout_add_local_once(
                        std::time::Duration::from_millis(600),
                        move || {
                            // Only show if still no recent attach (best-effort hint)
                            log::debug!("clipboard paste chain finished without guaranteed image");
                            let _ = toast;
                        },
                    );
                }
            },
        );
    }

    fn paste_clipboard_uri_file(self: &Arc<Self>) {
        let Some(display) = gtk::gdk::Display::default() else {
            return;
        };
        let clipboard = display.clipboard();
        let this = self.clone();
        clipboard.read_async(
            &["text/uri-list", "text/plain"],
            glib::Priority::DEFAULT,
            gio::Cancellable::NONE,
            move |result| {
                let Ok((stream, _mime)) = result else {
                    return;
                };
                let this = this.clone();
                glib::spawn_future_local(async move {
                    let Ok(bytes) = read_gio_stream_bytes(stream).await else {
                        return;
                    };
                    let text = String::from_utf8_lossy(&bytes);
                    let uri = text
                        .lines()
                        .map(str::trim)
                        .find(|l| !l.is_empty() && !l.starts_with('#'));
                    let Some(uri) = uri else {
                        return;
                    };
                    let path = if uri.starts_with("file:") {
                        glib::filename_from_uri(uri).ok().map(|(p, _)| p)
                    } else if uri.starts_with('/') {
                        Some(std::path::PathBuf::from(uri))
                    } else {
                        None
                    };
                    let Some(path) = path else {
                        return;
                    };
                    let Ok(file_bytes) = std::fs::read(&path) else {
                        return;
                    };
                    if file_bytes.is_empty() {
                        return;
                    }
                    let name = path
                        .file_name()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_else(|| "paste.bin".into());
                    // Only treat as image if extension looks like one
                    let lower = name.to_lowercase();
                    if !(lower.ends_with(".png")
                        || lower.ends_with(".jpg")
                        || lower.ends_with(".jpeg")
                        || lower.ends_with(".webp")
                        || lower.ends_with(".bmp")
                        || lower.ends_with(".gif"))
                    {
                        return;
                    }
                    this.deliver_pasted_image(file_bytes, &name);
                });
            },
        );
    }

    fn deliver_pasted_image(&self, bytes: Vec<u8>, filename: &str) {
        let chat_id = match self.current_chat_id() {
            Some(id) => id,
            None => {
                self.show_error("Сначала выберите чат");
                return;
            }
        };
        log::info!(
            "Pasting clipboard image {} ({} bytes) to {}",
            filename,
            bytes.len(),
            chat_id
        );
        if let Some(ref cb) = *self.on_attach.lock().unwrap() {
            cb(chat_id, bytes, filename.to_string());
        } else {
            self.show_error("Вложение не инициализировано");
        }
    }

    pub fn set_input_text(&self, text: &str) {
        self.input_entry.buffer().set_text(text);
    }

    pub fn input_text(&self) -> String {
        let buffer = self.input_entry.buffer();
        let (start, end) = buffer.bounds();
        buffer.text(&start, &end, false).to_string()
    }

    pub fn clear_input(&self) {
        self.input_entry.buffer().set_text("");
    }

    pub fn current_chat_id(&self) -> Option<String> {
        let chat_lock = self.chat.lock().unwrap();
        chat_lock.as_ref().map(|c| c.id.clone())
    }

    pub fn bind_callbacks(
        &self,
        on_send: impl Fn(String, String, Option<String>, Option<String>) + 'static,
        on_attach: impl Fn(String, Vec<u8>, String) + 'static,
        on_call: impl Fn(String) + 'static,
    ) {
        *self.on_send.lock().unwrap() = Some(StdBox::new(on_send));
        *self.on_attach.lock().unwrap() = Some(StdBox::new(on_attach));
        *self.on_call.lock().unwrap() = Some(StdBox::new(on_call));
    }

    pub fn bind_schedule_callbacks(
        &self,
        on_schedule: impl Fn(String, String, chrono::DateTime<chrono::Utc>) + 'static,
        on_cancel_schedule: impl Fn(String, String) + 'static,
    ) {
        *self.on_schedule.lock().unwrap() = Some(StdBox::new(on_schedule));
        *self.on_cancel_schedule.lock().unwrap() = Some(StdBox::new(on_cancel_schedule));
    }

    pub fn init_factory(self: &Arc<Self>) {
        let factory = gtk::SignalListItemFactory::new();
        let this = self.clone();

        factory.connect_setup(|_, list_item_obj| {
            let list_item = list_item_obj.downcast_ref::<gtk::ListItem>().unwrap();
            list_item.set_activatable(false);
            list_item.set_selectable(false);
            let row = GtkBox::new(Orientation::Vertical, 0);
            row.set_hexpand(true);
            row.set_halign(gtk::Align::Fill);
            row.set_vexpand(false);
            // No fixed min-width — that inflated paned end-child requisition
            row.set_size_request(-1, -1);
            list_item.set_child(Some(&row));
        });

        factory.connect_bind(move |_, list_item_obj| {
            let list_item = list_item_obj.downcast_ref::<gtk::ListItem>().unwrap();
            let item = list_item
                .item()
                .unwrap()
                .downcast::<crate::ui::message_object::MessageObject>()
                .unwrap();
            let msg = item.message();

            let position = list_item.position();
            let show_date_sep = if position == 0 {
                true
            } else {
                if let Some(prev_item_obj) = this.messages_store.item(position - 1) {
                    if let Ok(prev_item) =
                        prev_item_obj.downcast::<crate::ui::message_object::MessageObject>()
                    {
                        let prev_msg = prev_item.message();
                        let msg_local = msg.created.with_timezone(&chrono::Local);
                        let prev_local = prev_msg.created.with_timezone(&chrono::Local);
                        msg_local.date_naive() != prev_local.date_naive()
                    } else {
                        true
                    }
                } else {
                    true
                }
            };

            // Generate full widget and swap it into the child box
            if let Some(child) = list_item.child() {
                if let Ok(box_) = child.downcast::<GtkBox>() {
                    // Remove existing children
                    while let Some(c) = box_.first_child() {
                        box_.remove(&c);
                    }

                    let widget = {
                        let mut cache = this.message_rows.lock().unwrap();
                        let cache_key = (msg.id.clone(), show_date_sep);
                        let w = if let Some(cached_widget) = cache.get(&cache_key) {
                            cached_widget.clone()
                        } else {
                            let new_widget = this.create_message_row(&msg, show_date_sep);
                            cache.insert(cache_key, new_widget.clone());
                            new_widget
                        };
                        if let Some(parent) = w.parent() {
                            if let Ok(parent_box) = parent.downcast::<gtk::Box>() {
                                parent_box.remove(&w);
                            } else {
                                w.unparent();
                            }
                        }
                        w
                    };
                    box_.append(&widget);
                }
            }
        });
        self.message_list_view.set_factory(Some(&factory));
    }

    pub fn get_all_images(&self) -> Vec<(String, String)> {
        let mut all_images = Vec::new();
        for msg in self.messages.lock().unwrap().iter() {
            for m in &msg.media {
                if m.type_ == crate::models::MediaType::Image {
                    all_images.push((m.url.clone(), "image.jpg".to_string()));
                }
            }
        }
        all_images
    }

    pub fn render_messages(&self) {
        let messages = self.messages.lock().unwrap();
        log::info!("Rendering {} messages via ListView", messages.len());

        if messages.is_empty() {
            drop(messages);
            if self.current_chat_id().is_some() {
                self.show_empty_conversation();
            } else {
                self.show_welcome();
            }
            self.messages_store.remove_all();
            return;
        }

        self.show_messages_list();

        let mut sorted_messages = messages.clone();
        // Fix absurd timestamps before sort (ms/µs mix → 1970 or year 50k)
        let now = chrono::Utc::now();
        for m in &mut sorted_messages {
            let y: i32 = m.created.format("%Y").to_string().parse().unwrap_or(0);
            if y < 2000 || y > 2100 {
                m.created = now;
            }
        }
        sorted_messages.sort_by(|a, b| a.created.cmp(&b.created));

        let start_time = std::time::Instant::now();
        let new_objects: Vec<glib::Object> = sorted_messages
            .into_iter()
            .map(|msg| crate::ui::message_object::MessageObject::new(msg).upcast::<glib::Object>())
            .collect();

        let old_count = self.messages_store.n_items();
        self.messages_store.splice(0, old_count, &new_objects);
        eprintln!(
            "[PERF] render_messages splice of {} items took {:?}",
            new_objects.len(),
            start_time.elapsed()
        );

        // Always land on the newest messages when (re)binding a chat history.
        self.scroll_to_latest();
    }

    /// Scroll message list to the newest message (bottom).
    /// Marks stick-to-bottom and applies immediately + after async ListView measure.
    pub fn scroll_to_latest(&self) {
        let n = self.messages_store.n_items();
        if n == 0 {
            return;
        }
        *self.stick_to_bottom.lock().unwrap() = true;
        self.apply_scroll_to_bottom();

        // ListView measures after splice — keep forcing bottom until layout settles
        let this_scrolled = self.scrolled.clone();
        let this_list = self.message_list_view.clone();
        let last = n.saturating_sub(1);
        for delay_ms in [1u32, 16, 32, 50, 80, 120, 200, 350, 500, 800, 1200] {
            let list = this_list.clone();
            let scrolled = this_scrolled.clone();
            glib::timeout_add_local_once(
                std::time::Duration::from_millis(delay_ms as u64),
                move || {
                    list.scroll_to(last, gtk::ListScrollFlags::NONE, None);
                    let adj = scrolled.vadjustment();
                    let target = (adj.upper() - adj.page_size()).max(0.0);
                    if target > 0.0 {
                        adj.set_value(target);
                    }
                },
            );
        }
    }

    fn apply_scroll_to_bottom(&self) {
        let n = self.messages_store.n_items();
        if n == 0 {
            return;
        }
        let last = n.saturating_sub(1);
        self.message_list_view
            .scroll_to(last, gtk::ListScrollFlags::NONE, None);
        let adj = self.scrolled.vadjustment();
        let target = (adj.upper() - adj.page_size()).max(0.0);
        if target > 0.0 || adj.upper() > adj.page_size() {
            adj.set_value(target);
        }
    }

    /// While stick_to_bottom is set, re-pin to the end whenever ListView height grows.
    fn setup_stick_to_bottom(self: &Arc<Self>) {
        let this = self.clone();
        let adj = self.scrolled.vadjustment();
        adj.connect_changed(move |adj| {
            if !*this.stick_to_bottom.lock().unwrap() {
                return;
            }
            // Don't fight pagination (user scrolled near top)
            if adj.value() < 120.0 && adj.upper() > adj.page_size() + 200.0 {
                // Still opening: if far from bottom, force bottom
            }
            let target = (adj.upper() - adj.page_size()).max(0.0);
            if (adj.value() - target).abs() > 1.0 {
                adj.set_value(target);
            }
            // Clear stick after we actually reached the bottom with real content height
            if target > 1.0 && (adj.value() - target).abs() <= 2.0 {
                // Keep stick for a short while more via scroll_to_latest timeouts;
                // clear only when user scrolls away (handled in value_changed).
            }
        });

        let this2 = self.clone();
        adj.connect_value_changed(move |adj| {
            if !*this2.stick_to_bottom.lock().unwrap() {
                return;
            }
            let target = (adj.upper() - adj.page_size()).max(0.0);
            // User scrolled up intentionally → release stick
            if target > 50.0 && adj.value() < target - 80.0 {
                *this2.stick_to_bottom.lock().unwrap() = false;
            }
        });
    }

    pub fn create_message_row(self: &Arc<Self>, msg: &Message, show_date_sep: bool) -> GtkBox {
        let current_user_id = self.auth.user_id();
        let text = msg
            .text
            .as_ref()
            .map(|t| t.trim())
            .filter(|t| !t.is_empty())
            .map(|t| t.to_string())
            .unwrap_or_else(|| msg.preview());
        let query = self.search_query.lock().unwrap().to_lowercase();

        let formatted_text = format_message_text(&text);

        let display_text = if !query.is_empty() {
            let escaped_query = glib::markup_escape_text(&query).to_string();
            let re = regex::RegexBuilder::new(&regex::escape(&escaped_query))
                .case_insensitive(true)
                .build()
                .unwrap();
            // Use dark-mode friendly highlight color: brand yellow background with dark text
            re.replace_all(&formatted_text, "<span background='#ffdc60' color='#2a2a3f' style='border-radius: 2px; padding: 0 1px;'>$0</span>").to_string()
        } else {
            formatted_text
        };

        let mut sticker_url = None;
        let mut sticker_payload = None;
        if let Some(ref text_val) = msg.text {
            #[derive(serde::Deserialize)]
            struct StickerPayload {
                sticker_id: String,
                #[serde(default)]
                pack_id: String,
            }
            if let Ok(parsed) = serde_json::from_str::<StickerPayload>(text_val) {
                sticker_payload = Some(parsed);
            }
        }
        let looks_like_sticker_path = msg
            .text
            .as_deref()
            .map(|t| t.starts_with("stickers/") || t.contains("/stickers/"))
            .unwrap_or(false);
        let is_sticker = msg.type_ == MessageType::Sticker
            || sticker_payload.is_some()
            || looks_like_sticker_path;
        if is_sticker {
            if let Some(ref payload) = sticker_payload {
                let packs = self.sticker_packs.lock().unwrap();
                for pack in packs.iter() {
                    if payload.pack_id.is_empty() || pack.pack_id == payload.pack_id {
                        for sticker in &pack.stickers {
                            if sticker.sticker_id == payload.sticker_id {
                                sticker_url = Some(sticker.file_url.clone());
                                break;
                            }
                        }
                    }
                    if sticker_url.is_some() {
                        break;
                    }
                }
                // CDN direct URL when pack metadata not loaded yet
                if sticker_url.is_none() && !payload.sticker_id.is_empty() {
                    sticker_url = Some(format!(
                        "{}/{}?size=large",
                        crate::config::FILE_PUBLIC_HOST,
                        payload.sticker_id.trim_start_matches('/')
                    ));
                }
            }
            if sticker_url.is_none() {
                if let Some(ref text_val) = msg.text {
                    let packs = self.sticker_packs.lock().unwrap();
                    'outer: for pack in packs.iter() {
                        for sticker in &pack.stickers {
                            if sticker.sticker_id == *text_val {
                                sticker_url = Some(sticker.file_url.clone());
                                break 'outer;
                            }
                        }
                    }
                    // Path-style sticker id from history
                    if sticker_url.is_none()
                        && (text_val.starts_with("stickers/")
                            || text_val.contains("stickers/images/"))
                    {
                        let id = text_val.trim_start_matches('/');
                        sticker_url = Some(format!(
                            "{}/{}?size=large",
                            crate::config::FILE_PUBLIC_HOST,
                            id
                        ));
                    }
                }
            }
        }

        let is_bot_msg = self
            .bot_info
            .lock()
            .unwrap()
            .as_ref()
            .map(|b| b.bot_id == msg.from_id)
            .unwrap_or(false);

        let is_sent = if let Some(ref uid) = current_user_id {
            msg.from_id == *uid
        } else {
            false
        };

        let row = GtkBox::new(Orientation::Horizontal, 0);
        row.set_hexpand(true);
        row.set_halign(Align::Fill);

        let bubble = GtkBox::new(Orientation::Vertical, 4);
        bubble.add_css_class(if is_sent {
            "bubble-sent"
        } else {
            "bubble-received"
        });
        bubble.add_css_class("message-bubble");
        if is_bot_msg {
            bubble.add_css_class("bot-message");
        }

        let right_click_gesture = gtk::GestureClick::new();
        right_click_gesture.set_button(3); // Right click
        let msg_clone2 = msg.clone();

        let reply_box = self.reply_preview_box.clone();
        let reply_label = self.reply_preview_label.clone();
        let input_entry = self.input_entry.clone();
        let reply_to_id = self.reply_to_msg_id.clone();
        let edit_id = self.edit_msg_id.clone();
        let is_sent_msg = is_sent;

        let undo_bar_del_outer = self.undo_bar.clone();
        let pending_msg_del_outer = self.pending_delete_msg_id.clone();
        let pending_row_del_outer = self.pending_delete_row.clone();
        let row_del_outer = row.clone();

        let pinned_box_outer = self.pinned_box.clone();
        let pinned_label_outer = self.pinned_label.clone();
        let pinned_id_outer = self.pinned_message_id.clone();
        let on_thread_open_outer = self.on_thread_open.clone();
        let on_save_outer = self.on_save.clone();
        let on_delete_outer = self.on_delete.clone();

        let view_for_menu = self.clone();
        let bubble_for_menu = bubble.clone();
        // Proper weak widget ref: the temporary-Arc downgrade used before
        // always failed, leaving the popover parentless (menu never shown).
        let bubble_weak = bubble.downgrade();
        right_click_gesture.connect_pressed(move |gesture, _n_press, x, y| {
            let _ = gesture;
            let _ = x;
            let _ = y;
            let _ = _n_press;
            let popover = gtk::Popover::builder()
                .has_arrow(false)
                .autohide(true)
                .build();
            let Some(bubble_strong): Option<GtkBox> = bubble_weak.upgrade() else {
                return;
            };
            popover.set_parent(&bubble_strong);

            let vbox = gtk::Box::new(gtk::Orientation::Vertical, 2);
            vbox.add_css_class("message-context-menu");

            let btn_reply = gtk::Button::with_label("Ответить");
            let msg_id_reply = msg_clone2.id.clone();
            let msg_text_reply = msg_clone2.text.clone().unwrap_or_default();
            let popover_clone_reply = popover.clone();
            let reply_box_clone = reply_box.clone();
            let reply_label_clone = reply_label.clone();
            let input_entry_clone = input_entry.clone();
            let reply_to_id_clone = reply_to_id.clone();
            let edit_id_clone = edit_id.clone();

            btn_reply.connect_clicked(move |_| {
                reply_label_clone.set_label(&format!("Ответ: {}", msg_text_reply));
                reply_box_clone.set_visible(true);
                *reply_to_id_clone.lock().unwrap() = Some(msg_id_reply.clone());
                *edit_id_clone.lock().unwrap() = None;
                input_entry_clone.grab_focus();
                popover_clone_reply.popdown();
            });
            vbox.append(&btn_reply);

            let btn_reaction = gtk::Button::with_label("Реакция");
            let popover_clone_reaction = popover.clone();
            let msg_for_reaction = msg_clone2.clone();
            let view_for_reaction = view_for_menu.clone();
            let bubble_for_reaction = bubble_for_menu.clone();
            btn_reaction.connect_clicked(move |_| {
                view_for_reaction.show_reaction_picker(&msg_for_reaction, &bubble_for_reaction);
                popover_clone_reaction.popdown();
            });
            vbox.append(&btn_reaction);

            let btn_thread_reply = gtk::Button::with_label("Ответить в треде");
            let popover_clone_thread = popover.clone();
            let msg_id_thread = msg_clone2.id.clone();
            let chat_id_thread = msg_clone2.chat_id.clone();
            let on_thread_open_clone = on_thread_open_outer.clone();
            btn_thread_reply.connect_clicked(move |_| {
                if let Some(cb) = on_thread_open_clone.lock().unwrap().as_ref() {
                    cb(chat_id_thread.clone(), msg_id_thread.clone());
                }
                popover_clone_thread.popdown();
            });
            vbox.append(&btn_thread_reply);

            if is_sent_msg {
                let btn_edit = gtk::Button::with_label("Редактировать");
                let msg_id_edit = msg_clone2.id.clone();
                let msg_text_edit = msg_clone2.text.clone().unwrap_or_default();
                let popover_clone_edit = popover.clone();
                let reply_box_clone2 = reply_box.clone();
                let reply_label_clone2 = reply_label.clone();
                let input_entry_clone2 = input_entry.clone();
                let reply_to_id_clone2 = reply_to_id.clone();
                let edit_id_clone2 = edit_id.clone();

                btn_edit.connect_clicked(move |_| {
                    reply_label_clone2.set_label(&format!("Редактирование: {}", msg_text_edit));
                    reply_box_clone2.set_visible(true);
                    *edit_id_clone2.lock().unwrap() = Some(msg_id_edit.clone());
                    *reply_to_id_clone2.lock().unwrap() = None;
                    let buffer = input_entry_clone2.buffer();
                    buffer.set_text(&msg_text_edit);
                    input_entry_clone2.grab_focus();
                    let end_iter = buffer.end_iter();
                    buffer.place_cursor(&end_iter);
                    popover_clone_edit.popdown();
                });
                vbox.append(&btn_edit);
            }

            let btn_copy = gtk::Button::with_label("Копировать");
            let msg_text = msg_clone2.text.clone().unwrap_or_default();
            let popover_clone1 = popover.clone();
            btn_copy.connect_clicked(move |_| {
                if let Some(display) = gtk::gdk::Display::default() {
                    display.clipboard().set_text(&msg_text);
                }
                popover_clone1.popdown();
            });

            let btn_forward = gtk::Button::with_label("Переслать");
            let msg_id_fwd = msg_clone2.id.clone();
            let popover_clone2 = popover.clone();
            btn_forward.connect_clicked(move |_| {
                log::info!("Forward message: {}", msg_id_fwd);
                popover_clone2.popdown();
            });

            let btn_save = gtk::Button::with_label("Сохранить в Избранное");
            let msg_id_save = msg_clone2.id.clone();
            let chat_id_save = msg_clone2.chat_id.clone();
            let text_save = msg_clone2.text.clone().unwrap_or_default();
            let popover_clone_save = popover.clone();
            let on_save = on_save_outer.clone();
            btn_save.connect_clicked(move |button| {
                let _ = button;
                log::info!("Saving message {} to favorites", msg_id_save);
                if let Some(ref cb) = on_save.lock().unwrap().as_ref() {
                    cb(
                        chat_id_save.clone(),
                        msg_id_save.clone(),
                        Some(text_save.clone()),
                    );
                }
                popover_clone_save.popdown();
            });

            let btn_pin = gtk::Button::with_label("Закрепить");
            let msg_id_pin = msg_clone2.id.clone();
            let msg_text_pin = msg_clone2.text.clone().unwrap_or_default();
            let popover_clone_pin = popover.clone();
            let pinned_box_pin = pinned_box_outer.clone();
            let pinned_label_pin = pinned_label_outer.clone();
            let pinned_id_pin = pinned_id_outer.clone();

            btn_pin.connect_clicked(move |_| {
                *pinned_id_pin.lock().unwrap() = Some(msg_id_pin.clone());
                pinned_label_pin.set_label(&msg_text_pin);
                pinned_box_pin.set_visible(true);
                popover_clone_pin.popdown();
            });

            let btn_delete = gtk::Button::with_label("Удалить");
            let msg_id_del = msg_clone2.id.clone();
            let chat_id_del = msg_clone2.chat_id.clone();
            let on_delete_del = on_delete_outer.clone();
            let popover_clone3 = popover.clone();
            let undo_bar_del = undo_bar_del_outer.clone();
            let pending_msg_del = pending_msg_del_outer.clone();
            let pending_row_del = pending_row_del_outer.clone();
            let row_del = row_del_outer.clone();
            btn_delete.connect_clicked(move |_| {
                log::info!("Delete message initiated: {}", msg_id_del);

                // Hide the row
                row_del.set_visible(false);

                // Show undo bar
                undo_bar_del.set_visible(true);

                *pending_msg_del.lock().unwrap() = Some(msg_id_del.clone());
                *pending_row_del.lock().unwrap() = Some(row_del.clone());

                let msg_id_timeout = msg_id_del.clone();
                let chat_id_timeout = chat_id_del.clone();
                let undo_bar_timeout = undo_bar_del.clone();
                let pending_msg_timeout = pending_msg_del.clone();
                let pending_row_timeout = pending_row_del.clone();
                let on_delete_timeout = on_delete_del.clone();

                glib::timeout_add_local_once(std::time::Duration::from_secs(5), move || {
                    let current_pending = pending_msg_timeout.lock().unwrap().clone();
                    if current_pending.as_ref() == Some(&msg_id_timeout) {
                        // Undo window expired — confirm server-side delete.
                        undo_bar_timeout.set_visible(false);
                        *pending_msg_timeout.lock().unwrap() = None;
                        *pending_row_timeout.lock().unwrap() = None;
                        log::info!("Message {} delete confirmed", msg_id_timeout);
                        if let Some(cb) = on_delete_timeout.lock().unwrap().as_ref() {
                            cb(chat_id_timeout.clone(), msg_id_timeout.clone());
                        }
                    }
                });

                popover_clone3.popdown();
            });

            // Debug helper for broken attachments: copy media ids/urls +
            // resolved download candidates to the clipboard.
            if !msg_clone2.media.is_empty() {
                let btn_urls = gtk::Button::with_label("🔗 URL вложения");
                let media_dump = msg_clone2.media.clone();
                let popover_clone_urls = popover.clone();
                btn_urls.connect_clicked(move |_| {
                    let mut lines = Vec::new();
                    for m in &media_dump {
                        lines.push(format!(
                            "[{:?}] id={} url={} file={}",
                            m.type_,
                            m.id,
                            m.url,
                            m.filename.clone().unwrap_or_default()
                        ));
                        for cand in candidate_image_urls(&m.url, Some(&m.id)) {
                            lines.push(format!("  -> {}", cand));
                        }
                    }
                    let dump = lines.join("\n");
                    log::info!("Attachment URLs:\n{}", dump);
                    if let Some(display) = gtk::gdk::Display::default() {
                        display.clipboard().set_text(&dump);
                    }
                    popover_clone_urls.popdown();
                });
                vbox.append(&btn_urls);
            }

            vbox.append(&btn_copy);
            vbox.append(&btn_forward);
            vbox.append(&btn_save);
            vbox.append(&btn_pin);
            vbox.append(&btn_delete);

            popover.set_child(Some(&vbox));
            popover.set_has_arrow(false);
            let rect = gtk::gdk::Rectangle::new(x as i32, y as i32, 1, 1);
            popover.set_pointing_to(Some(&rect));
            popover.popup();
            gesture.set_state(gtk::EventSequenceState::Claimed);
        });
        bubble.add_controller(right_click_gesture);

        // Inline image preview
        let has_image = msg
            .media
            .iter()
            .any(|m| m.type_ == crate::models::MediaType::Image);
        if has_image {
            let image_media = msg
                .media
                .iter()
                .filter(|m| m.type_ == crate::models::MediaType::Image)
                .next();

            if let Some(media) = image_media {
                let url = &media.url;
                let img = gtk::Image::new();
                img.set_css_classes(&["inline-image"]);
                img.set_margin_bottom(4);
                // Prefer max size via CSS/pixel; avoid huge min-width on the row
                img.set_pixel_size(240);
                img.set_size_request(-1, -1);

                // Show loading placeholder (or thumbnail if available, but for now we just use a placeholder icon)
                img.set_from_icon_name(Some("image-x-generic-symbolic"));

                // Load image asynchronously (downscaled + auth — never full 4K on UI thread)
                let img_clone = img.clone();
                let url_clone = url.clone();
                let thumb_url = media.thumbnail_url.clone();
                let file_id = media.id.clone();

                glib::spawn_future_local(async move {
                    // Local / quick thumb first (outgoing pastes)
                    if let Some(t_url) = thumb_url {
                        if load_inline_image(&img_clone, &t_url, None).await.is_ok() {
                            // still try remote for better quality later only if not local
                            if t_url.starts_with("file:") || t_url.starts_with('/') {
                                return;
                            }
                        }
                    }

                    if let Err(e) = load_inline_image(&img_clone, &url_clone, Some(&file_id)).await
                    {
                        log::warn!("Failed to load inline image: {}", e);
                    }
                });

                let url2 = url.clone();
                let on_image_open = self.on_image_open.clone();
                let view_clone = self.clone();
                let gesture = gtk::GestureClick::new();
                gesture.connect_pressed(move |_gesture, _n_press, _x, _y| {
                    log::info!("Opening image: {}", url2);
                    if let Some(cb) = on_image_open.lock().unwrap().as_ref() {
                        cb(
                            url2.clone(),
                            "image.jpg".to_string(),
                            view_clone.get_all_images(),
                        );
                    }
                });
                img.add_controller(gesture);

                bubble.append(&img);
            }
        }

        // Inline video preview
        let has_video = msg
            .media
            .iter()
            .any(|m| m.type_ == crate::models::MediaType::Video);
        if has_video {
            let video_url = msg
                .media
                .iter()
                .filter(|m| m.type_ == crate::models::MediaType::Video)
                .map(|m| m.url.clone())
                .next();

            if let Some(url) = video_url {
                let video_box = gtk::Box::new(Orientation::Vertical, 4);
                video_box.set_css_classes(&["inline-video"]);
                video_box.set_margin_bottom(4);
                video_box.set_size_request(-1, -1);
                video_box.set_hexpand(false);

                // Video thumbnail
                let thumbnail = gtk::Image::new();
                thumbnail.set_css_classes(&["inline-video-thumbnail"]);
                thumbnail.set_margin_bottom(4);

                // Play button overlay
                let play_overlay = gtk::Image::new();
                play_overlay.set_from_icon_name(Some("media-playback-start-symbolic"));
                play_overlay.set_css_classes(&["icon-btn", "circular"]);
                play_overlay.set_size_request(48, 48);
                play_overlay.set_halign(Align::Center);
                play_overlay.set_valign(Align::Center);
                play_overlay.set_css_classes(&["video-play-btn"]);

                let play_overlay_clone = Arc::new(play_overlay.clone());
                play_overlay_clone.set_visible(false);
                let play_overlay_ref = play_overlay_clone.as_ref().clone();
                video_box.append(&play_overlay_ref);

                // Hover to show play button
                let hover_in = gtk::EventControllerMotion::new();
                let hover_overlay = play_overlay_clone.clone();
                hover_in.connect_enter(move |_, _, _| {
                    hover_overlay.set_visible(true);
                });
                video_box.add_controller(hover_in);

                let hover_out = gtk::EventControllerMotion::new();
                let hover_overlay2 = play_overlay_clone.clone();
                hover_out.connect_leave(move |_| {
                    hover_overlay2.set_visible(false);
                });
                video_box.add_controller(hover_out);

                // Click swaps the preview for a real inline player.
                let gesture = gtk::GestureClick::new();
                let video_box_player = video_box.clone();
                gesture.connect_pressed(move |_gesture, _n_press, _x, _y| {
                    while let Some(child) = video_box_player.first_child() {
                        video_box_player.remove(&child);
                    }
                    let player = crate::ui::video_player::VideoPlayer::new();
                    video_box_player.append(player.container());
                    player.open_url(&url);
                });
                video_box.add_controller(gesture);

                // Duration label (if available)
                let duration = msg
                    .media
                    .iter()
                    .find(|m| m.type_ == crate::models::MediaType::Video)
                    .and_then(|m| m.duration)
                    .map(Self::format_duration);

                let duration_label = Label::builder()
                    .label(duration.as_deref().unwrap_or("▶"))
                    .css_classes(vec!["inline-video-duration"])
                    .xalign(0.0)
                    .build();

                video_box.append(&thumbnail);
                video_box.append(&play_overlay);
                video_box.append(&duration_label);
                bubble.append(&video_box);
            }
        }

        // Document / generic file attachments
        for media in msg.media.iter().filter(|m| {
            matches!(
                m.type_,
                crate::models::MediaType::Document
                    | crate::models::MediaType::Unknown
                    | crate::models::MediaType::Audio
            ) || (m.filename.is_some()
                && !matches!(
                    m.type_,
                    crate::models::MediaType::Image
                        | crate::models::MediaType::Video
                        | crate::models::MediaType::Voice
                        | crate::models::MediaType::Sticker
                        | crate::models::MediaType::AnimatedEmoji
                ))
        }) {
            bubble.append(&self.build_file_attachment_row(media));
        }

        // Also MessageType::File without typed media
        if msg.type_ == MessageType::File
            && msg.media.is_empty()
            && msg.text.as_ref().map(|t| t.contains('[')).unwrap_or(false)
        {
            // Fake media from text like "[Файл: name]"
            let name = msg
                .text
                .as_deref()
                .unwrap_or("file")
                .trim_start_matches("[Файл: ")
                .trim_end_matches(']')
                .to_string();
            let fake = crate::models::MediaAttachment {
                id: msg.id.clone(),
                type_: crate::models::MediaType::Document,
                url: String::new(),
                thumbnail_url: None,
                width: None,
                height: None,
                size: None,
                duration: None,
                filename: Some(name),
                mime_type: None,
                waveform: None,
                transcription: None,
            };
            bubble.append(&self.build_file_attachment_row(&fake));
        }

        // Voice message
        if crate::config::ym_enable_voice() {
            if let Some(voice_media) = msg
                .media
                .iter()
                .find(|m| m.type_ == crate::models::MediaType::Voice)
            {
                let duration = voice_media.duration.unwrap_or(0) as f64;
                let waveform = voice_media.waveform.clone().unwrap_or_default();
                // Server-side recognition arrives with the attachment.
                let transcribed = voice_media
                    .transcription
                    .clone()
                    .filter(|t| !t.trim().is_empty());

                let voice_msg = crate::models::VoiceMessage {
                    message_id: msg.id.clone(),
                    url: voice_media.url.clone(),
                    duration,
                    waveform,
                    transcribed_text: transcribed,
                    is_transcribing: false,
                    transcribe_error: None,
                };

                let player = crate::ui::voice_message_player::VoiceMessagePlayer::new(voice_msg);

                // Set up play callback (stub implementation just logs, could trigger playback)
                player.on_play_click(move |id| {
                    log::info!("Play voice message clicked: {}", id);
                    // Actual GStreamer playback integration would go here
                });

                bubble.append(player.container());
            }
        }

        if is_sticker {
            bubble.add_css_class("bubble-sticker");
            if let Some(url) = sticker_url {
                let img = gtk::Image::new();
                img.add_css_class("sticker-message-image");
                // Constrain size in code (GTK CSS has no max-width/max-height)
                img.set_pixel_size(128);
                img.set_size_request(128, 128);
                img.set_halign(Align::Center);

                let img_clone = img.clone();
                let url_clone = url.clone();
                glib::spawn_future_local(async move {
                    if let Err(e) = load_inline_image(&img_clone, &url_clone, None).await {
                        log::warn!("Failed to load sticker image: {}", e);
                    }
                });
                bubble.append(&img);
            } else {
                let label = Label::builder()
                    .label("Стикер")
                    .xalign(0.0)
                    .max_width_chars(32)
                    .build();
                bubble.append(&label);
            }
        } else {
            let is_emoji = if let Some(ref text_val) = msg.text {
                is_emoji_only(text_val)
            } else {
                false
            };

            if is_emoji {
                bubble.add_css_class("bubble-emoji-only");
            }

            // Pure visual messages (photo/video/voice with no text) already
            // render their own preview/player above — a redundant "📷 Фото"
            // text label underneath only confuses. Captions still show.
            let text_empty = msg
                .text
                .as_ref()
                .map(|t| t.trim().is_empty())
                .unwrap_or(true);
            let has_visual = msg.media.iter().any(|m| {
                matches!(
                    m.type_,
                    crate::models::MediaType::Image
                        | crate::models::MediaType::Video
                        | crate::models::MediaType::Voice
                )
            });
            if text_empty && has_visual {
                bubble.add_css_class("bubble-media-only");
            } else {
                // max_width_chars prevents GtkBox height-for-width CRITICAL:
                // "minimum width of N, but minimum width for height of 1048576 is M"
                let label = Label::builder()
                    .label(&display_text)
                    .use_markup(true)
                    .wrap(true)
                    .wrap_mode(gtk::pango::WrapMode::WordChar)
                    .max_width_chars(if is_emoji { 16 } else { 42 })
                    .width_chars(1)
                    .xalign(0.0)
                    .hexpand(false)
                    .css_classes(vec!["message-text".to_string()])
                    .build();
                label.set_natural_wrap_mode(gtk::NaturalWrapMode::None);

                bubble.append(&label);
            }
        }

        // Bot badge indicator
        if is_bot_msg {
            if let Some(_bot) = self.bot_info.lock().unwrap().as_ref() {
                let bot_badge = Label::builder()
                    .label("BOT")
                    .css_classes(vec!["bot-badge-indicator".to_string()])
                    .build();
                bubble.append(&bot_badge);
            }
        }

        let time_str = msg
            .created
            .with_timezone(&chrono::Local)
            .format("%H:%M")
            .to_string();

        // Meta footer: time + delivery ticks (outgoing only)
        let meta = GtkBox::new(Orientation::Horizontal, 4);
        meta.set_halign(if is_sent { Align::End } else { Align::Start });
        meta.add_css_class("message-meta");
        if msg.edited {
            let edited = Label::builder()
                .label("изм.")
                .css_classes(vec!["message-edited".to_string()])
                .build();
            meta.append(&edited);
        }
        let time_label = Label::builder()
            .label(&time_str)
            .css_classes(vec!["message-time".to_string()])
            .build();
        meta.append(&time_label);
        if is_sent {
            let ticks = Label::builder()
                .use_markup(true)
                .label(&delivery_ticks_markup(msg.sent, msg.delivered, msg.read))
                .css_classes(vec![
                    "message-ticks".to_string(),
                    if msg.read {
                        "ticks-read".to_string()
                    } else if msg.delivered {
                        "ticks-delivered".to_string()
                    } else {
                        "ticks-pending".to_string()
                    },
                ])
                .build();
            meta.append(&ticks);
        }
        bubble.append(&meta);

        let double_click = gtk::GestureClick::new();
        let msg_for_picker = msg.clone();
        let view_for_picker = self.clone();
        let bubble_for_picker = bubble.clone();
        double_click.connect_pressed(move |gesture, n_press, _, _| {
            if n_press == 2 {
                view_for_picker.show_reaction_picker(&msg_for_picker, &bubble_for_picker);
                gesture.set_state(gtk::EventSequenceState::Claimed);
            }
        });
        bubble.add_controller(double_click);

        bubble.set_hexpand(false);
        bubble.set_vexpand(false);
        // Cap bubble natural width (~42 chars ≈ 420–480px) without invalid GTK CSS max-width
        bubble.set_size_request(-1, -1);
        bubble.add_css_class("message-fade-in");

        if is_sent {
            row.set_margin_start(72);
            row.set_margin_end(10);
            bubble.set_halign(Align::End);
        } else {
            row.set_margin_start(10);
            row.set_margin_end(72);
            bubble.set_halign(Align::Start);
        }
        row.set_margin_top(1);
        row.set_margin_bottom(1);
        row.append(&bubble);

        let main_box = GtkBox::new(Orientation::Vertical, 3);
        main_box.set_hexpand(true);
        // Prevent ListView measure from treating this as infinitely tall HFW pass
        main_box.set_vexpand(false);

        if !msg.reactions.is_empty() {
            let chips = self.create_reaction_chips_row(msg, is_sent, &bubble);
            main_box.append(&chips);
        }

        if show_date_sep {
            let date_str = format_date_separator(&msg.created);
            let sep_label = Label::builder()
                .label(&date_str)
                .css_classes(vec!["date-separator".to_string()])
                .halign(gtk::Align::Center)
                .build();
            main_box.append(&sep_label);
        }

        main_box.append(&row);
        main_box
    }

    // ── Scheduled message methods ──

    /// Открыть popover для планирования
    pub fn show_send_at_popover(&self) {
        if let Some(pop) = self.send_at_popover.lock().unwrap().as_ref() {
            if *self.popover_open.lock().unwrap() {
                pop.popover().popdown();
                *self.popover_open.lock().unwrap() = false;
            } else {
                if let Some(ref panel) = pop.scheduled_panel {
                    panel.update_messages(&self.get_scheduled_messages());
                }
                pop.popover().popup();
                *self.popover_open.lock().unwrap() = true;
            }
            return;
        }

        let popover = Popover::builder().has_arrow(false).autohide(true).build();
        popover.set_css_classes(&["send-at-popover"]);
        popover.set_parent(&self.attach_btn);
        popover.set_position(gtk::PositionType::Top);

        let container = GtkBox::new(Orientation::Vertical, 12);
        container.add_css_class("send-at-body");
        container.set_margin_top(4);
        container.set_margin_bottom(4);
        container.set_margin_start(4);
        container.set_margin_end(4);

        // Title
        let title = Label::builder()
            .label("Запланировать отправку")
            .xalign(0.0)
            .css_classes(vec!["title".to_string()])
            .build();

        // Calendar
        let calendar = Calendar::new();
        calendar.set_show_day_names(true);
        calendar.set_margin_bottom(4);

        // Time entry
        let time_entry = Entry::builder()
            .placeholder_text("Время (например, 14:30)")
            .hexpand(true)
            .build();
        time_entry.set_css_classes(&["scheduled-time-input"]);

        // Quick presets
        let quick_presets = MessageSchedule::quick_presets();
        let quick_box = GtkBox::new(Orientation::Horizontal, 6);
        quick_box.set_css_classes(&["quick-presets-row"]);

        let on_schedule_presets = self.on_schedule.clone();
        let input_entry_presets = self.input_entry.clone();
        let popover_presets = popover.clone();
        for (name, seconds) in &quick_presets {
            let btn = Button::builder()
                .label(name)
                .css_classes(vec!["quick-preset".to_string()])
                .hexpand(true)
                .build();

            let seconds_clone = *seconds;
            let chat_id_clone = self.chat.lock().unwrap().as_ref().map(|c| c.id.clone());
            let input_entry_presets_clone = input_entry_presets.clone();
            let on_schedule_presets_clone = on_schedule_presets.clone();
            let popover_presets_clone = popover_presets.clone();
            btn.connect_clicked(move |_| {
                popover_presets_clone.popdown();
                let buffer = input_entry_presets_clone.buffer();
                let (start, end) = buffer.bounds();
                let text = buffer.text(&start, &end, false).to_string();
                if let Some(chat_id) = &chat_id_clone {
                    if !text.is_empty() {
                        let utc_dt =
                            chrono::Utc::now() + chrono::Duration::seconds(seconds_clone as i64);
                        if let Some(cb) = &*on_schedule_presets_clone.lock().unwrap() {
                            cb(chat_id.clone(), text, utc_dt);
                        }
                        buffer.set_text("");
                    }
                }
            });

            quick_box.append(&btn);
        }

        // Confirm button
        let confirm_btn = Button::with_label("Подтвердить");
        confirm_btn.add_css_class("suggested-action");
        confirm_btn.set_hexpand(true);

        let cancel_btn = Button::with_label("Отмена");
        cancel_btn.add_css_class("flat");
        cancel_btn.set_hexpand(true);

        let buttons_box = GtkBox::new(Orientation::Horizontal, 8);
        buttons_box.append(&cancel_btn);
        buttons_box.append(&confirm_btn);

        container.append(&title);
        container.append(&calendar);
        container.append(&time_entry);
        container.append(&quick_box);
        container.append(&buttons_box);

        // Instantiate ScheduledPanel, set height request to 250, and append
        let scheduled_panel = std::rc::Rc::new(ScheduledPanel::new());
        scheduled_panel.container.set_height_request(250);
        scheduled_panel.update_messages(&self.get_scheduled_messages());
        container.append(&scheduled_panel.container);

        let chat_id_cancel = self.chat.lock().unwrap().as_ref().map(|c| c.id.clone());
        let on_cancel_schedule_clone = self.on_cancel_schedule.clone();
        let popover_cancel = popover.clone();
        scheduled_panel.connect_cancel(move |msg_id| {
            popover_cancel.popdown();
            if let Some(ref chat_id) = chat_id_cancel {
                if let Some(cb) = &*on_cancel_schedule_clone.lock().unwrap() {
                    cb(chat_id.clone(), msg_id);
                }
            }
        });

        popover.set_child(Some(&container));

        let popover_confirm = popover.clone();
        let calendar_confirm = calendar.clone();
        let time_entry_confirm = time_entry.clone();
        let input_entry_confirm = self.input_entry.clone();
        let on_schedule_confirm = self.on_schedule.clone();
        let chat_id_confirm = self.chat.lock().unwrap().as_ref().map(|c| c.id.clone());
        confirm_btn.connect_clicked(move |_| {
            popover_confirm.popdown();
            let buffer = input_entry_confirm.buffer();
            let (start, end) = buffer.bounds();
            let text = buffer.text(&start, &end, false).to_string();
            if let Some(chat_id) = &chat_id_confirm {
                if !text.is_empty() {
                    let glib_dt = calendar_confirm.date();
                    let year = glib_dt.year();
                    let month = glib_dt.month();
                    let day = glib_dt.day_of_month();

                    let time_str = time_entry_confirm.text().to_string();
                    let (hour, min) = if let Some((h_str, m_str)) = time_str.split_once(':') {
                        let h = h_str.trim().parse::<u32>().unwrap_or(12);
                        let m = m_str.trim().parse::<u32>().unwrap_or(0);
                        (h, m)
                    } else {
                        (12, 0)
                    };

                    if let Some(naive_date) =
                        chrono::NaiveDate::from_ymd_opt(year, month as u32, day as u32)
                    {
                        if let Some(naive_time) = chrono::NaiveTime::from_hms_opt(hour, min, 0) {
                            let naive_dt = chrono::NaiveDateTime::new(naive_date, naive_time);
                            let utc_dt =
                                chrono::TimeZone::from_utc_datetime(&chrono::Utc, &naive_dt);
                            if let Some(cb) = &*on_schedule_confirm.lock().unwrap() {
                                cb(chat_id.clone(), text, utc_dt);
                            }
                            buffer.set_text("");
                        }
                    }
                }
            }
        });

        let popover_clone = popover.clone();
        cancel_btn.connect_clicked(move |_| {
            popover_clone.popdown();
        });

        popover.popup();

        *self.send_at_popover.lock().unwrap() = Some(SendAtPopover::new_with_popover(
            popover,
            Some(scheduled_panel),
        ));
        *self.popover_open.lock().unwrap() = true;
        self.schedule_btn.add_css_class("active");
    }

    /// Получить callback для планирования (static helper)
    fn get_schedule_callback(chat_id: &str, text: &str, seconds: u64) -> Option<Box<dyn Fn()>> {
        let chat_id = chat_id.to_string();
        let text = text.to_string();
        Some(Box::new(move || {
            log::info!(
                "Scheduling message to chat {}: '{}', in {} seconds",
                chat_id,
                text,
                seconds
            );
        }))
    }

    /// Обновить список запланированных сообщений
    pub fn update_scheduled_messages(&self, messages: Vec<crate::models::ScheduledMessage>) {
        *self.scheduled_messages.lock().unwrap() = messages.clone();
        if let Some(pop) = self.send_at_popover.lock().unwrap().as_ref() {
            if let Some(ref panel) = pop.scheduled_panel {
                panel.update_messages(&messages);
            }
        }
    }

    /// Получить запланированные сообщения
    pub fn get_scheduled_messages(&self) -> Vec<crate::models::ScheduledMessage> {
        self.scheduled_messages.lock().unwrap().clone()
    }

    fn create_empty_state() -> GtkBox {
        let empty = GtkBox::new(Orientation::Vertical, 16);
        empty.set_halign(Align::Center);
        empty.set_valign(Align::Center);
        empty.set_vexpand(true);
        empty.set_hexpand(true);
        empty.add_css_class("empty-chat-state");

        let icon = gtk::Image::from_icon_name("user-available-symbolic");
        icon.set_pixel_size(48);
        icon.add_css_class("empty-chat-icon");
        icon.set_halign(Align::Center);

        let title = Label::builder()
            .label("Выберите чат")
            .xalign(0.5)
            .halign(Align::Center)
            .css_classes(vec!["empty-chat-title".to_string()])
            .build();

        let text = Label::builder()
            .label("Выберите диалог слева, чтобы начать общение")
            .xalign(0.5)
            .halign(Align::Center)
            .wrap(true)
            .max_width_chars(36)
            .width_chars(1)
            .css_classes(vec![
                "dim-label".to_string(),
                "empty-chat-subtitle".to_string(),
            ])
            .build();
        text.set_natural_wrap_mode(gtk::NaturalWrapMode::None);

        empty.append(&icon);
        empty.append(&title);
        empty.append(&text);
        empty
    }

    fn create_header() -> (GtkBox, Label, Label, Button, Button, Button) {
        let header = GtkBox::new(Orientation::Horizontal, 8);
        header.set_css_classes(&["chat-header"]);
        let title_box = GtkBox::new(Orientation::Vertical, 2);
        title_box.set_hexpand(true);
        title_box.set_vexpand(false);
        title_box.set_valign(gtk::Align::Center);
        let title_label = Label::builder()
            .label("")
            .xalign(0.0)
            .css_classes(vec!["chat-header-title".to_string()])
            .build();
        let status_label = Label::builder()
            .label("")
            .xalign(0.0)
            .css_classes(vec!["chat-status".to_string()])
            .build();
        title_box.append(&title_label);
        title_box.append(&status_label);

        let search_btn = Button::builder()
            .icon_name("system-search-symbolic")
            .build();
        search_btn.add_css_class("icon-btn");
        search_btn.add_css_class("header-search-btn");
        search_btn.set_valign(gtk::Align::Center);
        search_btn.set_margin_start(4);
        search_btn.set_margin_end(4);

        let call_btn = Button::builder().icon_name("call-start-symbolic").build();
        call_btn.add_css_class("icon-btn");
        call_btn.add_css_class("header-call-btn");
        call_btn.set_valign(gtk::Align::Center);
        call_btn.set_margin_start(4);
        call_btn.set_margin_end(4);
        call_btn.set_visible(false);

        let menu_btn = Button::builder().icon_name("view-more-symbolic").build();
        menu_btn.add_css_class("icon-btn");
        menu_btn.add_css_class("header-menu-btn");
        menu_btn.set_valign(gtk::Align::Center);
        menu_btn.set_margin_start(4);
        menu_btn.set_margin_end(4);

        header.append(&title_box);
        header.append(&search_btn);
        header.append(&call_btn);
        header.append(&menu_btn);
        (
            header,
            title_label,
            status_label,
            search_btn,
            call_btn,
            menu_btn,
        )
    }

    fn create_input() -> (
        GtkBox,
        TextView,
        Button,
        Button,
        Button,
        Button,
        Button,
        Button,
        Button,
    ) {
        // TG-like composer: [attach] [text expands full width] [emoji] [voice|send]
        // Extra actions (poll/schedule/sticker) live in attach menu — not in the bar.
        let input_area = GtkBox::new(Orientation::Horizontal, 6);
        input_area.set_hexpand(true);
        input_area.set_vexpand(false);
        input_area.set_halign(gtk::Align::Fill);
        input_area.add_css_class("message-input-area");

        let input_pill = GtkBox::new(Orientation::Horizontal, 4);
        input_pill.set_hexpand(true);
        input_pill.set_halign(gtk::Align::Fill);
        input_pill.add_css_class("input-pill");

        let text_view = TextView::builder()
            .hexpand(true)
            .vexpand(false)
            .halign(gtk::Align::Fill)
            .wrap_mode(gtk::WrapMode::WordChar)
            .accepts_tab(false)
            .left_margin(8)
            .right_margin(8)
            .top_margin(8)
            .bottom_margin(8)
            .build();
        text_view.add_css_class("message-entry-view");
        // Critical: TextView natural width is content-sized; force expand to allocated width
        text_view.set_pixels_above_lines(0);
        text_view.set_pixels_below_lines(0);

        let scrolled = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .propagate_natural_height(true)
            .propagate_natural_width(false) // don't shrink field to short text
            .max_content_height(120)
            .hexpand(true)
            .halign(gtk::Align::Fill)
            .vexpand(false)
            .build();
        scrolled.set_child(Some(&text_view));
        scrolled.set_min_content_width(120);

        let overlay = gtk::Overlay::builder()
            .hexpand(true)
            .halign(gtk::Align::Fill)
            .vexpand(false)
            .build();
        overlay.set_child(Some(&scrolled));
        overlay.set_overflow(gtk::Overflow::Hidden);

        let placeholder = gtk::Label::builder()
            .label("Введите сообщение...")
            .halign(gtk::Align::Start)
            .valign(gtk::Align::Center)
            .hexpand(true)
            .margin_start(12)
            .css_classes(vec!["placeholder-label".to_string()])
            .build();
        placeholder.set_can_target(false);
        overlay.add_overlay(&placeholder);

        placeholder.set_visible(true);
        let ph_clone = placeholder.clone();
        text_view.buffer().connect_changed(move |buf| {
            let (start, end) = buf.bounds();
            let text = buf.text(&start, &end, false);
            ph_clone.set_visible(text.is_empty());
        });

        let send_btn = Button::builder().icon_name("mail-send-symbolic").build();
        send_btn.add_css_class("icon-btn");
        send_btn.add_css_class("send-btn-premium");
        send_btn.set_hexpand(false);
        send_btn.set_halign(gtk::Align::Center);

        let voice_btn = Button::builder()
            .icon_name("audio-input-microphone-symbolic")
            .build();
        voice_btn.add_css_class("icon-btn");
        voice_btn.add_css_class("voice-btn-premium");
        voice_btn.set_hexpand(false);

        let attach_btn = Button::builder()
            .icon_name("mail-attachment-symbolic")
            .build();
        attach_btn.add_css_class("icon-btn");
        attach_btn.set_hexpand(false);
        let emoji_btn = Button::builder().icon_name("face-smile-symbolic").build();
        emoji_btn.add_css_class("icon-btn");
        emoji_btn.set_hexpand(false);

        // Hidden anchors for popovers (not in the bar — saves width for text)
        let sticker_btn = Button::builder().icon_name("face-cool-symbolic").build();
        sticker_btn.add_css_class("icon-btn");
        sticker_btn.set_visible(false);
        let poll_btn = Button::builder().icon_name("view-list-symbolic").build();
        poll_btn.add_css_class("icon-btn");
        poll_btn.set_visible(false);
        let schedule_btn = Button::builder().icon_name("clock-symbolic").build();
        schedule_btn.add_css_class("icon-btn");
        schedule_btn.add_css_class("send-schedule-btn");
        schedule_btn.set_visible(false);

        // Visible bar (Telegram-like)
        input_pill.append(&attach_btn);
        input_pill.append(&overlay); // expands
        input_pill.append(&emoji_btn);
        input_pill.append(&voice_btn);
        input_pill.append(&send_btn);
        // Keep anchors in tree for popovers/set_parent
        input_pill.append(&sticker_btn);
        input_pill.append(&poll_btn);
        input_pill.append(&schedule_btn);

        input_area.append(&input_pill);
        input_area.set_vexpand(false);
        input_area.set_valign(gtk::Align::End);

        (
            input_area,
            text_view,
            send_btn,
            voice_btn,
            attach_btn,
            emoji_btn,
            sticker_btn,
            poll_btn,
            schedule_btn,
        )
    }
}

/// Max side length for chat bubble previews (keeps UI from freezing on 4K pastes).
const INLINE_PREVIEW_MAX_SIDE: u32 = 480;
const INLINE_DOWNLOAD_MAX_BYTES: usize = 25 * 1024 * 1024;

/// Load OAuth + session cookies for files.messenger downloads (sync, cheap).
pub(crate) fn messenger_auth_for_fetch() -> (Option<String>, Option<String>) {
    let mut oauth = None;
    let mut cookie = None;
    if let Some(dir) = dirs::config_dir() {
        let base = dir.join("yandex-messenger-native");
        if let Ok(raw) = std::fs::read_to_string(base.join("token.json")) {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&raw) {
                if let Some(t) = v.get("access_token").and_then(|x| x.as_str()) {
                    if !t.is_empty() {
                        oauth = Some(if t.starts_with("OAuth ") {
                            t.to_string()
                        } else {
                            format!("OAuth {}", t)
                        });
                    }
                }
            }
        }
        if let Ok(raw) = std::fs::read_to_string(base.join("session.json")) {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&raw) {
                if let Some(map) = v.get("cookies").and_then(|c| c.as_object()) {
                    let h = map
                        .iter()
                        .filter_map(|(k, val)| val.as_str().map(|s| format!("{}={}", k, s)))
                        .collect::<Vec<_>>()
                        .join("; ");
                    if !h.is_empty() {
                        cookie = Some(h);
                    }
                }
            }
        }
    }
    (oauth, cookie)
}

/// Downscale image bytes for inline preview (runs off UI thread).
fn downscale_image_for_preview(bytes: &[u8], max_side: u32) -> Result<Vec<u8>, String> {
    use image::ImageReader;
    use std::io::Cursor;

    if bytes.is_empty() {
        return Err("empty image".into());
    }
    // Guard against multi‑MB screenshots OOMing the process (crash after paste)
    if bytes.len() > 40 * 1024 * 1024 {
        return Err(format!("image too large ({} bytes)", bytes.len()));
    }

    let mut reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| format!("image format: {}", e))?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(8192);
    limits.max_image_height = Some(8192);
    limits.max_alloc = Some(64 * 1024 * 1024);
    reader.limits(limits);
    let img = reader
        .decode()
        .map_err(|e| format!("image decode: {}", e))?;
    let (w, h) = (img.width(), img.height());
    let scaled = if w > max_side || h > max_side {
        img.thumbnail(max_side, max_side)
    } else {
        img
    };
    let mut out = Vec::new();
    {
        let mut cursor = Cursor::new(&mut out);
        // PNG is lossless and Texture-friendly; previews are small after thumbnail()
        scaled
            .write_to(&mut cursor, image::ImageFormat::Png)
            .map_err(|e| format!("encode preview: {}", e))?;
    }
    if out.is_empty() {
        return Err("empty preview png".into());
    }
    Ok(out)
}

fn candidate_image_urls(url: &str, file_id: Option<&str>) -> Vec<String> {
    let mut out = Vec::new();
    let push_unique = |v: &mut Vec<String>, s: String| {
        if !s.is_empty() && !v.iter().any(|x| x == &s) {
            v.push(s);
        }
    };

    if !url.is_empty() {
        push_unique(&mut out, url.to_string());
    }
    if let Some(id) = file_id.filter(|s| !s.is_empty() && !s.starts_with("http")) {
        let id = id.trim_start_matches('/');
        for host in [
            crate::config::FILE_PUBLIC_HOST,
            crate::config::FILE_PRIVATE_HOST,
        ] {
            let host = host.trim_end_matches('/');
            push_unique(&mut out, format!("{}/file_shortterm/{}", host, id));
            push_unique(&mut out, format!("{}/{}", host, id));
            // Some CDNs want stripped `file/` prefix
            if let Some(rest) = id.strip_prefix("file/") {
                push_unique(&mut out, format!("{}/file_shortterm/{}", host, rest));
                push_unique(&mut out, format!("{}/{}", host, rest));
            }
        }
    }
    out
}

/// Asynchronously load an inline image and set a **downscaled** texture on the widget.
/// Decode/scale runs on a worker thread — never full 4K on the GTK main loop.
async fn load_inline_image(
    img: &gtk::Image,
    url: &str,
    file_id: Option<&str>,
) -> Result<(), String> {
    let img_clone = img.clone();
    let urls = candidate_image_urls(url, file_id);
    if urls.is_empty() {
        return Err("no image url/id".into());
    }

    let (oauth, cookie) = messenger_auth_for_fetch();

    let download_handle = tokio::spawn(async move {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(12))
            .build()
            .map_err(|e| format!("client: {}", e))?;

        for u in urls {
            // Local file:// or absolute path (outgoing paste preview)
            if let Some(path) = local_path_from_url(&u) {
                let path2 = path.clone();
                let scaled = tokio::task::spawn_blocking(move || {
                    let bytes = std::fs::read(&path2).map_err(|e| e.to_string())?;
                    if bytes.is_empty() {
                        return Err("empty local file".into());
                    }
                    downscale_image_for_preview(&bytes, INLINE_PREVIEW_MAX_SIDE)
                })
                .await
                .map_err(|e| format!("join: {}", e))?;
                if let Ok(preview) = scaled {
                    return Ok(preview);
                }
                continue;
            }

            if !(u.starts_with("http://") || u.starts_with("https://")) {
                continue;
            }

            let mut req = client
                .get(&u)
                .header(
                    "User-Agent",
                    "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
                )
                .header("Origin", "https://yandex.ru")
                .header("Referer", "https://yandex.ru/chat");
            if let Some(ref a) = oauth {
                req = req.header("Authorization", a);
            }
            if let Some(ref c) = cookie {
                req = req.header("Cookie", c);
            }

            match req.send().await {
                Ok(resp) if resp.status().is_success() => {
                    let bytes = match resp.bytes().await {
                        Ok(b) => b,
                        Err(e) => {
                            log::debug!("inline image body {}: {}", u, e);
                            continue;
                        }
                    };
                    if bytes.is_empty() {
                        continue;
                    }
                    if bytes.len() > INLINE_DOWNLOAD_MAX_BYTES {
                        return Err(format!(
                            "image too large for preview ({} bytes)",
                            bytes.len()
                        ));
                    }
                    let raw = bytes.to_vec();
                    match tokio::task::spawn_blocking(move || {
                        downscale_image_for_preview(&raw, INLINE_PREVIEW_MAX_SIDE)
                    })
                    .await
                    {
                        Ok(Ok(preview)) => return Ok(preview),
                        Ok(Err(e)) => {
                            log::debug!("downscale failed for {}: {}", u, e);
                            continue;
                        }
                        Err(e) => {
                            log::debug!("downscale join {}: {}", u, e);
                            continue;
                        }
                    }
                }
                Ok(resp) => {
                    log::debug!("inline image HTTP {} for {}", resp.status(), u);
                }
                Err(e) => {
                    log::debug!("inline image fetch {}: {}", u, e);
                }
            }
        }
        Err("all image candidates failed".into())
    });

    match download_handle.await {
        Ok(Ok(preview_png)) => {
            let bytes_glib = glib::Bytes::from(&preview_png);
            match gtk::gdk::Texture::from_bytes(&bytes_glib) {
                Ok(texture) => {
                    img_clone.set_from_paintable(Some(&texture));
                    img_clone.set_pixel_size(240);
                    Ok(())
                }
                Err(e) => {
                    match load_texture_via_pixbuf_scaled(&preview_png, INLINE_PREVIEW_MAX_SIDE) {
                        Ok(texture) => {
                            img_clone.set_from_paintable(Some(&texture));
                            img_clone.set_pixel_size(240);
                            Ok(())
                        }
                        Err(e2) => Err(format!("texture: {}; pixbuf: {}", e, e2)),
                    }
                }
            }
        }
        Ok(Err(e)) => Err(e),
        Err(join_err) => Err(format!("Join error: {}", join_err)),
    }
}

fn local_path_from_url(url: &str) -> Option<std::path::PathBuf> {
    if url.starts_with("file:") {
        return glib::filename_from_uri(url).ok().map(|(p, _)| p);
    }
    if url.starts_with('/') {
        let p = std::path::PathBuf::from(url);
        if p.exists() {
            return Some(p);
        }
    }
    None
}

fn load_texture_via_pixbuf_scaled(
    bytes: &[u8],
    max_side: u32,
) -> Result<gtk::gdk::Texture, String> {
    let loader = gtk::gdk_pixbuf::PixbufLoader::new();
    loader
        .write(bytes)
        .map_err(|e| format!("pixbuf write: {}", e))?;
    loader.close().map_err(|e| format!("pixbuf close: {}", e))?;
    let pixbuf = loader
        .pixbuf()
        .ok_or_else(|| "pixbuf loader empty".to_string())?;
    let w = pixbuf.width() as u32;
    let h = pixbuf.height() as u32;
    let scaled = if w > max_side || h > max_side {
        let scale = (max_side as f64 / w.max(h) as f64).min(1.0);
        let nw = ((w as f64) * scale).round().max(1.0) as i32;
        let nh = ((h as f64) * scale).round().max(1.0) as i32;
        pixbuf
            .scale_simple(nw, nh, gtk::gdk_pixbuf::InterpType::Bilinear)
            .unwrap_or(pixbuf)
    } else {
        pixbuf
    };
    Ok(gtk::gdk::Texture::for_pixbuf(&scaled))
}

/// Menu row: [symbolic icon] label — no emoji chrome.
fn menu_row_button(icon_name: &str, label: &str) -> Button {
    let btn = Button::builder()
        .css_classes(vec!["flat".to_string(), "attach-menu-btn".to_string()])
        .halign(gtk::Align::Fill)
        .build();
    let row = GtkBox::new(Orientation::Horizontal, 10);
    row.set_margin_start(4);
    row.set_margin_end(8);
    let icon = gtk::Image::from_icon_name(icon_name);
    icon.set_pixel_size(18);
    icon.set_valign(gtk::Align::Center);
    let lab = Label::builder()
        .label(label)
        .xalign(0.0)
        .hexpand(true)
        .build();
    row.append(&icon);
    row.append(&lab);
    btn.set_child(Some(&row));
    btn
}

/// Read all bytes from a GIO InputStream (clipboard image payloads).
async fn read_gio_stream_bytes(stream: gio::InputStream) -> Result<Vec<u8>, String> {
    use gio::prelude::InputStreamExt;
    let mut out = Vec::new();
    loop {
        let chunk = stream
            .read_bytes_future(64 * 1024, glib::Priority::DEFAULT)
            .await
            .map_err(|e| format!("stream read: {}", e))?;
        if chunk.is_empty() {
            break;
        }
        out.extend_from_slice(&chunk);
        if out.len() > 40 * 1024 * 1024 {
            return Err("clipboard image too large".into());
        }
    }
    Ok(out)
}

/// Delivery ticks markup for outgoing messages.
/// pending · delivered ✓ · read ✓✓
fn delivery_ticks_markup(sent: bool, delivered: bool, read: bool) -> String {
    if read {
        "<span size='small'>✓✓</span>".to_string()
    } else if delivered || sent {
        "<span size='small'>✓</span>".to_string()
    } else {
        "<span size='small'>…</span>".to_string()
    }
}

fn format_file_size(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    let b = bytes as f64;
    if b >= MB {
        format!("{:.1} МБ", b / MB)
    } else if b >= KB {
        format!("{:.0} КБ", b / KB)
    } else {
        format!("{} Б", bytes)
    }
}

fn format_message_text(text: &str) -> String {
    let mut escaped = glib::markup_escape_text(text).to_string();

    static RE_LIST: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    static RE_BOLD: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    static RE_ITALIC: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    static RE_CODE_BLOCK: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    static RE_CODE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();

    let re_list = RE_LIST.get_or_init(|| regex::Regex::new(r"\s+(\d+)\.\s+\*\*").unwrap());
    escaped = re_list.replace_all(&escaped, "\n$1. **").to_string();

    // Bold
    let re_bold = RE_BOLD
        .get_or_init(|| regex::Regex::new(r"\*\*([^\*\s]|[^\*\s][^\*]*?[^\*\s])\*\*").unwrap());
    escaped = re_bold.replace_all(&escaped, "<b>$1</b>").to_string();

    // Italic
    let re_italic = RE_ITALIC
        .get_or_init(|| regex::Regex::new(r"\*([^\*\s]|[^\*\s][^\*]*?[^\*\s])\*").unwrap());
    escaped = re_italic.replace_all(&escaped, "<i>$1</i>").to_string();

    // Code block
    let re_code_block =
        RE_CODE_BLOCK.get_or_init(|| regex::Regex::new(r"```([\s\S]+?)```").unwrap());
    escaped = re_code_block
        .replace_all(&escaped, "<tt>$1</tt>")
        .to_string();

    // Inline code
    let re_code = RE_CODE.get_or_init(|| regex::Regex::new(r"`(.+?)`").unwrap());
    escaped = re_code.replace_all(&escaped, "<tt>$1</tt>").to_string();

    escaped
}

fn is_emoji(c: char) -> bool {
    match c {
        '\u{1F300}'..='\u{1F5FF}' | // Misc Symbols and Pictographs
        '\u{1F600}'..='\u{1F64F}' | // Emoticons
        '\u{1F680}'..='\u{1F6FF}' | // Transport and Map
        '\u{1F900}'..='\u{1F9FF}' | // Supplemental Symbols and Pictographs
        '\u{1FA70}'..='\u{1FAFF}' | // Symbols and Pictographs Extended-A
        '\u{2600}'..='\u{26FF}' |   // Misc Symbols
        '\u{2700}'..='\u{27BF}' |   // Dingbats
        '\u{1F1E6}'..='\u{1F1FF}' | // Regional Indicator Symbols (Flags)
        '\u{FE00}'..='\u{FE0F}' |   // Variation Selectors
        '\u{200D}' => true,         // Zero Width Joiner
        _ => false,
    }
}

fn is_emoji_only(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return false;
    }

    // Все символы должны быть эмодзи
    for c in trimmed.chars() {
        if !is_emoji(c) {
            return false;
        }
    }

    // Считаем количество эмодзи
    let mut count = 0;
    let mut chars = trimmed.chars().peekable();
    while let Some(c) = chars.next() {
        // Пропускаем вспомогательные символы
        if c == '\u{200D}'
            || (c >= '\u{FE00}' && c <= '\u{FE0F}')
            || (c >= '\u{1F3FB}' && c <= '\u{1F3FF}')
        {
            continue;
        }
        // Если это региональный индикатор (часть флага), то пара считается за один
        if c >= '\u{1F1E6}' && c <= '\u{1F1FF}' {
            if let Some(&next_c) = chars.peek() {
                if next_c >= '\u{1F1E6}' && next_c <= '\u{1F1FF}' {
                    chars.next(); // поглощаем второй символ флага
                }
            }
            count += 1;
        } else {
            count += 1;
        }
    }

    count >= 1 && count <= 3
}

fn format_date_separator(dt: &chrono::DateTime<chrono::Utc>) -> String {
    use chrono::Datelike;
    let local_dt = dt.with_timezone(&chrono::Local);
    let now = chrono::Local::now();

    if local_dt.date_naive() == now.date_naive() {
        return "Сегодня".to_string();
    } else if local_dt.date_naive() == now.date_naive() - chrono::Duration::days(1) {
        return "Вчера".to_string();
    }

    let months = [
        "января",
        "февраля",
        "марта",
        "апреля",
        "мая",
        "июня",
        "июля",
        "августа",
        "сентября",
        "октября",
        "ноября",
        "декабря",
    ];
    let month_idx = (local_dt.month() as usize).saturating_sub(1);
    let month_name = months.get(month_idx).unwrap_or(&"");

    if local_dt.year() == now.year() {
        format!("{} {}", local_dt.day(), month_name)
    } else {
        format!("{} {} {}", local_dt.day(), month_name, local_dt.year())
    }
}

