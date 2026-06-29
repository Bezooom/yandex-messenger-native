#![allow(dead_code)]

use gtk::prelude::*;
use gtk::{Align, Box as GtkBox, Button, Entry, Label, Orientation, Separator, ToggleButton};
use std::cell::RefCell;

use crate::models::{Poll, PollAnswer};

/// Poll creation form
pub struct PollCreator {
    container: GtkBox,
    question_entry: Entry,
    answers_container: GtkBox,
    answers: RefCell<Vec<(Entry, Button)>>, // (input, remove_button)
    is_anonymous: ToggleButton,
    is_multi_select: ToggleButton,
    quiz_mode: ToggleButton,
    has_correct_answer: ToggleButton,
    correct_answer_selector: RefCell<Option<GtkBox>>,
    on_submit: RefCell<Option<Box<dyn Fn(Poll) + 'static>>>,
    on_cancel: RefCell<Option<Box<dyn Fn() + 'static>>>,
}

impl PollCreator {
    /// Create a new PollCreator instance.
    pub fn new() -> Self {
        let container = GtkBox::new(Orientation::Vertical, 12);
        container.set_margin_start(24);
        container.set_margin_end(24);
        container.set_margin_top(24);
        container.set_margin_bottom(24);
        container.set_css_classes(&["poll-creator"]);

        // Question
        let question_label = Label::builder()
            .label("Вопрос")
            .halign(Align::Start)
            .build();
        let question_entry = Entry::builder()
            .placeholder_text("Введите вопрос для опроса")
            .hexpand(true)
            .build();

        // Answers container
        let answers_label = Label::builder()
            .label("Варианты ответа")
            .halign(Align::Start)
            .build();
        let answers_container = GtkBox::new(Orientation::Vertical, 6);

        // Add initial 2 answer inputs
        let add_answer_btn = Button::builder()
            .label("+ Добавить вариант")
            .hexpand(true)
            .build();

        // Options
        let options_label = Label::builder()
            .label("Настройки")
            .halign(Align::Start)
            .build();
        let is_anonymous = ToggleButton::builder()
            .label("Анонимный опрос")
            .halign(Align::Start)
            .build();
        let is_multi_select = ToggleButton::builder()
            .label("Множественный выбор")
            .halign(Align::Start)
            .build();
        let quiz_mode = ToggleButton::builder()
            .label("Викторина (есть правильный ответ)")
            .halign(Align::Start)
            .build();
        let has_correct_answer = ToggleButton::builder()
            .label("Показать выбор правильного ответа")
            .visible(false)
            .halign(Align::Start)
            .build();

        // Correct answer selector (hidden by default)
        let correct_answer_selector = GtkBox::new(Orientation::Vertical, 6);
        correct_answer_selector.set_visible(false);

        // Buttons
        let button_box = GtkBox::new(Orientation::Horizontal, 12);
        button_box.set_halign(Align::End);
        let cancel_btn = Button::builder()
            .label("Отмена")
            .build();
        let create_btn = Button::builder()
            .label("Создать опрос")
            .css_classes(vec!["suggested-action".to_string()])
            .build();

        // Assemble
        container.append(&question_label);
        container.append(&question_entry);
        container.append(&Separator::new(Orientation::Horizontal));
        container.append(&answers_label);
        container.append(&answers_container);
        container.append(&add_answer_btn);
        container.append(&Separator::new(Orientation::Horizontal));
        container.append(&options_label);
        container.append(&is_anonymous);
        container.append(&is_multi_select);
        container.append(&quiz_mode);
        container.append(&has_correct_answer);
        container.append(&Separator::new(Orientation::Horizontal));
        container.append(&Label::builder()
            .label("Правильный ответ (если викторина)")
            .halign(Align::Start)
            .build());
        container.append(&correct_answer_selector);
        container.append(&Separator::new(Orientation::Horizontal));
        container.append(&button_box);
        button_box.append(&cancel_btn);
        button_box.append(&create_btn);

        Self {
            container,
            question_entry,
            answers_container,
            answers: RefCell::new(vec![]),
            is_anonymous,
            is_multi_select,
            quiz_mode,
            has_correct_answer,
            correct_answer_selector: RefCell::new(Some(correct_answer_selector)),
            on_submit: RefCell::new(None),
            on_cancel: RefCell::new(None),
        }
    }

