use base64::Engine;
use log::{debug, info};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex as StdMutex};
use std::time::SystemTime;
use tokio::sync::Mutex;

use crate::config;
use crate::models::Account;

/// Authentication error types for fine-grained error handling
#[derive(Debug, Clone)]
pub enum AuthError {
    /// Token not found (user has not authenticated)
    NotFound,
    /// Access token has expired
    Expired,
    /// Token refresh failed with a description
    RefreshFailed(String),
    /// HTTP error during token operations
    HttpError(String),
    /// Failed to parse token or response data
    ParseError(String),
}

impl fmt::Display for AuthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AuthError::NotFound => write!(f, "Token not found"),
            AuthError::Expired => write!(f, "Token expired"),
            AuthError::RefreshFailed(msg) => write!(f, "Token refresh failed: {}", msg),
            AuthError::HttpError(msg) => write!(f, "HTTP error: {}", msg),
            AuthError::ParseError(msg) => write!(f, "Parse error: {}", msg),
        }
    }
}

impl std::error::Error for AuthError {}

impl From<AuthError> for String {
    fn from(err: AuthError) -> Self {
        err.to_string()
    }
}

/// OAuth token response with internal expiry tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthToken {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_in: u64,
    pub token_type: String,
    pub user_id: Option<String>,
    /// Unix timestamp (seconds) when the token was received, used for expiry calculation
    #[serde(default)]
    pub received_at: u64,
}

impl OAuthToken {
    pub fn is_expired(&self) -> bool {
        // Consider expired 5 minutes before actual expiry to allow for network latency
        if self.received_at == 0 {
            // Legacy token without received_at — use simple heuristic
            self.expires_in <= 300
        } else {
            let now = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            let expiry = self.received_at + self.expires_in;
            // Fixed: was (now as u64).saturating_sub(expiry) >= 300
            // which incorrectly returns false when now < expiry
            now >= expiry - 300
        }
    }

    pub fn oauth_header(&self) -> String {
        format!("OAuth {}", self.access_token)
    }
}

/// Auth manager handles OAuth flow and token storage
#[derive(Clone)]
pub struct AuthManager {
    token: Arc<Mutex<Option<OAuthToken>>>,
    data_dir: PathBuf,
    /// ID of the currently active account (None = no account selected)
    current_account_id: Arc<Mutex<Option<String>>>,
    /// Non-async mirror of current_account_id for sync contexts (e.g. file paths).
    current_account_id_sync: Arc<StdMutex<Option<String>>>,
    /// List of all known accounts
    accounts: Arc<Mutex<Vec<Account>>>,
}

impl AuthManager {
    pub fn user_id(&self) -> Option<String> {
        // Use the same token path as get/set_token to ensure consistency
        // after switching between accounts
        let path = self.token_file();
        if let Ok(data) = std::fs::read_to_string(path) {
            if let Ok(token) = serde_json::from_str::<OAuthToken>(&data) {
                return token.user_id;
            }
        }
        None
    }

    pub fn set_user_id(&self, user_id: &str) {
        // Use the same token path as get/set_token to ensure consistency
        // after switching between accounts
        let path = self.token_file();
        if let Ok(data) = std::fs::read_to_string(&path) {
            if let Ok(mut token) = serde_json::from_str::<OAuthToken>(&data) {
                token.user_id = Some(user_id.to_string());
                if let Ok(json) = serde_json::to_string(&token) {
                    let _ = std::fs::write(&path, json);
                }
            }
        }
    }

