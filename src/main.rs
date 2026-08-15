use std::sync::{Arc, Mutex};

use adw::prelude::*;
use gtk::prelude::*;
use gtk::Orientation;
use libadwaita as adw;

use crate::api::auth::AuthManager;
use crate::core::AppController;
use crate::ui::{AuthDialog, TelemostWindow};

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

    // Load persisted settings
    let settings_store = ui::settings::SettingsStore::new().ok();
    let app_settings = settings_store
        .as_ref()
        .map(|s| s.load())
        .unwrap_or_default();
    ui::notifications::set_notifications_enabled(app_settings.notifications_enabled);
    ui::settings::apply_reduced_motion(app_settings.reduced_motion);
    if let Some(gtk_settings) = gtk::Settings::default() {
        gtk_settings.set_gtk_application_prefer_dark_theme(app_settings.dark_theme);
    }
    let minimize_to_tray = Arc::new(std::sync::atomic::AtomicBool::new(
        app_settings.minimize_to_tray,
    ));

    // Comfortable default that fits dialogs (~280) + chat without overflow
    let win = adw::ApplicationWindow::builder()
        .application(app)
        .title("Yandex Messenger")
        .default_width(1100)
        .default_height(700)
        .build();
    win.set_icon_name(Some("yandex-messenger"));

    // System tray
    let tray = Arc::new(ui::tray::TrayHandle::init());
    let tray_for_poll = tray.clone();
    let win_for_tray = win.clone();
    let app_for_tray = app.clone();
    glib::timeout_add_local(std::time::Duration::from_millis(400), move || {
        while let Some(cmd) = tray_for_poll.try_recv() {
            match cmd {
                ui::tray::TrayCommand::Show => {
                    win_for_tray.set_visible(true);
                    win_for_tray.present();
                }
                ui::tray::TrayCommand::Quit => {
                    app_for_tray.quit();
                }
            }
        }
        glib::ControlFlow::Continue
    });

    // Close → tray (if enabled)
    let min_tray = minimize_to_tray.clone();
    let win_hide = win.clone();
    win.connect_close_request(move |_| {
        if min_tray.load(std::sync::atomic::Ordering::Relaxed) {
            win_hide.set_visible(false);
            glib::Propagation::Stop
        } else {
            glib::Propagation::Proceed
        }
    });

    let root = create_app_layout(
        app,
        &win,
        controller,
        tray,
        minimize_to_tray,
        settings_store,
    );
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
    tray: Arc<ui::tray::TrayHandle>,
    minimize_to_tray: Arc<std::sync::atomic::AtomicBool>,
    settings_store: Option<ui::settings::SettingsStore>,
) -> gtk::Box {
    let overlay = gtk::Overlay::new();

    // ── Root split: FIXED dialogs column (TG Desktop style) ──
    //
    // Root cause of "aligns then clips": after messages load, the chat pane
    // reports a large min-width → GtkPaned max-position drops → any clamp
    // that does set_position(max) *shrinks* the sidebar and clips rows.
    //
    // Fix: never auto-touch position after create; don't shrink the start child;
    // keep chat pane min-width low so max-position stays large.
    const SIDEBAR_W: i32 = 320;
    let root = gtk::Paned::builder()
        .orientation(Orientation::Horizontal)
        .position(SIDEBAR_W)
        .wide_handle(false)
        .resize_start_child(false) // extra window width goes to chat
        .resize_end_child(true)
        .shrink_start_child(false) // NEVER squeeze dialogs column
        .shrink_end_child(true)    // chat absorbs pressure
        .hexpand(true)
        .vexpand(true)
        .build();
    root.add_css_class("main-paned");
    // Pin the property so theme/layout passes don't fight the divider
    root.set_position(SIDEBAR_W);

    let sidebar_stack = gtk::Stack::builder()
        .transition_type(gtk::StackTransitionType::Crossfade)
        .transition_duration(100)
        .hexpand(false)
        .vexpand(true)
        .width_request(SIDEBAR_W)
        .build();
    sidebar_stack.add_css_class("sidebar-stack");
    sidebar_stack.set_size_request(SIDEBAR_W, -1);
    sidebar_stack.set_hexpand(false);

    // ── Sidebar (chat list) ──
    let chat_list = Arc::new(Mutex::new(ui::ChatListPanel::new(
        controller.auth().clone(),
    )));

    let saved_panel = Arc::new(ui::saved_panel::SavedPanel::new(controller.auth().clone()));

    sidebar_stack.add_named(chat_list.lock().unwrap().container(), Some("chat_list"));
    sidebar_stack.add_named(saved_panel.container(), Some("saved_panel"));
    root.set_start_child(Some(&sidebar_stack));

    // ── Chat view (message area) ──
    let chat_view = ui::ChatView::new(controller.auth().clone());
    let cv_container = chat_view.container().clone();
    cv_container.set_hexpand(true);
    cv_container.set_vexpand(true);
    // Soft min only — large min here was collapsing paned max-position
    cv_container.set_size_request(200, -1);
    cv_container.set_halign(gtk::Align::Fill);
    root.set_end_child(Some(&cv_container));

    // If the window is very narrow, still only shrink the *chat* side.
    // Do not reassign position on max-position changes (that caused the snap-back).
    {
        let paned = root.clone();
        win.connect_default_width_notify(move |w| {
            let w = w.default_width();
            if w > 0 && w < SIDEBAR_W + 240 {
                // Keep a usable chat strip; only then allow a slightly narrower sidebar
                let pos = (w / 3).clamp(200, SIDEBAR_W);
                if paned.position() != pos {
                    paned.set_position(pos);
                }
            } else if paned.position() < SIDEBAR_W - 20 {
                // Restored wide window → put dialogs column back
                paned.set_position(SIDEBAR_W);
            }
        });
    }

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

    // Settings gear → preferences (notifications / tray / theme)
    let win_for_settings = win.clone();
    let min_tray_settings = minimize_to_tray.clone();
    let saved_panel_for_settings = saved_panel.clone();
    let sidebar_for_settings = sidebar_stack.clone();
    let ctrl_for_saved = controller.clone();
    chat_list.lock().unwrap().connect_settings(move || {
        // Open preferences; secondary: long-press not available — also load Избранное entry
        // via dedicated path below after prefs.
        if let Some(ref store) = settings_store {
            let min_tray = min_tray_settings.clone();
            let stack = sidebar_for_settings.clone();
            let sp = saved_panel_for_settings.clone();
            let ctrl = ctrl_for_saved.clone();
            ui::settings::show_settings_window(&win_for_settings, store, move |s| {
                ui::notifications::set_notifications_enabled(s.notifications_enabled);
                ui::settings::apply_reduced_motion(s.reduced_motion);
                min_tray.store(
                    s.minimize_to_tray,
                    std::sync::atomic::Ordering::Relaxed,
                );
            });
            // Also refresh saved messages in background so «Избранное» panel stays warm
            glib::spawn_future_local(async move {
                if let Ok(msgs) = ctrl.get_saved_messages(50, 0).await {
                    sp.set_messages(msgs);
                    let _ = stack; // keep panel data ready without auto-switch
                }
            });
        }
    });

    // ── Wire: Send message (with reply / edit) ──
    let ctrl_send = controller.clone();
    let cv_for_send = chat_view.clone();
    let ctrl_call = controller.clone();
    let app_clone = app.clone();
    // Clone before move into closure to allow reuse in the file-attach callback
    let ctrl_send_for_file = ctrl_send.clone();
    let cv_for_send_for_file = cv_for_send.clone();
    chat_view.bind_callbacks(
        move |chat_id: String, text: String, reply_to: Option<String>, edit_id: Option<String>| {
            let ctrl = ctrl_send.clone();
            let cv = cv_for_send.clone();
            glib::spawn_future_local(async move {
                // Ensure push WS is up before send (OAuth-authenticated Xiva)
                if !ctrl.ws().is_connected().await {
                    log::info!("WS not connected — waiting briefly for reconnect");
                    for _ in 0..20 {
                        if ctrl.ws().is_connected().await {
                            break;
                        }
                        glib::timeout_future(std::time::Duration::from_millis(100)).await;
                    }
                }
                match ctrl
                    .send_text_message_ex(
                        &chat_id,
                        &text,
                        reply_to.as_deref(),
                        edit_id.as_deref(),
                    )
                    .await
                {
                    Ok(msg) => {
                        ctrl.drafts().clear(&chat_id);
                        let pending = !msg.sent;
                        if edit_id.is_some() {
                            if let Ok(messages) = ctrl.fetch_fresh_messages(&chat_id).await {
                                cv.set_messages(messages);
                            } else {
                                cv.add_message(msg);
                            }
                        } else {
                            cv.add_message(msg);
                        }
                        if pending {
                            cv.show_error(
                                "Сообщение в очереди (нет связи с сервером). Проверьте вход.",
                            );
                        }
                    }
                    Err(e) => {
                        eprintln!("Failed to send message: {}", e);
                        cv.show_error(&format!("Не отправлено: {}", e));
                    }
                }
            });
        },
        move |chat_id: String, bytes: Vec<u8>, filename: String| {
            log::info!("Attach file {} ({} bytes) to chat {}", filename, bytes.len(), chat_id);
            let ctrl = ctrl_send_for_file.clone();
            let cv = cv_for_send_for_file.clone();
            glib::spawn_future_local(async move {
                match ctrl.send_file_message(&chat_id, &bytes, &filename).await {
                    Ok(msg) => {
                        cv.add_message(msg);
                    }
                    Err(e) => {
                        eprintln!("Failed to send file: {}", e);
                        cv.show_error(&format!("Файл не отправлен: {}", e));
                    }
                }
            });
        },
        move |chat_id: String| {
            if !crate::config::ym_enable_telemost_ui() {
                log::warn!("Telemost is disabled");
                return;
            }

            let ctrl = ctrl_call.clone();
            let telemost_client = ctrl.telemost_client();
            let app_clone = app_clone.clone();
            glib::spawn_future_local(async move {
                match ctrl.start_call(&chat_id).await {
                    Ok(call) => {
                        let _call_url = ctrl.telemost_url(&chat_id, Some(&call.call_id));
                        let telemost_win =
                            TelemostWindow::new(app_clone.upcast_ref(), telemost_client.clone());
                        telemost_win.show();
                    }
                    Err(e) => {
                        log::error!("Failed to start call: {}", e);
                        let _call_url = ctrl.telemost_url(&chat_id, None);
                        let telemost_win =
                            TelemostWindow::new(app_clone.upcast_ref(), telemost_client.clone());
                        telemost_win.show();
                    }
                }
            });
        },
    );

    let ctrl_voice = controller.clone();
    let cv_for_voice = chat_view.clone();
    chat_view.on_voice_send(
        move |chat_id: String, audio_data: Vec<u8>, duration: f64, waveform: Vec<f32>| {
            if !crate::config::ym_enable_voice() {
                log::warn!("Voice messages are disabled");
                return;
            }
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
    // File download / open
    let ctrl_file = controller.clone();
    let cv_file = chat_view.clone();
    chat_view.on_file_download(move |file_id, url, filename, open_after| {
        let ctrl = ctrl_file.clone();
        let cv = cv_file.clone();
        glib::spawn_future_local(async move {
            match ctrl.download_attachment(&file_id, &url).await {
                Ok(bytes) => {
                    let downloads = dirs::download_dir()
                        .or_else(dirs::home_dir)
                        .unwrap_or_else(|| std::path::PathBuf::from("."));
                    let safe_name = filename
                        .chars()
                        .map(|c| {
                            if c.is_ascii_alphanumeric() || "._- ".contains(c) {
                                c
                            } else {
                                '_'
                            }
                        })
                        .collect::<String>();
                    let safe_name = if safe_name.trim().is_empty() {
                        "download.bin".to_string()
                    } else {
                        safe_name
                    };
                    let mut path = downloads.join(&safe_name);
                    // Avoid overwrite
                    if path.exists() {
                        let stem = path
                            .file_stem()
                            .map(|s| s.to_string_lossy().to_string())
                            .unwrap_or_else(|| "file".into());
                        let ext = path
                            .extension()
                            .map(|s| format!(".{}", s.to_string_lossy()))
                            .unwrap_or_default();
                        for i in 1..100 {
                            let candidate =
                                downloads.join(format!("{} ({}){}", stem, i, ext));
                            if !candidate.exists() {
                                path = candidate;
                                break;
                            }
                        }
                    }
                    match std::fs::write(&path, &bytes) {
                        Ok(()) => {
                            log::info!("Saved file to {}", path.display());
                            if open_after {
                                let _ = std::process::Command::new("xdg-open")
                                    .arg(&path)
                                    .spawn();
                                cv.show_toast(&format!("Открыто: {}", path.display()));
                            } else {
                                cv.show_toast(&format!("Сохранено: {}", path.display()));
                            }
                        }
                        Err(e) => {
                            cv.show_error(&format!("Не удалось сохранить: {}", e));
                        }
                    }
                }
                Err(e) => {
                    cv.show_error(&format!("Скачивание не удалось: {}", e));
                }
            }
        });
    });

    // History pagination
    let ctrl_older = controller.clone();
    let cv_older = chat_view.clone();
    chat_view.on_load_older(move |chat_id, oldest_id| {
        let ctrl = ctrl_older.clone();
        let cv = cv_older.clone();
        glib::spawn_future_local(async move {
            match ctrl.load_older_messages(&chat_id, &oldest_id, 50).await {
                Ok(older) => {
                    cv.prepend_messages(older);
                }
                Err(e) => {
                    log::warn!("load_older_messages: {}", e);
                    cv.prepend_messages(vec![]);
                }
            }
        });
    });

    chat_list
        .lock()
        .unwrap()
        .connect_chat_selected(move |chat| {
            let chat_id = chat.id.clone();

            // Save draft for previous chat, restore draft for new chat
            if let Some(prev) = cv_for_select.current_chat_id() {
                if prev != chat_id {
                    let text = cv_for_select.input_text();
                    ctrl_select.drafts().set(&prev, &text);
                }
            }
            cv_for_select.set_chat(chat);
            if let Some(draft) = ctrl_select.drafts().get(&chat_id) {
                cv_for_select.set_input_text(&draft);
            } else {
                cv_for_select.clear_input();
            }

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
                        if current_chat_id == Some(chat_id_future.clone()) {
                            // Always apply + scroll to latest on open (even if cache matched).
                            let start_set = std::time::Instant::now();
                            cv.set_messages(messages);
                            cv.scroll_to_latest();
                            eprintln!("[PERF] set_messages (fresh) took {:?}", start_set.elapsed());
                        }
                        // Mark as read on open
                        if let Err(e) = ctrl.mark_chat_read(&chat_id_future).await {
                            log::debug!("mark_chat_read: {}", e);
                        } else {
                            cl_for_preview.lock().unwrap().apply_chat_flags(
                                &chat_id_future,
                                None,
                                None,
                                None,
                                Some(0),
                            );
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
    let rt_add = tokio::runtime::Handle::current();
    chat_list.lock().unwrap().connect_add_account(move || {
        let auth = auth_add.clone();
        let ctrl = ctrl_add.clone();
        let cl = cl_for_add.clone();
        let cv = cv_for_add.clone();
        let win = win_add.clone();
        let rt = rt_add.clone();
        // Run dialog from idle so we never nest MainLoop inside other handlers
        glib::idle_add_local_once(move || {
            let auth_dialog = AuthDialog::new(&win, auth.clone(), rt);
            if auth_dialog.authenticate_with_selection().is_ok() {
                glib::spawn_future_local(async move {
                    if let Some(_active_id) = auth.get_current_account_id().await {
                        if let Ok(token) = auth.get_token().await {
                            ctrl.set_token(&token.access_token);
                            ctrl.reload_session();
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
    });

    let auth_logout = controller.auth().clone();
    let ctrl_logout = controller.clone();
    let cl_for_logout = chat_list.clone();
    let cv_for_logout = chat_view.clone();
    let win_logout = win.clone();
    // Capture runtime handle now — not from inside glib async (can panic / abort).
    let rt_logout = tokio::runtime::Handle::current();
    chat_list.lock().unwrap().connect_logout(move || {
        let auth = auth_logout.clone();
        let ctrl = ctrl_logout.clone();
        let cl = cl_for_logout.clone();
        let cv = cv_for_logout.clone();
        let win = win_logout.clone();
        let rt = rt_logout.clone();
        glib::spawn_future_local(async move {
            // 1) Logout + clear UI on the async side only (no nested GTK dialogs here)
            if let Err(e) = auth.logout().await {
                eprintln!("Failed to logout: {}", e);
            }
            // Drop Passport session so re-login harvests fresh cookies / CSRF
            crate::api::session_store::clear_session();
            ctrl.clear_session_cookies();
            let _ = ctrl.ws().force_reconnect().await;
            cv.set_empty();
            cl.lock().unwrap().set_chats(vec![]);
            cl.lock().unwrap().refresh_header(&auth);

            let accounts = auth.list_accounts().await;
            if !accounts.is_empty() {
                // Switch to another stored account without opening auth UI
                let first_acc = accounts[0].id.clone();
                if let Ok(_) = auth.switch_account(&first_acc).await {
                    if let Ok(token) = auth.get_token().await {
                        ctrl.set_token(&token.access_token);
                        ctrl.reload_session();
                        ctrl.ws().force_reconnect().await;
                        if let Ok(chats) = ctrl.load_chats().await {
                            cl.lock().unwrap().set_chats(chats);
                        }
                        cl.lock().unwrap().refresh_header(&auth);
                    }
                }
                return;
            }

            // 2) No accounts left → open AuthDialog on idle (NOT inside this future).
            // Nested MainLoop::run from spawn_future_local aborts GTK (SIGABRT).
            glib::idle_add_local_once(move || {
                let auth_dialog = AuthDialog::new(&win, auth.clone(), rt.clone());
                match auth_dialog.authenticate_with_selection() {
                    Ok(_token) => {
                        let auth = auth.clone();
                        let ctrl = ctrl.clone();
                        let cl = cl.clone();
                        let cv = cv.clone();
                        glib::spawn_future_local(async move {
                            if let Ok(token) = auth.get_token().await {
                                ctrl.set_token(&token.access_token);
                                ctrl.reload_session();
                                ctrl.ws().force_reconnect().await;
                                cv.set_empty();
                                if let Ok(chats) = ctrl.load_chats().await {
                                    cl.lock().unwrap().set_chats(chats);
                                }
                                cl.lock().unwrap().refresh_header(&auth);
                            }
                        });
                    }
                    Err(e) => {
                        eprintln!("Re-login cancelled or failed: {}", e);
                    }
                }
            });
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

    // ── Wire: chat context menu actions ──
    let ctrl_actions = controller.clone();
    let cl_for_actions = chat_list.clone();
    let tray_for_actions = tray.clone();
    let cv_for_actions = chat_view.clone();
    chat_list.lock().unwrap().connect_chat_action(move |chat_id, action| {
        let ctrl = ctrl_actions.clone();
        let cl = cl_for_actions.clone();
        let tray = tray_for_actions.clone();
        let cv = cv_for_actions.clone();
        glib::spawn_future_local(async move {
            let result = match action.as_str() {
                "mark_read" => {
                    let r = ctrl.mark_chat_read(&chat_id).await;
                    if r.is_ok() {
                        cl.lock().unwrap().apply_chat_flags(
                            &chat_id,
                            None,
                            None,
                            None,
                            Some(0),
                        );
                    }
                    r
                }
                "mute" => {
                    let currently = ctrl.is_chat_muted(&chat_id).await;
                    let r = ctrl.set_chat_muted(&chat_id, !currently).await;
                    // Always apply local flag even if API fails (responsive UI)
                    cl.lock().unwrap().apply_chat_flags(
                        &chat_id,
                        Some(!currently),
                        None,
                        None,
                        None,
                    );
                    r
                }
                "pin" => {
                    let chats_arc = cl.lock().unwrap().chats().clone();
                    let pinned = chats_arc
                        .lock()
                        .unwrap()
                        .iter()
                        .find(|c| c.id == chat_id)
                        .map(|c| c.pinned)
                        .unwrap_or(false);
                    let r = ctrl.set_chat_pinned(&chat_id, !pinned).await;
                    cl.lock().unwrap().apply_chat_flags(
                        &chat_id,
                        None,
                        Some(!pinned),
                        None,
                        None,
                    );
                    r
                }
                "archive" => {
                    let chats_arc = cl.lock().unwrap().chats().clone();
                    let archived = chats_arc
                        .lock()
                        .unwrap()
                        .iter()
                        .find(|c| c.id == chat_id)
                        .map(|c| c.archived)
                        .unwrap_or(false);
                    let r = ctrl.set_chat_archived(&chat_id, !archived).await;
                    cl.lock().unwrap().apply_chat_flags(
                        &chat_id,
                        None,
                        None,
                        Some(!archived),
                        None,
                    );
                    r
                }
                "delete" => {
                    let r = ctrl.delete_chat(&chat_id).await;
                    cl.lock().unwrap().remove_chat(&chat_id);
                    if cv.current_chat_id().as_deref() == Some(chat_id.as_str()) {
                        cv.set_empty();
                    }
                    r
                }
                other => {
                    log::warn!("Unknown chat action: {}", other);
                    Ok(())
                }
            };
            if let Err(e) = result {
                log::warn!("Chat action '{}' failed: {}", action, e);
            }
            tray.set_unread_count(ctrl.total_unread().await);
        });
    });

    // Periodic outbox retry (every 45s)
    let ctrl_outbox = controller.clone();
    glib::timeout_add_local(std::time::Duration::from_secs(45), move || {
        let ctrl = ctrl_outbox.clone();
        glib::spawn_future_local(async move {
            if !ctrl.outbox().is_empty() {
                let (ok, left) = ctrl.flush_outbox().await;
                log::info!("Periodic outbox flush: sent={}, remaining={}", ok, left);
            }
        });
        glib::ControlFlow::Continue
    });

    // Soft: if authenticated but no session cookies, log a clear hint
    if !controller.http().has_session() {
        log::warn!(
            "No Passport session cookies — history/files/WS may be incomplete. \
             Log out and sign in again (session is captured in the login WebView)."
        );
    }

    // Welcome state for chat pane
    chat_view.set_empty();

    // Instant UI from SQLite before network; otherwise skeleton
    {
        let cached_chats = controller.load_chats_from_db();
        if !cached_chats.is_empty() {
            log::info!("Hydrating UI with {} chats from SQLite", cached_chats.len());
            chat_list.lock().unwrap().set_chats(cached_chats);
        } else {
            chat_list.lock().unwrap().show_skeleton();
        }
    }

    // ── Load chats + refresh account header with real profile ──
    let ctrl_load = controller.clone();
    let cl_for_load = chat_list.clone();
    let auth_for_header = controller.auth().clone();
    let tray_for_load = tray.clone();
    glib::spawn_future_local(async move {
        // Best-effort OAuth profile (often limited scopes — may only have login)
        if let Ok(user) = auth_for_header.get_user_info().await {
            let _ = auth_for_header.apply_user_profile(&user).await;
            cl_for_load.lock().unwrap().refresh_header(&auth_for_header);
        }

        match ctrl_load.load_chats().await {
            Ok(chats) => {
                let total: u32 = chats.iter().map(|c| c.unread_count).sum();
                tray_for_load.set_unread_count(total);
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
                cl_for_load.lock().unwrap().show_list_or_empty();
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
    let tray_ws = tray.clone();
    glib::spawn_future_local(async move {
        // Subscribe to state changes
        let ws_for_state = ctrl_ws.ws().clone();
        let ctrl_for_state = ctrl_ws.clone();
        let ws_state_cb = move |state: crate::api::WSState| {
            let _ws = ws_for_state.clone();
            let ctrl = ctrl_for_state.clone();
            glib::idle_add_once(move || {
                glib::spawn_future_local(async move {
                    match state {
                        crate::api::WSState::Connected => {
                            log::info!("WebSocket connected — flushing outbox");
                            ctrl.reload_session();
                            let (ok, left) = ctrl.flush_outbox().await;
                            if ok > 0 || left > 0 {
                                log::info!("Outbox flush: sent={}, remaining={}", ok, left);
                            }
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
        let tray_for_rx = tray_ws.clone();

        glib::spawn_future_local(async move {
            while let Some(ws_msg) = rx_msg.recv().await {
                let cv = cv_for_rx.clone();
                let cl = cl_for_rx.clone();
                let ctrl = ctrl_for_rx.clone();
                let tray_ws = tray_for_rx.clone();
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
                                let text = msg
                                    .text
                                    .clone()
                                    .filter(|t| !t.trim().is_empty())
                                    .unwrap_or_else(|| msg.preview());
                                let muted = ctrl.is_chat_muted(&msg.chat_id).await;
                                let selected = ctrl.get_selected_chat_id().await;
                                let window_focused_same_chat =
                                    selected.as_deref() == Some(msg.chat_id.as_str());
                                if !text.is_empty() && !window_focused_same_chat {
                                    ui::notifications::send_notification_for_chat(
                                        &sender,
                                        &text,
                                        Some(&msg.chat_id),
                                        muted,
                                    );
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

                            // Update tray unread badge
                            let total = ctrl.total_unread().await;
                            tray_ws.set_unread_count(total);
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
                    // Delivery / read ticks
                    "message_status"
                    | "status_update"
                    | "delivery_update"
                    | "message_delivered"
                    | "message_read"
                    | "read"
                    | "read_update" => {
                        if let Some(update) =
                            crate::api::HttpClient::parse_status_update_payload(&ws_msg.message)
                        {
                            let changed = ctrl.apply_status_update(update.clone()).await;
                            if let Some(selected) = ctrl.get_selected_chat_id().await {
                                if update.chat_id.as_deref() == Some(selected.as_str())
                                    || update.chat_id.is_none()
                                {
                                    if update.message_id.is_none() && update.read {
                                        cv.mark_all_outgoing_read();
                                    } else if !changed.is_empty() {
                                        let pairs: Vec<_> = changed
                                            .into_iter()
                                            .map(|id| (id, update.delivered, update.read))
                                            .collect();
                                        cv.apply_status_updates(&pairs);
                                    }
                                }
                            }
                        }
                    }
                    _ => {
                        // Some payloads use method names we don't list — try status parse generically
                        if let Some(update) =
                            crate::api::HttpClient::parse_status_update_payload(&ws_msg.message)
                        {
                            if update.delivered || update.read {
                                let changed = ctrl.apply_status_update(update.clone()).await;
                                if !changed.is_empty() {
                                    if let Some(selected) = ctrl.get_selected_chat_id().await {
                                        if update.chat_id.as_deref() == Some(selected.as_str())
                                            || update.chat_id.is_none()
                                        {
                                            let pairs: Vec<_> = changed
                                                .into_iter()
                                                .map(|id| (id, update.delivered, update.read))
                                                .collect();
                                            cv.apply_status_updates(&pairs);
                                        }
                                    }
                                }
                            }
                        }
                    }
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
    // Minimal titlebar (TG uses its own chrome; keep window controls only)
    let header_bar = adw::HeaderBar::new();
    header_bar.set_show_end_title_buttons(true);
    header_bar.set_show_start_title_buttons(true);
    header_bar.set_title_widget(Some(
        &gtk::Label::builder()
            .label("Yandex Messenger")
            .css_classes(vec!["title".to_string()])
            .build(),
    ));
    header_bar.add_css_class("flat");
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
