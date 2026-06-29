use serde::{Deserialize, Serialize};

/// Account — represents a Yandex OAuth account
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    /// Unique account identifier (derived from user_id + device_id)
    pub id: String,
    /// Display name for the account
    pub display_name: Option<String>,
    /// Avatar URL
    pub avatar_url: Option<String>,
    /// Current access token
    pub access_token: String,
    /// Refresh token (if available)
    pub refresh_token: Option<String>,
    /// Token expiry timestamp (unix seconds)
    pub expires_at: u64,
    /// Whether the token is currently valid
    pub is_valid: bool,
}

impl Account {
    pub fn new(id: String, access_token: String) -> Self {
        Self {
            id,
            display_name: None,
            avatar_url: None,
            access_token,
            refresh_token: None,
            expires_at: 0,
            is_valid: true,
        }
    }

    pub fn display_label(&self) -> String {
        if let Some(name) = &self.display_name {
            if !name.is_empty() {
                return name.clone();
            }
        }
        format!("Account {}", &self.id[..8])
    }
}

/// Saved accounts file — stores account IDs in the data directory
pub const ACCOUNTS_LIST_FILE: &str = "accounts.json";
