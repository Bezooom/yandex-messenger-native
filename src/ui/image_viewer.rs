#![allow(dead_code)]

use gtk::prelude::*;
use gtk::{Box as GtkBox, Button, Orientation, Popover, ScrolledWindow};
use std::cell::RefCell;
use std::rc::Rc;

/// Image viewer overlay shown on top of chat view
pub struct ImageViewer {
    pub container: ScrolledWindow,
    image: gtk::Image,
    current_url: RefCell<String>,
    current_filename: RefCell<String>,
    zoom_level: RefCell<f64>,
    closed: RefCell<bool>,
    /// Image index in a swipeable sequence
    image_index: RefCell<usize>,
    image_count: RefCell<usize>,
    on_navigate: RefCell<Option<Rc<dyn Fn(usize)>>>,
}

impl ImageViewer {
    pub fn new() -> Self {
        let container = ScrolledWindow::builder()
            .hexpand(true)
            .vexpand(true)
            .build();
        container.set_css_classes(&["image-viewer"]);

        let image = gtk::Image::new();
        image.set_halign(gtk::Align::Center);
        image.set_valign(gtk::Align::Center);
        image.set_css_classes(&["viewer-image"]);

        container.set_child(Some(&image));

        Self {
            container,
            image,
            current_url: RefCell::new(String::new()),
            current_filename: RefCell::new(String::new()),
            zoom_level: RefCell::new(1.0),
            closed: RefCell::new(false),
            image_index: RefCell::new(0),
            image_count: RefCell::new(0),
            on_navigate: RefCell::new(None),
        }
    }

    pub fn show(&self, url: &str, filename: &str) {
        *self.zoom_level.borrow_mut() = 1.0;
        *self.closed.borrow_mut() = false;
        *self.current_url.borrow_mut() = url.to_string();
        *self.current_filename.borrow_mut() = filename.to_string();
        // Placeholder until bytes arrive. NOTE: gtk Image::set_from_file
        // cannot fetch remote URLs (that path rendered as a red
        // "broken image" icon) — download with messenger auth instead.
        self.image
            .set_from_icon_name(Some("image-x-generic-symbolic"));
        self.image.set_pixel_size(96);
        let img = self.image.clone();
        let url_owned = url.to_string();
        glib::spawn_future_local(async move {
            match fetch_viewer_bytes(&url_owned).await {
                Ok(bytes) => {
                    if let Err(e) = set_viewer_bytes(&img, &bytes) {
                        log::warn!("Image viewer decode failed: {}", e);
                        img.set_from_icon_name(Some("image-missing-symbolic"));
                    }
                }
                Err(e) => {
                    log::warn!("Image viewer download failed: {}", e);
                    img.set_from_icon_name(Some("image-missing-symbolic"));
                    img.set_tooltip_text(Some(&format!("Не загрузилось: {}", e)));
                }
            }
        });

        // Add swipe gesture for navigation
        let swipe = gtk::GestureSwipe::new();
        let ctrl = Rc::new(ImageViewer {
            container: self.container.clone(),
            image: self.image.clone(),
            current_url: self.current_url.clone(),
            current_filename: self.current_filename.clone(),
            zoom_level: self.zoom_level.clone(),
            closed: self.closed.clone(),
            image_index: self.image_index.clone(),
            image_count: self.image_count.clone(),
            on_navigate: self.on_navigate.clone(),
        });
        let ctrl2 = ctrl.clone();
        swipe.connect_swipe(move |_swipe, dx, _dy| {
            if dx.abs() > 50.0 {
                if dx < 0.0 {
                    ctrl2.prev_image();
                } else {
                    ctrl2.next_image();
                }
            }
        });
        self.container.add_controller(swipe);
    }

    pub fn close(&self) {
        *self.closed.borrow_mut() = true;
    }

    pub fn is_closed(&self) -> bool {
        *self.closed.borrow()
    }

    pub fn zoom_in(&self) {
        let mut level = self.zoom_level.borrow_mut();
        *level = (*level * 1.2).min(5.0);
    }

