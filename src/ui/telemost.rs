use gtk::prelude::*;
use gtk::{Application, ApplicationWindow, Box, Button, Label, Orientation};

pub struct TelemostWindow {
    window: ApplicationWindow,
}

impl TelemostWindow {
    pub fn new(app: &Application, call_url: &str) -> Self {
        let window = ApplicationWindow::builder()
            .application(app)
            .title("Telemost Call")
            .default_width(960)
            .default_height(640)
            .build();

        let root = Box::new(Orientation::Vertical, 8);
        root.set_margin_start(12);
        root.set_margin_end(12);
        root.set_margin_top(12);
        root.set_margin_bottom(12);

        let label = Label::builder()
            .label(&format!("Открыть звонок: {}", call_url))
            .xalign(0.0)
            .wrap(true)
            .build();
        root.append(&label);

        let controls = Box::new(Orientation::Horizontal, 8);
        let mute = Button::with_label("Mute");
        let video = Button::with_label("Video");
        let end = Button::with_label("End");
        controls.append(&mute);
        controls.append(&video);
        controls.append(&end);
        root.append(&controls);

        let window_clone = window.clone();
        end.connect_clicked(move |_| {
            window_clone.close();
        });

        window.set_child(Some(&root));
        Self { window }
    }

    pub fn show(&self) {
        self.window.present();
    }
}
