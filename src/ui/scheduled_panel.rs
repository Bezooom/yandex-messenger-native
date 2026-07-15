#![allow(dead_code)]

use gtk::prelude::*;
use gtk::{
    Align, Box as GtkBox, Button, Calendar, Entry, Label, Orientation, Popover, ScrolledWindow,
};

use std::cell::RefCell;
use std::rc::Rc;

use crate::models::scheduled_message::{ScheduledMessage, ScheduledStatus};

/// Popover для выбора даты/времени отправки
pub struct SendAtPopover {
    popover: Popover,
    pub scheduled_panel: Option<std::rc::Rc<ScheduledPanel>>,
}

impl SendAtPopover {
    pub fn new() -> Self {
        Self {
            popover: Popover::new(),
            scheduled_panel: None,
        }
    }

    pub fn new_with_popover(
        popover: Popover,
        scheduled_panel: Option<std::rc::Rc<ScheduledPanel>>,
    ) -> Self {
        Self {
            popover,
            scheduled_panel,
        }
    }

    pub fn popover(&self) -> &Popover {
        &self.popover
    }
}

/// Панел с запланированными сообщениями
pub struct ScheduledPanel {
    /// Контейнер панели
    pub container: GtkBox,
    /// Список запланированных сообщений
    scheduled_messages: RefCell<Vec<ScheduledMessage>>,
    /// Заголовок панели
    title_label: Label,
    /// Список сообщений
    message_list: GtkBox,
    /// Кнопка "Запланировать"
    schedule_btn: Button,
    /// Кнопка "Очистить"
    clear_btn: Button,
    /// Popover с выбором даты
    popover: RefCell<Option<Popover>>,
    /// Количество запланированных сообщений
    count_label: Label,
    /// Состояние видимости
    visible: RefCell<bool>,
    /// Callback для отмены сообщения
    on_cancel: Rc<RefCell<Option<Box<dyn Fn(String)>>>>,
    /// Callback для изменения времени
    on_edit: Rc<RefCell<Option<Box<dyn Fn(String, chrono::NaiveDate, u32, u32)>>>>,
}

impl ScheduledPanel {
    pub fn new() -> Self {
        let container = GtkBox::new(Orientation::Vertical, 0);
        container.set_css_classes(&["scheduled-panel"]);
        container.set_margin_top(8);

        // Заголовок
        let title_box = GtkBox::new(Orientation::Horizontal, 8);
        title_box.set_margin_start(12);
        title_box.set_margin_end(12);
        title_box.set_margin_top(8);
        title_box.set_margin_bottom(4);

        let title_label = Label::builder()
            .label("Запланированные")
            .xalign(0.0)
            .css_classes(vec!["title".to_string()])
            .build();

        let count_label = Label::builder()
            .label("(0)")
            .xalign(0.0)
            .css_classes(vec!["dim-label".to_string()])
            .build();

        title_box.append(&title_label);
        title_box.append(&count_label);

        // Кнопки
        let buttons_box = GtkBox::new(Orientation::Horizontal, 4);
        let schedule_btn = Button::with_label("Запланировать");
        schedule_btn.add_css_class("suggested-action");
        schedule_btn.set_hexpand(true);

        let clear_btn = Button::with_label("Очистить");
        clear_btn.add_css_class("flat");

        buttons_box.append(&schedule_btn);
        buttons_box.append(&clear_btn);

        // Список сообщений
        let scrolled = ScrolledWindow::builder()
            .hexpand(true)
            .vexpand(true)
            .build();

        let message_list = GtkBox::new(Orientation::Vertical, 4);
        message_list.set_margin_start(8);
        message_list.set_margin_end(8);
        message_list.set_margin_top(4);
        message_list.set_margin_bottom(8);

        scrolled.set_child(Some(&message_list));

        container.append(&title_box);
        container.append(&buttons_box);
        container.append(&scrolled);

        Self {
            container,
            scheduled_messages: RefCell::new(Vec::new()),
            title_label,
            message_list,
            schedule_btn,
            clear_btn,
            popover: RefCell::new(None),
            count_label,
            visible: RefCell::new(true),
            on_cancel: Rc::new(RefCell::new(None)),
            on_edit: Rc::new(RefCell::new(None)),
        }
    }

    pub fn update_messages(&self, messages: &[ScheduledMessage]) {
        *self.scheduled_messages.borrow_mut() = messages.to_vec();
        self.render_messages();
        self.update_count();
    }

    fn update_count(&self) {
        let count = self.scheduled_messages.borrow().len();
        self.count_label.set_label(&format!("({})", count));
    }

    fn render_messages(&self) {
        // Очищаем список
        while let Some(child) = self.message_list.first_child() {
            self.message_list.remove(&child);
        }

        let messages = self.scheduled_messages.borrow();
        if messages.is_empty() {
            let empty = Label::builder()
                .label("Нет запланированных сообщений")
                .css_classes(vec!["dim-label".to_string()])
                .xalign(0.5)
                .build();
            self.message_list.append(&empty);
            return;
        }

        for msg in messages.iter() {
            let row = self.create_message_row(msg);
            self.message_list.append(&row);
        }
    }

