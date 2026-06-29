#![allow(dead_code)]

use gtk::prelude::*;
use gtk::{Align, Box as GtkBox, Button, Orientation, Popover};
use std::cell::RefCell;
use std::rc::Rc;

use crate::models::{ExtendedReactionsConfig, Reaction};

/// Quick reaction emojis that appear as the first row of buttons.
const DEFAULT_QUICK_EMOJIS: &[&str] = &[
    "\u{2764}\u{FE0F}", // ❤️  red heart
    "\u{1F44D}",        // 👍 thumbs up
    "\u{1F602}",        // 😂 laughing
    "\u{1F914}",        // 🤔 thinking
    "\u{1F622}",        // 😢 sad
    "\u{1F525}",        // 🔥 fire
];

/// ReactionPanel — a popup panel showing available reactions for a message.
///
/// Displays quick reaction emojis as clickable buttons, a "+" button for
/// extended reactions, and the current reaction state on the message.
pub struct ReactionPanel {
    container: GtkBox,
    message_id: String,
    reactions: RefCell<Vec<Reaction>>,
    config: RefCell<Option<ExtendedReactionsConfig>>,
    quick_emojis: Vec<String>,
    popover: RefCell<Option<Popover>>,
    on_reaction_click: Rc<RefCell<Option<Box<dyn Fn(String, String) + 'static>>>>,
    on_remove_reaction: Rc<RefCell<Option<Box<dyn Fn(String, String) + 'static>>>>,
}

impl ReactionPanel {
    /// Create a new ReactionPanel for a given message.
    pub fn new(message_id: String) -> Self {
        let container = GtkBox::new(Orientation::Vertical, 4);
        container.set_css_classes(&["reaction-panel"]);

        ReactionPanel {
            container,
            message_id,
            reactions: RefCell::new(Vec::new()),
            config: RefCell::new(None),
            quick_emojis: DEFAULT_QUICK_EMOJIS.iter().map(|s| s.to_string()).collect(),
            popover: RefCell::new(None),
            on_reaction_click: Rc::new(RefCell::new(None)),
            on_remove_reaction: Rc::new(RefCell::new(None)),
        }
    }

    /// Show the reaction panel as a popover anchored to a target widget.
    pub fn show(&self, target: &impl IsA<gtk::Widget>) {
        let popover = self.build_popover();

        // Set container as popover child
        popover.set_child(Some(&self.container));

        // Store reference for later updates
        *self.popover.borrow_mut() = Some(popover.clone());

        popover.set_parent(&target.clone().upcast());
        popover.set_position(gtk::PositionType::Bottom);
        popover.set_has_arrow(false);
        popover.popup();
    }

    /// Hide the reaction panel popover.
    pub fn hide(&self) {
        if let Some(pop) = self.popover.borrow().as_ref() {
            pop.popdown();
        }
    }

    /// Build the reaction panel popover content.
    fn build_popover(&self) -> Popover {
        let popover = Popover::builder().build();
        popover.set_css_classes(&["reaction-panel", "context-menu"]);

        // ── Quick reactions row ──
        let quick_row = GtkBox::new(Orientation::Horizontal, 4);
        quick_row.set_css_classes(&["reaction-row"]);

        for emoji in &self.quick_emojis {
            let btn = self.create_reaction_button(emoji);
            quick_row.append(&btn);
        }

        // ── Extended reactions separator ──
        let divider = GtkBox::new(Orientation::Horizontal, 0);
        divider.set_css_classes(&["divider"]);
        divider.set_size_request(-1, 1);

        // ── Extended reactions row ──
        let extended_row = GtkBox::new(Orientation::Horizontal, 4);
        extended_row.set_css_classes(&["reaction-row"]);

        if let Some(config) = self.config.borrow().as_ref() {
            for ext_reaction in &config.reactions {
                let btn = self.create_reaction_button(&ext_reaction.emoji);
                extended_row.append(&btn);
            }
        } else {
            // Show "+" button to expand
            let more_btn = Button::builder().label("+").sensitive(false).build();
            more_btn.set_css_classes(&["reaction-btn", "circular"]);
            more_btn.set_size_request(32, 32);
            extended_row.append(&more_btn);
        }

        self.container.append(&quick_row);
        self.container.append(&divider);
        self.container.append(&extended_row);

        popover
    }

    /// Create a single reaction emoji button with circular styling.
    fn create_reaction_button(&self, emoji: &str) -> Button {
        let btn = Button::builder().label(emoji).build();

        btn.set_css_classes(&["reaction-btn", "circular"]);
        btn.set_size_request(36, 36);
        btn.set_valign(Align::Center);

        let message_id = self.message_id.clone();
        let emoji = emoji.to_string();

        let on_click = Rc::clone(&self.on_reaction_click);
        let on_remove = Rc::clone(&self.on_remove_reaction);

        // Track which reactions this user has already reacted with
        let current_reactions = self.reactions.borrow();
        let user_reacted_with = current_reactions
            .iter()
            .filter(|r| r.selected)
            .map(|r| r.emoji.clone())
            .collect::<Vec<_>>();
        drop(current_reactions);

        let btn_clone_for_closure = btn.clone();
        btn.connect_clicked(move |_| {
            // Toggle reaction: if already selected, remove; otherwise add
            let is_selected = user_reacted_with.contains(&emoji);

            if is_selected {
                // Remove reaction
                if let Some(cb) = on_remove.borrow().as_ref() {
                    cb(message_id.clone(), emoji.clone());
                }
            } else {
                // Add reaction
                if let Some(cb) = on_click.borrow().as_ref() {
                    cb(message_id.clone(), emoji.clone());
                }
            }

            // Animate button press
            btn_clone_for_closure.add_css_class("pressed");
            let btn_clone_for_timeout = btn_clone_for_closure.clone();
            glib::timeout_add_local(std::time::Duration::from_millis(150), move || {
                btn_clone_for_timeout.remove_css_class("pressed");
                glib::ControlFlow::Break
            });
        });

        btn
    }

    /// Update the displayed reactions on the message.
    pub fn set_reactions(&self, reactions: Vec<Reaction>) {
        *self.reactions.borrow_mut() = reactions;
    }

    /// Set the extended reactions configuration.
    pub fn set_config(&self, config: ExtendedReactionsConfig) {
        *self.config.borrow_mut() = Some(config);

        // Rebuild popover to show extended reactions
        if let Some(pop) = self.popover.borrow().as_ref() {
            // Clear and rebuild
            while let Some(child) = self.container.first_child() {
                self.container.remove(&child);
            }
            let new_popover = self.build_popover();
            new_popover.set_parent(&pop.parent().unwrap());
            new_popover.set_position(pop.position());
            *self.popover.borrow_mut() = Some(new_popover);
        }
    }

    /// Register callback for reaction addition/click.
    pub fn on_reaction_click(&self, callback: impl Fn(String, String) + 'static) {
        *self.on_reaction_click.borrow_mut() = Some(Box::new(callback));
    }

    /// Register callback for reaction removal.
    pub fn on_remove_reaction(&self, callback: impl Fn(String, String) + 'static) {
        *self.on_remove_reaction.borrow_mut() = Some(Box::new(callback));
    }

    /// Get the container widget to add to a parent layout.
    pub fn container(&self) -> &GtkBox {
        &self.container
    }
}