    fn effective_client_id(&self) -> String {
        if let Ok(v) = std::env::var("YANDEX_CLIENT_ID") {
            let trimmed = v.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
        config::OAUTH_CLIENT_ID.to_string()
    }

    fn effective_redirect_uri(&self) -> Option<String> {
        if let Ok(v) = std::env::var("YANDEX_REDIRECT_URI") {
            let trimmed = v.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
        let configured = config::REDIRECT_URI.trim();
        if configured.is_empty() {
            None
        } else {
            Some(configured.to_string())
        }
    }

    fn effective_auth_proxy_url(&self) -> Option<String> {
        std::env::var("YANDEX_AUTH_PROXY_URL")
            .ok()
            .map(|v| v.trim().trim_end_matches('/').to_string())
            .filter(|v| !v.is_empty())
    }

    fn effective_authorize_url(&self) -> String {
        std::env::var("YANDEX_OAUTH_AUTHORIZE_URL")
            .ok()
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| config::OAUTH_AUTHORIZE_URL.to_string())
    }

    fn token_urls(&self) -> Vec<String> {
        let mut urls = Vec::new();
        if let Ok(custom) = std::env::var("YANDEX_OAUTH_TOKEN_URL") {
            let trimmed = custom.trim();
            if !trimmed.is_empty() {
                urls.push(trimmed.to_string());
            }
        }
        if urls.is_empty() {
            urls.push(config::OAUTH_TOKEN_URL.to_string());
        }
        if urls.iter().any(|u| u.contains("oauth.yandex.com")) {
            urls.push("https://oauth.yandex.ru/token".to_string());
        }
        urls
    }

    pub fn new() -> Result<Self, String> {
        let data_dir = dirs::config_dir()
            .ok_or_else(|| "Cannot get config dir".to_string())?
            .join("yandex-messenger-native");

        fs::create_dir_all(&data_dir).map_err(|e| format!("Cannot create data dir: {}", e))?;

        let manager = Self {
            token: Arc::new(Mutex::new(None)),
            data_dir: data_dir.clone(),
            current_account_id: Arc::new(Mutex::new(None)),
            current_account_id_sync: Arc::new(StdMutex::new(None)),
            accounts: Arc::new(Mutex::new(Vec::new())),
        };

        // Eagerly load persisted accounts from disk so the UI sees them on startup.
        manager.load_accounts_sync();

        Ok(manager)
    }

    /// Load accounts from disk synchronously. Called once from `new()`.
    fn load_accounts_sync(&self) {
        let accounts_file = self
            .data_dir
            .join(crate::models::account::ACCOUNTS_LIST_FILE);
        let mut loaded: Vec<Account> = Vec::new();

        if accounts_file.exists() {
            if let Ok(data) = fs::read_to_string(&accounts_file) {
                if let Ok(list) = serde_json::from_str::<Vec<Account>>(&data) {
                    loaded = list;
                }
            }
        }

        // If the accounts file is missing but a legacy `token.json` is present,
        // wrap it in a synthesized account so existing installs keep working.
        if loaded.is_empty() {
            let legacy_token = self.data_dir.join("token.json");
            if legacy_token.exists() {
                if let Ok(data) = fs::read_to_string(&legacy_token) {
                    if let Ok(token) = serde_json::from_str::<OAuthToken>(&data) {
                        let id = token
                            .user_id
                            .clone()
                            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
                        let mut account = Account::new(id.clone(), token.access_token.clone());
                        account.refresh_token = token.refresh_token.clone();
                        account.expires_at = token.received_at + token.expires_in;
                        loaded.push(account);

                        // Mirror the legacy token into the per-account directory so
                        // token_file() returns a sensible path after switching.
                        let account_dir = self.data_dir.join("accounts").join(&id);
                        if fs::create_dir_all(&account_dir).is_ok() {
                            let _ = fs::copy(&legacy_token, account_dir.join("token.json"));
                        }

                        self.set_current_account_sync(Some(id.clone()));
                        if let Ok(mut current) = self.current_account_id.try_lock() {
                            *current = Some(id);
                        }
                    }
                }
            }
        }

        // Keep both sync and async current_account_id pointing at a known account.
        let need_current = self
            .current_account_id_sync
            .lock()
            .ok()
            .and_then(|g| g.clone())
            .is_none();
        if need_current {
            if let Some(first) = loaded.first() {
                self.set_current_account_sync(Some(first.id.clone()));
                if let Ok(mut current) = self.current_account_id.try_lock() {
                    *current = Some(first.id.clone());
                }
            }
        } else if let Some(id) = self
            .current_account_id_sync
            .lock()
            .ok()
            .and_then(|g| g.clone())
        {
            // Mirror sync → async so get_current_account_id() works after cold start
            if let Ok(mut current) = self.current_account_id.try_lock() {
                if current.is_none() {
                    *current = Some(id);
                }
            }
        }

        if !loaded.is_empty() {
            if let Ok(mut guard) = self.accounts.try_lock() {
                *guard = loaded.clone();
            }
            // Ensure accounts.json exists for subsequent profile updates
            self.persist_accounts(&loaded);
        }
    }

    /// Persist the current accounts list to disk.
    fn persist_accounts(&self, accounts: &[Account]) {
        let path = self
            .data_dir
            .join(crate::models::account::ACCOUNTS_LIST_FILE);
        if let Ok(json) = serde_json::to_string_pretty(accounts) {
            let _ = fs::write(path, json);
        }
    }

    /// Get the path to stored token — account-specific if an account is selected.
    /// Uses a sync mirror of the current account id so we can be called outside
    /// an async context without panicking on blocking locks.
    fn token_file(&self) -> PathBuf {
        let id = self
            .current_account_id_sync
            .lock()
            .ok()
            .and_then(|g| g.clone());
        if let Some(id) = id {
            let account_dir = self.data_dir.join("accounts").join(id);
            fs::create_dir_all(&account_dir).ok();
            account_dir.join("token.json")
        } else {
            self.data_dir.join("token.json")
        }
    }

    /// Update the sync mirror of the current account id.
    fn set_current_account_sync(&self, id: Option<String>) {
        if let Ok(mut guard) = self.current_account_id_sync.lock() {
            *guard = id;
        }
    }

    pub fn get_device_id(&self) -> String {
        let path = self.data_dir.join("device_id.txt");
        if let Ok(id) = std::fs::read_to_string(&path) {
            let t = id.trim();
            if !t.is_empty() {
                return t.to_string();
            }
        }
        let new_id = uuid::Uuid::new_v4().to_string();
        let _ = std::fs::write(&path, &new_id);
        new_id
    }

    /// Start OAuth flow - returns the authorization URL
    pub fn auth_url(&self) -> String {
        self.auth_code_url()
    }

    pub fn auth_runtime_info(&self) -> (String, Option<String>, Option<String>) {
        (
            self.effective_client_id(),
            self.effective_redirect_uri(),
            self.effective_auth_proxy_url(),
        )
    }

    /// OAuth code flow URL for desktop apps.
    pub fn auth_code_url(&self) -> String {
        if let Some(proxy_url) = self.effective_auth_proxy_url() {
            return format!("{}/oauth/start", proxy_url);
        }

        let state = uuid::Uuid::new_v4().to_string();
        let device_id = self.get_device_id();
        let device_name = "Yandex Messenger Native";
        let client_id = self.effective_client_id();
        let mut params = vec![
            format!("response_type=token"),
            format!("client_id={}", urlencoding::encode(&client_id)),
            format!("state={}", urlencoding::encode(&state)),
            format!("device_id={}", urlencoding::encode(&device_id)),
            format!("device_name={}", urlencoding::encode(device_name)),
            "force_confirm=yes".to_string(),
        ];

        // Intentionally do not send `scope` to avoid invalid_scope mismatches.
        // Yandex OAuth will use permissions configured for the OAuth app.

        if let Some(redirect_uri) = self.effective_redirect_uri() {
            params.push(format!(
                "redirect_uri={}",
                urlencoding::encode(&redirect_uri)
            ));
        }

        format!("{}?{}", self.effective_authorize_url(), params.join("&"))
    }

    /// Parse access token from redirect URL, validating state if provided.
    /// Returns the parsed token and the validated state (if any).
    pub fn parse_token_from_url(
        &self,
        url: &str,
        expected_state: Option<&str>,
    ) -> Result<OAuthToken, String> {
        let fragment = url
            .split('#')
            .nth(1)
            .ok_or_else(|| "No fragment in URL".to_string())?;

        let params: Vec<(&str, &str)> = fragment
            .split('&')
            .filter_map(|pair| pair.split_once('='))
            .collect();

        // Validate state parameter (prevents CSRF attacks)
        if let Some(expected) = expected_state {
            if let Some((_, actual)) = params.iter().find(|(k, _)| *k == "state") {
                if *actual != expected {
                    return Err(format!(
                        "OAuth state mismatch: expected={}, got={}",
                        expected, actual
                    ));
                }
            } else if expected.trim().is_empty() {
                // Expected empty state — OK
            } else {
                return Err("OAuth state parameter missing (possible CSRF)".to_string());
            }
        }

        let access_token = params
            .iter()
            .find(|(k, _)| *k == "access_token")
            .map(|(_, v)| v.to_string())
            .ok_or_else(|| "No access_token in fragment".to_string())?;

        let expires_in = params
            .iter()
            .find(|(k, _)| *k == "expires_in")
            .and_then(|(_, v)| v.parse().ok())
            .unwrap_or(3600);

        let user_id = params
            .iter()
            .find(|(k, _)| *k == "user_id")
            .map(|(_, v)| v.to_string());

        let refresh_token = params
            .iter()
            .find(|(k, _)| *k == "refresh_token")
            .map(|(_, v)| v.to_string());

        let received_at = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        Ok(OAuthToken {
            access_token,
            refresh_token,
            expires_in,
            token_type: "Bearer".to_string(),
            user_id,
            received_at,
        })
    }

    /// Save token to disk
    pub async fn save_token(&self, token: &OAuthToken) -> Result<(), String> {
        let token_json = serde_json::to_string_pretty(token)
            .map_err(|e| format!("Token serialize failed: {}", e))?;

        fs::write(&self.token_file(), token_json)
            .map_err(|e| format!("Token write failed: {}", e))?;

        info!("Token saved to {}", self.token_file().display());
        Ok(())
    }

    /// Load token from disk
    pub async fn load_token(&self) -> Result<OAuthToken, String> {
        let path = self.token_file();
        if !path.exists() {
            return Err("Token file not found".to_string());
        }

        let token_json =
            fs::read_to_string(&path).map_err(|e| format!("Token read failed: {}", e))?;

        let token: OAuthToken =
            serde_json::from_str(&token_json).map_err(|e| format!("Token parse failed: {}", e))?;

        debug!("Token loaded from {}", path.display());
        Ok(token)
    }

    /// Get current token (from memory or disk)
    pub async fn get_token(&self) -> Result<OAuthToken, String> {
        let token = self.token.lock().await;

        if let Some(ref t) = *token {
            if !t.is_expired() {
                return Ok(t.clone());
            }
        }

        drop(token);

        let token = self.load_token().await?;

        let mut t = self.token.lock().await;
        *t = Some(token.clone());

        Ok(token)
    }

    /// Set token in memory
    pub async fn set_token(&self, token: OAuthToken) -> Result<(), String> {
        let mut t = self.token.lock().await;
        *t = Some(token.clone());
        drop(t);

        self.save_token(&token).await
    }

    /// Refresh token using refresh_token. Extracts user_id from response if present.
    pub async fn refresh_token(&self, refresh_token: &str) -> Result<OAuthToken, String> {
        let client = reqwest::Client::new();
        let client_id = self.effective_client_id();
        let mut last_error = "unknown refresh error".to_string();
        let mut json: Option<serde_json::Value> = None;

        for token_url in self.token_urls() {
            let response = match client
                .post(&token_url)
                .form(&[
                    ("grant_type", "refresh_token"),
                    ("client_id", &client_id),
                    ("refresh_token", refresh_token),
                ])
                .send()
                .await
            {
                Ok(v) => v,
                Err(e) => {
                    last_error = format!("Refresh request failed: {}", e);
                    continue;
                }
            };

            match response.json::<serde_json::Value>().await {
                Ok(v) => {
                    json = Some(v.clone());
                    if v.get("error").is_none() {
                        break;
                    }
                    let err = v.get("error").and_then(|e| e.as_str()).unwrap_or("unknown");
                    let desc = v
                        .get("error_description")
                        .and_then(|d| d.as_str())
                        .unwrap_or("unknown");
                    last_error = format!("Refresh failed: {} ({})", err, desc);
                }
                Err(e) => last_error = format!("Refresh parse failed: {}", e),
            }
        }

        let json = json.ok_or(last_error.clone())?;

        if let Some(err) = json.get("error").and_then(|v| v.as_str()) {
            let desc = json
                .get("error_description")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown error");
            return Err(format!("Refresh failed: {} ({})", err, desc));
        }
        if json.get("access_token").is_none() {
            return Err(last_error);
        }

        let access_token = json["access_token"]
            .as_str()
            .ok_or_else(|| "No access_token in refresh response".to_string())?
            .to_string();

        let expires_in = json["expires_in"].as_u64().unwrap_or(3600);

        let user_id = json["user_id"].as_str().map(|s| s.to_string());

        let received_at = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        // Fix: check for new refresh_token in response (Yandex may rotate it)
        let new_refresh_token = json
            .get("refresh_token")
            .and_then(|v| v.as_str())
            .map(|v| v.to_string());
        let refresh_token = new_refresh_token.or(Some(refresh_token.to_string()));

        Ok(OAuthToken {
            access_token,
            refresh_token,
            expires_in,
            token_type: "Bearer".to_string(),
            user_id,
            received_at,
        })
    }

    /// Exchange authorization code for OAuth token.
    pub async fn exchange_code(&self, code: &str) -> Result<OAuthToken, String> {
        if let Some(proxy_url) = self.effective_auth_proxy_url() {
            return self.exchange_code_via_proxy(&proxy_url, code).await;
        }

        let client = reqwest::Client::new();
        let client_id = self.effective_client_id();
        let mut params = vec![
            ("grant_type", "authorization_code".to_string()),
            ("code", code.trim().to_string()),
            ("client_id", client_id.clone()),
            ("device_id", self.get_device_id()),
            ("device_name", "Yandex Messenger Native".to_string()),
        ];

        if let Some(redirect_uri) = self.effective_redirect_uri() {
            params.push(("redirect_uri", redirect_uri));
        }

        let mut secret_for_basic: Option<String> = None;
        if let Ok(secret) = std::env::var("YANDEX_CLIENT_SECRET") {
            let trimmed = secret.trim();
            if !trimmed.is_empty() {
                secret_for_basic = Some(trimmed.to_string());
            }
        }

        // Try RFC-compatible Basic auth first when client_secret is provided.
        // Some OAuth server configurations expect credentials in Authorization header.
        let mut last_error = "unknown oauth exchange error".to_string();
        let mut json: Option<serde_json::Value> = None;
        for token_url in self.token_urls() {
            if let Some(secret) = secret_for_basic.clone() {
                let basic = format!("{}:{}", params[2].1, secret);
                let basic_b64 = base64::engine::general_purpose::STANDARD.encode(basic.as_bytes());
                let mut basic_params = vec![
                    ("grant_type", "authorization_code".to_string()),
                    ("code", code.trim().to_string()),
                ];
                if let Some(redirect_uri) = self.effective_redirect_uri() {
                    basic_params.push(("redirect_uri", redirect_uri));
                }
                let response = match client
                    .post(&token_url)
                    .header("Authorization", format!("Basic {}", basic_b64))
                    .form(&basic_params)
                    .send()
                    .await
                {
                    Ok(v) => v,
                    Err(e) => {
                        last_error = format!("Token exchange request failed (basic): {}", e);
                        continue;
                    }
                };
                let first_json: serde_json::Value = response
                    .json()
                    .await
                    .map_err(|e| format!("Token exchange parse failed (basic): {}", e))?;
                let has_auth_error = first_json
                    .get("error")
                    .and_then(|e| e.as_str())
                    .map(|e| e == "invalid_client" || e == "unauthorized_client")
                    .unwrap_or(false);
                if !has_auth_error {
                    json = Some(first_json);
                    break;
                }
                last_error = first_json
                    .get("error_description")
                    .and_then(|v| v.as_str())
                    .unwrap_or("oauth auth error")
                    .to_string();
            }

            if let Some(secret) = secret_for_basic.clone() {
                // If it wasn't already pushed in a previous loop iteration
                if !params.iter().any(|(k, _)| *k == "client_secret") {
                    params.push(("client_secret", secret.clone()));
                }
            }

            let response = client
                .post(&token_url)
                .form(&params)
                .send()
                .await
                .map_err(|e| format!("Token exchange request failed: {}", e))?;
            let fallback_json: serde_json::Value = response
                .json()
                .await
                .map_err(|e| format!("Token exchange parse failed: {}", e))?;
            if fallback_json.get("error").is_none() {
                json = Some(fallback_json);
                break;
            }
            last_error = fallback_json
                .get("error_description")
                .and_then(|v| v.as_str())
                .unwrap_or("oauth exchange error")
                .to_string();
            json = Some(fallback_json);
        }
        let json = json.ok_or(last_error)?;

        if let Some(err) = json.get("error").and_then(|e| e.as_str()) {
            let desc = json
                .get("error_description")
                .and_then(|d| d.as_str())
                .unwrap_or("unknown error");
            return Err(format!(
                "OAuth exchange failed: {} ({}) at {}",
                err,
                desc,
                self.token_urls()
                    .first()
                    .cloned()
                    .unwrap_or_else(|| config::OAUTH_TOKEN_URL.to_string())
            ));
        }

        let access_token = json
            .get("access_token")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "No access_token in token response".to_string())?
            .to_string();

        let refresh_token = json
            .get("refresh_token")
            .and_then(|v| v.as_str())
            .map(|v| v.to_string());

        let expires_in = json
            .get("expires_in")
            .and_then(|v| v.as_u64())
            .unwrap_or(3600);

        let user_id = json
            .get("user_id")
            .and_then(|v| v.as_str())
            .map(|v| v.to_string());

        let received_at = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        Ok(OAuthToken {
            access_token,
            refresh_token,
            expires_in,
            token_type: "Bearer".to_string(),
            user_id,
            received_at,
        })
    }