    fn create_message_row(&self, msg: &ScheduledMessage) -> GtkBox {
        let row = GtkBox::new(Orientation::Horizontal, 8);
        row.set_css_classes(&["scheduled-message-row"]);
        row.set_margin_start(4);
        row.set_margin_end(4);
        row.set_margin_top(2);
        row.set_margin_bottom(2);

        // Индикатор статуса
        let status_icon = match msg.status {
            ScheduledStatus::Pending => "◷",
            ScheduledStatus::Sending => "⟳",
            ScheduledStatus::Sent => "✓",
            ScheduledStatus::Failed => "✗",
        };
        let status_btn = Button::builder()
            .label(status_icon)
            .css_classes(vec!["icon-btn".to_string()])
            .build();
        status_btn.set_halign(Align::Start);

        // Текст сообщения
        let text_label = Label::builder()
            .label(&msg.text)
            .hexpand(true)
            .halign(Align::Start)
            .xalign(0.0)
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .build();

        // Время отправки
        let time_str = msg.scheduled_at.format("%d.%m %H:%M").to_string();
        let time_label = Label::builder()
            .label(&time_str)
            .css_classes(vec!["scheduled-time".to_string()])
            .halign(Align::End)
            .build();

        // Кнопка отмены
        let cancel_btn = Button::builder()
            .icon_name("window-close-symbolic")
            .css_classes(vec!["icon-btn".to_string()])
            .build();
        let msg_id = msg.message_id.clone();
        let on_cancel_cb = self.on_cancel.clone();
        cancel_btn.connect_clicked(move |_| {
            log::info!("Canceling scheduled message: {}", msg_id);
            if let Some(ref cb) = *on_cancel_cb.borrow() {
                cb(msg_id.clone());
            }
        });

        // Кнопка изменения
        let edit_btn = Button::builder()
            .icon_name("object-select-symbolic")
            .css_classes(vec!["icon-btn".to_string()])
            .build();
        let msg_id2 = msg.message_id.clone();
        let on_edit_cb = self.on_edit.clone();
        edit_btn.connect_clicked(move |_| {
            log::info!("Editing scheduled message: {}", msg_id2);
            if let Some(ref cb) = *on_edit_cb.borrow() {
                cb(msg_id2.clone(), chrono::Utc::now().date_naive(), 12, 0);
            }
        });

        row.append(&status_btn);
        row.append(&text_label);
        row.append(&time_label);
        row.append(&edit_btn);
        row.append(&cancel_btn);

        row
    }

    fn show_popover(&self) {
        let popover = Popover::builder()
            .has_arrow(false)
            .autohide(true)
            .build();
        popover.set_css_classes(&["send-at-popover"]);

        let container = GtkBox::new(Orientation::Vertical, 12);
        container.add_css_class("send-at-body");
        container.set_margin_top(4);
        container.set_margin_bottom(4);
        container.set_margin_start(4);
        container.set_margin_end(4);

        let cal = Calendar::new();
        cal.set_show_day_names(true);

        let time_entry = Entry::builder().placeholder_text("12:00").build();

        let confirm_btn = Button::with_label("Подтвердить");
        confirm_btn.add_css_class("suggested-action");
        confirm_btn.set_hexpand(true);

        let cancel_btn = Button::with_label("Отмена");
        cancel_btn.add_css_class("flat");
        cancel_btn.set_hexpand(true);

        let buttons_box = GtkBox::new(Orientation::Horizontal, 8);
        buttons_box.append(&cancel_btn);
        buttons_box.append(&confirm_btn);

        container.append(&cal);
        container.append(&time_entry);
        container.append(&buttons_box);

        popover.set_child(Some(&container));

        let confirm_clone = confirm_btn.clone();
        let popover_clone = popover.clone();
        confirm_clone.connect_clicked(move |_| {
            popover_clone.popdown();
        });

        let cancel_clone = cancel_btn.clone();
        let popover_clone2 = popover.clone();
        cancel_clone.connect_clicked(move |_| {
            popover_clone2.popdown();
        });

        *self.popover.borrow_mut() = Some(popover);
    }

    fn clear_all(&self) {
        *self.scheduled_messages.borrow_mut() = Vec::new();
        self.render_messages();
        self.update_count();
    }

    pub fn connect_cancel<F: Fn(String) + 'static>(&self, callback: F) {
        *self.on_cancel.borrow_mut() = Some(Box::new(callback));
    }

    pub fn connect_edit<F: Fn(String, chrono::NaiveDate, u32, u32) + 'static>(&self, callback: F) {
        *self.on_edit.borrow_mut() = Some(Box::new(callback));
    }
}
