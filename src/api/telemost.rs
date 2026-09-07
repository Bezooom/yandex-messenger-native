//! Telemost Cloud API client — port of the APK meeting lifecycle.
//!
//! Method names are verified in DEX (`create_personal_meeting`,
//! `start_meeting_call[_ringing]`, `end_personal_meeting`, `meeting_info[s]`,
//! `meetingId(s)`, `join_url`); the HTTP mapping below
//! (`POST {api}/telemost/<method>`) is best-effort and must be confirmed
//! against a live server. Parsing stays tolerant: unknown fields are kept,
//! nesting (`success`/`data`/`meeting`) is unwrapped, and `UserErrors`
//! envelopes become `Err`.

use reqwest::Client;
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::sync::Arc;

use crate::api::auth::AuthManager;
use crate::config;
use crate::models::telemost::{MeetingCall, MeetingInfo, MeetingUserError, PersonalMeeting};

/// Verified REST root from DEX strings.
pub const MESSENGER_CLOUD_API: &str = "https://api.messenger.yandex.net/api";

pub struct TelemostClient {
    auth: Arc<AuthManager>,
    client: Client,
    api_base: String,
}

impl TelemostClient {
    pub fn new(auth: Arc<AuthManager>) -> Self {
        Self::with_base(auth, MESSENGER_CLOUD_API)
    }

    pub fn with_base(auth: Arc<AuthManager>, api_base: &str) -> Self {
        let mut builder = Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .user_agent("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36");
        if api_base.starts_with("http://127.0.0.1") || api_base.starts_with("http://localhost") {
            builder = builder.no_proxy();
        }
        Self {
            auth: auth.clone(),
            client: builder.build().unwrap_or_default(),
            api_base: api_base.trim_end_matches('/').to_string(),
        }
    }

    pub fn api_base(&self) -> &str {
        &self.api_base
    }

    fn auth_header(&self) -> String {
        match self.auth.get_access_token() {
            Ok(token) if token.starts_with("OAuth ") => token,
            Ok(token) => format!("OAuth {token}"),
            Err(_) => String::new(),
        }
    }

    async fn post_json<T: DeserializeOwned>(&self, path: &str, body: Value) -> Result<T, String> {
        let url = format!("{}/{path}", self.api_base);
        let mut req = self.client.post(&url);
        let auth = self.auth_header();
        if !auth.is_empty() {
            req = req.header("Authorization", auth);
        }
        let response = req
            .header("Content-Type", "application/json")
            .header("Origin", "https://yandex.ru")
            .header("Referer", "https://yandex.ru/chat")
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("POST {path} failed: {e}"))?;

