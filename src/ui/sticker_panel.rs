#![allow(dead_code)]

use gtk::prelude::*;
use gtk::{
    Align, Box as GtkBox, Button, Entry, Orientation, Popover, ScrolledWindow,
};
use std::cell::RefCell;
use std::rc::Rc;

use crate::models::{Sticker, StickerPack};

/// CSS class names for sticker panel widgets.
const CSS_STICKER_PANEL: &str = "sticker-panel";
const CSS_PACK_LIST: &str = "sticker-pack-list";
const CSS_PACK_ITEM: &str = "sticker-pack-item";
const CSS_PACK_ITEM_SELECTED: &str = "sticker-pack-item";
const CSS_PACK_TITLE: &str = "sticker-pack-title";
const CSS_PACK_COUNT: &str = "sticker-pack-count";
const CSS_STICKER_GRID: &str = "sticker-grid";
const CSS_STICKER_ITEM: &str = "sticker-item";
const CSS_SEARCH_ENTRY: &str = "sticker-search-entry";
const CSS_THUMB: &str = "sticker-thumb";

/// StickerPanel — a popover/overlay that displays sticker packs and their stickers.
///
/// Layout:
///   +─────────────────────────────────────────────+
///   |  [Search bar]                                |
///   +───────────────+────────────────────────────+
///   | Packs list    |  Sticker grid               |
///   |  (left)       |  (right)                    |
///   +───────────────+────────────────────────────+
#[derive(Clone)]
pub struct StickerPanel {
    /// Root widget for the panel.
    pub container: GtkBox,
    /// All loaded sticker packs.
    pub packs: RefCell<Vec<StickerPack>>,
    /// Index of the currently selected pack.
    pub selected_pack: RefCell<usize>,
    /// Callback fired when a user selects a sticker.
    pub on_select: RefCell<Option<Rc<dyn Fn(String, String)>>>,
    /// Search entry widget.
    pub search_entry: Entry,
    /// Scrollable list of pack items (left side).
    pub pack_list_box: GtkBox,
    /// Grid of stickers in the selected pack (right side).
    pub sticker_grid: GtkBox,
    /// Optional popover for displaying the panel.
    pub popover: RefCell<Option<Popover>>,
}

impl StickerPanel {
    /// Create a new sticker panel with the given packs.
    pub fn new(packs: Vec<StickerPack>) -> Self {
        let container = GtkBox::new(Orientation::Vertical, 0);
        container.set_css_classes(&[CSS_STICKER_PANEL]);
        container.set_hexpand(true);
        container.set_vexpand(true);

        // ── Search bar ──
        let search_entry = Entry::builder()
            .placeholder_text("Поиск стикеров…")
            .build();
        search_entry.set_css_classes(&[CSS_SEARCH_ENTRY]);
        search_entry.set_hexpand(true);
        search_entry.set_margin_start(12);
        search_entry.set_margin_end(12);
        search_entry.set_margin_top(8);
        search_entry.set_margin_bottom(8);

        // ── Main content ──
        let content = GtkBox::new(Orientation::Horizontal, 0);
        content.set_css_classes(&["sticker-panel-content"]);
        content.set_margin_start(8);
        content.set_margin_end(8);
        content.set_margin_bottom(8);

        // ── Pack list (left) ──
        let pack_list_scroll = ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .build();
        pack_list_scroll.set_min_content_width(180);
        pack_list_scroll.set_css_classes(&["sticker-panel-scroll"]);

        let pack_list_box = GtkBox::new(Orientation::Vertical, 4);
        pack_list_box.set_css_classes(&[CSS_PACK_LIST]);
        pack_list_box.set_margin_top(4);
        pack_list_box.set_margin_start(4);
        pack_list_box.set_margin_end(4);
        pack_list_box.set_margin_bottom(4);
        pack_list_scroll.set_child(Some(&pack_list_box));

        // ── Sticker grid (right) ──
        let sticker_scroll = ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .build();
        sticker_scroll.set_css_classes(&["sticker-panel-scroll"]);
        sticker_scroll.set_hexpand(true);

        let sticker_grid = GtkBox::new(Orientation::Vertical, 8);
        sticker_grid.set_css_classes(&[CSS_STICKER_GRID]);
        sticker_grid.set_margin_top(4);
        sticker_grid.set_margin_start(4);
        sticker_grid.set_margin_end(4);
        sticker_grid.set_margin_bottom(4);
        sticker_scroll.set_child(Some(&sticker_grid));

        content.append(&pack_list_scroll);
        content.append(&sticker_scroll);

        container.append(&search_entry);
        container.append(&content);

        let panel = StickerPanel {
            container,
            packs: RefCell::new(packs),
            selected_pack: RefCell::new(0),
            on_select: RefCell::new(None),
            search_entry,
            pack_list_box,
            sticker_grid,
            popover: RefCell::new(None),
        };

        // Render initial content
        panel.render_packs();

        panel
    }

