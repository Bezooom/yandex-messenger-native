# Yandex Messenger Native — Architecture

## Overview

Native Linux desktop client for Yandex Messenger built with Rust and GTK4.
The application follows a layered architecture with clear separation between UI,
business logic, and API communication.

```
┌─────────────────────────────────────────────────────────────────┐
│                        User (Desktop)                          │
└────────────────────────────┬────────────────────────────────────┘
                             │ GTK4 Events / System Calls
┌────────────────────────────▼────────────────────────────────────┐
│                    UI Layer (src/ui/*)                          │
│  AuthDialog  │  ChatListPanel  │  ChatView  │  TelemostWindow  │
│  TrayHandle  │  Notifications  │  Settings  │  Theme (CSS)     │
└────────────────────────────┬────────────────────────────────────┘
                             │ Callbacks / Shared State
┌────────────────────────────▼────────────────────────────────────┐
│                  Core Layer (src/core.rs)                       │
│  AppController  │  AppState (Arc<Mutex>)  │  AppEvent enum     │
└────────────────────────────┬────────────────────────────────────┘
                             │ Async API Calls
┌────────────────────────────▼────────────────────────────────────┐
│                   API Layer (src/api/*)                         │
│  AuthManager (OAuth2)  │  HttpClient (reqwest)  │  WebSocketClient │
└────────────────────────────┬────────────────────────────────────┘
                             │ HTTP / WS / Disk
┌────────────────────────────▼────────────────────────────────────┐
│              External: Yandex Messenger API (REST + WS)         │
│  OAuth endpoints  │  Messenger API  │  Uniproxy WS  │  Files   │
└─────────────────────────────────────────────────────────────────┘
```

## Layer Descriptions

### UI Layer

The UI layer consists of GTK4 standalone widgets, each managing its own rendering
and user interaction. Widgets communicate with the core through callback closures
and shared state references.

- **AuthDialog** (`auth_dialog.rs`): OAuth login dialog. Supports external browser
  flow and in-app WebView (feature `in_app_webview`). Parses token from URL fragment.
- **ChatListPanel** (`chat_list.rs`): Sidebar displaying chats with title, avatar,
  last message preview, unread count, and pin/archive indicators. Emits `chat_selected`
  signal.
- **ChatView** (`chat_view.rs`): Main message area with MessageList (virtualized),
  MessageInput (text + attachments), and call button. Handles send, reply, forward,
  and reactions display.
- **TelemostWindow** (`telemost.rs`): Window wrapper for Yandex Telemost video calls.
  Supports mute, video toggle, and end call controls.
- **TrayHandle** (`tray.rs`): System tray icon with menu (open, settings, quit).
- **Notifications** (`notifications.rs`): Desktop notifications via `notify-rust`.
- **Settings** (`settings.rs`): JSON-based persistent settings (dark theme, minimize
  to tray). Stored in `~/.config/yandex-messenger-native/settings.json`.
- **Theme** (`theme.css`): GTK CSS provider for global styling. Supports dark mode
  via `gtk::Settings::gtk_application_prefer_dark_theme`.

### Core Layer

`AppController` (`src/core.rs`) is the central orchestrator. It owns:

- `AuthManager` — OAuth token lifecycle
- `HttpClient` — REST API communication
- `WebSocketClient` — Real-time communication
- `AppState` — Shared mutable application state

`AppState` holds:
```rust
pub struct AppState {
    pub chats: Vec<Chat>,                    // Cached chat list
    pub selected_chat_id: Option<String>,    // Currently active chat
    pub messages_by_chat: HashMap<String, Vec<Message>>, // Message history per chat
}
```

The controller exposes async methods for all operations. UI code calls these via
`tokio::Runtime::block_on()` in the GTK main context.

### API Layer

#### AuthManager (`src/api/auth.rs`)

Handles the complete OAuth2 Authorization Code Flow:

1. Generates auth URL with `response_type=code`, `client_id`, `state`, `device_id`
2. User authenticates in browser, redirected with `#access_token=...`
3. Token parsed from URL fragment and saved to `~/.config/yandex-messenger-native/token.json`
4. On startup, loads token from disk; checks expiry (300s buffer)
5. If expired, uses `refresh_token` to obtain new access_token
6. Supports auth-proxy mode for centralized OAuth

Token storage uses file locking (via `fs::write`) and stores JSON with:
- `access_token`, `refresh_token`, `expires_in`, `token_type`, `user_id`