        let status = response.status();
        let text = response
            .text()
            .await
            .map_err(|e| format!("POST {path} read failed: {e}"))?;
        log::debug!("POST {path} → {status}: {}", truncate(&text, 500));
        if !status.is_success() {
            // Try to surface a server UserError before falling back to HTTP.
            if let Ok(value) = serde_json::from_str::<Value>(&text) {
                if let Some(err) = user_error_message(&value) {
                    return Err(format!("{path}: {err}"));
                }
            }
            return Err(format!("{path} failed: HTTP {status}"));
        }
        parse_payload(&text).map_err(|e| format!("{path}: {e}"))
    }

    /// Create a personal meeting (a callable room) for a user.
    pub async fn create_personal_meeting(&self, user_id: &str) -> Result<PersonalMeeting, String> {
        self.post_json(
            "telemost/create_personal_meeting",
            serde_json::json!({ "userId": user_id }),
        )
        .await
    }

    /// Start a call in a meeting; returns join credentials for Goloom.
    pub async fn start_meeting_call(&self, meeting_id: &str) -> Result<MeetingCall, String> {
        self.post_json(
            "telemost/start_meeting_call",
            serde_json::json!({ "meetingId": meeting_id }),
        )
        .await
    }

    /// End a personal meeting.
    pub async fn end_personal_meeting(&self, meeting_id: &str) -> Result<(), String> {
        let url = format!("{}/telemost/end_personal_meeting", self.api_base);
        let mut req = self.client.post(&url);
        let auth = self.auth_header();
        if !auth.is_empty() {
            req = req.header("Authorization", auth);
        }
        let response = req
            .header("Content-Type", "application/json")
            .header("Origin", "https://yandex.ru")
            .header("Referer", "https://yandex.ru/chat")
            .json(&serde_json::json!({ "meetingId": meeting_id }))
            .send()
            .await
            .map_err(|e| format!("end_personal_meeting failed: {e}"))?;
        if !response.status().is_success() {
            return Err(format!(
                "end_personal_meeting failed: HTTP {}",
                response.status()
            ));
        }
        Ok(())
    }

    /// Fetch one meeting's metadata/state.
    pub async fn meeting_info(&self, meeting_id: &str) -> Result<MeetingInfo, String> {
        let url = format!(
            "{}/telemost/meeting_info?meetingId={}",
            self.api_base,
            urlencoding::encode(meeting_id)
        );
        let mut req = self.client.get(&url);
        let auth = self.auth_header();
        if !auth.is_empty() {
            req = req.header("Authorization", auth);
        }
        let response = req
            .header("Origin", "https://yandex.ru")
            .header("Referer", "https://yandex.ru/chat")
            .send()
            .await
            .map_err(|e| format!("meeting_info failed: {e}"))?;
        let status = response.status();
        let text = response
            .text()
            .await
            .map_err(|e| format!("meeting_info read failed: {e}"))?;
        if !status.is_success() {
            return Err(format!("meeting_info failed: HTTP {status}"));
        }
        parse_payload(&text).map_err(|e| format!("meeting_info: {e}"))
    }

    /// Fetch several meetings at once.
    pub async fn meeting_infos(&self, meeting_ids: &[String]) -> Result<Vec<MeetingInfo>, String> {
        let list: Vec<MeetingInfo> = self
            .post_json(
                "telemost/meeting_infos",
                serde_json::json!({ "meetingIds": meeting_ids }),
            )
            .await?;
        Ok(list)
    }

    /// Join URL for "open in browser" fallback / copy-link.
    pub fn join_url(&self, meeting: &PersonalMeeting) -> String {
        if let Some(url) = meeting.join_url.as_deref().filter(|u| !u.is_empty()) {
            return url.to_string();
        }
        format!(
            "{}/j/{}",
            config::TELEMOST_URL.trim_end_matches('/'),
            meeting.meeting_id
        )
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    let mut end = max;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…({} bytes total)", &s[..end], s.len())
}

/// Pull a human-readable message out of a `UserErrors` envelope, if present.
fn user_error_message(value: &Value) -> Option<String> {
    let errors = value
        .get("user_errors")
        .or_else(|| value.get("UserErrors"))
        .or_else(|| value.get("userErrors"))?;
    let first = errors.get("errors").and_then(|e| e.as_array())?.first()?;
    let parsed: Result<MeetingUserError, _> = serde_json::from_value(first.clone());
    match parsed {
        Ok(e) => {
            let code = e.code.unwrap_or_default();
            let msg = e.message.unwrap_or_default();
            let text = format!("{code} {msg}").trim().to_string();
            if text.is_empty() {
                None
            } else {
                Some(text)
            }
        }
        Err(_) => Some(first.to_string()),
    }
}

