use reqwest::Client;
use serde_json::Value;
use std::sync::Arc;

use crate::api::auth::AuthManager;
use crate::config;
use crate::models::telemost::{
    CapabilitiesOffer, ConferenceParams, CreateConferenceRequest, CreateConferenceResponse,
    JoinConferenceRequest, JoinConferenceResponse, TelemostConference,
};

pub struct TelemostClient {
    auth: Arc<AuthManager>,
    client: Client,
    cloud_api_base: String,
}

impl TelemostClient {
    pub fn new(auth: Arc<AuthManager>) -> Self {
        Self {
            auth: auth.clone(),
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(15))
                .user_agent("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
                .build()
                .unwrap_or_default(),
            cloud_api_base: config::TELEMOST_CLOUD_API.to_string(),
        }
    }

    fn auth_header(&self) -> String {
        match self.auth.get_access_token() {
            Ok(token) => format!("OAuth {}", token),
            Err(_) => String::new(),
        }
    }

    fn session_cookies(&self) -> Option<String> {
        None
    }

    /// Создание конференции через Cloud API
    pub async fn create_conference(
        &self,
        request: CreateConferenceRequest,
    ) -> Result<CreateConferenceResponse, String> {
        let url = format!("{}/v1/telemost/conferences", self.cloud_api_base);

        let mut req_builder = self
            .client
            .post(&url)
            .header("Authorization", self.auth_header())
            .header("Content-Type", "application/json")
            .header("Origin", "https://yandex.ru")
            .header("Referer", "https://yandex.ru/chat")
            .json(&request);

        if let Some(cookies) = self.session_cookies() {
            req_builder = req_builder.header("Cookie", cookies);
        }

        let response = req_builder
            .send()
            .await
            .map_err(|e| format!("Failed to create conference: {}", e))?;

        let status = response.status();
        let text = response
            .text()
            .await
            .map_err(|e| format!("Response read failed: {}", e))?;

        if !status.is_success() {
            log::error!("Create conference failed HTTP {}: {}", status, text);
            return Err(format!("Create conference failed: HTTP {}", status));
        }

        let parsed: CreateConferenceResponse =
            serde_json::from_str(&text).map_err(|e| format!("Parse failed: {}", e))?;

        Ok(parsed)
    }

    /// Получение информации о конференции
    pub async fn get_conference(
        &self,
        conference_id: &str,
    ) -> Result<TelemostConference, String> {
        let url = format!(
            "{}/v1/telemost/conferences/{}",
            self.cloud_api_base, conference_id
        );

        let response = self
            .client
            .get(&url)
            .header("Authorization", self.auth_header())
            .header("Origin", "https://yandex.ru")
            .header("Referer", "https://yandex.ru/chat")
            .send()
            .await
            .map_err(|e| format!("Failed to get conference: {}", e))?;

        let status = response.status();
        let text = response
            .text()
            .await
            .map_err(|e| format!("Response read failed: {}", e))?;

        if !status.is_success() {
            return Err(format!("Get conference failed: HTTP {}", status));
        }

        let parsed: TelemostConference =
            serde_json::from_str(&text).map_err(|e| format!("Parse failed: {}", e))?;

        Ok(parsed)
    }

    /// Завершение конференции
    pub async fn end_conference(&self, conference_id: &str) -> Result<(), String> {
        let url = format!(
            "{}/v1/telemost/conferences/{}/end",
            self.cloud_api_base, conference_id
        );

        let response = self
            .client
            .post(&url)
            .header("Authorization", self.auth_header())
            .header("Origin", "https://yandex.ru")
            .header("Referer", "https://yandex.ru/chat")
            .send()
            .await
            .map_err(|e| format!("Failed to end conference: {}", e))?;

        let status = response.status();
        if !status.is_success() {
            return Err(format!("End conference failed: HTTP {}", status));
        }

        Ok(())
    }

    /// Вход в конференцию
    pub async fn join_conference(
        &self,
        request: JoinConferenceRequest,
    ) -> Result<JoinConferenceResponse, String> {
        let url = format!(
            "{}/v1/telemost/conferences/{}/join",
            self.cloud_api_base, request.conference_id
        );

        let body = serde_json::json!({
            "capabilities": request.capabilities
        });

        let mut req_builder = self
            .client
            .post(&url)
            .header("Authorization", self.auth_header())
            .header("Content-Type", "application/json")
            .header("Origin", "https://yandex.ru")
            .header("Referer", "https://yandex.ru/chat")
            .json(&body);

        if let Some(cookies) = self.session_cookies() {
            req_builder = req_builder.header("Cookie", cookies);
        }

        let response = req_builder
            .send()
            .await
            .map_err(|e| format!("Failed to join conference: {}", e))?;

        let status = response.status();
        let text = response
            .text()
            .await
            .map_err(|e| format!("Response read failed: {}", e))?;

        if !status.is_success() {
            log::error!("Join conference failed HTTP {}: {}", status, text);
            return Err(format!("Join conference failed: HTTP {}", status));
        }

        let parsed: JoinConferenceResponse =
            serde_json::from_str(&text).map_err(|e| format!("Parse failed: {}", e))?;

        Ok(parsed)
    }

    /// Получить ссылку для присоединения к конференции
    pub async fn get_join_link(
        &self,
        conference_id: &str,
    ) -> Result<String, String> {
        let conference = self.get_conference(conference_id).await?;

        if let Some(link) = conference.join_url {
            if !link.is_empty() {
                return Ok(link);
            }
        }

        Ok(format!(
            "{}/join/{}",
            config::TELEMOST_URL.trim_end_matches('/'),
            conference_id
        ))
    }
}
