# Yandex Messenger Native — Development Guide

[Русская версия](DEVELOPMENT.ru.md)

Current release: **2.173.0**.

## 1. Goal

Deliver a production-ready Linux desktop client for Yandex Messenger built with Rust and GTK4, featuring:
- A stable chat core;
- Real-time event streams and file transfers;
- Basic video call integration (Telemost);
- Seamless desktop integration;
- A reproducible build and packaging pipeline.

## 2. Architecture Overview

### Layers

- `UI` — Widgets and user-facing event handlers (`src/ui/*`)
- `Core` — Orchestration and shared application state (`src/core.rs`)
- `API` — OAuth, HTTP REST, and WebSocket clients (`src/api/*`)
- `Models` — Domain data structures (`src/models/mod.rs`)
- `Packaging/CI` — Debian packaging metadata (`debian/*`) and GitHub workflows (`.github/workflows/ci.yml`)

### Main Runtime Flow

1. `main.rs` initializes the GTK application and prompts user authentication.
2. An `AppController` is instantiated alongside an `HttpClient` and a `WebSocketClient`.
3. The chat list is fetched; selecting a chat requests its historical messages.
4. Sending text and attachments is routed through the `AppController`.
5. Initiating a call opens the `TelemostWindow`.

## 3. Architecture Deep Dive

### UI Layer (`src/ui/`)

Each widget is a standalone GTK widget with isolated rendering logic.
`ChatListPanel` displays the list of chats with preview text and unread counters.
`ChatView` renders messages, has an input field, and triggers attachments or calls.
`AuthDialog` manages the OAuth authentication flow using an embedded WebView (under the `in_app_webview` feature flag).
`TelemostWindow` acts as an embedded wrapper for Yandex Telemost.
`settings.rs` handles persistent JSON configuration (dark theme, tray, notifications, reduce animations).
Theme styling is a global CSS provider (`theme.css`) with Telegram Desktop night tokens.
Persistence helpers: `src/core/db.rs` (SQLite), `drafts.rs`, `outbox.rs`, `src/api/session_store.rs`.

### Core Layer (`src/core.rs`)

`AppController` is the single orchestrator of the application. It contains the shared state reference `AppState` (`Arc<Mutex<AppState>>`) shared between the UI and API layers.
All controller methods are asynchronous and run on a Tokio runtime.
`AppState` keeps track of the chat list, the selected chat ID, and message histories.
All state changes are performed within `Arc<Mutex<AppState>>`, and the UI subscribes to updates via callback closures registered in `main.rs`.

### API Layer (`src/api/`)

`AuthManager` is the OAuth2 client handling: authorization URL generation, exchange code for token, token refresh, and disk storage.
`HttpClient` is the REST client built on `reqwest` with TLS support, automated `OAuth` headers, and CSRF token handling.
`WebSocketClient` is the skeleton WS client holding a sequence counter and callback registries for incoming notifications and state events.
The HTTP client supports both flat arrays and wrapped `ListResponse` formats for backward compatibility.

### Models (`src/models/mod.rs`)

Domain types are serialized and deserialized using `serde`. Key data structures include: `Chat`, `Message`, `User`, `WSMessage`, and `WSResponse`.
`MessageType` and `MediaType` are exhaustive enums representing different content types.
`Message` supports replies, forwards, reactions, pins, edit flags, and media entities.

## 4. Coding Standards

- **Async/Await**: All I/O operations must be asynchronous. `block_on` is restricted to the UI thread via `Arc<Runtime>`.
- **Error Handling**: Standard Result types wrapping string-based errors (no custom error enums are used yet).
- **Naming**: Strict Rust naming conventions (`snake_case` for functions/variables, `PascalCase` for types).
- **Imports**: Group imports sequentially: standard library, external crates, internal modules (`crate::`).
- **Config**: Avoid hardcoding parameters; place configuration variables in `config.rs`.
- **Arc<Mutex<T>>**: Shared state must use Arc + Mutex. Clone the Arc when passing it to closures.
- **Comments**: Write doc comments for public methods and inline comments for non-trivial logic.

## 5. Testing

