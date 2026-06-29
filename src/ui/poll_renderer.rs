#![allow(dead_code)]

use gtk::prelude::*;
use gtk::{Align, Box as GtkBox, Button, Label, Orientation, ProgressBar};
use std::cell::RefCell;
use std::rc::Rc;

use crate::models::Poll;

/// Poll renderer for displaying polls in chat
pub struct PollRenderer {
    container: GtkBox,
    poll: RefCell<Poll>,
    chat_id: String,
    on_vote: Rc<RefCell<Option<Box<dyn Fn(String, Vec<String>) + 'static>>>>,
}

impl PollRenderer {
    /// Create a new PollRenderer instance.
    pub fn new(poll: Poll, chat_id: String) -> Self {
        let container = GtkBox::new(Orientation::Vertical, 8);
        container.set_margin_start(16);
        container.set_margin_end(16);
        container.set_margin_top(8);
        container.set_margin_bottom(8);

        Self {
            container,
            poll: RefCell::new(poll),
            chat_id,
            on_vote: Rc::new(RefCell::new(None)),
        }
    }

    /// Update the poll with new data (e.g., after voting).
    pub fn update_poll(&self, poll: Poll) {
        *self.poll.borrow_mut() = poll;
        self.render();
    }

    /// Set the vote callback.
    pub fn on_vote<F>(&self, callback: F)
    where
        F: Fn(String, Vec<String>) + 'static,
    {
        *self.on_vote.borrow_mut() = Some(Box::new(callback));
    }

    /// Get the container widget.
    pub fn container(&self) -> &GtkBox {
        &self.container
    }

    /// Render the poll UI.
    fn render(&self) {
        // Clear existing children
        while let Some(child) = self.container.first_child() {
            self.container.remove(&child);
        }

        let poll = self.poll.borrow();

        // Question
        let question = Label::builder()
            .label(&poll.question)
            .wrap(true)
            .xalign(0.0)
            .css_classes(vec!["poll-question".to_string()])
            .build();
        self.container.append(&question);

        // Answers
        for answer in &poll.answers {
            let answer_row = GtkBox::new(Orientation::Horizontal, 8);
            answer_row.set_halign(Align::Fill);

            // Answer text
            let answer_label = Label::builder()
                .label(&answer.text)
                .xalign(0.0)
                .hexpand(true)
                .build();
            answer_row.append(&answer_label);

            // Progress bar
            let percentage = if poll.total_voters > 0 {
                (answer.votes as f64 / poll.total_voters as f64) * 100.0
            } else {
                0.0
            };
            let progress_bar = ProgressBar::new();
            progress_bar.set_fraction(percentage / 100.0);
            progress_bar.set_show_text(true);
            progress_bar.set_text(Some(&format!("{:.1}%", percentage)));
            progress_bar.set_hexpand(true);
            progress_bar.add_css_class("poll-progress-bar");
            answer_row.append(&progress_bar);

            // Vote button (if not voted and not anonymous, or always in quiz mode?)
            let can_vote = poll.can_vote();
            let already_voted = answer.is_selected;
            if can_vote && !already_voted {
                let vote_btn = Button::builder()
                    .label("Проголосовать")
                    .sensitive(true)
                    .build();
                let poll_id = poll.poll_id.clone();
                let answer_id = answer.answer_id.clone();
                let on_vote = self.on_vote.clone();
                vote_btn.connect_clicked(move |_| {
                    if let Some(this_ref) = on_vote.borrow().as_ref() {
                        this_ref(poll_id.clone(), vec![answer_id.clone()]);
                    }
                });
                answer_row.append(&vote_btn);
            } else if already_voted {
                let voted_label = Label::builder()
                    .label("Ваш выбор")
                    .xalign(0.0)
                    .css_classes(vec!["dim-label".to_string()])
                    .build();
                answer_row.append(&voted_label);
            }

            self.container.append(&answer_row);
        }

        // Footer: total voters, close status
        let footer = GtkBox::new(Orientation::Horizontal, 12);
        footer.set_halign(Align::End);
        let voters_label = Label::builder()
            .label(format!("{} проголосовало", poll.total_voters))
            .css_classes(vec!["dim-label".to_string()])
            .build();
        footer.append(&voters_label);

        if poll.is_closed {
            let closed_label = Label::builder()
                .label("Опрос закрыт")
                .css_classes(vec!["dim-label".to_string()])
                .build();
            footer.append(&closed_label);
        }

        self.container.append(&footer);
    }
}

impl PollRenderer {
    /// Get the poll_id of this renderer's poll.
    pub fn poll_id(&self) -> String {
        self.poll.borrow().poll_id.clone()
    }
}