    /// Add a new answer input field.
    pub fn add_answer(&self, placeholder: &str) {
        let row = GtkBox::new(Orientation::Horizontal, 6);
        row.set_css_classes(&["poll-answer-row"]);

        let entry = Entry::builder()
            .placeholder_text(placeholder)
            .hexpand(true)
            .build();

        let remove_btn = Button::builder()
            .label("✕")
            .css_classes(vec!["flat", "circular"].to_vec())
            .build();

        let remove_clone = remove_btn.clone();
        let answers = self.answers.clone();
        remove_btn.connect_clicked(move |_| {
            let mut ans = answers.borrow_mut();
            if let Some(pos) = ans.iter().position(|(_, btn)| btn == &remove_clone) {
                let (entry, _) = ans.remove(pos);
                if let Some(parent) = entry.parent() {
                    if let Ok(box_parent) = parent.downcast::<gtk::Box>() {
                        box_parent.remove(&entry);
                    }
                }
            }
        });

        row.append(&entry);
        row.append(&remove_btn);
        self.answers_container.append(&row);

        let entry_clone = entry.clone();
        self.answers.borrow_mut().push((entry_clone, remove_btn));

        entry.grab_focus();
    }

    /// Remove the last answer input.
    pub fn remove_last_answer(&self) {
        let mut ans = self.answers.borrow_mut();
        if let Some((entry, _)) = ans.pop() {
            if let Some(parent) = entry.parent() {
                if let Ok(box_parent) = parent.downcast::<gtk::Box>() {
                    box_parent.remove(&entry);
                }
            }
        }
    }

    /// Clear all answer inputs.
    pub fn clear_answers(&self) {
        let mut ans = self.answers.borrow_mut();
        for (entry, _) in ans.drain(..) {
            if let Some(parent) = entry.parent() {
                if let Ok(box_parent) = parent.downcast::<gtk::Box>() {
                    box_parent.remove(&entry);
                }
            }
        }
    }

    /// Create a Poll from the current form state.
    pub fn create_poll(&self) -> Option<Poll> {
        let question = self.question_entry.text().to_string();
        if question.trim().is_empty() {
            return None;
        }

        let ans = self.answers.borrow();
        if ans.len() < 2 {
            return None;
        }

        let answers: Vec<PollAnswer> = ans
            .iter()
            .map(|(entry, _)| PollAnswer {
                answer_id: format!("a_{}", uuid::Uuid::new_v4().simple()),
                text: entry.text().to_string(),
                votes: 0,
                is_correct: false,
                is_selected: false,
            })
            .collect();

        let quiz_mode = self.quiz_mode.is_active();
        let mut poll = Poll {
            poll_id: format!("p_{}", uuid::Uuid::new_v4().simple()),
            message_id: String::new(),
            chat_id: String::new(),
            question,
            answers,
            total_voters: 0,
            is_anonymous: self.is_anonymous.is_active(),
            is_multi_select: self.is_multi_select.is_active(),
            quiz_mode,
            correct_answer_ids: vec![],
            created_by: String::new(),
            expires_at: None,
            is_closed: false,
            final_results: None,
        };

        if quiz_mode {
            poll.correct_answer_ids = self.answers.borrow()
                .iter()
                .enumerate()
                .filter(|(_i, (entry, _))| entry.text().is_empty())
                .map(|(i, _)| poll.answers[i].answer_id.clone())
                .collect();
        }

        Some(poll)
    }

    /// Reset the form to initial state.
    pub fn reset(&self) {
        self.question_entry.set_text("");
        self.is_anonymous.set_active(false);
        self.is_multi_select.set_active(false);
        self.quiz_mode.set_active(false);
        self.has_correct_answer.set_active(false);
        self.clear_answers();
        // Add 2 default answer inputs
        self.add_answer("Вариант 1");
        self.add_answer("Вариант 2");
    }

    /// Set the submit callback.
    pub fn on_submit<F>(&self, callback: F)
    where
        F: Fn(Poll) + 'static,
    {
        *self.on_submit.borrow_mut() = Some(Box::new(callback));
    }

    /// Set the cancel callback.
    pub fn on_cancel<F>(&self, callback: F)
    where
        F: Fn() + 'static,
    {
        *self.on_cancel.borrow_mut() = Some(Box::new(callback));
    }

    /// Get the container widget.
    pub fn container(&self) -> &GtkBox {
        &self.container
    }

    /// Hide the creator (called by parent).
    pub fn hide(&self) {
        self.container.set_visible(false);
    }

    /// Show the creator.
    pub fn show(&self) {
        self.container.set_visible(true);
        self.question_entry.grab_focus();
    }
}