    /// Render the pack list from the current packs data.
    pub fn render_packs(&self) {
        // Clear existing items
        while let Some(child) = self.pack_list_box.first_child() {
            self.pack_list_box.remove(&child);
        }

        let packs = self.packs.borrow();
        let selected = *self.selected_pack.borrow();

        for (idx, pack) in packs.iter().enumerate() {
            let item = self.create_pack_item(idx, pack);
            self.pack_list_box.append(&item);
        }

        drop(packs);
        self.select_pack(selected);
    }

    /// Create a single pack list item widget.
    fn create_pack_item(&self, idx: usize, pack: &StickerPack) -> GtkBox {
        let item = GtkBox::new(Orientation::Horizontal, 8);
        item.set_css_classes(&[CSS_PACK_ITEM]);
        item.set_hexpand(true);
        item.set_valign(Align::Start);
        item.set_margin_start(4);
        item.set_margin_end(4);
        item.set_margin_top(4);
        item.set_margin_bottom(4);
        item.set_size_request(160, 60);
        item.set_cursor_from_name(Some("pointer"));

        // Thumb
        let thumb = gtk::Image::builder()
            .icon_name("image-missing")
            .build();
        thumb.set_css_classes(&[CSS_THUMB]);
        thumb.set_size_request(50, 50);
        thumb.set_halign(Align::Start);
        thumb.set_valign(Align::Start);

        let thumb_clone = thumb.clone();
        let thumb_url = pack.thumb_url.clone();
        glib::spawn_future_local(async move {
            if !thumb_url.is_empty() {
                let _ = load_sticker_image(&thumb_clone, &thumb_url).await;
            }
        });

        // Title
        let title = gtk::Label::builder()
            .label(&pack.title)
            .lines(1)
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .build();
        title.set_css_classes(&[CSS_PACK_TITLE]);
        title.set_halign(Align::Start);
        title.set_valign(Align::Start);
        title.set_xalign(0.0);
        title.set_hexpand(true);

        // Count
        let count_label = gtk::Label::builder()
            .label(&format!(" {}", pack.sticker_count))
            .build();
        count_label.set_css_classes(&[CSS_PACK_COUNT]);
        count_label.set_halign(Align::End);
        count_label.set_valign(Align::Start);

        item.append(&thumb);
        item.append(&title);
        item.append(&count_label);

        let _item_rc = Rc::new(item.clone());
        let panel_rc = Rc::new(self.clone());
        let _pack_id = pack.pack_id.clone();

        item.set_css_classes(&[CSS_PACK_ITEM]);
        let gesture = gtk::GestureClick::new();
        gesture.connect_pressed(move |_, _, _, _| {
            panel_rc.select_pack(idx);
        });
        item.add_controller(gesture);

        item
    }

