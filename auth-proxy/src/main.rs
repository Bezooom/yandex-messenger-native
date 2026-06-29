use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Redirect};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::Arc;

#[derive(Clone)]
struct AppState {
    client_id: String,
    client_secret: String,
    redirect_uri: String,
    oauth_scopes: String,
    oauth_authorize_url: String,
    oauth_token_url: String,
}

#[derive(Debug, Deserialize)]
struct ExchangeRequest {
    code: String,
}

#[derive(Debug, Serialize)]
struct ExchangeResponse {
    access_token: String,
    refresh_token: Option<String>,
    expires_in: u64,
    token_type: String,
    user_id: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), String> {
    let client_id = env_required("YANDEX_CLIENT_ID")?;
    let client_secret = env_required("YANDEX_CLIENT_SECRET")?;
    let redirect_uri = env_required("YANDEX_REDIRECT_URI")?;
    let oauth_scopes = env_optional(
        "YANDEX_OAUTH_SCOPES",
        "login:info login:avatar login:birthday login:email login:photos login:skills login:social",
    );
    let oauth_authorize_url = env_optional("YANDEX_OAUTH_AUTHORIZE_URL", "https://oauth.yandex.com/authorize");
    let oauth_token_url = env_optional("YANDEX_OAUTH_TOKEN_URL", "https://oauth.yandex.com/token");
    let bind_addr = env_optional("AUTH_PROXY_BIND", "127.0.0.1:8080");
    let addr: SocketAddr = bind_addr
        .parse()
        .map_err(|e| format!("Invalid AUTH_PROXY_BIND '{}': {}", bind_addr, e))?;

    let state = Arc::new(AppState {
        client_id,
        client_secret,
        redirect_uri,
        oauth_scopes,
        oauth_authorize_url,
        oauth_token_url,
    });

    let app = Router::new()
        .route("/health", get(health))
        .route("/oauth/start", get(oauth_start))
        .route("/oauth/callback", get(oauth_callback))
        .route("/oauth/exchange", post(oauth_exchange))
        .with_state(state);

    println!("auth-proxy listening on http://{}", addr);
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .map_err(|e| format!("Bind failed: {}", e))?;
    axum::serve(listener, app)
        .await
        .map_err(|e| format!("Server failed: {}", e))?;
    Ok(())
}

async fn health() -> &'static str {
    "ok"
}

async fn oauth_start(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let authorize_url = format!(
        "{}?response_type=code&client_id={}&redirect_uri={}&scope={}&force_confirm=yes",
        state.oauth_authorize_url,
        urlencoding::encode(&state.client_id),
        urlencoding::encode(&state.redirect_uri),
        urlencoding::encode(&state.oauth_scopes),
    );
    Redirect::temporary(&authorize_url)
}

#[derive(Debug, Deserialize)]
struct CallbackQuery {
    code: Option<String>,
    error: Option<String>,
    error_description: Option<String>,
}

async fn oauth_callback(
    State(_state): State<Arc<AppState>>,
    axum::extract::Query(query): axum::extract::Query<CallbackQuery>,
) -> impl IntoResponse {
    if let Some(error) = query.error {
        let description = query.error_description.unwrap_or_else(|| "unknown error".to_string());
        let html = format!(
            "<html><body><h2>OAuth error</h2><p>{}</p><p>{}</p><p>Return to the app and retry.</p></body></html>",
            html_escape(&error),
            html_escape(&description)
        );
        return Html(html);
    }

    let code = query.code.unwrap_or_default();
    let html = format!(
        "<html><body><h2>Authorization code received</h2><p>Copy this code and paste it into the app:</p><pre style=\"font-size:20px;\">{}</pre><p>You can close this tab.</p></body></html>",
        html_escape(&code)
    );
    Html(html)
}

async fn oauth_exchange(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ExchangeRequest>,
) -> Result<Json<ExchangeResponse>, (StatusCode, String)> {
    if payload.code.trim().is_empty() {
        return Err((StatusCode::BAD_REQUEST, "code is required".to_string()));
    }

    let client = reqwest::Client::new();
    let response = client
        .post(&state.oauth_token_url)
        .form(&[
            ("grant_type", "authorization_code"),
            ("code", payload.code.trim()),
            ("client_id", state.client_id.as_str()),
            ("client_secret", state.client_secret.as_str()),
            ("redirect_uri", state.redirect_uri.as_str()),
        ])
        .send()
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, format!("token request failed: {}", e)))?;

    let status = response.status();
    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, format!("token parse failed: {}", e)))?;

    if !status.is_success() {
        return Err((
            StatusCode::BAD_GATEWAY,
            format!("oauth upstream error: {}", json),
        ));
    }

    if let Some(err) = json.get("error").and_then(|v| v.as_str()) {
        let desc = json
            .get("error_description")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown error");
        return Err((
            StatusCode::BAD_REQUEST,
            format!("oauth exchange failed: {} ({})", err, desc),
        ));
    }

    let access_token = json
        .get("access_token")
        .and_then(|v| v.as_str())
        .ok_or_else(|| (StatusCode::BAD_GATEWAY, "missing access_token".to_string()))?
        .to_string();
    let refresh_token = json
        .get("refresh_token")
        .and_then(|v| v.as_str())
        .map(|v| v.to_string());
    let expires_in = json.get("expires_in").and_then(|v| v.as_u64()).unwrap_or(3600);

    Ok(Json(ExchangeResponse {
        access_token,
        refresh_token,
        expires_in,
        token_type: "Bearer".to_string(),
        user_id: None,
    }))
}

fn env_required(name: &str) -> Result<String, String> {
    std::env::var(name)
        .map(|v| v.trim().to_string())
        .map_err(|_| format!("Missing required env var {}", name))
        .and_then(|v| {
            if v.is_empty() {
                Err(format!("Env var {} cannot be empty", name))
            } else {
                Ok(v)
            }
        })
}

fn env_optional(name: &str, default_value: &str) -> String {
    std::env::var(name)
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| default_value.to_string())
}

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
