use crate::models::Message;
use gtk::glib;
use gtk::subclass::prelude::*;
use std::cell::RefCell;

glib::wrapper! {
    pub struct MessageObject(ObjectSubclass<imp::MessageObject>);
}

impl MessageObject {
    pub fn new(message: Message) -> Self {
        let obj: Self = glib::Object::builder().build();
        obj.imp().message.replace(Some(message));
        obj
    }

    pub fn message(&self) -> Message {
        self.imp().message.borrow().as_ref().unwrap().clone()
    }
}

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct MessageObject {
        pub message: RefCell<Option<Message>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for MessageObject {
        const NAME: &'static str = "YandexMessengerMessageObject";
        type Type = super::MessageObject;
        type ParentType = glib::Object;
    }

    impl ObjectImpl for MessageObject {}
}
