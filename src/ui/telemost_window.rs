use gtk::prelude::*;
use gtk::{
    Application, ApplicationWindow, Box, Button, Image, Label, Orientation, ScrolledWindow, Stack,
};
use std::sync::Arc;

use tokio::sync::Mutex;

use crate::api::telemost::TelemostClient;
use crate::models::telemost::TelemostConference;

#[derive(Clone)]
pub struct TelemostWindow {
    pub window: ApplicationWindow,
    telemost_client: Arc<TelemostClient>,
    current_conference: Arc<Mutex<Option<TelemostConference>>>,
    // UI elements
    video_stack: Stack,
    call_timer_label: Label,
    participants_list: Box,
    participants_container: ScrolledWindow,
    participant_count_label: Label,
}

impl TelemostWindow {
    pub fn new(app: &Application, telemost_client: Arc<TelemostClient>) -> Self {
        let window = ApplicationWindow::builder()
            .application(app)
            .title("Telemost")
            .default_width(1200)
            .default_height(800)
            .build();

        // Main vertical layout
        let main_box = Box::new(Orientation::Vertical, 0);
        window.set_child(Some(&main_box));

        // Top bar with call info
        let top_bar = Box::new(Orientation::Horizontal, 12);
        top_bar.set_margin_top(16);
        top_bar.set_margin_bottom(16);
        top_bar.set_margin_start(24);
        top_bar.set_margin_end(24);

        // Conference title
        let title_label = Label::new(Some("Video Call"));
        title_label.add_css_class("title-2");
        title_label.set_halign(gtk::Align::Start);
        top_bar.append(&title_label);

        // Spacer
        let spacer = Box::new(Orientation::Horizontal, 0);
        top_bar.append(&spacer);

        // Call timer
        let call_timer_label = Label::new(Some("00:00"));
        call_timer_label.add_css_class("dim-label");
        top_bar.append(&call_timer_label);

        main_box.append(&top_bar);

        // Video area with stack for different states
        let video_stack = Stack::new();
        video_stack.set_hexpand(true);
        video_stack.set_vexpand(true);
        video_stack.set_margin_start(24);
        video_stack.set_margin_end(24);
        video_stack.set_margin_bottom(16);

        // Video placeholder
        let video_placeholder = Box::new(Orientation::Vertical, 0);
        video_placeholder.set_halign(gtk::Align::Center);
        video_placeholder.set_valign(gtk::Align::Center);
        video_placeholder.set_hexpand(true);
        video_placeholder.set_vexpand(true);
        video_placeholder.add_css_class("card");

        let video_icon = Image::from_icon_name("video-display-symbolic");
        video_icon.add_css_class("icon-size-large");
        video_placeholder.append(&video_icon);

        let video_status = Label::new(Some("Connecting to video stream..."));
        video_status.add_css_class("dim-label");
        video_status.set_margin_top(8);
        video_placeholder.append(&video_status);

        video_stack.add_named(&video_placeholder, Some("video-placeholder"));

        // Video ready state
        let video_ready = Box::new(Orientation::Vertical, 0);
        video_ready.set_halign(gtk::Align::Center);
        video_ready.set_valign(gtk::Align::Center);
        video_ready.set_hexpand(true);
        video_ready.set_vexpand(true);
        video_ready.add_css_class("card");
        video_ready.add_css_class("success");

        let check_icon = Image::from_icon_name("object-select-symbolic");
        check_icon.add_css_class("icon-size-large");
        video_ready.append(&check_icon);

        let ready_label = Label::new(Some("Connected"));
        ready_label.set_margin_top(8);
        video_ready.append(&ready_label);

        video_stack.add_named(&video_ready, Some("video-ready"));

        main_box.append(&video_stack);

        // Bottom control bar
        let control_box = Box::new(Orientation::Horizontal, 16);
        control_box.set_margin_start(24);
        control_box.set_margin_end(24);
        control_box.set_margin_bottom(24);
        control_box.set_halign(gtk::Align::Center);

        // Mute button
        let mute_btn = Button::new();
        mute_btn.add_css_class("circular");
        mute_btn.add_css_class("suggested-action");
        mute_btn.set_size_request(48, 48);
        mute_btn.set_tooltip_text(Some("Mute microphone"));
        let mute_icon = Image::from_icon_name("audio-speakers-symbolic");
        mute_btn.set_child(Some(&mute_icon));

        // Camera button
        let camera_btn = Button::new();
        camera_btn.add_css_class("circular");
        camera_btn.add_css_class("suggested-action");
        camera_btn.set_size_request(48, 48);
        camera_btn.set_tooltip_text(Some("Toggle camera"));
        let camera_icon = Image::from_icon_name("camera-video-symbolic");
        camera_btn.set_child(Some(&camera_icon));

        // Screen share button
        let screen_share_btn = Button::new();
        screen_share_btn.add_css_class("circular");
        screen_share_btn.add_css_class("suggested-action");
        screen_share_btn.set_size_request(48, 48);
        screen_share_btn.set_tooltip_text(Some("Share screen"));
        let screen_icon = Image::from_icon_name("view-fullscreen-symbolic");
        screen_share_btn.set_child(Some(&screen_icon));

        // Chat button
        let chat_btn = Button::new();
        chat_btn.add_css_class("circular");
        chat_btn.add_css_class("suggested-action");
        chat_btn.set_size_request(48, 48);
        chat_btn.set_tooltip_text(Some("Open chat"));
        let chat_icon = Image::from_icon_name("mail-unread-symbolic");
        chat_btn.set_child(Some(&chat_icon));

        // Participants button
        let participants_btn = Button::new();
        participants_btn.add_css_class("circular");
        participants_btn.add_css_class("suggested-action");
        participants_btn.set_size_request(48, 48);
        participants_btn.set_tooltip_text(Some("Participants"));
        let people_icon = Image::from_icon_name("system-users-symbolic");
        participants_btn.set_child(Some(&people_icon));
        participants_btn.set_margin_start(8);

        // End call button
        let end_call_btn = Button::with_label("End Call");
        end_call_btn.add_css_class("destructive-action");
        end_call_btn.set_size_request(120, 48);
        end_call_btn.set_margin_start(16);

        control_box.append(&mute_btn);
        control_box.append(&camera_btn);
        control_box.append(&screen_share_btn);
        control_box.append(&chat_btn);
        control_box.append(&participants_btn);
        control_box.append(&end_call_btn);

        main_box.append(&control_box);

        // Participants panel
        let participants_box = Box::new(Orientation::Vertical, 8);
        participants_box.set_margin_start(24);
        participants_box.set_margin_end(24);
        participants_box.set_margin_bottom(24);

        let participants_header = Box::new(Orientation::Horizontal, 8);
        participants_header.set_margin_bottom(8);

        let participants_icon = Image::from_icon_name("system-users-symbolic");
        participants_header.append(&participants_icon);

        let participants_title = Label::new(Some("Participants"));
        participants_title.add_css_class("title-3");
        participants_header.append(&participants_title);

        let participant_count_label = Label::new(Some("0"));
        participant_count_label.add_css_class("dim-label");
        participants_header.append(&participant_count_label);

        participants_box.append(&participants_header);

        let participants_container = ScrolledWindow::new();
        participants_container.set_vexpand(true);
        participants_container.set_height_request(300);

        let participants_list = Box::new(Orientation::Vertical, 4);
        participants_container.set_child(Some(&participants_list));
        participants_box.append(&participants_container);

        main_box.append(&participants_box);

        let telemost_client_clone = telemost_client.clone();

        end_call_btn.connect_clicked(move |_| {
            let client = telemost_client_clone.clone();
            tokio::spawn(async move {
                let _ = client.end_conference("").await;
            });
        });

        let window_clone = window.clone();
        window.connect_close_request(move |_| {
            window_clone.close();
            gtk::glib::Propagation::Proceed
        });

        Self {
            window,
            telemost_client,
            current_conference: Arc::new(Mutex::new(None)),
            video_stack,
            call_timer_label,
            participants_list,
            participants_container,
            participant_count_label,
        }
    }