#### HttpClient (`src/api/mod.rs`)

REST client built on `reqwest` with `rustls-tls`:

- **Authentication**: `OAuth <token>` header on all requests
- **CSRF**: Fetches CSRF token from `/csrf-token/` before mutations
- **Chat list**: `GET /api/get_chat_list?offset=&limit=`
- **History**: `GET /api/get_history?chatId=&offset=&limit=`
- **Send message**: `POST /api/send_text` with JSON body
- **Upload**: `PUT /media_upload/<chatId>/<filename>?<uuid>` to Yandex Files
- **Download**: `GET /file_shortterm/<fileId>` from Yandex Files
- **User profile**: `GET /api/get_profile`
- **Telemost URL**: Generates `https://telemost.yandex.ru/?chatId=<id>`

Response parsing handles both flat arrays and wrapped responses (`ListResponse<T>`
with `items`/`chats`/`messages` fields).

#### WebSocketClient (`src/api/mod.rs`)

WebSocket client for Yandex Uniproxy (`wss://uniproxy.messenger.yandex.ru/uni.ws`):

- **Sequence counter**: Each message gets a monotonically increasing `seq`
- **Message format**: `{"seq": N, "message": {"method": "...", "params": {...}}}`
- **Methods**: `subscribe`, `unsubscribe`, `bootstrap`
- **Callbacks**: `on_message()` for incoming WS messages, `on_state_change()` for
  connection state events (Disconnected → Connecting → Connected → Reconnecting)
- **Current state**: Stub connect/send (full transport loop pending)

### Models (`src/models/mod.rs`)

Domain types with `serde` serialization:

- **Chat**: id, title, type (Private/Group/Channel/Bot), participants, unread count,
  last message, pinned/archived/muted flags
- **Message**: id, chat_id, from_id, type, text, entities, reply_to, forward, media,
  reactions, read/delivered/sent status, edit tracking
- **User**: id, phone, email, display_name, username, avatar_id, status, bot/premium flags
- **WSMessage/WSResponse**: WebSocket protocol envelope with seq, result, error
- **MediaAttachment**: file metadata with thumbnail, dimensions, size, duration
- **TelemostCall**: call lifecycle with participants and status

## Data Flow Diagrams

### Chat Selection Flow

```
User clicks chat in ChatListPanel
        │
        ▼
ChatListPanel emits chat_selected signal
        │
        ▼
main.rs callback: controller.select_chat(chat_id)
        │
        ├──▶ WebSocketClient.subscribe(chat_id)
        │        │
        │        ▼
        │   Send WS {"method":"subscribe","params":{"chatId":"..."}}
        │        │
        │        ▼
        │   Wait for WS incoming messages (pending)
        │
        └──▶ HttpClient.get_messages(chat_id, limit=50)
                 │
                 ▼
            GET /api/get_history?chatId=...&offset=0&limit=50
                 │
                 ▼
            Parse response → Vec<Message>
                 │
                 ▼
            Update AppState.messages_by_chat[chat_id]
                 │
                 ▼
            ChatView.set_messages(messages)
                 │
                 ▼
            Render message list in UI
```

### Message Send Flow

```
User types text + clicks send
        │
        ▼
ChatView emits send_message(chat_id, text)
        │
        ▼
main.rs callback: controller.send_text_message(chat_id, text)
        │
        ▼
HttpClient.send_message(chat_id, text)
        │
        ▼
POST /api/send_text {"chatId":"...","text":"..."}
        │
        ▼
Parse response → Message
        │
        ▼
Update AppState.messages_by_chat[chat_id].push(message)
        │
        ▼
ChatView.append_message(message)
        │
        ▼
Render new message at bottom of list
        │
        ▼
Send desktop notification
```

### OAuth Flow

```
App starts
    │
    ▼
AuthManager.load_token()
    │
    ├── Token exists AND not expired ──▶ Continue to main UI
    │
    └── No token OR expired
            │
            ▼
    AuthDialog.show()
            │
            ▼
    Open browser → Yandex OAuth authorize page
            │
            ▼
    User enters credentials + confirms
            │
            ▼
    Redirect to callback with #access_token=...
            │
            ▼
    AuthDialog.parse_token_from_url()
            │
            ▼
    AuthManager.save_token(token)
            │
            ▼
    AppController.new(auth, token)
            │
            ▼
    AppController.connect_realtime()
            │
            ▼
    WebSocketClient.connect() → WSState.Connected
            │
            ▼
    AppController.load_chats()
            │
            ▼
    Main UI rendered
```

