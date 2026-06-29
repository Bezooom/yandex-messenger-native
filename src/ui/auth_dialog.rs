use gtk::prelude::*;
use gtk::{Entry, Label, ResponseType, Window, Box as GtkBox, Orientation, Button};
use std::sync::Arc;
use std::sync::mpsc;
use tokio::runtime::Handle;
use tiny_http::{Server, Response};

#[cfg(feature = "in_app_webview")]
use webkit6::prelude::*;

use crate::api::auth::{AuthManager, OAuthToken};
use crate::models::Account;

#[derive(Clone, Debug)]
enum SelectionResult {
    Account(usize),
    AddNew,
    Cancelled,
}

/// Authentication dialog for OAuth flow.
pub struct AuthDialog {
    parent: Window,
    auth_manager: Arc<AuthManager>,
    /// Shared tokio runtime handle. The dialog is driven from the GTK main
    /// thread so we cannot create a fresh runtime for every async call
    /// (that would nest runtimes and panic).
    rt: Handle,
}

impl AuthDialog {
    pub fn new(parent: &impl IsA<Window>, auth_manager: Arc<AuthManager>, rt: Handle) -> Self {
        Self {
            parent: parent.as_ref().clone(),
            auth_manager,
            rt,
        }
    }

    /// Blocking helper: run an async task on the shared runtime from the GTK thread.
    fn block_on<F: std::future::Future>(&self, fut: F) -> F::Output {
        self.rt.block_on(fut)
    }

    /// Show account selection or auth dialog, and return a valid OAuth token.
    pub fn authenticate_with_selection(&self) -> Result<OAuthToken, String> {
        let accounts = self.block_on(self.auth_manager.list_accounts());

        if !accounts.is_empty() {
            self.select_account(&accounts)
        } else {
            self.show_auth_dialog()
        }
    }


    /// Complete the auth flow using a raw code / access token produced by the user.
    /// Exchanges the code for a real token when needed, persists it and registers
    /// the resulting account so the multi-account UI stays in sync.
    fn finalize_token(&self, raw: &str) -> Result<OAuthToken, String> {
        let code = extract_auth_code(raw);
        if code.is_empty() {
            return Err("Empty confirmation code".to_string());
        }

        // Heuristic: Yandex access tokens are alphanumeric strings of 30+ chars
        // while OAuth codes are shorter. If the value doesn't look like a code,
        // treat it as a bare access token.
        let looks_like_access_token = code.len() >= 32 && !code.contains('.');

        let token = if looks_like_access_token {
            OAuthToken {
                access_token: code,
                refresh_token: None,
                expires_in: 31_536_000,
                token_type: "Bearer".to_string(),
                user_id: None,
                received_at: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0),
            }
        } else {
            self.block_on(self.auth_manager.exchange_code(&code))?
        };

        self.block_on(self.auth_manager.set_token(token.clone()))
            .map_err(|e| e.to_string())?;

        // Try to resolve the user profile so the account has a display name.
        // Missing profile data is non-fatal — we still register the account.
        match self.block_on(self.auth_manager.get_user_info()) {
            Ok(user) => {
                let _ = self.block_on(self.auth_manager.add_account(&token, &user));
            }
            Err(e) => {
                log::warn!("Failed to fetch user info after login: {}", e);
                let placeholder = crate::models::User {
                    id: token.user_id.clone().unwrap_or_else(|| "unknown".to_string()),
                    phone: None,
                    email: None,
                    first_name: None,
                    last_name: None,
                    display_name: None,
                    username: None,
                    avatar_id: None,
                    status: None,
                    is_bot: false,
                    is_premium: false,
                };
                let _ = self.block_on(self.auth_manager.add_account(&token, &placeholder));
            }
        }

