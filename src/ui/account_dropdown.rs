use gtk::prelude::*;
use gtk::{Box as GtkBox, Button, Label, Orientation, Popover, Separator};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use crate::api::auth::AuthManager;

/// Account dropdown — shows the current account name and allows switching
#[derive(Clone)]
pub struct AccountDropdown {
    auth: Arc<AuthManager>,
    popover: Rc<RefCell<Popover>>,
    menu: Rc<RefCell<GtkBox>>,
    switch_callback: Rc<RefCell<Option<Box<dyn Fn(&str)>>>>,
    logout_callback: Rc<RefCell<Option<Box<dyn Fn()>>>>,
    add_account_callback: Rc<RefCell<Option<Box<dyn Fn()>>>>,
}

impl AccountDropdown {
    pub fn new(auth: Arc<AuthManager>) -> Self {
        let popover = Rc::new(RefCell::new(Popover::builder().has_arrow(false).build()));

        let menu = Rc::new(RefCell::new(GtkBox::new(Orientation::Vertical, 4)));
        menu.borrow_mut().add_css_class("account-dropdown-menu");
        menu.borrow_mut().set_margin_start(6);
        menu.borrow_mut().set_margin_end(6);
        menu.borrow_mut().set_margin_top(6);
        menu.borrow_mut().set_margin_bottom(6);

        Self {
            auth,
            popover,
            menu,
            switch_callback: Rc::new(RefCell::new(None)),
            logout_callback: Rc::new(RefCell::new(None)),
            add_account_callback: Rc::new(RefCell::new(None)),
        }
    }

    pub fn popup<W: IsA<gtk::Widget>>(&self, parent: &W) {
        let parent_widget = parent.as_ref().clone();
        let auth = self.auth.clone();
        let menu_clone = self.menu.clone();
        let popover_clone = self.popover.clone();
        let switch_cb = self.switch_callback.clone();
        let logout_cb = self.logout_callback.clone();
        let add_cb = self.add_account_callback.clone();

        glib::spawn_future_local(async move {
            let accounts = auth.list_accounts().await;
            let active_id = auth.get_current_account_id().await;

            let menu = menu_clone.borrow();
            // Clear existing children
            while let Some(child) = menu.first_child() {
                menu.remove(&child);
            }

            // Header Title
            let title_label = Label::builder()
                .label("Мои аккаунты")
                .css_classes(vec!["sidebar-section-title".to_string()])
                .xalign(0.0)
                .margin_start(10)
                .margin_top(4)
                .margin_bottom(4)
                .build();
            menu.append(&title_label);

            // List each account
            for account in accounts {
                let row_box = GtkBox::new(Orientation::Horizontal, 10);
                row_box.set_margin_start(4);
                row_box.set_margin_end(4);

                // Avatar initials with gradient background
                let avatar = GtkBox::new(Orientation::Horizontal, 0);
                avatar.add_css_class("avatar");
                let label_text = account.display_label();
                let initials: String = label_text
                    .chars()
                    .take(2)
                    .map(|c| c.to_ascii_uppercase())
                    .collect();

                let avatar_label = Label::builder()
                    .label(&initials)
                    .css_classes(vec!["avatar-label".to_string()])
                    .build();
                avatar.append(&avatar_label);
                avatar.set_size_request(32, 32);

                // Hash account ID to choose a background color gradient
                let mut hash: usize = 5381;
                for byte in account.id.bytes() {
                    hash = hash
                        .wrapping_shl(5)
                        .wrapping_add(hash)
                        .wrapping_add(byte as usize);
                }
                avatar.add_css_class(&format!("avatar-gradient-{}", hash % 8));
                row_box.append(&avatar);

                // Display Label
                let name_label = Label::builder()
                    .label(&account.display_label())
                    .css_classes(vec!["chat-title".to_string()])
                    .xalign(0.0)
                    .hexpand(true)
                    .build();
                row_box.append(&name_label);

                // Active Checkmark icon
                let is_active = Some(account.id.clone()) == active_id;
                if is_active {
                    let check_img = gtk::Image::builder()
                        .icon_name("object-select-symbolic")
                        .css_classes(vec!["chat-type-icon".to_string()])
                        .build();
                    row_box.append(&check_img);
                }

                // Wrap in button
                let btn = Button::builder()
                    .css_classes(vec!["flat".to_string(), "dropdown-account-btn".to_string()])
                    .build();
                btn.set_child(Some(&row_box));

                let switch_cb_clone = switch_cb.clone();
                let account_id = account.id.clone();
                let pop_close = popover_clone.clone();
                btn.connect_clicked(move |_| {
                    pop_close.borrow().popdown();
                    if let Some(ref cb) = *switch_cb_clone.borrow() {
                        cb(&account_id);
                    }
                });

                menu.append(&btn);
            }

            menu.append(&Separator::new(Orientation::Horizontal));

            // "Add Account" Button
            let add_box = GtkBox::new(Orientation::Horizontal, 10);
            add_box.set_margin_start(4);
            let add_img = gtk::Image::builder().icon_name("list-add-symbolic").build();
            let add_label = Label::builder()
                .label("Добавить аккаунт")
                .xalign(0.0)
                .build();
            add_box.append(&add_img);
            add_box.append(&add_label);

            let add_btn = Button::builder()
                .css_classes(vec!["flat".to_string()])
                .child(&add_box)
                .build();
            let add_cb_clone = add_cb.clone();
            let pop_close2 = popover_clone.clone();
            add_btn.connect_clicked(move |_| {
                pop_close2.borrow().popdown();
                if let Some(ref cb) = *add_cb_clone.borrow() {
                    cb();
                }
            });
            menu.append(&add_btn);

            // "Logout" Button
            let logout_box = GtkBox::new(Orientation::Horizontal, 10);
            logout_box.set_margin_start(4);
            let logout_img = gtk::Image::builder()
                .icon_name("system-log-out-symbolic")
                .build();
            let logout_label = Label::builder().label("Выйти").xalign(0.0).build();
            logout_box.append(&logout_img);
            logout_box.append(&logout_label);

            let logout_btn = Button::builder()
                .css_classes(vec!["flat".to_string(), "destructive-action".to_string()])
                .child(&logout_box)
                .build();
            let logout_cb_clone = logout_cb.clone();
            let pop_close3 = popover_clone.clone();
            logout_btn.connect_clicked(move |_| {
                pop_close3.borrow().popdown();
                if let Some(ref cb) = *logout_cb_clone.borrow() {
                    cb();
                }
            });
            menu.append(&logout_btn);

            let popover = popover_clone.borrow();
            popover.set_parent(&parent_widget);
            popover.set_child(Some(&*menu));
            popover.popup();
        });
    }

    pub fn connect_switch<F: Fn(&str) + 'static>(&self, callback: F) {
        *self.switch_callback.borrow_mut() = Some(Box::new(callback));
    }

    pub fn connect_logout<F: Fn() + 'static>(&self, callback: F) {
        *self.logout_callback.borrow_mut() = Some(Box::new(callback));
    }

    pub fn connect_add_account<F: Fn() + 'static>(&self, callback: F) {
        *self.add_account_callback.borrow_mut() = Some(Box::new(callback));
    }
}