### Token Refresh Flow

```
AccessToken expires (expires_in <= 300)
    │
    ▼
AuthManager.refresh_token(refresh_token)
    │
    ▼
POST /token {grant_type: "refresh_token", client_id, refresh_token}
    │
    ├── Success ──▶ New token saved to disk
    │                  │
    │                  ▼
    │             Update AppState in memory
    │
    └── Failure ──▶ AuthDialog shown again
                     │
                     ▼
                  User re-authenticates
```

### File Upload Flow

```
User attaches file in ChatView
    │
    ▼
ChatView emits upload_file(chat_id, bytes, filename)
    │
    ▼
AppController.upload_file(chat_id, bytes, filename)
    │
    ▼
HttpClient.upload_file()
    │
    ▼
PUT https://files.messenger.yandex.net/media_upload/<chatId>/<filename>?<uuid>
    Headers: Authorization: OAuth <token>
    Body: file bytes
    │
    ▼
Parse response → fileId
    │
    ▼
Send message with file attachment via send_message()
    │
    ▼
Desktop notification: "File uploaded"
```

## State Management

The application uses a single shared state pattern:

```
AppController (owns)
    └── Arc<Mutex<AppState>>
            ├── chats: Vec<Chat>
            ├── selected_chat_id: Option<String>
            └── messages_by_chat: HashMap<String, Vec<Message>>
```

UI components receive `Arc<Mutex<AppState>>` clones and lock for reads.
Mutations happen in `AppController` async methods, then UI is updated via
callback closures defined in `main.rs`.

This approach avoids complex state management libraries while providing
thread-safe access through Tokio's `Mutex`.

## WebSocket Protocol

The WebSocket client communicates with Yandex Uniproxy using a sequence-based
protocol:

### Outgoing Message Format
```json
{
  "seq": 0,
  "message": {
    "method": "subscribe",
    "params": {"chatId": "chat-uuid"}
  }
}
```

### Supported Methods
- `subscribe` — Start receiving messages for a chat
- `unsubscribe` — Stop receiving messages for a chat
- `bootstrap` — Request initial state (flags: with_deleted, compact)

### Incoming Message Format
```json
{
  "seq": 0,
  "result": {...},
  "error": null
}
```

Or for event notifications:
```json
{
  "seq": 1,
  "message": {
    "method": "message",
    "params": {"chatId": "...", "message": {...}}
  }
}
```

### Connection States
```
Disconnected → Connecting → Connected → Reconnecting(0, 1, 2, ...)
```

Max reconnect attempts: 10 (from config `WS_MAX_RECONNECT_ATTEMPTS`).

## Configuration Constants (`src/config.rs`)

| Constant | Value | Purpose |
|---|---|---|
| `OAUTH_CLIENT_ID` | `bef24ec2889b481bb39af0b430099845` | Yandex OAuth client ID |
| `API_BASE_URL` | `https://yandex.ru/messenger/api/registry/api/` | REST API base |
| `UNIPROXY_URL` | `wss://uniproxy.messenger.yandex.ru/uni.ws` | WebSocket endpoint |
| `FILE_PUBLIC_HOST` | `https://files.messenger.yandex.net` | File upload/download |
| `TELEMOST_URL` | `https://telemost.yandex.ru` | Video calls |
| `MAX_MESSAGE_LENGTH` | 4096 | Max message text length |
| `MAX_FILE_SIZE` | 50MB | Max file upload size |
| `HISTORY_CHUNK_SIZE` | 50 | Messages per history page |
| `WS_HEARTBEAT_INTERVAL` | 30s | WebSocket heartbeat |
| `MAX_MEMBERS_COUNT` | 1000 | Group member limit |

## Component Interaction Summary

| Component | Depends On | Provides To |
|---|---|---|
| `main.rs` | All layers | Entry point, wiring |
| `AppController` | AuthManager, HttpClient, WebSocketClient | AppState, API methods |
| `ChatListPanel` | AppController (via callbacks) | chat_selected signal |
| `ChatView` | AppController (via callbacks) | send_message, upload_file, start_call |
| `AuthDialog` | AuthManager | OAuth token |
| `WebSocketClient` | AuthManager | WS send/subscribe/callbacks |
| `HttpClient` | None (uses reqwest) | REST API methods |
| `SettingsStore` | filesystem (dirs crate) | Settings struct |