    /// Render the sticker grid for a specific pack index.
    pub fn render_stickers(&self, pack_idx: usize) {
        // Clear existing stickers
        while let Some(child) = self.sticker_grid.first_child() {
            self.sticker_grid.remove(&child);
        }

        let packs = self.packs.borrow();
        if pack_idx >= packs.len() {
            drop(packs);
            return;
        }

        let pack = &packs[pack_idx];

        // Add title row
        let title_row = GtkBox::new(Orientation::Horizontal, 0);
        title_row.set_css_classes(&["sticker-grid-header"]);
        title_row.set_margin_start(4);
        title_row.set_margin_end(4);
        title_row.set_margin_top(4);
        title_row.set_margin_bottom(8);
        title_row.set_hexpand(true);

        let pack_title = gtk::Label::builder()
            .label(&pack.title)
            .build();
        pack_title.set_css_classes(&["sticker-grid-title"]);
        pack_title.set_halign(Align::Start);
        pack_title.set_hexpand(true);

        let pack_count = gtk::Label::builder()
            .label(&format!("{} стикеров", pack.sticker_count))
            .build();
        pack_count.set_css_classes(&["sticker-grid-count"]);
        pack_count.set_halign(Align::End);

        title_row.append(&pack_title);
        title_row.append(&pack_count);
        self.sticker_grid.append(&title_row);

        // Render stickers in a vertical layout (Telegram-style)
        let stickers = &pack.stickers;
        for sticker in stickers {
            let btn = self.create_sticker_button(sticker);
            self.sticker_grid.append(&btn);
        }

        drop(packs);
    }

    /// Create a single sticker button.
    fn create_sticker_button(&self, sticker: &Sticker) -> Button {
        let btn = Button::builder().build();
        btn.set_css_classes(&[CSS_STICKER_ITEM]);
        btn.set_size_request(100, 100);
        btn.set_halign(Align::Start);
        btn.set_valign(Align::Start);

        let image = gtk::Image::builder()
            .icon_name("image-missing")
            .build();
        image.set_css_classes(&[CSS_THUMB]);
        image.set_pixel_size(80);
        image.set_halign(Align::Center);
        image.set_valign(Align::Center);
        btn.set_child(Some(&image));

        let image_clone = image.clone();
        let sticker_url = sticker.thumb_url.clone();
        glib::spawn_future_local(async move {
            if !sticker_url.is_empty() {
                let _ = load_sticker_image(&image_clone, &sticker_url).await;
            }
        });

        let sticker_id = sticker.sticker_id.clone();
        let pack_id = sticker.pack_id.clone();
        let btn_rc = Rc::new(btn.clone());

        let panel_rc = Rc::new(self.clone());
        btn.connect_clicked(move |_| {
            panel_rc
                .on_select
                .borrow()
                .as_ref()
                .map(|cb| cb(sticker_id.clone(), pack_id.clone()));
            btn_rc.add_css_class("hover");
            let btn_clone = btn_rc.clone();
            glib::timeout_add_local(std::time::Duration::from_millis(150), move || {
                btn_clone.remove_css_class("hover");
                glib::ControlFlow::Break
            });
        });

        btn
    }

    /// Select a pack by index and re-render the sticker grid.
    pub fn select_pack(&self, idx: usize) {
        *self.selected_pack.borrow_mut() = idx;

        // Update pack item highlights
        if let Some(child) = self.pack_list_box.first_child() {
            let mut current = Some(child);
            let mut count = 0;
            while let Some(ref mut node) = current {
                let widget = node.clone();
                if count == idx {
                    widget.set_css_classes(&[CSS_PACK_ITEM_SELECTED]);
                } else {
                    widget.set_css_classes(&[CSS_PACK_ITEM]);
                }
                count += 1;
                current = node.next_sibling();
            }
        }

        self.render_stickers(idx);
    }