    pub fn zoom_out(&self) {
        let mut level = self.zoom_level.borrow_mut();
        *level = (*level / 1.2).max(0.1);
    }

    pub fn reset_zoom(&self) {
        *self.zoom_level.borrow_mut() = 1.0;
    }

    /// Download the current image to a file
    pub fn download(&self) -> Option<std::path::PathBuf> {
        let filename = self.current_filename.borrow().clone();
        let url = self.current_url.borrow().clone();

        // Use the filename or generate one from URL
        let file_name = if filename.is_empty() {
            url.rsplit('/').next().unwrap_or("image.jpg")
        } else {
            &filename
        };

        let save_path = dirs::download_dir()
            .unwrap_or_else(|| dirs::home_dir().unwrap_or_default().join("Downloads"))
            .join(file_name);

        // Spawn async download
        let url_clone = url.clone();
        let save_path_clone = save_path.clone();
        glib::spawn_future_local(async move {
            log::info!("Downloading image to: {}", save_path_clone.display());
            match reqwest::get(&url_clone).await {
                Ok(response) => {
                    if let Ok(bytes) = response.bytes().await {
                        if let Err(e) = std::fs::write(&save_path_clone, bytes) {
                            log::error!("Failed to write image: {}", e);
                        } else {
                            log::info!("Image downloaded successfully");
                        }
                    }
                }
                Err(e) => {
                    log::error!("Failed to download image: {}", e);
                }
            }
        });

        Some(save_path)
    }

    /// Navigate to previous image
    pub fn prev_image(&self) {
        let idx = *self.image_index.borrow();
        let count = *self.image_count.borrow();
        if idx > 0 {
            let new_idx = idx - 1;
            *self.image_index.borrow_mut() = new_idx;
            log::info!("Swiped to image {}/{}", new_idx + 1, count);
            if let Some(cb) = self.on_navigate.borrow().as_ref() {
                cb(new_idx);
            }
        }
    }

    /// Navigate to next image
    pub fn next_image(&self) {
        let idx = *self.image_index.borrow();
        let count = *self.image_count.borrow();
        if idx < count - 1 {
            let new_idx = idx + 1;
            *self.image_index.borrow_mut() = new_idx;
            log::info!("Swiped to image {}/{}", new_idx + 1, count);
            if let Some(cb) = self.on_navigate.borrow().as_ref() {
                cb(new_idx);
            }
        }
    }

    /// Set swipe navigation state
    pub fn set_image_sequence(
        &self,
        count: usize,
        current_idx: usize,
        on_navigate: impl Fn(usize) + 'static,
    ) {
        *self.image_count.borrow_mut() = count;
        *self.image_index.borrow_mut() = current_idx;
        *self.on_navigate.borrow_mut() = Some(Rc::new(on_navigate));
    }