    async fn exchange_code_via_proxy(
        &self,
        proxy_url: &str,
        code: &str,
    ) -> Result<OAuthToken, String> {
        let client = reqwest::Client::new();
        let endpoint = format!("{}/oauth/exchange", proxy_url.trim_end_matches('/'));

        let response = client
            .post(&endpoint)
            .json(&serde_json::json!({ "code": code.trim() }))
            .send()
            .await
            .map_err(|e| format!("Auth proxy request failed: {}", e))?;

        if !response.status().is_success() {
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "failed to read proxy error".to_string());
            return Err(format!("Auth proxy exchange failed: {}", body));
        }

        let json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| format!("Auth proxy parse failed: {}", e))?;

        let access_token = json
            .get("access_token")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "No access_token in auth-proxy response".to_string())?
            .to_string();

        let refresh_token = json
            .get("refresh_token")
            .and_then(|v| v.as_str())
            .map(|v| v.to_string());

        let expires_in = json
            .get("expires_in")
            .and_then(|v| v.as_u64())
            .unwrap_or(3600);

        let user_id = json
            .get("user_id")
            .and_then(|v| v.as_str())
            .map(|v| v.to_string());

        let received_at = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        Ok(OAuthToken {
            access_token,
            refresh_token,
            expires_in,
            token_type: "Bearer".to_string(),
            user_id,
            received_at,
        })
    }

    /// Logout - remove token
    pub async fn logout(&self) -> Result<(), String> {
        let mut token = self.token.lock().await;
        *token = None;
        drop(token);

        // Remove the current account (if any) so the UI doesn't try to use stale data.
        let current = self.current_account_id.lock().await.clone();
        if let Some(id) = current {
            let _ = self.remove_account(&id).await;
        } else if self.token_file().exists() {
            fs::remove_file(self.token_file())
                .map_err(|e| format!("Token removal failed: {}", e))?;
        }

        info!("Logged out, token removed");
        Ok(())
    }

    /// Async logout for UI integration — returns a Future that always completes successfully.
    pub fn logout_async(&self) -> impl std::future::Future<Output = ()> {
        let manager = self.clone();
        async move {
            let _ = manager.logout().await;
        }
    }

    /// Check if user is authenticated
    pub async fn is_authenticated(&self) -> bool {
        if std::env::var("YANDEX_FORCE_AUTH")
            .ok()
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("yes"))
            .unwrap_or(false)
        {
            return false;
        }

        let token = self.token.lock().await;
        if let Some(ref t) = *token {
            return !t.is_expired();
        }
        drop(token);

        match self.load_token().await {
            Ok(token) => !token.is_expired(),
            Err(_) => false,
        }
    }

    // ---------------------------------------------------------------------------
    // New methods (requested improvements)
    // ---------------------------------------------------------------------------

    /// Check whether the current token has a refresh_token available.
    pub async fn has_refresh_token(&self) -> bool {
        let token = self.token.lock().await;
        token.as_ref().is_some_and(|t| t.refresh_token.is_some())
            || self
                .load_token()
                .await
                .is_ok_and(|t| t.refresh_token.is_some())
    }

    /// Returns the number of seconds until the token expires, or None if no token.
    pub fn time_until_expiry(&self) -> Option<u64> {
        let token = self.token.try_lock().ok()?;
        let t = token.as_ref()?;
        Self::time_until_expiry_for(t)
    }

    /// Internal helper: calculate seconds until expiry for a given token.
    fn time_until_expiry_for(t: &OAuthToken) -> Option<u64> {
        if t.received_at == 0 {
            // Legacy token without received_at — use simple heuristic
            if t.expires_in <= 300 {
                return Some(0);
            }
            return Some(t.expires_in.saturating_sub(300));
        }
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let expiry = t.received_at + t.expires_in;
        if now >= expiry {
            Some(0)
        } else {
            Some(expiry - now)
        }
    }

    /// Validates the access token and returns it, or an appropriate AuthError.
    pub fn get_access_token(&self) -> Result<String, AuthError> {
        let token = self.token.try_lock().ok().ok_or(AuthError::NotFound)?;
        let t = token.as_ref().ok_or(AuthError::NotFound)?;

        if t.is_expired() {
            return Err(AuthError::Expired);
        }

        Ok(t.access_token.clone())
    }

    /// Auto-refreshes the token if it is expired or about to expire (within 5 minutes).
    /// Returns Ok(()) if the token is valid or was successfully refreshed.
    pub async fn refresh_if_needed(&self) -> Result<(), String> {
        // Fast path: check in-memory token first
        {
            let token = self.token.lock().await;
            if let Some(ref t) = *token {
                if !t.is_expired() {
                    return Ok(());
                }
            }
        }

        // Load token from disk to get refresh_token
        let token = self.load_token().await?;

        let refresh = token
            .refresh_token
            .ok_or_else(|| "No refresh_token available; user must re-authenticate".to_string())?;

        let new_token = self.refresh_token(&refresh).await?;

        // Update in-memory and on-disk
        self.set_token(new_token).await?;

        Ok(())
    }

    /// Fetches the user profile from Yandex OAuth API using the current valid access token.
    pub async fn get_user_info(&self) -> Result<crate::models::User, String> {
        // Ensure we have a valid token (auto-refresh if needed)
        let access_token = match self.get_access_token() {
            Ok(t) => t,
            Err(AuthError::Expired) => {
                self.refresh_if_needed().await?;
                self.get_access_token().map_err(|e| e.to_string())?
            }
            Err(e) => return Err(e.to_string()),
        };

        let client = reqwest::Client::new();
        let response = client
            .get("https://login.yandex.ru/info")
            .header("Authorization", format!("OAuth {}", access_token))
            .send()
            .await
            .map_err(|e| format!("Failed to fetch user info: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "unknown".to_string());
            return Err(format!("User info request failed ({}): {}", status, body));
        }

        let body = response
            .text()
            .await
            .map_err(|e| format!("Failed to read response: {}", e))?;
        let json: serde_json::Value = serde_json::from_str(&body)
            .map_err(|e| format!("Failed to parse user info response: {}", e))?;

        let id = json["id"]
            .as_str()
            .or_else(|| json["uid"].as_str())
            .ok_or_else(|| "No id in user info response".to_string())?
            .to_string();

        let login = json["login"].as_str().map(|s| s.to_string());

        let phone = json
            .get("default_phone")
            .and_then(|p| p.get("number"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let first_name = json["first_name"].as_str().map(|s| s.to_string());
        let last_name = json["last_name"].as_str().map(|s| s.to_string());

        // Prefer human names: real_name → display_name → first+last → login
        let display_name = json["real_name"]
            .as_str()
            .or_else(|| json["display_name"].as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .or_else(|| {
                let mut parts = Vec::new();
                if let Some(f) = &first_name {
                    parts.push(f.as_str());
                }
                if let Some(l) = &last_name {
                    parts.push(l.as_str());
                }
                if parts.is_empty() {
                    None
                } else {
                    Some(parts.join(" "))
                }
            })
            .or_else(|| login.clone());

        let username = login.clone();

        // OAuth returns numeric default_avatar_id; messenger uses user_avatar/yapic/...
        let avatar_id = json["default_avatar_id"]
            .as_str()
            .map(|s| {
                if s.starts_with("user_avatar/") || s.contains('/') {
                    s.to_string()
                } else {
                    format!("user_avatar/yapic/{}", s)
                }
            })
            .or_else(|| {
                json["default_avatar_id"]
                    .as_i64()
                    .map(|n| format!("user_avatar/yapic/{}", n))
            });

        let status_raw = json["status"].as_str();
        let is_bot = json["is_bot"].as_bool().unwrap_or(false);
        let is_premium = json["is_premium"].as_bool().unwrap_or(false);

        Ok(crate::models::User {
            id,
            phone,
            email: login,
            first_name,
            last_name,
            display_name,
            username,
            avatar_id,
            status: status_raw.map(|s| s.to_string()),
            is_bot,
            is_premium,
        })
    }

    /// Update current account display name / avatar (from bootstrap or OAuth).
    pub async fn update_current_profile(
        &self,
        display_name: Option<String>,
        avatar_id: Option<String>,
    ) -> Result<(), String> {
        let id = self
            .get_current_account_id()
            .await
            .or_else(|| {
                self.current_account_id_sync
                    .lock()
                    .ok()
                    .and_then(|g| g.clone())
            })
            .ok_or_else(|| "No current account".to_string())?;

        // Ensure async id is set
        *self.current_account_id.lock().await = Some(id.clone());
        self.set_current_account_sync(Some(id.clone()));

        let mut accounts = self.accounts.lock().await;
        if let Some(acc) = accounts.iter_mut().find(|a| a.id == id) {
            if let Some(name) = display_name {
                let name = name.trim().to_string();
                if !name.is_empty() {
                    acc.display_name = Some(name);
                }
            }
            if let Some(avatar) = avatar_id {
                if !avatar.is_empty() {
                    acc.avatar_url = Some(avatar);
                }
            }
            let name_log = acc.display_name.clone();
            let avatar_log = acc
                .avatar_url
                .as_ref()
                .map(|s| s.chars().take(40).collect::<String>());
            self.persist_accounts(&accounts);
            log::info!(
                "Updated account profile: name={:?}, avatar={:?}",
                name_log,
                avatar_log
            );
            Ok(())
        } else {
            // Account missing from list — synthesize from token
            let token = self.token.lock().await.clone();
            let access = token
                .as_ref()
                .map(|t| t.access_token.clone())
                .unwrap_or_default();
            let mut account = Account::new(id.clone(), access);
            account.display_name = display_name.filter(|s| !s.trim().is_empty());
            account.avatar_url = avatar_id.filter(|s| !s.is_empty());
            if let Some(t) = token {
                account.refresh_token = t.refresh_token;
                account.expires_at = t.received_at + t.expires_in;
            }
            accounts.push(account);
            self.persist_accounts(&accounts);
            Ok(())
        }
    }

    /// Apply a User profile onto the current account and persist.
    pub async fn apply_user_profile(&self, user: &crate::models::User) -> Result<(), String> {
        let name = user
            .display_name
            .clone()
            .or_else(|| {
                match (&user.first_name, &user.last_name) {
                    (Some(f), Some(l)) => Some(format!("{} {}", f, l)),
                    (Some(f), None) => Some(f.clone()),
                    (None, Some(l)) => Some(l.clone()),
                    _ => None,
                }
            })
            .or_else(|| user.username.clone())
            .or_else(|| user.email.clone());

        // Prefer messenger guid as account id when token has messenger-style id
        if let Some(token_uid) = self.user_id() {
            if token_uid != user.id && token_uid.contains('-') {
                // keep messenger guid as account id
                let _ = token_uid;
            }
        }

        self.update_current_profile(name, user.avatar_id.clone())
            .await
    }

    /// Validates the current session by fetching the user profile from the OAuth API.
    /// This checks that the token is still valid (not just not expired) and works
    /// correctly even if the token was revoked on the server side.
    pub async fn validate_session(&self) -> Result<(), String> {
        // Get a valid token (auto-refresh if expired)
        self.refresh_if_needed().await?;

        let access_token = self.get_access_token().map_err(|e| e.to_string())?;

        let client = reqwest::Client::new();
        let response = client
            .get("https://login.yandex.ru/info")
            .header("Authorization", format!("OAuth {}", access_token))
            .send()
            .await
            .map_err(|e| format!("Session validation request failed: {}", e))?;

        match response.status().as_u16() {
            200 => Ok(()),
            401 | 403 => {
                // Token was revoked or invalidated on the server
                let mut token = self.token.lock().await;
                *token = None;
                drop(token);
                Err("Session invalidated: token revoked or expired".to_string())
            }
            _ => {
                let status = response.status();
                let body = response
                    .text()
                    .await
                    .unwrap_or_else(|_| "unknown".to_string());
                Err(format!("Session validation failed ({}): {}", status, body))
            }
        }
    }

    // ─────────────────────────────────────────────
    // Multi-account support
    // ─────────────────────────────────────────────

    /// Get the currently active account ID (if any)
    pub async fn get_current_account_id(&self) -> Option<String> {
        let async_id = self.current_account_id.lock().await.clone();
        if async_id.is_some() {
            return async_id;
        }
        // Fall back to sync mirror (set during cold-start load)
        let sync_id = self
            .current_account_id_sync
            .lock()
            .ok()
            .and_then(|g| g.clone());
        if let Some(ref id) = sync_id {
            *self.current_account_id.lock().await = Some(id.clone());
        }
        sync_id
    }

    /// Get the currently active account (if any)
    pub async fn get_current_account(&self) -> Option<Account> {
        let id = self.get_current_account_id().await?;
        let accounts = self.accounts.lock().await;
        accounts.iter().cloned().find(|a| a.id == id)
    }

    /// List all known accounts
    pub async fn list_accounts(&self) -> Vec<Account> {
        self.accounts.lock().await.clone()
    }

    /// Get the display name of the currently active account
    pub async fn current_account_name(&self) -> Option<String> {
        let id = self.get_current_account_id().await?;
        let accounts = self.accounts.lock().await;
        accounts
            .iter()
            .find(|a| a.id == id)
            .map(|a| a.display_label())
    }

    /// Switch to a specific account by ID
    pub async fn switch_account(&self, account_id: &str) -> Result<(), String> {
        let accounts = self.accounts.lock().await;
        if let Some(account) = accounts.iter().find(|a| a.id == account_id).cloned() {
            drop(accounts);
            // Point sync/async mirrors at the new account BEFORE touching the file system
            *self.current_account_id.lock().await = Some(account_id.to_string());
            self.set_current_account_sync(Some(account_id.to_string()));

            // Load token for the requested account
            let account_dir = self.data_dir.join("accounts").join(&account.id);
            let token_path = account_dir.join("token.json");
            if !token_path.exists() {
                return Err("Token file not found for account".to_string());
            }
            let token_json =
                fs::read_to_string(&token_path).map_err(|e| format!("Token read failed: {}", e))?;
            let token: OAuthToken = serde_json::from_str(&token_json)
                .map_err(|e| format!("Token parse failed: {}", e))?;
            *self.token.lock().await = Some(token);
            Ok(())
        } else {
            Err(format!("Account not found: {}", account_id))
        }
    }

    /// Add a new account from a token
    pub async fn add_account(
        &self,
        token: &OAuthToken,
        user: &crate::models::User,
    ) -> Result<String, String> {
        // Prefer a stable id based on the Yandex user id so repeated logins
        // don't create duplicate account entries.
        let account_id = token
            .user_id
            .clone()
            .or_else(|| Some(user.id.clone()))
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

        let account_dir = self.data_dir.join("accounts").join(&account_id);
        fs::create_dir_all(&account_dir)
            .map_err(|e| format!("Cannot create account dir: {}", e))?;

        let token_json = serde_json::to_string_pretty(token)
            .map_err(|e| format!("Token serialize failed: {}", e))?;
        fs::write(account_dir.join("token.json"), token_json)
            .map_err(|e| format!("Token write failed: {}", e))?;

        let mut accounts = self.accounts.lock().await;
        // Replace an existing entry with the same id, otherwise append.
        let mut account = Account::new(account_id.clone(), token.access_token.clone());
        account.display_name = user
            .display_name
            .clone()
            .or_else(|| user.first_name.clone())
            .or_else(|| user.username.clone());
        account.avatar_url = user.avatar_id.clone();
        account.expires_at = token.received_at + token.expires_in;
        account.refresh_token = token.refresh_token.clone();

        if let Some(existing) = accounts.iter_mut().find(|a| a.id == account_id) {
            *existing = account;
        } else {
            accounts.push(account);
        }

        self.persist_accounts(&accounts);
        drop(accounts);

        // Make the new account current, both in async and sync mirrors.
        *self.current_account_id.lock().await = Some(account_id.clone());
        self.set_current_account_sync(Some(account_id.clone()));

        Ok(account_id)
    }

    /// Save the current account's token to its directory
    async fn save_account_token(&self, account: &Account) -> Result<(), String> {
        let account_dir = self.data_dir.join("accounts").join(&account.id);
        let token_path = account_dir.join("token.json");
        if !token_path.exists() {
            return Err("Token file not found".to_string());
        }
        let token_json =
            fs::read_to_string(&token_path).map_err(|e| format!("Token read failed: {}", e))?;
        let mut token: OAuthToken =
            serde_json::from_str(&token_json).map_err(|e| format!("Token parse failed: {}", e))?;
        // Update expiry
        token.received_at = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let updated_json = serde_json::to_string_pretty(&token)
            .map_err(|e| format!("Token serialize failed: {}", e))?;
        fs::write(&token_path, updated_json).map_err(|e| format!("Token write failed: {}", e))?;
        Ok(())
    }

    /// Remove an account by ID
    pub async fn remove_account(&self, account_id: &str) -> Result<(), String> {
        let mut accounts = self.accounts.lock().await;
        let idx = accounts.iter().position(|a| a.id == account_id);
        if idx.is_none() {
            return Err("Account not found".to_string());
        }

        let is_current = self.current_account_id.lock().await.as_deref() == Some(account_id);
        accounts.remove(idx.unwrap());
        self.persist_accounts(&accounts);
        drop(accounts);

        if is_current {
            *self.current_account_id.lock().await = None;
            *self.token.lock().await = None;
            self.set_current_account_sync(None);
        }

        // Remove account directory
        let account_dir = self.data_dir.join("accounts").join(account_id);
        let _ = fs::remove_dir_all(&account_dir);

        Ok(())
    }

    /// Remove the current account
    pub async fn remove_current_account(&self) -> Result<(), String> {
        let id = self.get_current_account_id().await;
        match id {
            Some(id) => self.remove_account(&id).await,
            None => Err("No current account".to_string()),
        }
    }

    /// Check if multi-account mode is enabled (more than one account)
    pub async fn is_multi_account(&self) -> bool {
        self.accounts.lock().await.len() > 1
    }
}
