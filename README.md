# Yandex Messenger Native

A native Linux desktop client for Yandex Messenger built with Rust, GTK4, and Libadwaita. The project offers high performance, low resource consumption, and a premium Obsidian-style dark theme.

> ⚠️ **Project Status: Active Development / Unfinished**
> This is an open-source project that is currently unfinished and under active development. While the core backend API integrations and messaging components are functional, the user interface (UI) requires significant polishing, refinement, and bug fixing. 
> 
> We would be absolutely thrilled to have your help! If you are interested in Rust, GTK4, Libadwaita, or reverse-engineering APIs, we warmly welcome any contributions, bug reports, and pull requests. Check out our [Contributing Guidelines](CONTRIBUTING.md) to get started!
> 
> **Important Disclaimer**: This is an **unofficial, community-driven client** for Yandex Messenger. It is not affiliated with, endorsed by, or associated in any way with Yandex LLC or its affiliates. The developers of this project do not claim any rights to Yandex trademarks, branding, or media assets. Use this software at your own risk.

## Features

### Authorization & Security
* **OAuth2 Protocol**: Supports Authorization Code and Implicit Grant flows with a local loopback callback server.
* **Non-blocking UI**: Interactive login dialog featuring a loading spinner (`Spinner`) and a 180-second timeout.
* **Secure Storage**: Tokens are stored at `~/.config/yandex-messenger-native/token.json` with `0600` file permissions.
* **Auto-refresh**: Automatic access token refresh (via refresh token) triggered 5 minutes before expiration.
* **Manual Entry**: Option to manually enter an Access Token if needed.
* **Network & Proxy**: Encryption handled via `rustls` (no external OpenSSL dependency) and support for enterprise auth proxy mode via the `YANDEX_AUTH_PROXY_URL` environment variable.

### User Interface (UI)
* **Adaptive Layout**: Uses `gtk::Paned` as a separator supporting a compact sidebar mode (collapses text when width is < 180px).
* **Chat Lists**: Sorted chat entries (pinned chats remain at the top, others are sorted by the latest message time), pin indicators 📌, and avatars rendered using `adw::Avatar`.
* **Performance**: Asynchronous caching of avatar textures in RAM (`AVATAR_CACHE`) to prevent flickering/re-downloads, instant L2 JSON cache-based history loading (`load_cache_l2_async`) upon selecting a chat, and background network updates.
* **Premium Dark Theme**: Dark Obsidian-like styling with smooth transitions, custom thin scrollbars (expanding on hover), and blur effects.

### Messaging & Interactivity
* **Input Editor**: Multi-line input field (`TextView`) with automatic height adjustment (up to 120px). Messages are sent by pressing `Enter`, while a new line is inserted with `Shift+Enter`.
* **Quick Edit**: Pressing the `Up` arrow key when the input is empty instantly opens edit mode for the last sent message.
* **Date Separators**: Logical chat message grouping by days ("Today", "Yesterday", and formatted dates).
* **Special Formats**: Rendering of sticker images (128x128px) and "emoji-only" mode (large 40px emoji size without a chat bubble when sending 1 to 3 emojis).
* **Context Menu**: Actions available on right-click (reply, copy, delete, edit).
* **Attachments Menu**: A unified attachment popover (file picker, poll creator, message scheduling).
* **Media Viewer**: Full-screen image gallery with swipe gesture navigation and downloads to the `Downloads` directory.
* **Voice Messages**: Audio recording and waveform visualization using an optional GStreamer pipeline.
* **Global Search**: Quick search overlay triggered with `Ctrl+K` for global message and contact filtering.
* **Advanced Entities**: Support for bots (inline buttons, custom keyboards), scheduled messages, "Saved Messages" section (favorites), group chats, and channels.

---

## System Requirements

To build and run the application on Linux (Ubuntu/Debian), you need the following system development libraries:

```bash
sudo apt update
sudo apt install -y \
  build-essential pkg-config cargo \
  libgtk-4-dev libadwaita-1-dev libsqlite3-dev \
  libssl-dev libnotify-dev libwebkitgtk-6.0-dev
```

*Note: For voice recording support, it is recommended to install GStreamer packages (`libgstreamer1.0-dev`, `libgstreamer-plugins-base1.0-dev`).*

---

## Building and Installation

### 1. Compilation
Build the project in release mode:

```bash
cargo build --release
```

The compiled binary will be placed at `target/release/yandex-messenger`.

### 2. Makefile Actions
Build and run the project in a single step:

```bash
make build
make run
```

### 3. System Installation
To install the binary, icons, and desktop entries into system directories:

```bash
sudo make install
```

To build a local `.deb` package for Debian/Ubuntu distributions:

```bash
make dist
sudo apt install -y ./dist/yandex-messenger-native_*_amd64.deb
```

---

## Environment Variables

The client can be configured using the following environment variables:

| Variable | Description | Default Value |
|---|---|---|
| `YANDEX_CLIENT_ID` | Application OAuth client ID | `bef24ec2889b481bb39af0b430099845` |
| `YANDEX_CLIENT_SECRET` | Application OAuth client secret | — |
| `YANDEX_REDIRECT_URI` | Redirect URI for the callback server | Auto-detected (local port) |
| `YANDEX_AUTH_PROXY_URL` | URL of the external authorization proxy | — |
| `YANDEX_OAUTH_AUTHORIZE_URL`| Yandex OAuth authorization endpoint | `https://oauth.yandex.com/authorize` |
| `YANDEX_OAUTH_TOKEN_URL` | Yandex OAuth token exchange endpoint | `https://oauth.yandex.com/token` |
| `RUST_LOG` | Logging level for the application | `info` |

---

## Development & Documentation

* **Architecture Specification**: [ARCHITECTURE.md](ARCHITECTURE.md) ([Russian version](ARCHITECTURE.ru.md))
* **Development Guide**: [DEVELOPMENT.md](DEVELOPMENT.md) ([Russian version](DEVELOPMENT.ru.md))
* **Security Auditing**: [SECURITY.md](SECURITY.md)
* **API Specification**: [API.md](API.md) ([Russian version](API.ru.md))
* **Changelog**: [CHANGELOG.md](CHANGELOG.md) ([Russian version](CHANGELOG.ru.md))
* **Roadmap**: [ROADMAP.md](ROADMAP.md) ([Russian version](ROADMAP.ru.md))

## License

This project is licensed under the MIT License. See [LICENSE](LICENSE) for details.