/// Tolerant success parsing: nested envelopes first, then flat.
///
/// Nested-first because tolerant structs (all-`default` fields) parse
/// *anything* — including a useless empty value — so a direct-first order
/// would shadow the real payload under `success`/`data`/…
fn parse_payload<T: DeserializeOwned>(text: &str) -> Result<T, String> {
    let value: Value = serde_json::from_str(text).map_err(|e| format!("invalid JSON: {e}"))?;
    if let Some(err) = user_error_message(&value) {
        return Err(err);
    }
    for key in ["success", "data", "meeting", "result", "call", "info"] {
        if let Some(nested) = value.get(key) {
            if let Ok(parsed) = serde_json::from_value::<T>(nested.clone()) {
                // Guard against `{"success": true}`-style markers: only take
                // the nested value when it is an object or an array.
                if nested.is_object() || nested.is_array() {
                    return Ok(parsed);
                }
            }
        }
    }
    serde_json::from_value::<T>(value).map_err(|_| "unrecognized response shape".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tiny_http::{Response, Server};

    struct Mock {
        addr: String,
        _handle: std::thread::JoinHandle<Vec<(String, String, String)>>,
        seen: Arc<std::sync::Mutex<Vec<(String, String, String)>>>,
    }

    /// Serve scripted `(path, response_body)` pairs; record method+path+body.
    fn mock_server(routes: Vec<(&'static str, &'static str)>) -> Mock {
        let server = Server::http("127.0.0.1:0").expect("mock bind");
        let addr = match server.server_addr() {
            tiny_http::ListenAddr::IP(addr) => addr.to_string(),
            tiny_http::ListenAddr::Unix(_) => panic!("unexpected unix socket"),
        };
        let seen = Arc::new(std::sync::Mutex::new(Vec::new()));
        let seen_task = seen.clone();
        let routes: Vec<(String, String)> = routes
            .into_iter()
            .map(|(p, b)| (p.to_string(), b.to_string()))
            .collect();
        let handle = std::thread::spawn(move || {
            let want = routes.len();
            for mut req in server.incoming_requests() {
                let mut body = String::new();
                let _ = req.as_reader().read_to_string(&mut body);
                let method = format!("{:?}", req.method()).to_uppercase();
                seen_task
                    .lock()
                    .expect("seen")
                    .push((method, req.url().to_string(), body));
                let path = req.url().split('?').next().unwrap_or("").to_string();
                let payload = routes
                    .iter()
                    .find(|(p, _)| *p == path)
                    .map(|(_, b)| b.clone())
                    .unwrap_or_else(|| {
                        r#"{"user_errors":{"errors":[{"code":"NOT_FOUND"}]}}"#.to_string()
                    });
                let _ = req.respond(Response::from_string(payload));
                if seen_task.lock().expect("seen").len() >= want {
                    break;
                }
            }
            seen_task.lock().expect("seen").clone()
        });
        Mock {
            addr,
            _handle: handle,
            seen,
        }
    }

    fn test_client(mock: &Mock) -> TelemostClient {
        let auth = Arc::new(AuthManager::new().expect("auth manager"));
        TelemostClient::with_base(auth, &format!("http://{}/api", mock.addr))
    }

    fn block_on<F: std::future::Future>(f: F) -> F::Output {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime")
            .block_on(f)
    }

    #[test]
    fn meeting_lifecycle_paths_and_bodies() {
        let mock = mock_server(vec![
            (
                "/api/telemost/create_personal_meeting",
                r#"{"success":{"meetingId":"m1","join_url":"https://telemost.yandex.ru/j/m1"}}"#,
            ),
            (
                "/api/telemost/start_meeting_call",
                r#"{"success":{"roomId":"r1","credentials":"cred","participantId":"p1"}}"#,
            ),
            ("/api/telemost/end_personal_meeting", r#"{}"#),
            (
                "/api/telemost/meeting_info",
                r#"{"meetingId":"m1","title":"Sync","status":"active"}"#,
            ),
        ]);
        let client = test_client(&mock);

        let meeting = block_on(client.create_personal_meeting("u9")).expect("create");
        assert_eq!(meeting.meeting_id, "m1");
        assert_eq!(
            meeting.join_url.as_deref(),
            Some("https://telemost.yandex.ru/j/m1")
        );

        let call = block_on(client.start_meeting_call("m1")).expect("start");
        assert_eq!(call.effective_room_id("m1"), "r1");
        assert_eq!(call.credentials.as_deref(), Some("cred"));

        block_on(client.end_personal_meeting("m1")).expect("end");

        let info = block_on(client.meeting_info("m1")).expect("info");
        assert_eq!(info.title.as_deref(), Some("Sync"));

        let seen = mock.seen.lock().expect("seen").clone();
        let by_path = |p: &str| seen.iter().find(|(_, u, _)| u.starts_with(p)).cloned();
        let (m, _, b) = by_path("/api/telemost/create_personal_meeting").expect("create seen");
        assert_eq!(m, "POST");
        assert!(b.contains(r#""userId":"u9""#), "body: {b}");
        let (m, _, b) = by_path("/api/telemost/start_meeting_call").expect("start seen");
        assert_eq!(m, "POST");
        assert!(b.contains(r#""meetingId":"m1""#), "body: {b}");
        let (m, _, _) = by_path("/api/telemost/meeting_info").expect("info seen");
        assert_eq!(m, "GET");
    }

    #[test]
    fn user_errors_become_err() {
        let mock = mock_server(vec![(
            "/api/telemost/create_personal_meeting",
            r#"{"UserErrors":{"errors":[{"code":"LIMIT","message":"too many"}]}}"#,
        )]);
        let client = test_client(&mock);
        let err = block_on(client.create_personal_meeting("u9")).expect_err("must fail");
        assert!(
            err.contains("LIMIT") && err.contains("too many"),
            "err: {err}"
        );
    }
}