        Ok(token)
    }

    /// Try to authenticate using session cookies
    #[allow(dead_code)]
    fn session_auth_flow(&self) -> Result<OAuthToken, String> {
        let dialog = Window::builder()
            .transient_for(&self.parent)
            .modal(true)
            .title("Сессия истекла")
            .default_width(420)
            .css_classes(["auth-window"])
            .build();
        
        let content = GtkBox::new(Orientation::Vertical, 0);
        dialog.set_child(Some(&content));
        content.set_spacing(16);
        content.set_margin_top(40);
        content.set_margin_bottom(40);
        content.set_margin_start(40);
        content.set_margin_end(40);

        let header = Label::builder()
            .label("Сессия истекла")
            .css_classes(["auth-header"])
            .xalign(0.5)
            .build();
        content.append(&header);

        let subtitle = Label::builder()
            .label("Ваша сессия истекла. Войдите заново через Яндекс ID, чтобы продолжить.")
            .css_classes(["auth-subtitle"])
            .xalign(0.5)
            .wrap(true)
            .build();
        content.append(&subtitle);

        // Browser login button
        let browser_btn = gtk::Button::with_label("Войти через Яндекс");
        browser_btn.add_css_class("primary-action");
        browser_btn.set_margin_bottom(8);
        
        let auth_url = self.auth_manager.auth_code_url();
        let auth_url_owned = auth_url.to_string();
        browser_btn.connect_clicked(move |_| {
            let _ = gio::AppInfo::launch_default_for_uri(&auth_url_owned, gio::AppLaunchContext::NONE);
        });
        content.append(&browser_btn);

        // Status label for callback mode
        let status_label = Label::builder()
            .label("Нажмите «Войти через Яндекс» или вставьте Access Token вручную")
            .css_classes(["auth-subtitle"])
            .xalign(0.5)
            .wrap(true)
            .build();
        status_label.set_margin_bottom(8);
        content.append(&status_label);

        let entry = Entry::builder()
            .placeholder_text("Вставьте Access Token из браузера")
            .css_classes(["auth-entry"])
            .hexpand(true)
            .build();
        content.append(&entry);

        let button_box = GtkBox::new(Orientation::Horizontal, 12);
        button_box.set_halign(gtk::Align::Center);
        button_box.set_margin_top(8);

        let response_cell = std::rc::Rc::new(std::cell::RefCell::new(None));
        let main_loop = glib::MainLoop::new(None, false);

        let cancel_btn = gtk::Button::with_label("Отмена");
        cancel_btn.add_css_class("secondary-action");
        let r_cell1 = response_cell.clone();
        let loop_clone1 = main_loop.clone();
        cancel_btn.connect_clicked(move |_| {
            *r_cell1.borrow_mut() = Some(ResponseType::Cancel);
            loop_clone1.quit();
        });
        button_box.append(&cancel_btn);

        let confirm_btn = gtk::Button::with_label("Продолжить");
        confirm_btn.add_css_class("primary-action");
        let r_cell2 = response_cell.clone();
        let loop_clone2 = main_loop.clone();
        confirm_btn.connect_clicked(move |_| {
            *r_cell2.borrow_mut() = Some(ResponseType::Ok);
            loop_clone2.quit();
        });
        button_box.append(&confirm_btn);

        content.append(&button_box);

        let r_cell3 = response_cell.clone();
        let loop_clone3 = main_loop.clone();
        dialog.connect_close_request(move |_| {
            *r_cell3.borrow_mut() = Some(ResponseType::Cancel);
            loop_clone3.quit();
            glib::Propagation::Proceed
        });

        dialog.present();
        main_loop.run();
        let response = response_cell.borrow().clone().unwrap_or(ResponseType::Cancel);
        let value = entry.text().to_string();
        dialog.close();

        if response != ResponseType::Ok {
            return Err("Auth cancelled".to_string());
        }
        if value.trim().is_empty() {
            return Err("Empty token".to_string());
        }
        let code = extract_auth_code(value.trim());
        self.finalize_token(&code)
    }

    /// Show the main auth dialog
    fn show_auth_dialog(&self) -> Result<OAuthToken, String> {
        let auth_url = self.auth_manager.auth_code_url();
        let (client_id, redirect_uri, proxy_url) = self.auth_manager.auth_runtime_info();

        let code = self.obtain_auth_code(&auth_url, &client_id, redirect_uri.as_deref(), proxy_url.as_deref())?;
        self.finalize_token(&code)
    }

    fn select_account(&self, accounts: &[Account]) -> Result<OAuthToken, String> {
        let dialog = Window::builder()
            .transient_for(&self.parent)
            .modal(true)
            .title("Выберите аккаунт")
            .default_width(420)
            .css_classes(["auth-window"])
            .build();
        
        let content = GtkBox::new(Orientation::Vertical, 0);
        dialog.set_child(Some(&content));
        content.set_spacing(16);
        content.set_margin_top(40);
        content.set_margin_bottom(40);
        content.set_margin_start(40);
        content.set_margin_end(40);

        let header = Label::builder()
            .label("Выберите аккаунт")
            .css_classes(["auth-header"])
            .xalign(0.5)
            .build();
        content.append(&header);

        // Account list
        let list_box = GtkBox::new(Orientation::Vertical, 4);
        list_box.set_margin_bottom(16);
        
        let outcome = std::rc::Rc::new(std::cell::RefCell::new(None::<SelectionResult>));
        let main_loop = glib::MainLoop::new(None, false);
        
        // Create account buttons
        for (i, account) in accounts.iter().enumerate() {
            let btn = Button::builder()
                .label(account.display_label())
                .halign(gtk::Align::Start)
                .css_classes(["auth-account-btn"])
                .build();
            let outcome_clone = outcome.clone();
            let loop_clone = main_loop.clone();
            btn.connect_clicked(move |_| {
                *outcome_clone.borrow_mut() = Some(SelectionResult::Account(i));
                loop_clone.quit();
            });
            list_box.append(&btn);
        }
        
        content.append(&list_box);

        // Add new account button
        let add_btn = Button::with_label("Добавить новый аккаунт");
        add_btn.add_css_class("primary-action");
        add_btn.set_margin_top(8);
        let outcome_clone = outcome.clone();
        let loop_clone = main_loop.clone();
        add_btn.connect_clicked(move |_| {
            *outcome_clone.borrow_mut() = Some(SelectionResult::AddNew);
            loop_clone.quit();
        });
        content.append(&add_btn);

        let cancel_btn = Button::with_label("Отмена");
        cancel_btn.add_css_class("secondary-action");
        let outcome_clone = outcome.clone();
        let loop_clone = main_loop.clone();
        cancel_btn.connect_clicked(move |_| {
            *outcome_clone.borrow_mut() = Some(SelectionResult::Cancelled);
            loop_clone.quit();
        });
        
        let button_box = GtkBox::new(Orientation::Horizontal, 12);
        button_box.set_halign(gtk::Align::Center);
        button_box.append(&cancel_btn);
        content.append(&button_box);

        let outcome_clone = outcome.clone();
        let loop_clone = main_loop.clone();
        dialog.connect_close_request(move |_| {
            if outcome_clone.borrow().is_none() {
                *outcome_clone.borrow_mut() = Some(SelectionResult::Cancelled);
            }
            loop_clone.quit();
            glib::Propagation::Proceed
        });

        dialog.present();
        main_loop.run();
        dialog.close();

        let result = outcome.borrow().clone().unwrap_or(SelectionResult::Cancelled);
        match result {
            SelectionResult::Account(idx) => {
                if idx < accounts.len() {
                    let selected_account = accounts[idx].clone();

                    // Switch to selected account
                    self.block_on(self.auth_manager.switch_account(&selected_account.id))
                        .map_err(|e| format!("Switch account failed: {}", e))?;
                    
                    Ok(OAuthToken {
                        access_token: selected_account.access_token.clone(),
                        refresh_token: selected_account.refresh_token.clone(),
                        expires_in: selected_account.expires_at.saturating_sub(
                            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs()
                        ) as u64,
                        token_type: "Bearer".to_string(),
                        user_id: selected_account.display_name.clone(),
                        received_at: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs(),
                    })
                } else {
                    Err("Invalid account selection".to_string())
                }
            }
            SelectionResult::AddNew => {
                self.show_auth_dialog()
            }
            SelectionResult::Cancelled => {
                Err("Auth cancelled".to_string())
            }
        }
    }

    fn obtain_auth_code(
        &self,
        auth_url: &str,
        _client_id: &str,
        _redirect_uri: Option<&str>,
        _proxy_url: Option<&str>,
    ) -> Result<String, String> {
        eprintln!("[AUTH] obtain_auth_code called, url={}", auth_url);

        #[cfg(feature = "in_app_webview")]
        {
            let dialog = Window::builder()
                .transient_for(&self.parent)
                .modal(true)
                .title("Вход через Яндекс ID")
                .default_width(500)
                .default_height(650)
                .css_classes(["auth-window"])
                .build();

            let content = GtkBox::new(Orientation::Vertical, 0);
            dialog.set_child(Some(&content));

            let header_bar = gtk::HeaderBar::new();
            content.append(&header_bar);

            let webview = webkit6::WebView::new();
            webview.load_uri(auth_url);

            let scrolled = gtk::ScrolledWindow::builder()
                .hscrollbar_policy(gtk::PolicyType::Automatic)
                .vscrollbar_policy(gtk::PolicyType::Automatic)
                .vexpand(true)
                .hexpand(true)
                .child(&webview)
                .build();
            content.append(&scrolled);

            let main_loop = glib::MainLoop::new(None, false);
            let outcome = std::rc::Rc::new(std::cell::RefCell::new(None::<Result<String, String>>));

            let outcome_clone = outcome.clone();
            let loop_clone = main_loop.clone();
            webview.connect_load_changed(move |wv, load_event| {
                if load_event == webkit6::LoadEvent::Finished {
                    if let Some(uri) = wv.uri() {
                        let uri_str = uri.to_string();
                        eprintln!("[AUTH] WebView loaded URI: {}", uri_str);
                        if uri_str.contains("access_token=") || uri_str.contains("code=") {
                            let code = extract_auth_code(&uri_str);
                            if !code.is_empty() {
                                eprintln!("[AUTH] Extracted code/token from WebView redirect!");
                                *outcome_clone.borrow_mut() = Some(Ok(code));
                                loop_clone.quit();
                            }
                        }
                    }
                }
            });

            let outcome_close = outcome.clone();
            let loop_close = main_loop.clone();
            dialog.connect_close_request(move |_| {
                if outcome_close.borrow().is_none() {
                    *outcome_close.borrow_mut() = Some(Err("Auth cancelled".to_string()));
                }
                loop_close.quit();
                glib::Propagation::Proceed
            });

            dialog.present();
            main_loop.run();
            dialog.close();

            let result = outcome.borrow_mut().take();
            result.unwrap_or(Err("Auth cancelled".to_string()))
        }

        #[cfg(not(feature = "in_app_webview"))]
        {
            let dialog = Window::builder()
                .transient_for(&self.parent)
                .modal(true)
                .title("Авторизация")
                .default_width(480)
                .default_height(520)
                .css_classes(["auth-window"])
                .build();

            let content = GtkBox::new(Orientation::Vertical, 0);
            dialog.set_child(Some(&content));
            content.set_spacing(14);
            content.set_margin_top(36);
            content.set_margin_bottom(28);
            content.set_margin_start(40);
            content.set_margin_end(40);

            let header = Label::builder()
                .label("Добро пожаловать")
                .css_classes(["auth-header"])
                .xalign(0.5)
                .build();
            content.append(&header);

            let subtitle = Label::builder()
                .label("Войдите через Яндекс ID, чтобы продолжить")
                .css_classes(["auth-subtitle"])
                .xalign(0.5)
                .wrap(true)
                .build();
            content.append(&subtitle);

            // Status label for feedback
            let status_label = Label::builder()
                .label("")
                .css_classes(["auth-status"])
                .xalign(0.5)
                .wrap(true)
                .build();
            status_label.set_margin_top(8);
            status_label.set_margin_bottom(4);
            content.append(&status_label);

            // "Open browser" button
            let login_btn = gtk::Button::with_label("  Открыть браузер для входа  ");
            login_btn.add_css_class("primary-action");
            login_btn.add_css_class("auth-primary");
            login_btn.set_margin_top(8);
            content.append(&login_btn);

            // Instruction text
            let instruction = Label::builder()
                .label("После входа в браузере скопируйте Access Token\nи вставьте его в поле ниже:")
                .css_classes(["auth-subtitle"])
                .xalign(0.5)
                .wrap(true)
                .justify(gtk::Justification::Center)
                .build();
            instruction.set_margin_top(16);
            content.append(&instruction);

            // Token entry — always visible, no expander
            let entry = Entry::builder()
                .placeholder_text("Вставьте Access Token сюда")
                .css_classes(["auth-entry"])
                .hexpand(true)
                .build();
            entry.set_margin_top(8);
            content.append(&entry);

            // Buttons row
            let button_box = GtkBox::new(Orientation::Horizontal, 12);
            button_box.set_halign(gtk::Align::Center);
            button_box.set_margin_top(16);
            button_box.set_homogeneous(true);

            let cancel_btn = gtk::Button::with_label("Отмена");
            cancel_btn.add_css_class("secondary-action");
            let confirm_btn = gtk::Button::with_label("Войти");
            confirm_btn.add_css_class("primary-action");
            confirm_btn.set_sensitive(false);
            button_box.append(&cancel_btn);
            button_box.append(&confirm_btn);
            content.append(&button_box);

            // Enable confirm when user types something
            let confirm_btn_for_entry = confirm_btn.clone();
            entry.connect_changed(move |e| {
                confirm_btn_for_entry.set_sensitive(!e.text().trim().is_empty());
            });

            // Shared result cell
            type AuthOutcome = Result<String, String>;
            let outcome: std::rc::Rc<std::cell::RefCell<Option<AuthOutcome>>> =
                std::rc::Rc::new(std::cell::RefCell::new(None));
            let main_loop = glib::MainLoop::new(None, false);

            // Callback receiver (optional — for automatic token capture from browser)
            let callback_rx: std::rc::Rc<std::cell::RefCell<Option<mpsc::Receiver<String>>>> =
                std::rc::Rc::new(std::cell::RefCell::new(None));

            // Login button — open browser via xdg-open (most reliable on Linux)
            let auth_url_owned = auth_url.to_string();
            let status_weak = status_label.clone();
            let rx_cell_c = callback_rx.clone();
            login_btn.connect_clicked(move |btn| {
                eprintln!("[AUTH] Login button clicked, opening URL: {}", auth_url_owned);

                // Try to start local callback listener (non-fatal if it fails)
                match Self::spawn_callback_listener(&auth_url_owned) {
                    Ok((_url, rx)) => {
                        eprintln!("[AUTH] Callback listener started");
                        *rx_cell_c.borrow_mut() = Some(rx);
                    }
                    Err(e) => {
                        eprintln!("[AUTH] Callback listener failed (non-fatal): {}", e);
                    }
                }

                // Open browser via xdg-open — most reliable method on Linux
                let open_result = std::process::Command::new("xdg-open")
                    .arg(&auth_url_owned)
                    .spawn();

                match open_result {
                    Ok(_) => {
                        eprintln!("[AUTH] xdg-open launched successfully");
                        btn.set_label("  Браузер открыт — войдите там  ");
                        btn.set_sensitive(false);
                        status_weak.set_label("Браузер открыт. Войдите и скопируйте токен.");
                    }
                    Err(e) => {
                        eprintln!("[AUTH] xdg-open failed: {}", e);
                        status_weak.set_label(&format!(
                            "Не удалось открыть браузер.\nОткройте эту ссылку вручную:\n{}",
                            auth_url_owned
                        ));
                    }
                }
            });

            // Cancel
            let outcome_cancel = outcome.clone();
            let main_loop_cancel = main_loop.clone();
            cancel_btn.connect_clicked(move |_| {
                eprintln!("[AUTH] Cancel clicked");
                *outcome_cancel.borrow_mut() = Some(Err("Auth cancelled".to_string()));
                main_loop_cancel.quit();
            });

            // Confirm — take the token from entry
            let entry_for_confirm = entry.clone();
            let outcome_confirm = outcome.clone();
            let main_loop_confirm = main_loop.clone();
            confirm_btn.connect_clicked(move |_| {
                let txt = entry_for_confirm.text().to_string();
                eprintln!("[AUTH] Confirm clicked, text length={}", txt.len());
                let code = extract_auth_code(txt.trim());
                if code.is_empty() {
                    eprintln!("[AUTH] Empty code after extraction");
                } else {
                    eprintln!("[AUTH] Extracted code, length={}", code.len());
                    *outcome_confirm.borrow_mut() = Some(Ok(code));
                    main_loop_confirm.quit();
                }
            });

            // Also accept Enter key in the entry
            let entry_for_activate = entry.clone();
            let outcome_activate = outcome.clone();
            let main_loop_activate = main_loop.clone();
            entry.connect_activate(move |_| {
                let txt = entry_for_activate.text().to_string();
                let code = extract_auth_code(txt.trim());
                if !code.is_empty() {
                    eprintln!("[AUTH] Enter pressed, extracted code length={}", code.len());
                    *outcome_activate.borrow_mut() = Some(Ok(code));
                    main_loop_activate.quit();
                }
            });

            let outcome_close = outcome.clone();
            let main_loop_close = main_loop.clone();
            dialog.connect_close_request(move |_| {
                if outcome_close.borrow().is_none() {
                    *outcome_close.borrow_mut() = Some(Err("Auth cancelled".to_string()));
                }
                main_loop_close.quit();
                glib::Propagation::Proceed
            });

            dialog.present();
            eprintln!("[AUTH] Auth dialog presented");

            // Set up periodic polling for callback channel (100ms interval) to consume 0% CPU
            let main_loop_poll = main_loop.clone();
            let outcome_poll = outcome.clone();
            let callback_rx_poll = callback_rx.clone();
            
            glib::timeout_add_local(std::time::Duration::from_millis(100), move || {
                let mut rx_ref = callback_rx_poll.borrow_mut();
                if let Some(rx) = rx_ref.as_ref() {
                    match rx.try_recv() {
                        Ok(code) => {
                            eprintln!("[AUTH] Got code from callback: length={}", code.len());
                            *rx_ref = None;
                            *outcome_poll.borrow_mut() = Some(Ok(extract_auth_code(&code)));
                            main_loop_poll.quit();
                            return glib::ControlFlow::Break;
                        }
                        Err(mpsc::TryRecvError::Disconnected) => {
                            *rx_ref = None;
                        }
                        Err(mpsc::TryRecvError::Empty) => {}
                    }
                }

                if outcome_poll.borrow().is_some() {
                    return glib::ControlFlow::Break;
                }
                glib::ControlFlow::Continue
            });

            main_loop.run();
            dialog.close();
            let taken = outcome.borrow_mut().take();
            taken.unwrap_or(Err("Auth cancelled".to_string()))
        }
    }

    /// Start a local HTTP server on an ephemeral port and spawn a listener thread
    /// that forwards the first `code` / `access_token` it sees to the receiver.
    /// Returns the OAuth URL to open and the receiver to poll.
    #[allow(dead_code)]
    fn spawn_callback_listener(auth_url: &str) -> Result<(String, mpsc::Receiver<String>), String> {
        let server = Server::http("127.0.0.1:0").map_err(|e| format!("Cannot start server: {}", e))?;
        let _port = match server.server_addr() {
            tiny_http::ListenAddr::IP(addr) => addr.port(),
            tiny_http::ListenAddr::Unix(_) => 0,
        };

        // Check if the original auth_url contains a redirect_uri.
        // If not, we append our local server as redirect_uri. If Yandex rejects it,
        // user will have to use verification_code. We can try to append it.
        // Actually, Yandex will strictly reject 127.0.0.1. We should just use auth_url as is,
        // and if it doesn't redirect to us, the user will paste manually.
        let final_url = auth_url.to_string();

        let (tx, rx) = mpsc::channel::<String>();

        std::thread::spawn(move || {
            // The implicit-grant flow delivers the token in the URL fragment,
            // which is not sent to the server. We serve an HTML page that
            // reads location.hash in the browser and POSTs the value back.
            loop {
                let Ok(mut request) = server.recv() else { break };
                let url = request.url().to_string();
                let method = request.method().as_str().to_string();

                let mut captured: Option<String> = None;
                if let Some((_, query)) = url.split_once('?') {
                    for pair in query.split('&') {
                        if let Some((key, val)) = pair.split_once('=') {
                            if (key == "code" || key == "access_token") && !val.is_empty() {
                                captured = Some(val.to_string());
                                break;
                            }
                        }
                    }
                }

                if captured.is_none() && method == "POST" && url.starts_with("/token") {
                    let mut body = String::new();
                    let _ = request.as_reader().read_to_string(&mut body).ok();
                    for pair in body.split('&') {
                        if let Some((key, val)) = pair.split_once('=') {
                            if (key == "access_token" || key == "code") && !val.is_empty() {
                                captured = Some(val.to_string());
                                break;
                            }
                        }
                    }
                }

                if let Some(code) = captured {
                    let success_html = success_page_html();
                    let _ = request.respond(
                        Response::from_string(success_html)
                            .with_header("Content-Type: text/html; charset=utf-8".parse::<tiny_http::Header>().unwrap()),
                    );
                    let _ = tx.send(code);
                    break;
                }

                // Serve the fragment-capture page for any other GET.
                let capture_html = capture_page_html();
                let _ = request.respond(
                    Response::from_string(capture_html)
                        .with_header("Content-Type: text/html; charset=utf-8".parse::<tiny_http::Header>().unwrap()),
                );
            }
        });

        Ok((final_url, rx))
    }
}

