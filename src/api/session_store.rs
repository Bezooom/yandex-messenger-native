//! Passport session cookies (`session.json`) — used for history / WS / RPC.
//!
//! Written by in-app WebView login (preferred) or legacy `scripts/login_browser.py`.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SessionData {
    pub cookies: HashMap<String, String>,
    #[serde(default)]
    pub csrf_token: Option<String>,
    #[serde(default)]
    pub saved_at: u64,
}

impl SessionData {
    pub fn cookie_header(&self) -> String {
        self.cookies
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>()
            .join("; ")
    }

    pub fn has_session_id(&self) -> bool {
        self.cookies
            .get("Session_id")
            .map(|v| !v.is_empty())
            .unwrap_or(false)
    }

    /// Passport user id for push.yandex.ru / messenger.
    /// Prefer Session_id-derived uid (numeric), NOT browser `yandexuid` (wrong for Xiva).
    pub fn yuid(&self) -> Option<String> {
        let sid = self.cookies.get("Session_id")?;
        // Format: ...:ts|uid.rest
        if let Some(pos) = sid.find('|') {
            let sub = &sid[pos + 1..];
            if let Some(dot) = sub.find('.') {
                let uid = &sub[..dot];
                if !uid.is_empty() && uid.chars().all(|c| c.is_ascii_digit()) {
                    return Some(uid.to_string());
                }
            }
        }
        // Fallbacks
        self.cookies
            .get("yandexuid")
            .filter(|s| !s.is_empty())
            .cloned()
    }

    /// OAuth-compatible passport uid string (same as yuid when Session_id present).
    pub fn passport_uid(&self) -> Option<String> {
        self.yuid()
    }
}

pub fn session_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("yandex-messenger-native")
        .join("session.json")
}

pub fn load_session() -> Option<SessionData> {
    let path = session_path();
    let content = std::fs::read_to_string(&path).ok()?;
    let data: SessionData = serde_json::from_str(&content).ok()?;
    if data.cookies.is_empty() {
        return None;
    }
    Some(data)
}

/// Remove stored Passport session (used on logout before re-login).
pub fn clear_session() {
    let path = session_path();
    if path.exists() {
        if let Err(e) = std::fs::remove_file(&path) {
            log::warn!("Failed to remove session.json: {}", e);
        } else {
            log::info!("Session cleared ({})", path.display());
        }
    }
}

pub fn save_session(data: &SessionData) -> Result<(), String> {
    let path = session_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let mut data = data.clone();
    if data.saved_at == 0 {
        data.saved_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
    }
    let json = serde_json::to_string_pretty(&data).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| e.to_string())?;
    log::info!(
        "Session saved ({} cookies, csrf={})",
        data.cookies.len(),
        data.csrf_token
            .as_ref()
            .map(|s| !s.is_empty())
            .unwrap_or(false)
    );
    Ok(())
}

pub fn save_cookies_map(
    cookies: HashMap<String, String>,
    csrf_token: Option<String>,
) -> Result<SessionData, String> {
    let data = SessionData {
        cookies,
        csrf_token,
        saved_at: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0),
    };
    if !data.has_session_id() {
        return Err("Session_id cookie missing — login incomplete".to_string());
    }
    save_session(&data)?;
    Ok(data)
}

/// Fetch CSRF token using session cookies (blocking-friendly async).
pub async fn fetch_csrf_token(cookie_header: &str) -> Option<String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .user_agent("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
        .build()
        .ok()?;
    let resp = client
        .get("https://yandex.ru/messenger/api/registry/csrf-token/")
        .header("Cookie", cookie_header)
        .header("Origin", "https://yandex.ru")
        .header("Referer", "https://yandex.ru/chat")
        .send()
        .await
        .ok()?;
    let json: serde_json::Value = resp.json().await.ok()?;
    json.get("token")
        .and_then(|t| t.as_str())
        .map(|s| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_yuid_from_session_id() {
        let mut cookies = HashMap::new();
        cookies.insert(
            "Session_id".into(),
            "3:1.5.0.1:x:e.1.2:1|239644570.0.20002.3:1|3:1.sig".into(),
        );
        let data = SessionData {
            cookies,
            csrf_token: None,
            saved_at: 0,
        };
        assert_eq!(data.yuid().as_deref(), Some("239644570"));
    }

    #[test]
    fn test_cookie_header() {
        let mut cookies = HashMap::new();
        cookies.insert("a".into(), "1".into());
        cookies.insert("b".into(), "2".into());
        let data = SessionData {
            cookies,
            csrf_token: None,
            saved_at: 0,
        };
        let h = data.cookie_header();
        assert!(h.contains("a=1"));
        assert!(h.contains("b=2"));
    }
}
