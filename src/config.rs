#![allow(dead_code)]

/// Configuration module - API endpoints reverse-engineered from Yandex Messenger Electron app

// Default OAuth application id (32-char hex). Package name `ru.yandex.yamb` is NOT valid.
// Override with env YANDEX_CLIENT_ID if you registered your own app at https://oauth.yandex.ru/.
// Without a valid id the WebView falls back to Passport session + optional token paste.
pub const OAUTH_CLIENT_ID: &str = "bef24ec2889b481bb39af0b430099845";
// Prefer .ru host for Russian accounts (avoids some "unknown client" edge cases on .com).
pub const OAUTH_AUTHORIZE_URL: &str = "https://oauth.yandex.ru/authorize";
pub const OAUTH_TOKEN_URL: &str = "https://oauth.yandex.ru/token";
pub const PASSPORT_HOST: &str = "passport.yandex.ru";
pub const PASSPORT_AUTH_URL: &str = "https://passport.yandex.ru/auth";
pub const PASSPORT_PROFILE_URL: &str = "https://passport.yandex.ru/profile";
/// Where to land after Passport login to harvest Session_id cookies.
pub const CHAT_WEB_URL: &str = "https://yandex.ru/chat";

// Primary API base - will be resolved to actual hostname
pub const API_BASE_TEMPLATE: &str = "https://yandex.{tld}/messenger/api/registry/api/";
pub const UNIPROXY_API_KEY: &str = "developers-simple-key";

// For .ru domain
pub const API_BASE_URL: &str = "https://yandex.ru/messenger/api/registry/api/";
pub const UNIPROXY_URL: &str = "wss://uniproxy.messenger.yandex.ru/uni.ws";

// File hosting
pub const FILE_PUBLIC_HOST: &str = "https://files.messenger.yandex.net";
pub const FILE_PRIVATE_HOST: &str = "https://files.messenger.yandex.ru";

// Telemost
pub const TELEMOST_URL: &str = "https://telemost.yandex.ru";
pub const TELEMOST_CLOUD_API: &str = "https://api.messenger.yandex.net";
pub const GOLOOM_WS_URL: &str = "wss://goloom.strm.yandex.net/join";
pub const TELEMOST_API_PATH: &str = "/v1/telemost";

// OAuth
// Desktop app uses implicit flow (response_type=token) — redirect_uri is
// optional for this flow. We leave it empty so Yandex OAuth falls back to its
// registered default callback for the app.
pub const REDIRECT_URI: &str = "";
pub const OAUTH_SCOPES: &str = "";

// App info
pub const APP_ID: &str = "ru.yandex.yamb";
pub const APP_NAME: &str = "Yandex Messenger";
pub const APP_PROTOCOL: &str = "ychat";
pub const PRODUCT_NAME: &str = "Chats";
pub const COMPANY_NAME: &str = "Yandex";

// API version
pub const API_VERSION: u32 = 5;
pub const IDB_PREFIX: &str = "mssngr";

// Message limits
pub const MAX_MESSAGE_LENGTH: usize = 4096;
pub const MAX_FILE_SIZE: u64 = 52_428_800; // 50MB
pub const HISTORY_CHUNK_SIZE: usize = 50;

// WebSocket config
pub const WS_RECONNECT_INTERVAL: u64 = 11; // seconds
pub const WS_HEARTBEAT_INTERVAL: u64 = 30; // seconds
pub const WS_MAX_RECONNECT_ATTEMPTS: u32 = 10;

// Chat limits
pub const MAX_CHAT_TITLE_LENGTH: usize = 250;
pub const MAX_MEMBERS_COUNT: u32 = 1000;
pub const MAX_MEMBERS_ORG_COUNT: u32 = 10000;
pub const MAX_FILE_UPLOAD_COUNT: usize = 30;

// Typing & presence
pub const TYPING_INTERVAL: u64 = 3; // seconds
pub const PRESENCE_PING_INTERVAL: u64 = 30; // seconds

// Push notification
pub const PUSH_SERVICE: &str = "push.yandex.ru";

// Backend config
pub const TOOLS_HOST: &str = "tools.messenger.yandex.net";
pub const BACKEND_CONFIG_URL: &str = "https://tools.messenger.yandex.net/config.json";
pub const BACKEND_CONFIG_INTERVAL: u64 = 600; // seconds

// Feedback
pub const FEEDBACK_URL: &str = "https://yandex.ru/support/messenger/feedback.html";
pub const SUPPORT_URL: &str = "https://yandex.ru/support/messenger";
pub const LICENSE_URL: &str = "https://yandex.ru/legal/messenger_termsofuse/";

// Crypto
pub const AES_KEY_LENGTH: usize = 32;
pub const IV_LENGTH: usize = 12;
pub const HMAC_KEY_LENGTH: usize = 32;

// Desktop
pub const DESKTOP_APP_ID: &str = "ru.yandex.yamb";
pub const DESKTOP_PROTOCOL: &str = "ychat://";

// Voice message configuration
pub const MAX_VOICE_DURATION: u32 = 600; // 10 минут максимум
pub const VOICE_SAMPLE_RATE: u32 = 16000; // Hz для записи (16kHz для Opus)
pub const VOICE_BITRATE: u32 = 64000; // kbps для Opus кодирования
pub const VOICE_MAX_FILE_SIZE: u64 = 5_242_880; // ~5MB макс для голосового сообщения

// Yandex SpeechKit for voice transcription
pub const SPEECHKIT_API_URL: &str = "https://api.speechkit.yandex.net/v1/stt";
pub const SPEECHKIT_LANG: &str = "ru-RU";
pub const SPEECHKIT_ENCODING: &str = "WEBM_OPUS";

pub fn ym_enable_voice() -> bool {
    std::env::var("YM_ENABLE_VOICE")
        .map(|s| s == "1" || s.to_lowercase() == "true")
        .unwrap_or(false)
}

pub fn ym_enable_telemost_ui() -> bool {
    std::env::var("YM_ENABLE_TELEMOST_UI")
        .map(|s| s == "1" || s.to_lowercase() == "true")
        .unwrap_or(false)
}
