use crate::api::HttpClient;
use crate::models::ChatFolder;
use serde_json::Value;

impl HttpClient {
    /// Get all folders (private, requires token)
    pub async fn get_folders(&self) -> Result<Vec<ChatFolder>, String> {
        let auth_header = self.get_token_header();
        if auth_header.is_empty() {
            return Err("No authentication token".to_string());
        }
        let url = format!("{}api/get_folders", self.base_url);

        let response = self
            .client
            .get(&url)
            .header("Authorization", &auth_header)
            .send()
            .await
            .map_err(|e| format!("Get folders failed: {}", e))?;

        if !response.status().is_success() {
            // For now, if API is not fully supported, we return an empty list
            // or we could fallback to local storage
            if response.status() == reqwest::StatusCode::NOT_FOUND {
                return Ok(vec![]);
            }
            return Err(format!(
                "Get folders failed with status: {}",
                response.status()
            ));
        }

        let json: Value = response
            .json()
            .await
            .map_err(|e| format!("Folders parse failed: {}", e))?;

        if let Ok(folders) = serde_json::from_value::<Vec<ChatFolder>>(json.clone()) {
            return Ok(folders);
        }

        if let Some(folders_arr) = json.get("folders") {
            if let Ok(folders) = serde_json::from_value::<Vec<ChatFolder>>(folders_arr.clone()) {
                return Ok(folders);
            }
        }

        Err("Folders response has unsupported format".to_string())
    }

    /// Create or update folder
    pub async fn update_folder(&self, folder: &ChatFolder) -> Result<ChatFolder, String> {
        let auth_header = self.get_token_header();
        if auth_header.is_empty() {
            return Err("No authentication token".to_string());
        }
        let url = format!("{}api/update_folder", self.base_url);

        let response = self
            .client
            .post(&url)
            .header("Authorization", &auth_header)
            .header("Content-Type", "application/json")
            .json(folder)
            .send()
            .await
            .map_err(|e| format!("Update folder failed: {}", e))?;

        if !response.status().is_success() {
            return Err(format!(
                "Update folder failed with status: {}",
                response.status()
            ));
        }

        let json: Value = response
            .json()
            .await
            .map_err(|e| format!("Update folder parse failed: {}", e))?;

        serde_json::from_value(json)
            .map_err(|e| format!("Update folder deserialization failed: {}", e))
    }
}
