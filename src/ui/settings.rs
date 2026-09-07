#![allow(dead_code)]

use gtk::prelude::*;
use gtk::{Box as GtkBox, CheckButton, Label, Orientation, Window};
use libadwaita as adw;
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
            // Yandex official look is light; night stays one toggle away.
            dark_theme: false,
            notifications_enabled: true,
            minimize_to_tray: true,
            reduced_motion: false,
        }
    }
}

/// Apply a theme variant: token palette + shared structure in ONE provider
/// (concatenated, so token order is deterministic).
pub fn apply_theme(dark: bool) {
    thread_local! {
        static PROVIDER: std::cell::RefCell<Option<gtk::CssProvider>> =
            const { std::cell::RefCell::new(None) };
    }

    let tokens = if dark {
        include_str!("theme-tokens-night.css")
    } else {
        include_str!("theme-tokens-light.css")
    };
    let css = format!("{tokens}\n{}", include_str!("theme.css"));
    PROVIDER.with(|cell| {
        if cell.borrow().is_none() {
            let p = gtk::CssProvider::new();
            if let Some(display) = gtk::gdk::Display::default() {
                gtk::style_context_add_provider_for_display(
                    &display,
                    &p,
                    gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
                );
            }
            *cell.borrow_mut() = Some(p);
        }
        cell.borrow()
            .as_ref()
            .expect("theme provider")
            .load_from_string(&css);
    });
    if let Some(settings) = gtk::Settings::default() {
        settings.set_gtk_application_prefer_dark_theme(dark);
    }
    // Libadwaita ignores prefer-dark-theme (see runtime warning) and follows
    // the system color-scheme instead — force ours so its base widgets
    // (list views, sidebars,一分 dialogs) match the active palette.
    adw::StyleManager::default().set_color_scheme(if dark {
        adw::ColorScheme::ForceDark
    } else {
        adw::ColorScheme::ForceLight
    });
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
            apply_theme(active);
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
        assert_eq!(default_settings.dark_theme, false);
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

    fn token_names(css: &str) -> std::collections::BTreeSet<String> {
        css.lines()
            .filter_map(|l| {
                let l = l.trim_start();
                l.strip_prefix("@define-color")
                    .map(|rest| rest.split_whitespace().next().unwrap_or("").to_string())
            })
            .filter(|n| !n.is_empty())
            .collect()
    }

    fn token_values(css: &str) -> std::collections::BTreeMap<String, String> {
        css.lines()
            .filter_map(|l| {
                let l = l.trim_start();
                let rest = l.strip_prefix("@define-color")?;
                let mut parts = rest.split_whitespace();
                let name = parts.next()?.to_string();
                let value: String = parts.collect::<Vec<_>>().join(" ");
                let value = value.split(';').next()?.trim().to_string();
                Some((name, value))
            })
            .collect()
    }

    /// sRGB hex → relative luminance (WCAG).
    fn luminance(hex: &str) -> Option<f64> {
        let hex = hex.strip_prefix('#')?;
        if hex.len() != 6 {
            return None;
        }
        let channel = |i: usize| {
            let v = u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).ok()? as f64 / 255.0;
            Some(if v <= 0.03928 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            })
        };
        Some(0.2126 * channel(0)? + 0.7152 * channel(1)? + 0.0722 * channel(2)?)
    }

    fn contrast(fg: &str, bg: &str) -> Option<f64> {
        let (l1, l2) = (luminance(fg)?, luminance(bg)?);
        let (hi, lo) = if l1 > l2 { (l1, l2) } else { (l2, l1) };
        Some((hi + 0.05) / (lo + 0.05))
    }

    #[test]
    fn test_default_is_light() {
        // Yandex official look is light out of the box.
        assert!(!AppSettings::default().dark_theme);
    }

    #[test]
    fn test_token_files_parity() {
        // Both palettes must define exactly the same tokens or structural
        // CSS breaks on one variant.
        let night = include_str!("theme-tokens-night.css");
        let light = include_str!("theme-tokens-light.css");
        let night_names = token_names(night);
        let light_names = token_names(light);
        assert!(!night_names.is_empty());
        assert_eq!(night_names, light_names);
        // Structural CSS must not define tokens of its own (single source).
        let structural = include_str!("theme.css");
        assert!(
            token_names(structural).is_empty(),
            "tokens belong in theme-tokens-*.css"
        );
        // Every @token used must be defined (at-rules like @keyframes and
        // @media are not color tokens).
        let at_rules = [
            "media",
            "import",
            "charset",
            "keyframes",
            "font-face",
            "supports",
        ];
        let used: std::collections::BTreeSet<String> = structural
            .split('@')
            .skip(1)
            .filter_map(|s| {
                s.split(|c: char| !(c.is_alphanumeric() || c == '_' || c == '-'))
                    .next()
                    .filter(|n| !n.is_empty() && !at_rules.contains(n))
                    .map(str::to_string)
            })
            .collect();
        let undefined: Vec<_> = used.difference(&night_names).collect();
        assert!(
            undefined.is_empty(),
            "undefined tokens used by theme.css: {undefined:?}"
        );
    }

    #[test]
    fn test_apply_theme_both_variants() {
        // Provider load must not crash on either palette (parse warnings,
        // if any, go to the log, not the test).
        crate::ui::run_gtk_test(|| {
            apply_theme(true);
            apply_theme(false);
            // Leave the suite in the default (light) state.
            apply_theme(AppSettings::default().dark_theme);
        });
    }

    /// WCAG contrast of the pairs that carry message text. Catches
    /// light-text-on-light-bubble regressions when palettes evolve.
    /// Thresholds: 4.5 body text, 3.0 secondary/timestamps.
    #[test]
    fn test_no_invalid_gtk_properties() {
        // GtkCssProvider only warns on unknown properties (see test log),
        // so ban the web-CSS strays structurally. Allowed: min-/max- forms.
        let structural = include_str!("theme.css");
        let banned = [
            "display:",
            "align-items:",
            "justify-content:",
            "flex:",
            "flex-direction:",
            "grid-",
            "margin-end:",
            "max-width:",
            "overflow:",
        ];
        let mut hits = Vec::new();
        for (i, line) in structural.lines().enumerate() {
            let code = line.split("/*").next().unwrap_or("");
            for prop in banned {
                // Match `name:` at declaration start (not min-/max- prefixed).
                let decl = code.trim_start();
                if decl.starts_with(prop) || decl.starts_with(&format!("*{prop}")) {
                    hits.push((i + 1, line.trim().to_string()));
                }
            }
        }
        assert!(hits.is_empty(), "non-GTK properties: {hits:?}");
    }
    #[test]
    fn test_token_contrast() {
        let palettes = [
            ("night", include_str!("theme-tokens-night.css")),
            ("light", include_str!("theme-tokens-light.css")),
        ];
        // (fg, bg, min_ratio)
        let pairs = [
            ("text_primary", "bg_chat", 4.5),
            ("text_primary", "bg_sidebar", 4.5),
            ("text_primary", "bg_composer", 4.5),
            ("text_primary", "bubble_received", 4.5),
            ("msg_out_text", "bubble_sent", 4.5),
            ("unread_fg", "unread_bg", 3.0),
            ("text_on_selected", "bg_selected", 3.0),
            ("avatar_text", "avatar_bg", 3.0),
            ("text_secondary", "bg_chat", 3.0),
            ("msg_out_subtle", "bubble_sent", 3.0),
            ("text_on_brand", "brand_yellow", 3.0),
        ];
        for (which, css) in palettes {
            let values = token_values(css);
            for (fg, bg, min) in pairs {
                let f = values
                    .get(fg)
                    .unwrap_or_else(|| panic!("{which}: missing token {fg}"));
                let b = values
                    .get(bg)
                    .unwrap_or_else(|| panic!("{which}: missing token {bg}"));
                let ratio = contrast(f, b)
                    .unwrap_or_else(|| panic!("{which}: non-hex pair {fg}={f} on {bg}={b}"));
                assert!(
                    ratio >= min,
                    "{which}: {fg} on {bg} = {ratio:.2} (need {min})"
                );
            }
        }
    }
}