    /// Create controls popover with zoom buttons, download, and close
    pub fn controls_popover(&self) -> Popover {
        let container = &self.container;
        let image = &self.image;
        let current_url = &self.current_url;
        let current_filename = &self.current_filename;
        let zoom_level = &self.zoom_level;
        let closed = &self.closed;
        let image_index = &self.image_index;
        let image_count = &self.image_count;
        let on_navigate = &self.on_navigate;

        let popover = Popover::builder().has_arrow(false).build();

        let box_ = GtkBox::new(Orientation::Horizontal, 4);
        box_.set_css_classes(&["image-controls"]);

        let zoom_in = Button::builder().icon_name("zoom-in-symbolic").build();
        let ctrl = Rc::new(ImageViewer {
            container: container.clone(),
            image: image.clone(),
            current_url: current_url.clone(),
            current_filename: current_filename.clone(),
            zoom_level: zoom_level.clone(),
            closed: closed.clone(),
            image_index: image_index.clone(),
            image_count: image_count.clone(),
            on_navigate: on_navigate.clone(),
        });
        let ctrl2 = ctrl.clone();
        zoom_in.connect_clicked(move |_| {
            ctrl2.zoom_in();
        });

        let zoom_out = Button::builder().icon_name("zoom-out-symbolic").build();
        let ctrl = Rc::new(ImageViewer {
            container: container.clone(),
            image: image.clone(),
            current_url: current_url.clone(),
            current_filename: current_filename.clone(),
            zoom_level: zoom_level.clone(),
            closed: closed.clone(),
            image_index: image_index.clone(),
            image_count: image_count.clone(),
            on_navigate: on_navigate.clone(),
        });
        let ctrl2 = ctrl.clone();
        zoom_out.connect_clicked(move |_| {
            ctrl2.zoom_out();
        });

        let reset = Button::builder()
            .icon_name("zoom-fit-best-symbolic")
            .build();
        let ctrl = Rc::new(ImageViewer {
            container: container.clone(),
            image: image.clone(),
            current_url: current_url.clone(),
            current_filename: current_filename.clone(),
            zoom_level: zoom_level.clone(),
            closed: closed.clone(),
            image_index: image_index.clone(),
            image_count: image_count.clone(),
            on_navigate: on_navigate.clone(),
        });
        let ctrl2 = ctrl.clone();
        reset.connect_clicked(move |_| {
            ctrl2.reset_zoom();
        });

        let download = Button::builder()
            .icon_name("object-download-symbolic")
            .build();
        download.set_tooltip_text(Some("Download"));
        let ctrl = Rc::new(ImageViewer {
            container: container.clone(),
            image: image.clone(),
            current_url: current_url.clone(),
            current_filename: current_filename.clone(),
            zoom_level: zoom_level.clone(),
            closed: closed.clone(),
            image_index: image_index.clone(),
            image_count: image_count.clone(),
            on_navigate: on_navigate.clone(),
        });
        let ctrl2 = ctrl.clone();
        download.connect_clicked(move |_| {
            let path = ctrl2.download();
            if let Some(p) = path {
                log::info!("Image saved to: {}", p.display());
            }
        });

        let prev = Button::builder().icon_name("go-previous-symbolic").build();
        let ctrl = Rc::new(ImageViewer {
            container: container.clone(),
            image: image.clone(),
            current_url: current_url.clone(),
            current_filename: current_filename.clone(),
            zoom_level: zoom_level.clone(),
            closed: closed.clone(),
            image_index: image_index.clone(),
            image_count: image_count.clone(),
            on_navigate: on_navigate.clone(),
        });
        let ctrl2 = ctrl.clone();
        prev.connect_clicked(move |_| {
            ctrl2.prev_image();
        });

        let next = Button::builder().icon_name("go-next-symbolic").build();
        let ctrl = Rc::new(ImageViewer {
            container: container.clone(),
            image: image.clone(),
            current_url: current_url.clone(),
            current_filename: current_filename.clone(),
            zoom_level: zoom_level.clone(),
            closed: closed.clone(),
            image_index: image_index.clone(),
            image_count: image_count.clone(),
            on_navigate: on_navigate.clone(),
        });
        let ctrl2 = ctrl.clone();
        next.connect_clicked(move |_| {
            ctrl2.next_image();
        });

        let close = Button::builder().icon_name("window-close-symbolic").build();
        let ctrl = Rc::new(ImageViewer {
            container: container.clone(),
            image: image.clone(),
            current_url: current_url.clone(),
            current_filename: current_filename.clone(),
            zoom_level: zoom_level.clone(),
            closed: closed.clone(),
            image_index: image_index.clone(),
            image_count: image_count.clone(),
            on_navigate: on_navigate.clone(),
        });
        let ctrl2 = ctrl.clone();
        close.connect_clicked(move |_| {
            ctrl2.close();
        });

        box_.append(&prev);
        box_.append(&zoom_in);
        box_.append(&zoom_out);
        box_.append(&reset);
        box_.append(&download);
        box_.append(&next);
        box_.append(&close);

        popover.set_child(Some(&box_));
        popover
    }
}

/// Viewer download cap (full-res view, but no multi-100MB surprises).
const VIEWER_DOWNLOAD_MAX_BYTES: usize = 40 * 1024 * 1024;
/// Downscale side for gigantic images so the viewer never OOMs.
const VIEWER_MAX_SIDE: i32 = 2048;

