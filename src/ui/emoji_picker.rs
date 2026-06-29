use gtk::prelude::*;
use gtk::{Box as GtkBox, Button, Orientation, ScrolledWindow, Widget};
use std::rc::Rc;
use std::cell::RefCell;

pub struct EmojiPicker {
    pub container: GtkBox,
    on_emoji_selected: Rc<RefCell<Option<Box<dyn Fn(String)>>>>,
}

impl EmojiPicker {
    pub fn new() -> Self {
        let container = GtkBox::new(Orientation::Vertical, 0);
        let scrolled = ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Automatic)
            .vscrollbar_policy(gtk::PolicyType::Never)
            .min_content_height(48)
            .min_content_width(320)
            .build();
        
        let hbox = GtkBox::new(Orientation::Horizontal, 4);
        hbox.set_margin_start(8);
        hbox.set_margin_end(8);
        hbox.set_margin_top(4);
        hbox.set_margin_bottom(4);

        let emojis = ["😀", "😂", "❤️", "👍", "🙏", "😭", "😢", "🥺", "🔥", "😊"];
        
        let on_emoji_selected: Rc<RefCell<Option<Box<dyn Fn(String)>>>> = Rc::new(RefCell::new(None));

        for emoji in emojis {
            let btn = Button::builder()
                .label(emoji)
                .has_frame(false)
                .build();
            btn.add_css_class("flat");

            let emoji_str = emoji.to_string();
            let cb_ref = on_emoji_selected.clone();
            btn.connect_clicked(move |_| {
                if let Some(cb) = cb_ref.borrow().as_ref() {
                    cb(emoji_str.clone());
                }
            });

            hbox.append(&btn);
        }

        scrolled.set_child(Some(&hbox));
        container.append(&scrolled);

        Self {
            container,
            on_emoji_selected,
        }
    }

    pub fn container(&self) -> &Widget {
        self.container.upcast_ref()
    }

    pub fn on_select(&self, callback: impl Fn(String) + 'static) {
        *self.on_emoji_selected.borrow_mut() = Some(Box::new(callback));
    }
}