fn extract_auth_code(input: &str) -> String {
    let trimmed = input.trim();
    
    // Check if it's a raw token/code without URL syntax
    if !trimmed.contains('?') && !trimmed.contains('#') && !trimmed.contains('&') && !trimmed.contains('=') {
        return trimmed.to_string();
    }

    // Check fragment first for implicit flow tokens (#access_token=...)
    if let Some((_, fragment)) = trimmed.split_once('#') {
        for pair in fragment.split('&') {
            if let Some((key, val)) = pair.split_once('=') {
                if (key == "access_token" || key == "code") && !val.trim().is_empty() {
                    return val.to_string();
                }
            }
        }
    }

    // Check query params (?code=...)
    if let Some((_, query)) = trimmed.split_once('?') {
        let actual_query = query.split('#').next().unwrap_or(query);
        for pair in actual_query.split('&') {
            if let Some((key, val)) = pair.split_once('=') {
                if (key == "code" || key == "access_token") && !val.trim().is_empty() {
                    return val.to_string();
                }
            }
        }
    }

    // Fallback: return the whole string
    trimmed.to_string()
}


/// HTML page served on the initial callback hit — reads `location.hash`
/// (where the implicit grant drops `access_token=…`) and POSTs it back
/// so the Rust thread can capture it.
#[allow(dead_code)]
fn capture_page_html() -> String {
    r#"<!doctype html>
<html lang="ru"><head><meta charset="utf-8"><title>Yandex Messenger</title>
<style>
  html,body{margin:0;height:100%;background:#0A0A0C;color:#F5F5F7;
    font-family:-apple-system,"Segoe UI",Roboto,sans-serif;}
  .wrap{height:100%;display:flex;align-items:center;justify-content:center;}
  .card{text-align:center;padding:40px 48px;border-radius:20px;
    background:rgba(255,255,255,0.04);border:1px solid rgba(255,255,255,0.08);
    box-shadow:0 24px 64px rgba(0,0,0,0.6);}
  h1{font-size:22px;margin:0 0 8px;font-weight:600;letter-spacing:-0.3px;}
  p{margin:0;color:#98989D;font-size:14px;}
  .spin{width:36px;height:36px;margin:0 auto 20px;border-radius:50%;
    border:3px solid rgba(255,204,0,0.2);border-top-color:#FFCC00;
    animation:s 0.9s linear infinite;}
  @keyframes s{to{transform:rotate(360deg);}}
</style></head>
<body><div class="wrap"><div class="card">
  <div class="spin"></div>
  <h1>Завершаем вход…</h1>
  <p>Можно закрыть эту вкладку через секунду.</p>
</div></div>
<script>
  (function(){
    var h = location.hash.replace(/^#/, '');
    var q = location.search.replace(/^\?/, '');
    var payload = h || q;
    if (!payload) return;
    fetch('/token', {method:'POST',
      headers:{'Content-Type':'application/x-www-form-urlencoded'},
      body: payload});
  })();
</script></body></html>"#.to_string()
}

/// HTML shown after the token has been captured.
#[allow(dead_code)]
fn success_page_html() -> String {
    r#"<!doctype html>
<html lang="ru"><head><meta charset="utf-8"><title>Yandex Messenger</title>
<style>
  html,body{margin:0;height:100%;background:#0A0A0C;color:#F5F5F7;
    font-family:-apple-system,"Segoe UI",Roboto,sans-serif;}
  .wrap{height:100%;display:flex;align-items:center;justify-content:center;}
  .card{text-align:center;padding:44px 56px;border-radius:20px;
    background:rgba(255,255,255,0.04);border:1px solid rgba(255,255,255,0.08);
    box-shadow:0 24px 64px rgba(0,0,0,0.6);}
  h1{font-size:22px;margin:0 0 8px;font-weight:600;letter-spacing:-0.3px;}
  p{margin:0;color:#98989D;font-size:14px;}
  .check{width:56px;height:56px;margin:0 auto 18px;border-radius:50%;
    background:linear-gradient(135deg,#FFCC00,#FF8A00);display:flex;
    align-items:center;justify-content:center;color:#111;font-size:30px;
    font-weight:700;box-shadow:0 8px 24px rgba(255,204,0,0.35);}
</style></head>
<body><div class="wrap"><div class="card">
  <div class="check">✓</div>
  <h1>Вход выполнен</h1>
  <p>Вернитесь в Yandex Messenger — эту вкладку можно закрыть.</p>
</div></div></body></html>"#.to_string()
}