/// URL fallback variants: both file hosts (net/ru).
fn viewer_url_variants(url: &str) -> Vec<String> {
    let mut out = vec![url.to_string()];
    const HOSTS: [&str; 2] = [
        crate::config::FILE_PUBLIC_HOST,
        crate::config::FILE_PRIVATE_HOST,
    ];
    for host in HOSTS {
        let host = host.trim_end_matches('/');
        if url.starts_with(host) {
            for other in HOSTS {
                let other = other.trim_end_matches('/');
                if other != host {
                    out.push(url.replacen(host, other, 1));
                }
            }
        }
    }
    out
}

/// Download image bytes with messenger auth (OAuth + session cookies).
async fn fetch_viewer_bytes(url: &str) -> Result<Vec<u8>, String> {
    if url.starts_with("file:") {
        if let Ok((path, _)) = glib::filename_from_uri(url) {
            let bytes =
                std::fs::read(&path).map_err(|e| format!("local read: {}", e))?;
            if bytes.is_empty() {
                return Err("empty local file".into());
            }
            return Ok(bytes);
        }
    }
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return Err("unsupported url".into());
    }
    let (oauth, cookie) = crate::ui::chat_view::messenger_auth_for_fetch();
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|e| format!("client: {}", e))?;
    let mut last_err = String::from("no candidates");
    for u in viewer_url_variants(url) {
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
            Ok(resp) if resp.status().is_success() => match resp.bytes().await {
                Ok(b) if !b.is_empty() && b.len() <= VIEWER_DOWNLOAD_MAX_BYTES => {
                    return Ok(b.to_vec())
                }
                Ok(b) => last_err = format!("bad body ({} bytes)", b.len()),
                Err(e) => last_err = format!("body: {}", e),
            },
            Ok(resp) => last_err = format!("HTTP {}", resp.status()),
            Err(e) => last_err = format!("fetch: {}", e),
        }
    }
    Err(last_err)
}

/// Decode bytes into the viewer widget (downscaling huge images).
fn set_viewer_bytes(img: &gtk::Image, bytes: &[u8]) -> Result<(), String> {
    let loader = gtk::gdk_pixbuf::PixbufLoader::new();
    loader
        .write(bytes)
        .map_err(|e| format!("pixbuf write: {}", e))?;
    loader.close().map_err(|e| format!("pixbuf close: {}", e))?;
    let pixbuf = loader
        .pixbuf()
        .ok_or_else(|| "pixbuf loader empty".to_string())?;
    let (w, h) = (pixbuf.width(), pixbuf.height());
    let shown = if w > VIEWER_MAX_SIDE || h > VIEWER_MAX_SIDE {
        let scale = (VIEWER_MAX_SIDE as f64 / w.max(h) as f64).min(1.0);
        let nw = ((w as f64) * scale).round().max(1.0) as i32;
        let nh = ((h as f64) * scale).round().max(1.0) as i32;
        pixbuf
            .scale_simple(nw, nh, gtk::gdk_pixbuf::InterpType::Bilinear)
            .unwrap_or(pixbuf)
    } else {
        pixbuf
    };
    img.set_pixel_size(-1);
    img.set_from_pixbuf(Some(&shown));
    img.set_tooltip_text(None);
    Ok(())
}

impl Clone for ImageViewer {
    fn clone(&self) -> Self {
        Self {
            container: self.container.clone(),
            image: self.image.clone(),
            current_url: RefCell::new((*self.current_url.borrow()).clone()),
            current_filename: RefCell::new((*self.current_filename.borrow()).clone()),
            zoom_level: RefCell::new(*self.zoom_level.borrow()),
            closed: RefCell::new(*self.closed.borrow()),
            image_index: RefCell::new(*self.image_index.borrow()),
            image_count: RefCell::new(*self.image_count.borrow()),
            on_navigate: RefCell::new(self.on_navigate.borrow().clone()),
        }
    }
}
