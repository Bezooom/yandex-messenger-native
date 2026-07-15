use std::sync::{Arc, Mutex};

use adw::prelude::*;
use gtk::prelude::*;
use gtk::Orientation;
use libadwaita as adw;

use crate::api::auth::AuthManager;
use crate::core::AppController;
use crate::ui::AuthDialog;

mod api;
mod config;
mod core;
mod models;
mod ui;

fn main() -> glib::ExitCode {
    env_logger::init();
    adw::init().expect("Failed to initialize Libadwaita");

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Failed to build Tokio runtime");
    let _guard = rt.enter();

    let app = adw::Application::builder()
        .application_id(config::DESKTOP_APP_ID)
        .build();

    app.connect_activate(|app| {
        run(app);
    });

    app.run()
}

fn run(app: &adw::Application) {
    let auth = match AuthManager::new() {
        Ok(a) => Arc::new(a),
        Err(e) => {
            eprintln!("Failed to create AuthManager: {}", e);
            return;
        }
    };

    let rt = tokio::runtime::Handle::current();
    let is_authenticated = rt.block_on(auth.is_authenticated());

    if !is_authenticated {
        let auth_win = adw::ApplicationWindow::builder()
            .application(app)
            .title("Авторизация — Yandex Messenger")
            .default_width(420)
            .default_height(500)
            .build();

        auth_win.present();
        let auth_dialog = AuthDialog::new(&auth_win, auth.clone(), rt.clone());
        if let Err(e) = auth_dialog.authenticate_with_selection() {
            eprintln!("Auth failed: {}", e);
            return;
        }

        if !rt.block_on(auth.is_authenticated()) {
            eprintln!("Auth succeeded but no token");
            auth_win.close();
            return;
        }

        auth_win.close();
    }

    let token = match rt.block_on(auth.get_token()) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("Failed to get token: {}", e);
            return;
        }
    };

    start_main_window(app, auth, token.access_token);
}

fn start_main_window(app: &adw::Application, auth: Arc<AuthManager>, access_token: String) {
    let controller = Arc::new(AppController::new(auth.clone(), access_token));

    // Load and apply the premium theme CSS
    load_theme_css();

    let win = adw::ApplicationWindow::builder()
        .application(app)
        .title("Yandex Messenger")
        .default_width(1100)
        .default_height(700)
        .build();
    win.set_icon_name(Some("yandex-messenger"));

    // Initialize System Tray
    let _tray = ui::tray::TrayHandle::init();

    let root = create_app_layout(app, &win, controller);
    win.set_content(Some(&root));
    win.present();
}

fn load_theme_css() {
    let provider = gtk::CssProvider::new();
    let css_bytes = include_str!("ui/theme.css");
    provider.load_from_string(css_bytes);
    if let Some(display) = gtk::gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}

