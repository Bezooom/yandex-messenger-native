use crate::api::HttpClient;
use serde_json::Value;

impl HttpClient {
    /// Translate a message text
    pub async fn translate_message(&self, text: &str, target_lang: &str) -> Result<String, String> {
        let auth_header = self.get_token_header();
        if auth_header.is_empty() {
            return Err("No authentication token".to_string());
        }
        let url = format!("{}api/translate", self.base_url);

        let body = serde_json::json!({
            "text": text,
            "targetLang": target_lang
        });

        let response = self
            .client
            .post(&url)
            .header("Authorization", &auth_header)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Translate message failed: {}", e))?;

        if !response.status().is_success() {
            // Mock or handle unsupported endpoint
            if response.status() == reqwest::StatusCode::NOT_FOUND {
                return Ok(format!("[Перевод на {} недоступен]", target_lang));
            }
            return Err(format!(
                "Translate message failed with status: {}",
                response.status()
            ));
        }

        let json: Value = response
            .json()
            .await
            .map_err(|e| format!("Translate parse failed: {}", e))?;

        // Extract translated text
        if let Some(translated_text) = json.get("text").and_then(|t| t.as_str()) {
            return Ok(translated_text.to_string());
        }

        if let Some(translated_text) = json.get("translatedText").and_then(|t| t.as_str()) {
            return Ok(translated_text.to_string());
        }

        Err("Translation response has unsupported format".to_string())
    }
}
