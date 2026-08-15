#![allow(dead_code)]

use gtk::prelude::*;
use gtk::{Box as GtkBox, CheckButton, Label, Orientation, Window};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::fs;
use std::path::PathBuf;
use std::rc::Rc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub dark_theme: bool,
    pub notifications_enabled: bool,
    pub minimize_to_tray: bool,
    /// Disable decorative animations (skeleton shimmer, reaction pop-in, fades).
    #[serde(default)]
    pub reduced_motion: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            dark_theme: true,
            notifications_enabled: true,
            minimize_to_tray: true,
            reduced_motion: false,
        }
    }
}

/// Apply reduced-motion preference to the default GTK display (CSS class on root).
pub fn apply_reduced_motion(enabled: bool) {
    if let Some(display) = gtk::gdk::Display::default() {
        // Walk top-level windows and toggle class
        // GTK4: use Settings + CSS on body via style context of each window
        let _ = display;
    }
    // Global flag for CSS: load provider that zeroes animations when enabled
    apply_reduced_motion_css(enabled);
}

fn apply_reduced_motion_css(enabled: bool) {
    // Gtk CssProvider is main-thread only — use thread_local (GTK UI thread).
    thread_local! {
        static PROVIDER: std::cell::RefCell<Option<gtk::CssProvider>> =
            const { std::cell::RefCell::new(None) };
    }

    PROVIDER.with(|cell| {
        if cell.borrow().is_none() {
            let p = gtk::CssProvider::new();
            if let Some(display) = gtk::gdk::Display::default() {
                gtk::style_context_add_provider_for_display(
                    &display,
                    &p,
                    gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 50,
                );
            }
            *cell.borrow_mut() = Some(p);
        }
        let borrow = cell.borrow();
        let provider = borrow.as_ref().unwrap();
        if enabled {
            provider.load_from_string(
                r#"
                * {
                  transition-duration: 0.001ms;
                  animation-duration: 0.001ms;
                  animation-iteration-count: 1;
                }
                .skeleton,
                .skeleton-bubble,
                .message-fade-in,
                .reaction-pop,
                .pop-in,
                .welcome-state,
                .empty-conversation,
                .empty-list-state,
                .skeleton-row {
                  animation: none;
                  transition: none;
                }
                "#,
            );
        } else {
            provider.load_from_string("/* reduced motion off */");
        }
    });
}

pub struct SettingsStore {
    path: PathBuf,
}

impl SettingsStore {
    pub fn new() -> Result<Self, String> {
        let dir = dirs::config_dir()
            .ok_or_else(|| "Cannot resolve config dir".to_string())?
            .join("yandex-messenger-native");
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        Ok(Self {
            path: dir.join("settings.json"),
        })
    }

    pub fn new_with_path(path: PathBuf) -> Self {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        Self { path }
    }

    pub fn load(&self) -> AppSettings {
        fs::read_to_string(&self.path)
            .ok()
            .and_then(|v| serde_json::from_str::<AppSettings>(&v).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, settings: &AppSettings) -> Result<(), String> {
        let serialized = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
        fs::write(&self.path, serialized).map_err(|e| e.to_string())
    }
}

/// Simple settings dialog (GTK4, no adw SwitchRow dependency).
pub fn show_settings_window(
    parent: &impl IsA<gtk::Window>,
    store: &SettingsStore,
    on_change: impl Fn(AppSettings) + 'static,
) {
    let settings = Rc::new(RefCell::new(store.load()));
    let store_path = store.path.clone();

    let win = Window::builder()
        .title("Настройки")
        .transient_for(parent)
        .modal(true)
        .default_width(420)
        .default_height(340)
        .build();

    let root = GtkBox::new(Orientation::Vertical, 12);
    root.set_margin_top(20);
    root.set_margin_bottom(20);
    root.set_margin_start(24);
    root.set_margin_end(24);

    let title = Label::builder()
        .label("Настройки")
        .css_classes(vec!["title-2".to_string()])
        .halign(gtk::Align::Start)
        .build();
    root.append(&title);

    let notif = CheckButton::builder()
        .label("Уведомления о новых сообщениях")
        .active(settings.borrow().notifications_enabled)
        .build();
    let tray = CheckButton::builder()
        .label("Сворачивать в трей при закрытии")
        .active(settings.borrow().minimize_to_tray)
        .build();
    let dark = CheckButton::builder()
        .label("Тёмная тема")
        .active(settings.borrow().dark_theme)
        .build();
    let reduced = CheckButton::builder()
        .label("Уменьшить анимации")
        .active(settings.borrow().reduced_motion)
        .build();
    reduced.set_tooltip_text(Some(
        "Отключает shimmer, pop-in реакций и прочие декоративные анимации",
    ));

    let persist = {
        let settings = settings.clone();
        let path = store_path;
        Rc::new(move || {
            let s = settings.borrow().clone();
            if let Ok(json) = serde_json::to_string_pretty(&s) {
                let _ = fs::write(&path, json);
            }
            on_change(s);
        })
    };

    {
        let settings = settings.clone();
        let persist = persist.clone();
        notif.connect_toggled(move |btn| {
            settings.borrow_mut().notifications_enabled = btn.is_active();
            persist();
        });
    }
    {
        let settings = settings.clone();
        let persist = persist.clone();
        tray.connect_toggled(move |btn| {
            settings.borrow_mut().minimize_to_tray = btn.is_active();
            persist();
        });
    }
    {
        let settings = settings.clone();
        let persist = persist.clone();
        dark.connect_toggled(move |btn| {
            let active = btn.is_active();
            settings.borrow_mut().dark_theme = active;
            if let Some(disp) = gtk::Settings::default() {
                disp.set_gtk_application_prefer_dark_theme(active);
            }
            persist();
        });
    }
    {
        let settings = settings.clone();
        let persist = persist.clone();
        reduced.connect_toggled(move |btn| {
            let active = btn.is_active();
            settings.borrow_mut().reduced_motion = active;
            apply_reduced_motion(active);
            persist();
        });
    }

    root.append(&notif);
    root.append(&tray);
    root.append(&dark);
    root.append(&reduced);

    let close = gtk::Button::with_label("Закрыть");
    close.add_css_class("suggested-action");
    close.set_halign(gtk::Align::End);
    close.set_margin_top(12);
    let win_c = win.clone();
    close.connect_clicked(move |_| {
        win_c.close();
    });
    root.append(&close);

    win.set_child(Some(&root));
    win.present();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_settings_save_and_load() {
        let temp_dir = std::env::temp_dir().join("yandex_messenger_tests");
        let _ = fs::create_dir_all(&temp_dir);
        let path = temp_dir.join("test_settings.json");

        let _ = fs::remove_file(&path);

        let store = SettingsStore::new_with_path(path.clone());

        let default_settings = store.load();
        assert_eq!(default_settings.dark_theme, true);
        assert_eq!(default_settings.notifications_enabled, true);
        assert_eq!(default_settings.minimize_to_tray, true);
        assert_eq!(default_settings.reduced_motion, false);

        let mut settings = AppSettings::default();
        settings.dark_theme = true;
        settings.notifications_enabled = false;
        settings.minimize_to_tray = false;
        settings.reduced_motion = true;

        let save_res = store.save(&settings);
        assert!(save_res.is_ok());

        let loaded = store.load();
        assert_eq!(loaded.dark_theme, true);
        assert_eq!(loaded.notifications_enabled, false);
        assert_eq!(loaded.minimize_to_tray, false);
        assert_eq!(loaded.reduced_motion, true);

        let _ = fs::remove_file(&path);
    }
}