    pub async fn create_conference(&self) -> Result<TelemostConference, String> {
        let request = crate::models::telemost::CreateConferenceRequest {
            chat_id: None,
            title: Some("Video Call".to_string()),
            waiting_room_enabled: Some(false),
            max_participants: Some(10),
        };

        let response = self.telemost_client.create_conference(request).await?;
        let conference = TelemostConference {
            id: response.conference.id.clone(),
            chat_id: response.conference.chat_id.clone(),
            state: crate::models::telemost::ConferenceState::Created,
            participants: vec![],
            created_at: chrono::Utc::now(),
            started_at: None,
            finished_at: None,
            join_url: response.conference.join_url.clone(),
            host_id: response.conference.host_id.clone(),
        };

        *self.current_conference.lock().await = Some(conference.clone());
        self.update_ui(&conference);
        Ok(conference)
    }

    pub async fn join_conference(&self, conference_id: &str) -> Result<(), String> {
        let request = crate::models::telemost::JoinConferenceRequest {
            conference_id: conference_id.to_string(),
            capabilities: Some(crate::models::telemost::CapabilitiesOffer::default()),
        };

        let response = self.telemost_client.join_conference(request).await?;
        let conference = TelemostConference {
            id: conference_id.to_string(),
            chat_id: None,
            state: crate::models::telemost::ConferenceState::Started,
            participants: response.conference.participants,
            created_at: chrono::Utc::now(),
            started_at: Some(chrono::Utc::now()),
            finished_at: None,
            join_url: response.conference.join_url,
            host_id: None,
        };

        *self.current_conference.lock().await = Some(conference.clone());
        self.update_ui(&conference);
        Ok(())
    }