### Unit Tests
```bash
cargo test
```
Current coverage includes checking configuration constants (`src/api/mod.rs::tests`).

### Integration Tests (Manual)
1. Run `cargo run --release` → verify OAuth login.
2. Send a text message → check UI rendering.
3. Upload an attachment → check upload/download status.
4. Toggle light/dark themes → check style updates.
5. Close/restore window → check tray functionality.

### CI Pipeline
Managed by GitHub Actions (`/.github/workflows/ci.yml`):
- `cargo fmt --check`
- `cargo clippy`
- `cargo test`
- `cargo build --release`

### Future Test Priorities
- Mock HTTP responses for testing the API layer.
- Integration tests for WebSocket subscriptions.
- OAuth flow simulation using a mock server.

## 6. Deployment

### Local Building
```bash
make build        # cargo build --release
make run          # cargo run --release
make dist         # generate release artifacts & deb package
make icons        # rebuild icon set
```

### Debian Packaging
```bash
debuild -us -uc -b
```
Requires `debhelper`, `dh-sequence-gtk4`, and `dh-cargo`.

### Docker (CI Environment)
```bash
docker build -t yandex-messenger -f Dockerfile .
```

## 7. Implementation Status by Phase

### Phase 1 — Foundations
- [x] Basic project layout structure
- [x] AuthManager & token filesystem persistence
- [x] Core models (Chat, Message, User)
- [x] HTTP and WS client hulls

### Phase 2 — Basic UI
- [x] ChatView widget
- [x] MessageList widget
- [x] MessageInput widget
- [x] Chat switching
- [x] Basic styles

### Phase 3 — Real-Time Stream
- [x] Subscribe and unsubscribe hooks
- [x] Text message sending
- [x] Contracts for incoming events (typing, read/unread status)
- [x] Status updates in UI/Core layers

### Phase 4 — File Sharing
- [x] Upload flow (API + controller)
- [x] Download flow (API + controller)
- [x] Attachment action in the input panel
- [x] Basic file preview hooks

### Phase 5 — Video Calls (Telemost)
- [x] Call window (`src/ui/telemost.rs`)
- [x] Video/audio controls (mute, camera toggle, disconnect)
- [x] Launching calls from the chat view

### Phase 6 — Polish & UX
- [x] System tray & minimize-to-tray
- [x] Desktop notifications (`notify-rust`)
- [x] Persistent JSON settings
- [x] Keyboard shortcuts
- [x] Dark theme settings
- [x] Desktop launcher entry (.desktop)

### Phase 7 — Packaging & CI
- [x] Debian package metadata and build rules
- [x] GitHub CI workflow
- [x] MIT License file
- [x] Man page documentation
- [x] PPA release notes

## 8. Current Sprint Tasks

### In Progress
- [ ] Implement complete WebSocket transport loop (receive loop, heartbeat/ping-pong)
- [ ] Handle read statuses and participant online states
- [ ] Unit tests for the API layer with mock responses
- [ ] In-chat message search
- [ ] Network error retry policies
- [ ] Text formatting (bold, italic, links) rendering in MessageList

### Documented
- [x] ARCHITECTURE.md
- [x] SECURITY.md
- [x] Updated README.md
- [x] Updated DEVELOPMENT.md

## 9. Current Limitations

- The WebSocket layer is currently implemented as a safe contract scaffold rather than a full-featured bidirectional event loop.
- Yandex Telemost integration is limited to window wrapping and call event scaffolding (no embedded browser execution).
- Final production releases require running checks (`cargo clippy`/`cargo test`) on environments with a configured Rust toolchain.

## 10. Acceptance Checklist

A release is ready when the following conditions are met:
- [x] Implementation covers all items in Phases 2–7.
- [x] Documentation is synchronized with codebase modifications.
- [x] Release artifacts and packaging files are present.
- [x] CI workflow is configured and active.
- [ ] Smoke tests pass successfully on target systems.
- [ ] Deb package builds on clean Debian/Ubuntu runner instances.

## 11. Next Technical Steps

1. Run `make check && make test && make build` locally.
2. Test package building via `debuild -us -uc`.
3. Transition the WebSocket client into a fully bidirectional event processor.