    /// Filter stickers by search query.
    pub fn search(&self, query: &str) {
        let pack_idx = *self.selected_pack.borrow();
        let packs = self.packs.borrow();
        if packs.is_empty() {
            return;
        }

        let pack_title = packs[pack_idx].title.clone();
        let filtered: Vec<_> = packs[pack_idx]
            .stickers
            .iter()
            .filter(|s| {
                s.emoji.to_lowercase().contains(&query.to_lowercase())
                    || s.pack_id.to_lowercase().contains(&query.to_lowercase())
            })
            .cloned()
            .collect();

        drop(packs);

        // Replace stickers in grid
        while let Some(child) = self.sticker_grid.first_child() {
            self.sticker_grid.remove(&child);
        }

        // Re-add title row
        let title_row = GtkBox::new(Orientation::Horizontal, 0);
        title_row.set_css_classes(&["sticker-grid-header"]);
        title_row.set_margin_start(4);
        title_row.set_margin_end(4);
        title_row.set_margin_top(4);
        title_row.set_margin_bottom(8);
        title_row.set_hexpand(true);

        let pack_title_label = gtk::Label::builder()
            .label(&pack_title)
            .build();
        pack_title_label.set_css_classes(&["sticker-grid-title"]);
        pack_title_label.set_halign(Align::Start);
        pack_title_label.set_hexpand(true);

        let pack_count = gtk::Label::builder()
            .label(&format!("{} стикеров", filtered.len()))
            .build();
        pack_count.set_css_classes(&["sticker-grid-count"]);
        pack_count.set_halign(Align::End);

        title_row.append(&pack_title_label);
        title_row.append(&pack_count);
        self.sticker_grid.append(&title_row);

        for sticker in filtered {
            let btn = self.create_sticker_button(&sticker);
            self.sticker_grid.append(&btn);
        }
    }

    /// Update the panel with new pack data.
    pub fn update_packs(&self, packs: Vec<StickerPack>) {
        *self.packs.borrow_mut() = packs;
        self.render_packs();
    }

    /// Set the callback for sticker selection events.
    pub fn on_select(&self, callback: impl Fn(String, String) + 'static) {
        *self.on_select.borrow_mut() = Some(Rc::new(callback));
    }

    /// Get the container widget to add to a parent.
    pub fn container(&self) -> &GtkBox {
        &self.container
    }

    /// Show the panel (as popover or inline).
    pub fn show(&self) {
        self.container.set_visible(true);
    }

    /// Hide the panel.
    pub fn hide(&self) {
        self.container.set_visible(false);
    }

    /// Set the visibility of the panel.
    pub fn set_visible(&self, visible: bool) {
        self.container.set_visible(visible);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_pack() -> StickerPack {
        StickerPack {
            pack_id: "test-pack".to_string(),
            title: "Test Pack".to_string(),
            stickers: vec![],
            is_installed: true,
            is_featured: false,
            category: "Emojis".to_string(),
            thumb_url: "https://example.com/thumb.webp".to_string(),
            sticker_count: 5,
        }
    }

    #[test]
    fn test_new_panel() {
        let packs = vec![sample_pack(), sample_pack()];
        let panel = StickerPanel::new(packs);
        assert!(panel.container().is_visible());
    }

    #[test]
    fn test_select_pack() {
        let packs = vec![
            StickerPack { pack_id: "a".to_string(), ..sample_pack() },
            StickerPack { pack_id: "b".to_string(), ..sample_pack() },
            StickerPack { pack_id: "c".to_string(), ..sample_pack() },
        ];
        let panel = StickerPanel::new(packs);
        panel.select_pack(2);
        assert_eq!(*panel.selected_pack.borrow(), 2);
    }

    #[test]
    fn test_update_packs() {
        let packs = vec![sample_pack()];
        let panel = StickerPanel::new(packs);
        let new_packs = vec![sample_pack(), sample_pack()];
        panel.update_packs(new_packs);
        assert_eq!(panel.packs.borrow().len(), 2);
    }
}

async fn load_sticker_image(img: &gtk::Image, url: &str) -> Result<(), String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .map_err(|e| format!("Failed to create client: {}", e))?;
    
    let response = client
        .get(url)
        .header("User-Agent", "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
        .send()
        .await
        .map_err(|e| format!("Failed to fetch: {}", e))?;
    
    if !response.status().is_success() {
        return Err(format!("Fetch failed: HTTP {}", response.status()));
    }
    
    let bytes = response.bytes().await.map_err(|e| format!("Failed to read bytes: {}", e))?;
    let bytes_glib = glib::Bytes::from(&bytes);
    match gtk::gdk::Texture::from_bytes(&bytes_glib) {
        Ok(texture) => {
            img.set_from_paintable(Some(&texture));
        }
        Err(e) => {
            return Err(format!("Failed to load texture: {}", e));
        }
    }
    Ok(())
}