    fn update_ui(&self, conference: &TelemostConference) {
        // Update video stack
        if conference.state == crate::models::telemost::ConferenceState::Started {
            self.video_stack.set_visible_child_name("video-ready");
        }

        // Update participant count
        let count = conference.participants.len();
        self.participant_count_label
            .set_text(&count.to_string());

        // Clear and rebuild participants list (GTK4: walk first_child / next_sibling)
        while let Some(child) = self.participants_list.first_child() {
            self.participants_list.remove(&child);
        }
        for participant in &conference.participants {
            let row = self.create_participant_row(participant);
            self.participants_list.append(&row);
        }
    }

    fn create_participant_row(&self, participant: &crate::models::telemost::TelemostParticipant) -> Box {
        let row = Box::new(Orientation::Horizontal, 12);
        row.set_margin_bottom(4);

        // Avatar
        let avatar = Box::new(Orientation::Vertical, 0);
        avatar.set_size_request(40, 40);
        avatar.set_valign(gtk::Align::Center);
        avatar.add_css_class("avatar");
        avatar.set_halign(gtk::Align::Center);
        avatar.set_valign(gtk::Align::Center);

        let initials = participant
            .name
            .as_ref()
            .map(|n| {
                n.chars()
                    .take(2)
                    .map(|c| c.to_uppercase().to_string())
                    .collect::<Vec<_>>()
                    .join("")
            })
            .unwrap_or_else(|| "?".to_string());

        let avatar_label = Label::new(Some(&initials));
        avatar_label.add_css_class("avatar-label");
        avatar.append(&avatar_label);

        row.append(&avatar);

        // Name
        let name_label = Label::new(Some(participant.name.as_deref().unwrap_or("Unknown")));
        name_label.set_halign(gtk::Align::Start);
        name_label.set_hexpand(true);
        row.append(&name_label);

        // Status
        let status_icon = Image::from_icon_name(if participant.audio_enabled.unwrap_or(false) {
            "object-select-symbolic"
        } else {
            "network-disconnected-symbolic"
        });
        status_icon.add_css_class("dim-label");
        row.append(&status_icon);

        row
    }

    pub async fn end_call(&self) -> Result<(), String> {
        let conference = self.current_conference.lock().await;
        if let Some(ref conf) = *conference {
            self.telemost_client.end_conference(&conf.id).await?;
        }
        Ok(())
    }

    pub async fn get_call_status(&self) -> Option<crate::models::telemost::ConferenceState> {
        let conference = self.current_conference.lock().await;
        conference.as_ref().map(|c| c.state.clone())
    }

    pub fn show(&self) {
        self.window.present();
    }

    pub fn hide(&self) {
        self.window.set_visible(false);
    }
}
