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
            let t = name.trim();
            if !t.is_empty() && !t.eq_ignore_ascii_case("messenger") {
                return t.to_string();
            }
        }
        // Avoid raw UUIDs as primary label when possible
        if self.id.len() >= 8 && self.id.contains('-') {
            "Аккаунт".to_string()
        } else if self.id.len() >= 4 {
            format!("@{}", &self.id[..self.id.len().min(16)])
        } else {
            "Аккаунт".to_string()
        }
    }

    /// Build a CDN URL for this account's avatar, if any.
    pub fn avatar_cdn_url(&self) -> Option<String> {
        Self::resolve_avatar_url(self.avatar_url.as_deref()?)
    }

    /// Convert messenger / OAuth avatar ids into a fetchable HTTPS URL.
    pub fn resolve_avatar_url(avatar_id: &str) -> Option<String> {
        let id = avatar_id.trim();
        if id.is_empty() {
            return None;
        }
        if id.starts_with("http://") || id.starts_with("https://") {
            return Some(id.to_string());
        }

        // user_avatar/yapic/<bucket>/<hash>  or  user_avatar/yapic/<id>
        if let Some(rest) = id.strip_prefix("user_avatar/") {
            if let Some(yapic) = rest.strip_prefix("yapic/") {
                return Some(format!(
                    "https://avatars.mds.yandex.net/get-yapic/{}/islands-200",
                    yapic
                ));
            }
            // other user_avatar/* → files.messenger host
            return Some(format!(
                "https://files.messenger.yandex.ru/{}?size=small",
                id
            ));
        }

        // bare yapic path "6214067/xxx" or numeric id
        if id.contains('/') {
            return Some(format!(
                "https://avatars.mds.yandex.net/get-yapic/{}/islands-200",
                id
            ));
        }

        Some(format!(
            "https://avatars.mds.yandex.net/get-yapic/{}/islands-200",
            id
        ))
    }
}

/// Saved accounts file — stores account IDs in the data directory
pub const ACCOUNTS_LIST_FILE: &str = "accounts.json";
