use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, Context, Result};
use dirs::cache_dir;
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};

/// Credentials for login
#[derive(Debug, Serialize)]
pub struct Credentials {
    pub username: String,
    pub password: String,
}

/// Holds access and refresh token with expiry
#[derive(Debug, Serialize, Deserialize)]
struct TokenStore {
    access_token: String,
    refresh_token: String,
    expires_at: u64,
}

pub struct AuthService {
    client: Client,
    token_file: PathBuf,
    api_base: String,
}

impl AuthService {
    /// Create new AuthService.
    pub fn new(api_base: impl Into<String>) -> Result<Self> {
        let base = api_base.into();
        let cache = cache_dir()
            .ok_or_else(|| anyhow!("Unable to find cache directory"))?; // may fail on minimal env
        let token_file = cache.join("yandex-messenger").join("auth_tokens.json");
        let client = Client::builder()
            .https_only(true)
            .user_agent("yandex-messenger/native")
            .timeout(Duration::from_secs(30))
            .build()?;
        Ok(Self { client, token_file, api_base: base })
    }

    /// Login credentials and store tokens.
    pub async fn login(&self, creds: Credentials) -> Result<()> {
        let url = format!("{}/auth/login", self.api_base);
        let resp = self
            .client
            .post(&url)
            .json(&creds)
            .send()
            .await
            .context("Failed to send login request")?;

        match resp.status() {
            StatusCode::OK => {
                let token: TokenResponse = resp
                    .json()
                    .await
                    .context("Failed to parse token response")?;
                self.save_tokens(&token).context("Failed to save tokens")?;
                Ok(())
            }
            status => Err(anyhow!("Login failed: {}", status)),
        }
    }

    /// Return current access token, refreshing if needed.
    pub async fn get_access_token(&self) -> Result<String> {
        let tokens = self.load_tokens()?;
        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
        if now >= tokens.expires_at - 60 {
            // refresh if less than 1 minute remaining
            let refreshed = self.refresh(&tokens.refresh_token).await?;
            self.save_tokens(&refreshed)?;
            Ok(refreshed.access_token)
        } else {
            Ok(tokens.access_token)
        }
    }

    /// Refresh tokens using refresh token.
    async fn refresh(&self, refresh_token: &str) -> Result<TokenResponse> {
        let url = format!("{}/auth/refresh", self.api_base);
        let resp = self
            .client
            .post(&url)
            .json(&serde_json::json!({"refresh_token": refresh_token}))
            .send()
            .await
            .context("Failed to send refresh request")?;
        match resp.status() {
            StatusCode::OK => {
                let token: TokenResponse = resp
                    .json()
                    .await
                    .context("Failed to parse token response")?;
                Ok(token)
            }
            status => Err(anyhow!("Refresh failed: {}", status)),
        }
    }

    fn save_tokens(&self, token: &TokenResponse) -> Result<()> {
        if let Some(parent) = self.token_file.parent() {
            std::fs::create_dir_all(parent).context("Create dirs for token file")?;
        }
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&self.token_file)
            .context("Open token file")?;
        let data = serde_json::to_string_pretty(token)?;
        file.write_all(data.as_bytes()).context("Write token file")?;
        Ok(())
    }

    fn load_tokens(&self) -> Result<TokenStore> {
        let mut file = File::open(&self.token_file).context("Open token file")?;
        let mut data = String::new();
        file.read_to_string(&mut data).context("Read token file")?;
        let store: TokenStore = serde_json::from_str(&data).context("Parse token file")?;
        Ok(store)
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: String,
    expires_in: u64,
}

impl From<TokenResponse> for TokenStore {
    fn from(tr: TokenResponse) -> Self {
        let expires_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            + tr.expires_in;
        Self {
            access_token: tr.access_token,
            refresh_token: tr.refresh_token,
            expires_at,
        }
    }
}

impl From<&TokenStore> for TokenResponse {
    fn from(ts: &TokenStore) -> Self {
        Self {
            access_token: ts.access_token.clone(),
            refresh_token: ts.refresh_token.clone(),
            expires_in: ts.expires_at - SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
        }
    }
}