fn create_app_layout(
    app: &adw::Application,
    win: &adw::ApplicationWindow,
    controller: Arc<AppController>,
) -> gtk::Box {
    let overlay = gtk::Overlay::new();

    // ── Root: Horizontal split (draggable split pane) ──
    let root = gtk::Paned::builder()
        .orientation(Orientation::Horizontal)
        .position(280)
        .hexpand(true)
        .vexpand(true)
        .build();

    let sidebar_stack = gtk::Stack::builder()
        .transition_type(gtk::StackTransitionType::SlideLeftRight)
        .hexpand(true)
        .vexpand(true)
        .width_request(260)
        .build();
    sidebar_stack.add_css_class("sidebar-stack");

    let sidebar_stack_clone = sidebar_stack.clone();
    root.connect_position_notify(move |paned| {
        let pos = paned.position();
        if pos < 180 {
            if !sidebar_stack_clone.has_css_class("compact") {
                sidebar_stack_clone.add_css_class("compact");
            }
        } else {
            if sidebar_stack_clone.has_css_class("compact") {
                sidebar_stack_clone.remove_css_class("compact");
            }
        }
    });

    // ── Sidebar (chat list) ──
    let chat_list = Arc::new(Mutex::new(ui::ChatListPanel::new(
        controller.auth().clone(),
    )));

    let saved_panel = Arc::new(ui::saved_panel::SavedPanel::new(controller.auth().clone()));

    sidebar_stack.add_named(chat_list.lock().unwrap().container(), Some("chat_list"));
    sidebar_stack.add_named(saved_panel.container(), Some("saved_panel"));
    root.set_start_child(Some(&sidebar_stack));

    // ── Chat view (message area) ──
    let chat_view = Arc::new(ui::ChatView::new(controller.auth().clone()));
    let cv_container = chat_view.container().clone();
    cv_container.set_hexpand(true);
    cv_container.set_vexpand(true);
    root.set_end_child(Some(&cv_container));

    // ── Init ChatView interactive components (requires Arc) ──
    chat_view.clone().init_factory();
    chat_view.clone().init_callbacks();
    chat_view.clone().init_emoji_picker();
    chat_view.clone().init_sticker_panel();
    chat_view.clone().init_poll_creator();

    // ── Saved Panel Click & Unsave ──
    let sidebar_stack_clone = sidebar_stack.clone();
    saved_panel.connect_back_clicked(move || {
        sidebar_stack_clone.set_visible_child_name("chat_list");
    });

    let sidebar_stack_clone2 = sidebar_stack.clone();
    let chat_list_clone = chat_list.clone();
    saved_panel.on_message_click(move |chat_id, _msg_id| {
        chat_list_clone.lock().unwrap().select_chat(&chat_id);
        sidebar_stack_clone2.set_visible_child_name("chat_list");
    });

    let ctrl_unsave = controller.clone();
    let saved_panel_clone = saved_panel.clone();
    saved_panel.on_message_unsave(move |msg_id| {
        let ctrl = ctrl_unsave.clone();
        let sp = saved_panel_clone.clone();
        glib::spawn_future_local(async move {
            match ctrl.unsave_message(&msg_id).await {
                Ok(_) => {
                    if let Ok(msgs) = ctrl.get_saved_messages(50, 0).await {
                        sp.set_messages(msgs);
                    }
                }
                Err(e) => {
                    eprintln!("Failed to unsave message: {}", e);
                }
            }
        });
    });

    let ctrl_settings = controller.clone();
    let saved_panel_clone2 = saved_panel.clone();
    let sidebar_stack_clone3 = sidebar_stack.clone();
    chat_list.lock().unwrap().connect_settings(move || {
        let ctrl = ctrl_settings.clone();
        let sp = saved_panel_clone2.clone();
        let stack = sidebar_stack_clone3.clone();
        glib::spawn_future_local(async move {
            match ctrl.get_saved_messages(50, 0).await {
                Ok(msgs) => {
                    sp.set_messages(msgs);
                    stack.set_visible_child_name("saved_panel");
                }
                Err(e) => {
                    eprintln!("Failed to load saved messages: {}", e);
                }
            }
        });
    });

    // ── Wire: Send message ──
    let ctrl_send = controller.clone();
    let cv_for_send = chat_view.clone();
    let ctrl_call = controller.clone();
    let app_clone = app.clone();
    chat_view.bind_callbacks(
        move |chat_id: String, text: String| {
            let ctrl = ctrl_send.clone();
            let cv = cv_for_send.clone();
            glib::spawn_future_local(async move {
                match ctrl.send_text_message(&chat_id, &text).await {
                    Ok(msg) => {
                        cv.add_message(msg);
                    }
                    Err(e) => {
                        eprintln!("Failed to send message: {}", e);
                        cv.show_error(&e.to_string());
                    }
                }
            });
        },
        move |chat_id: String, _bytes: Vec<u8>, filename: String| {
            log::info!("Attach file {} to chat {}", filename, chat_id);
        },
        move |chat_id: String| {
            let call_url = ctrl_call.telemost_url(&chat_id);
            let telemost_win = ui::telemost::TelemostWindow::new(app_clone.upcast_ref(), &call_url);
            telemost_win.show();
        },
    );

    let ctrl_voice = controller.clone();
    let cv_for_voice = chat_view.clone();
    chat_view.on_voice_send(
        move |chat_id: String, audio_data: Vec<u8>, duration: f64, waveform: Vec<f32>| {
            let ctrl = ctrl_voice.clone();
            let cv = cv_for_voice.clone();
            glib::spawn_future_local(async move {
                match ctrl
                    .send_voice_message(&chat_id, &audio_data, duration, waveform)
                    .await
                {
                    Ok(msg) => {
                        cv.add_message(msg);
                    }
                    Err(e) => {
                        eprintln!("Failed to send voice message: {}", e);
                        cv.show_error(&e.to_string());
                    }
                }
            });
        },
    );

    let ctrl_reactions = controller.clone();
    chat_view.on_reaction_toggle(move |message_id, emoji, add| {
        let ctrl = ctrl_reactions.clone();
        glib::spawn_future_local(async move {
            let result = if add {
                ctrl.add_reaction(&message_id, &emoji).await
            } else {
                ctrl.remove_reaction(&message_id, &emoji).await
            };
            if let Err(error) = result {
                eprintln!("Reaction failed: {}", error);
            }
        });
    });

    let ctrl_reactions_config = controller.clone();
    let cv_reactions_config = chat_view.clone();
    glib::spawn_future_local(async move {
        if let Ok(config) = ctrl_reactions_config.get_reactions_config().await {
            cv_reactions_config.set_reactions_config(config);
        }
    });

    // ── Wire: Chat selection → load messages ──
    let ctrl_select = controller.clone();
    let cv_for_select = chat_view.clone();
    let ctrl_ws_for_select = controller.clone();
    let cl_for_select = chat_list.clone();
    chat_list
        .lock()
        .unwrap()
        .connect_chat_selected(move |chat| {
            let chat_id = chat.id.clone();
            cv_for_select.set_chat(chat);

            let ctrl = ctrl_select.clone();
            let cv = cv_for_select.clone();
            let chat_id_future = chat_id.clone();
            let ctrl_ws = ctrl_ws_for_select.clone();
            let chat_id_clone = chat_id.clone();
            let cl_for_preview = cl_for_select.clone();

            glib::spawn_future_local(async move {
                // 1. Set the current chat for WebSocket subscription
                ctrl_ws.ws().set_current_chat(Some(chat_id_clone)).await;
                let _ = ctrl_ws
                    .ws()
                    .subscribe_typing_enhanced(&chat_id_future)
                    .await;

                // 2. Load cached messages first
                let start_cached = std::time::Instant::now();
                let cached = ctrl.get_cached_messages_async(chat_id_future.clone()).await;
                eprintln!(
                    "[PERF] get_cached_messages_async took {:?}",
                    start_cached.elapsed()
                );

                let current_id = cv.current_chat_id();
                if current_id == Some(chat_id_future.clone()) && !cached.is_empty() {
                    let start_set = std::time::Instant::now();
                    cv.set_messages(cached.clone());
                    eprintln!(
                        "[PERF] set_messages (cached) took {:?}",
                        start_set.elapsed()
                    );
                }

                // 3. Fetch fresh messages
                let start_select = std::time::Instant::now();
                match ctrl.select_chat(&chat_id_future).await {
                    Ok(messages) => {
                        eprintln!("[PERF] select_chat took {:?}", start_select.elapsed());
                        // Keep chat-list preview in sync with actual last message
                        if let Some(last) = messages.last() {
                            cl_for_preview
                                .lock()
                                .unwrap()
                                .update_last_message(&chat_id_future, last.clone());
                        }
                        let current_chat_id = cv.current_chat_id();
                        if current_chat_id == Some(chat_id_future.clone())
                            && !crate::models::messages_equivalent(&cached, &messages)
                        {
                            let start_set = std::time::Instant::now();
                            cv.set_messages(messages);
                            eprintln!("[PERF] set_messages (fresh) took {:?}", start_set.elapsed());
                        }
                    }
                    Err(e) => {
                        eprintln!(
                            "[PERF] select_chat failed after {:?}: {}",
                            start_select.elapsed(),
                            e
                        );
                        eprintln!("Failed to load messages for chat {}: {}", chat_id_future, e);
                    }
                }

                // 4. Load scheduled messages
                match ctrl.get_scheduled_messages(&chat_id_future).await {
                    Ok(sched_msgs) => {
                        let current_chat_id = cv.current_chat_id();
                        if current_chat_id == Some(chat_id_future) {
                            cv.update_scheduled_messages(sched_msgs);
                        }
                    }
                    Err(e) => {
                        eprintln!(
                            "Failed to load scheduled messages for chat {}: {}",
                            chat_id_future, e
                        );
                    }
                }
            });
        });

    // ── Wire: Multi-Account Switching ──
    let auth_switch = controller.auth().clone();
    let ctrl_switch = controller.clone();
    let cl_for_switch = chat_list.clone();
    let cv_for_switch = chat_view.clone();
    chat_list
        .lock()
        .unwrap()
        .connect_switch_account(move |account_id| {
            let auth = auth_switch.clone();
            let ctrl = ctrl_switch.clone();
            let cl = cl_for_switch.clone();
            let cv = cv_for_switch.clone();
            glib::spawn_future_local(async move {
                match auth.switch_account(&account_id).await {
                    Ok(_) => {
                        if let Ok(token) = auth.get_token().await {
                            ctrl.set_token(&token.access_token);
                            ctrl.ws().force_reconnect().await;
                            cv.set_empty();
                            if let Ok(chats) = ctrl.load_chats().await {
                                cl.lock().unwrap().set_chats(chats);
                            }
                            cl.lock().unwrap().refresh_header(&auth);
                        }
                    }
                    Err(e) => {
                        eprintln!("Failed to switch account: {}", e);
                    }
                }
            });
        });

    // ── Wire: Add Account & Logout ──
    let auth_add = controller.auth().clone();
    let ctrl_add = controller.clone();
    let cl_for_add = chat_list.clone();
    let cv_for_add = chat_view.clone();
    let win_add = win.clone();
    chat_list.lock().unwrap().connect_add_account(move || {
        let auth = auth_add.clone();
        let ctrl = ctrl_add.clone();
        let cl = cl_for_add.clone();
        let cv = cv_for_add.clone();

        let auth_dialog =
            AuthDialog::new(&win_add, auth.clone(), tokio::runtime::Handle::current());
        if let Ok(_) = auth_dialog.authenticate_with_selection() {
            glib::spawn_future_local(async move {
                if let Some(_active_id) = auth.get_current_account_id().await {
                    if let Ok(token) = auth.get_token().await {
                        ctrl.set_token(&token.access_token);
                        ctrl.ws().force_reconnect().await;
                        cv.set_empty();
                        if let Ok(chats) = ctrl.load_chats().await {
                            cl.lock().unwrap().set_chats(chats);
                        }
                        cl.lock().unwrap().refresh_header(&auth);
                    }
                }
            });
        }
    });

    let auth_logout = controller.auth().clone();
    let ctrl_logout = controller.clone();
    let cl_for_logout = chat_list.clone();
    let cv_for_logout = chat_view.clone();
    let win_logout = win.clone();
    chat_list.lock().unwrap().connect_logout(move || {
        let auth = auth_logout.clone();
        let ctrl = ctrl_logout.clone();
        let cl = cl_for_logout.clone();
        let cv = cv_for_logout.clone();
        let win = win_logout.clone();
        glib::spawn_future_local(async move {
            match auth.logout().await {
                Ok(_) => {
                    let accounts = auth.list_accounts().await;
                    if accounts.is_empty() {
                        let auth_dialog =
                            AuthDialog::new(&win, auth.clone(), tokio::runtime::Handle::current());
                        if let Ok(_) = auth_dialog.authenticate_with_selection() {
                            if let Ok(token) = auth.get_token().await {
                                ctrl.set_token(&token.access_token);
                                ctrl.ws().force_reconnect().await;
                                cv.set_empty();
                                if let Ok(chats) = ctrl.load_chats().await {
                                    cl.lock().unwrap().set_chats(chats);
                                }
                                cl.lock().unwrap().refresh_header(&auth);
                            }
                        }
                    } else {
                        let first_acc = &accounts[0];
                        if let Ok(_) = auth.switch_account(&first_acc.id).await {
                            if let Ok(token) = auth.get_token().await {
                                ctrl.set_token(&token.access_token);
                                ctrl.ws().force_reconnect().await;
                                cv.set_empty();
                                if let Ok(chats) = ctrl.load_chats().await {
                                    cl.lock().unwrap().set_chats(chats);
                                }
                                cl.lock().unwrap().refresh_header(&auth);
                            }
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Failed to logout: {}", e);
                }
            }
        });
    });

    // ── Wire: Group & Channel Dialog ──
    let auth_create = controller.auth().clone();
    let ctrl_create = controller.clone();
    let cl_for_create = chat_list.clone();
    let win_create = win.clone();
    chat_list.lock().unwrap().connect_create_group(move || {
        let auth = auth_create.clone();
        let ctrl = ctrl_create.clone();
        let cl = cl_for_create.clone();

        let dialog = ui::create_group_dialog::CreateGroupDialog::new(auth);
        dialog.set_transient_for(&win_create);

        let dialog_clone = std::rc::Rc::new(dialog);
        let dialog_for_confirm = dialog_clone.clone();
        let dialog_for_cancel = dialog_clone.clone();
        let dialog_for_load = dialog_clone.clone();
        let ctrl_for_load = ctrl.clone();

        dialog_clone.connect_cancel_clicked(move || {
            dialog_for_cancel.hide();
        });

        dialog_clone.connect_create_clicked(move || {
            let dg = dialog_for_confirm.clone();
            let ctrl = ctrl.clone();
            let cl = cl.clone();
            let title = dg.get_title();
            let description = dg.get_description();
            let is_public = dg.is_public();
            let is_channel = dg.is_channel();
            let members = dg.get_selected_members();

            if title.trim().is_empty() {
                eprintln!("Group title is required");
                return;
            }

            glib::spawn_future_local(async move {
                let res = if is_channel {
                    ctrl.create_channel(&title, description, is_public).await
                } else {
                    ctrl.create_group(&title, members, is_public).await
                };

                match res {
                    Ok(_) => {
                        dg.hide();
                        if let Ok(chats) = ctrl.load_chats().await {
                            cl.lock().unwrap().set_chats(chats);
                        }
                    }
                    Err(e) => {
                        eprintln!("Failed to create: {}", e);
                    }
                }
            });
        });

        dialog_clone.show();

        // Load real contacts asynchronously so the dialog opens immediately
        glib::spawn_future_local(async move {
            match ctrl_for_load.get_contact_candidates().await {
                Ok(contacts) => {
                    dialog_for_load.load_candidates(contacts);
                }
                Err(e) => {
                    log::warn!("Failed to load contacts for group dialog: {}", e);
                    dialog_for_load.load_candidates(Vec::new());
                }
            }
        });
    });

    // ── Wire: Scheduled Messages ──
    let ctrl_sched1 = controller.clone();
    let cv_for_sched1 = chat_view.clone();
    let ctrl_sched2 = controller.clone();
    let cv_for_sched2 = chat_view.clone();
    chat_view.bind_schedule_callbacks(
        move |chat_id: String, text: String, scheduled_at: chrono::DateTime<chrono::Utc>| {
            let ctrl = ctrl_sched1.clone();
            let cv = cv_for_sched1.clone();
            glib::spawn_future_local(async move {
                match ctrl.schedule_message(&chat_id, &text, scheduled_at).await {
                    Ok(_) => {
                        if let Ok(sched_msgs) = ctrl.get_scheduled_messages(&chat_id).await {
                            cv.update_scheduled_messages(sched_msgs);
                        }
                    }
                    Err(e) => {
                        eprintln!("Failed to schedule message: {}", e);
                    }
                }
            });
        },
        move |chat_id: String, message_id: String| {
            let ctrl = ctrl_sched2.clone();
            let cv = cv_for_sched2.clone();
            glib::spawn_future_local(async move {
                match ctrl.cancel_scheduled_message(&chat_id, &message_id).await {
                    Ok(_) => {
                        if let Ok(sched_msgs) = ctrl.get_scheduled_messages(&chat_id).await {
                            cv.update_scheduled_messages(sched_msgs);
                        }
                    }
                    Err(e) => {
                        eprintln!("Failed to cancel scheduled message: {}", e);
                    }
                }
            });
        },
    );

    // ── Load chats + refresh account header with real profile ──
    let ctrl_load = controller.clone();
    let cl_for_load = chat_list.clone();
    let auth_for_header = controller.auth().clone();
    glib::spawn_future_local(async move {
        // Best-effort OAuth profile (often limited scopes — may only have login)
        if let Ok(user) = auth_for_header.get_user_info().await {
            let _ = auth_for_header.apply_user_profile(&user).await;
            cl_for_load.lock().unwrap().refresh_header(&auth_for_header);
        }

        match ctrl_load.load_chats().await {
            Ok(chats) => {
                cl_for_load.lock().unwrap().set_chats(chats);
                // Bootstrap updates account name/avatar via auth.update_current_profile;
                // give it a moment then refresh header with real messenger display_name.
                let auth = auth_for_header.clone();
                let cl = cl_for_load.clone();
                glib::timeout_add_local_once(std::time::Duration::from_millis(400), move || {
                    cl.lock().unwrap().refresh_header(&auth);
                });
            }
            Err(e) => {
                eprintln!("Failed to load chats: {}", e);
                cl_for_load.lock().unwrap().refresh_header(&auth_for_header);
            }
        }
    });

    // ── Load sticker packs in background (CDN via bootstrap IDs) ──
    let ctrl_stickers = controller.clone();
    let cv_for_stickers = chat_view.clone();
    glib::spawn_future_local(async move {
        match ctrl_stickers.load_sticker_packs().await {
            Ok(catalog) if !catalog.packs.is_empty() => {
                log::info!("Loaded {} sticker packs from CDN", catalog.packs.len());
                cv_for_stickers.update_sticker_packs(catalog.packs);
            }
            Ok(_) => {
                log::warn!("Sticker catalog empty — using offline fallback packs");
                cv_for_stickers.update_sticker_packs(get_default_mock_packs());
            }
            Err(e) => {
                log::warn!(
                    "Sticker packs unavailable ({}). Using offline fallback packs.",
                    e
                );
                cv_for_stickers.update_sticker_packs(get_default_mock_packs());
            }
        }
    });

    // ── Connect WebSocket in background with auto-reconnect ──
    let ctrl_ws = controller.clone();
    let cv_for_ws = chat_view.clone();
    let cl_for_ws = chat_list.clone();
    glib::spawn_future_local(async move {
        // Subscribe to state changes
        let ws_for_state = ctrl_ws.ws().clone();
        let ws_state_cb = move |state: crate::api::WSState| {
            let _ws = ws_for_state.clone();
            glib::idle_add_once(move || {
                glib::spawn_future_local(async move {
                    match state {
                        crate::api::WSState::Connected => {
                            log::info!("WebSocket connected — auto-reconnected");
                        }
                        crate::api::WSState::Disconnected => {
                            log::warn!("WebSocket disconnected — reconnecting...");
                        }
                        crate::api::WSState::Connecting => {
                            log::info!("WebSocket connecting...");
                        }
                        crate::api::WSState::Reconnecting(attempts) => {
                            log::warn!("WebSocket reconnecting (attempt {})...", attempts);
                        }
                    }
                });
            });
        };
        ctrl_ws.ws().on_state_change(ws_state_cb).await;

        // Subscribe to incoming messages
        let (tx_msg, mut rx_msg) =
            tokio::sync::mpsc::unbounded_channel::<crate::models::WSMessage>();

        let ws_msg_cb = move |ws_msg: &crate::models::WSMessage| {
            let _ = tx_msg.send(ws_msg.clone());
        };
        ctrl_ws.ws().on_message(ws_msg_cb).await;

        let cv_for_rx = cv_for_ws.clone();
        let cl_for_rx = cl_for_ws.clone();
        let ctrl_for_rx = ctrl_ws.clone();

        glib::spawn_future_local(async move {
            while let Some(ws_msg) = rx_msg.recv().await {
                let cv = cv_for_rx.clone();
                let cl = cl_for_rx.clone();
                let ctrl = ctrl_for_rx.clone();
                let method = ws_msg
                    .message
                    .get("method")
                    .and_then(|m| m.as_str())
                    .unwrap_or("");

                match method {
                    "new_message" => {
                        let messages =
                            crate::api::HttpClient::parse_ws_incoming_messages(&ws_msg.message);
                        if !messages.is_empty() {
                            for msg in &messages {
                                let mut sender = "Yandex User".to_string();
                                if let Some(chat) = ctrl
                                    .state()
                                    .lock()
                                    .await
                                    .chats
                                    .iter()
                                    .find(|c| c.id == msg.chat_id)
                                {
                                    if let Some(title) = chat.title.as_ref() {
                                        sender = title.clone();
                                    }
                                }
                                let text = msg.text.as_deref().unwrap_or("").to_string();
                                if !text.is_empty() {
                                    ui::notifications::send_notification(&sender, &text);
                                }
                            }

                            if let Some(selected_chat_id) = ctrl.get_selected_chat_id().await {
                                for msg in messages {
                                    if msg.chat_id == selected_chat_id {
                                        cv.add_message(msg);
                                    }
                                }

                                let chats = ctrl.state().lock().await.chats.clone();
                                if let Some(chat) = chats.iter().find(|c| c.id == selected_chat_id)
                                {
                                    cl.lock()
                                        .unwrap()
                                        .update_unread(&chat.id, chat.unread_count);
                                }
                            }
                        }
                    }
                    "unread_update" => {
                        if let Some(chat_id) =
                            ws_msg.message.get("chat_id").and_then(|c| c.as_str())
                        {
                            if let Some(unread) =
                                ws_msg.message.get("unread_count").and_then(|u| u.as_u64())
                            {
                                cl.lock().unwrap().update_unread(chat_id, unread as u32);
                            }
                        }
                    }
                    "typing_enhanced" => {
                        let typing_user = ws_msg
                            .message
                            .get("user")
                            .and_then(|user| {
                                user.get("display_name")
                                    .or_else(|| user.get("public_name"))
                                    .or_else(|| user.get("contact_name"))
                            })
                            .and_then(|name| name.as_str())
                            .map(|name| name.to_string());
                        if let Some(user) = typing_user {
                            cv.set_typing(&user);
                        }
                    }
                    "reaction_update" => {
                        if let Some((message_id, reactions)) =
                            crate::api::HttpClient::parse_reaction_update_payload(&ws_msg.message)
                        {
                            cv.update_message_reactions(&message_id, reactions);
                        } else if let Some(chat_id) = ctrl.get_selected_chat_id().await {
                            if let Ok(messages) = ctrl.fetch_fresh_messages(&chat_id).await {
                                cv.set_messages(messages);
                            }
                        }
                    }
                    _ => {}
                }
            }
        });

        // Start WebSocket connection with auto-reconnect in a background thread
        let ctrl_ws_spawn = ctrl_ws.clone();
        tokio::spawn(async move {
            if let Err(e) = ctrl_ws_spawn.ws().connect().await {
                log::error!("Failed to connect WebSocket: {}", e);
            }
        });
    });

    overlay.set_child(Some(&root));

    // ── ImageViewer ──
    let image_viewer = Arc::new(ui::image_viewer::ImageViewer::new());
    let iv_container = image_viewer.container.clone();
    iv_container.set_visible(false);
    overlay.add_overlay(&iv_container);

    let iv_for_open = image_viewer.clone();
    chat_view.on_image_open(move |url, filename, all_images| {
        let current_idx = all_images.iter().position(|(u, _)| u == &url).unwrap_or(0);

        iv_for_open.show(&url, &filename);

        let iv_for_nav = iv_for_open.clone();
        iv_for_open.set_image_sequence(all_images.len(), current_idx, move |new_idx| {
            if let Some((new_url, new_filename)) = all_images.get(new_idx) {
                iv_for_nav.show(new_url, new_filename);
            }
        });

        iv_for_open.container.set_visible(true);
    });

    let main_box = gtk::Box::new(Orientation::Vertical, 0);
    let header_bar = adw::HeaderBar::new();
    main_box.append(&header_bar);
    main_box.append(&overlay);

    main_box
}

fn get_default_mock_packs() -> Vec<crate::models::StickerPack> {
    use crate::models::{Sticker, StickerPack};
    vec![StickerPack {
        pack_id: "yandex_cat".to_string(),
        title: "Котик YM".to_string(),
        stickers: vec![
            Sticker {
                sticker_id: "yc1".to_string(),
                pack_id: "yandex_cat".to_string(),
                file_url: "https://telegram.org.ru/uploads/posts/2017-10/1507404434_0.png"
                    .to_string(),
                thumb_url: "https://telegram.org.ru/uploads/posts/2017-10/1507404434_0.png"
                    .to_string(),
                width: 512,
                height: 512,
                emoji: "👋".to_string(),
                file_size: 10240,
                is_animated: false,
                is_text_sticker: false,
                text: None,
            },
            Sticker {
                sticker_id: "yc2".to_string(),
                pack_id: "yandex_cat".to_string(),
                file_url: "https://telegram.org.ru/uploads/posts/2017-10/1507404434_1.png"
                    .to_string(),
                thumb_url: "https://telegram.org.ru/uploads/posts/2017-10/1507404434_1.png"
                    .to_string(),
                width: 512,
                height: 512,
                emoji: "😊".to_string(),
                file_size: 10240,
                is_animated: false,
                is_text_sticker: false,
                text: None,
            },
            Sticker {
                sticker_id: "yc3".to_string(),
                pack_id: "yandex_cat".to_string(),
                file_url: "https://telegram.org.ru/uploads/posts/2017-10/1507404434_2.png"
                    .to_string(),
                thumb_url: "https://telegram.org.ru/uploads/posts/2017-10/1507404434_2.png"
                    .to_string(),
                width: 512,
                height: 512,
                emoji: "😂".to_string(),
                file_size: 10240,
                is_animated: false,
                is_text_sticker: false,
                text: None,
            },
            Sticker {
                sticker_id: "yc4".to_string(),
                pack_id: "yandex_cat".to_string(),
                file_url: "https://telegram.org.ru/uploads/posts/2017-10/1507404434_3.png"
                    .to_string(),
                thumb_url: "https://telegram.org.ru/uploads/posts/2017-10/1507404434_3.png"
                    .to_string(),
                width: 512,
                height: 512,
                emoji: "😮".to_string(),
                file_size: 10240,
                is_animated: false,
                is_text_sticker: false,
                text: None,
            },
            Sticker {
                sticker_id: "yc5".to_string(),
                pack_id: "yandex_cat".to_string(),
                file_url: "https://telegram.org.ru/uploads/posts/2017-10/1507404434_4.png"
                    .to_string(),
                thumb_url: "https://telegram.org.ru/uploads/posts/2017-10/1507404434_4.png"
                    .to_string(),
                width: 512,
                height: 512,
                emoji: "😡".to_string(),
                file_size: 10240,
                is_animated: false,
                is_text_sticker: false,
                text: None,
            },
            Sticker {
                sticker_id: "yc6".to_string(),
                pack_id: "yandex_cat".to_string(),
                file_url: "https://telegram.org.ru/uploads/posts/2017-10/1507404434_5.png"
                    .to_string(),
                thumb_url: "https://telegram.org.ru/uploads/posts/2017-10/1507404434_5.png"
                    .to_string(),
                width: 512,
                height: 512,
                emoji: "😭".to_string(),
                file_size: 10240,
                is_animated: false,
                is_text_sticker: false,
                text: None,
            },
        ],
        is_installed: true,
        is_featured: true,
        category: "Котики".to_string(),
        thumb_url: "https://telegram.org.ru/uploads/posts/2017-10/1507404434_0.png".to_string(),
        sticker_count: 6,
    }]
}
