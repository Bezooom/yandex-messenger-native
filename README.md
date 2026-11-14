<a id="readme-top"></a>
<!-- best-readme-template -->
<div align="center">

  <a href="https://github.com/Bezooom/yandex-messenger-native">
    <img src="assets/icon.png" alt="Logo" width="120" height="120">
  </a>
<h1 align="center">Yandex Messenger Native</h1>

<p align="center">
  Unofficial lightweight native Linux desktop client for Yandex Messenger — Rust, GTK4, Libadwaita.
  <br />
  <br />
  <a href="https://github.com/Bezooom/yandex-messenger-native/tree/HEAD/docs"><strong>Docs »</strong></a> · <a href="https://github.com/Bezooom/yandex-messenger-native/issues/new?labels=bug">Report Bug</a> · <a href="https://github.com/Bezooom/yandex-messenger-native/issues/new?labels=enhancement">Request Feature</a> · <a href="README.ru.md">Русская версия</a>
</p>

<p align="center">
  <a href="https://github.com/Bezooom/yandex-messenger-native/actions/workflows/ci.yml"><img src="https://img.shields.io/github/actions/workflow/status/Bezooom/yandex-messenger-native/ci.yml?style=for-the-badge&label=CI&logo=githubactions&logoColor=white" alt="CI" /></a>
  <a href="https://github.com/Bezooom/yandex-messenger-native/releases/latest"><img src="https://img.shields.io/github/v/release/Bezooom/yandex-messenger-native?style=for-the-badge" alt="Release" /></a>
  <a href="https://github.com/Bezooom/yandex-messenger-native/blob/HEAD/LICENSE"><img src="https://img.shields.io/badge/License-MIT-blue?style=for-the-badge" alt="License" /></a>
  <a href="https://github.com/Bezooom/yandex-messenger-native/stargazers"><img src="https://img.shields.io/github/stars/Bezooom/yandex-messenger-native?style=for-the-badge" alt="Stars" /></a>
  <a href="https://github.com/Bezooom/yandex-messenger-native/commits"><img src="https://img.shields.io/github/last-commit/Bezooom/yandex-messenger-native?style=for-the-badge" alt="Last commit" /></a>
  <a href="https://github.com/Bezooom/yandex-messenger-native/issues"><img src="https://img.shields.io/github/issues/Bezooom/yandex-messenger-native?style=for-the-badge" alt="Issues" /></a>
</p>

</div>

<details open>
<summary><b>📑 Table of Contents</b></summary>

- [📖 About The Project](#-about-the-project)
- [Feature Status Matrix](#feature-status-matrix)
- [Features](#features)
- [System Requirements](#system-requirements)
- [🧱 Built With](#-built-with)
- [Building and Installation](#building-and-installation)
- [Environment Variables](#environment-variables)
- [Development & Documentation](#development--documentation)
- [🗺️ Roadmap](#-roadmap)
- [🤝 Contributing](#-contributing)
- [License](#license)
- [📬 Contact](#-contact)
- [🙏 Acknowledgments](#-acknowledgments)

</details>

## 📖 About The Project


[![CI](https://github.com/Bezooom/yandex-messenger-native/actions/workflows/ci.yml/badge.svg)](https://github.com/Bezooom/yandex-messenger-native/actions/workflows/ci.yml) [![Release](https://img.shields.io/github/v/release/Bezooom/yandex-messenger-native)](https://github.com/Bezooom/yandex-messenger-native/releases/latest) [![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

**Version 2.175.1** — native Linux desktop client for Yandex Messenger (Telemost chats), built with Rust, GTK4, and Libadwaita. Fast, light, with a Telegram Desktop night theme and an nheko-style dense chat list.

[Русская версия](README.ru.md)

> ⚠️ **Project Status: Active Development / Unfinished**
> This is an open-source project that is currently unfinished and under active development. While the core backend API integrations and messaging components are functional, the user interface (UI) requires significant polishing, refinement, and bug fixing. 
> 
> We would be absolutely thrilled to have your help! If you are interested in Rust, GTK4, Libadwaita, or reverse-engineering APIs, we warmly welcome any contributions, bug reports, and pull requests. Check out our [Contributing Guidelines](CONTRIBUTING.md) to get started!
> 
> **Important Disclaimer**: This is an **unofficial, community-driven client** for Yandex Messenger. It is not affiliated with, endorsed by, or associated in any way with Yandex LLC or its affiliates. The developers of this project do not claim any rights to Yandex trademarks, branding, or media assets. Use this software at your own risk.

> ✅ **Verified on this tree (2.175.0):** `cargo check --bin yandex-messenger` clean; `cargo test --bin yandex-messenger` → **62 passed, 1 ignored**; `cargo build --release` → 16 MB binary with every `NEEDED` library resolved. GUI and live network flows are manual.

### What changed in 2.175.0

- **Chat list from real bootstrap**: `members_count`, `private`, `pinned_chats` and `chat_mutings` are parsed; groups no longer say "0 участников", and `chats[].members` fills the participant list with names and avatars.
- **Stable ordering**: preview timestamps come from `create_timestamp` / `version` instead of `Utc::now()`.
- **Unread has one source of truth**: incoming messages bump `state.chats[].unread_count`; window title, tray and list agree, and your own echo is ignored.
- **Telemost home**: action hub (new chat / new call / join by link), Все / Личные / Новые filters with matching empty states, window title "Яндекс Телемост — N новое сообщение".
- **Privacy**: the full bootstrap dump is opt-in via `YM_DUMP_BOOTSTRAP=/path`.

### What changed in 2.175.1

- **Emoji and stickers are a right-hand dock** with «Эмодзи / Стикеры» tabs, search and category grid, Telegram-style, instead of a popover above the composer. `Ctrl+Shift+E` toggles it.
- **Image viewer fixed**: `gtk::Picture` with `ContentFit::Contain` fills the window instead of showing a thumbnail; zoom buttons now scale the picture.
- **Default window** is 1180×780 so the list, chat and dock fit together.

## Feature Status Matrix

| Feature | Status | Description |
|---|---|---|
| OAuth / Login | Beta | OAuth + **in-app session cookie capture** (no Python script) |
| Chat List | Beta | nheko-style dense list, real member counts and pin/mute from bootstrap, Все / Личные / Новые filters |
| Text Messaging | Beta | Send + reply/edit + outbox + drafts + history pagination + **delivery/read ticks** |
| Files & Attachments | Beta | Upload→send; **Download / Open**; **DnD** files; **Ctrl+V** images |
| Voice Messages | Not in default build | Needs `--features gstreamer` and `YM_ENABLE_VOICE`; the default and CI binaries do not compile it |
| Video Playback | Not in default build | Inline player requires the `gstreamer` feature; absent from the default and CI binaries |
| Calls (Telemost) | Partial | Goloom signaling + `webrtcbin` in tree; in-call media needs `gstreamer`, `gstreamer1.0-nice` and `YM_ENABLE_TELEMOST_UI`; default binary has no WebRTC |
| Desktop Notifications | Beta | `notify-rust`, respects mute + settings |
| System Tray | Beta | StatusNotifierItem (`ksni`), close-to-tray, unread badge |
| Settings | Beta | Notifications / tray / dark theme / reduce animations |
| Offline Cache | Beta | SQLite (`cache.db`) upsert + JSON L2; cold-start hydrate |
| Theme | Beta | Light Yandex theme by default + dark night, switch in settings |

### Feature Flags

Two new environment variables control whether stub UI elements are visible:

| Flag | Default | Description |
|---|---|---|
| `YM_ENABLE_VOICE` | `false` | Show voice messages UI (waveform, play/record buttons) |
| `YM_ENABLE_TELEMOST_UI` | `false` | Show Telemost (video calls) UI elements in chat header |

When a flag is off, the corresponding stub elements are hidden from the interface so users see a cleaner UI until the feature is fully implemented.

### Known Limitations

- **Default build**: voice, video and call media are not compiled at all; they need the `gstreamer` feature
- **Session cookies**: captured on WebView login; `scripts/login_browser.py` remains as fallback
- **Voice**: record/play behind `YM_ENABLE_VOICE` (needs a `gstreamer`-feature build; transcription display only, no fetching)
- **Telemost**: live calls behind `YM_ENABLE_TELEMOST_UI` (needs a `gstreamer`-feature build + `gstreamer1.0-nice`); meetings REST and parts of SDP policy are best-effort, pending live verification
- **Video player**: inline in chat (play/pause/scrub); needs `gstreamer`
- **Chat actions API**: best-effort reverse-engineered RPC names
- **WS status events**: depend on server payload; history flags best-effort

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
* **Night Theme**: Telegram Desktop night tokens (`dialogsBg`, `msgIn` / `msgOut`), nheko-style dense sidebar, solid selection, custom thin scrollbars.

### Messaging & Interactivity
* **Input Editor**: Multi-line input field (`TextView`) with automatic height adjustment (up to 120px). Messages are sent by pressing `Enter`, while a new line is inserted with `Shift+Enter`.
* **Quick Edit**: Pressing the `Up` arrow key when the input is empty instantly opens edit mode for the last sent message.
* **Date Separators**: Logical chat message grouping by days ("Today", "Yesterday", and formatted dates).
* **Special Formats**: Rendering of sticker images (128x128px) and "emoji-only" mode (large 40px emoji size without a chat bubble when sending 1 to 3 emojis).
* **Context Menu**: Actions available on right-click (reply, copy, delete, edit).
* **Attachments**: File picker, drag-and-drop into the chat, Ctrl+V for clipboard images, Download / Open via `xdg-open`.
* **Drafts & Outbox**: Per-chat drafts survive chat switches; unsent messages retry after reconnect.
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
  libssl-dev libnotify-dev libdbus-1-dev libwebkitgtk-6.0-dev
```

*Note: For voice recording support, it is recommended to install GStreamer packages (`libgstreamer1.0-dev`, `libgstreamer-plugins-base1.0-dev`). Calls (the `gstreamer` feature: `webrtcbin`) additionally require the ICE plugin `gstreamer1.0-nice` — without it webrtcbin creates no transports and `! webrtc.` linking fails.*

---

## 🧱 Built With

<p>
<img src="https://img.shields.io/badge/Rust-stable-000000?style=for-the-badge&logo=rust&logoColor=white" alt="Rust" />
<img src="https://img.shields.io/badge/GTK-4-4A86CF?style=for-the-badge&logo=gtk&logoColor=white" alt="GTK" />
<img src="https://img.shields.io/badge/Libadwaita-GNOME-4A86CF?style=for-the-badge&logo=gnome&logoColor=white" alt="Libadwaita" />
<img src="https://img.shields.io/badge/GStreamer-media-FF3131?style=for-the-badge" alt="GStreamer" />
<img src="https://img.shields.io/badge/SQLite-cache-003B57?style=for-the-badge&logo=sqlite&logoColor=white" alt="SQLite" />
</p>

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
| `YANDEX_CLIENT_ID` | Application OAuth client ID | `<YOUR_YANDEX_CLIENT_ID>` |
| `YANDEX_CLIENT_SECRET` | Application OAuth client secret | — |
| `YANDEX_REDIRECT_URI` | Redirect URI for the callback server | Auto-detected (local port) |
| `YANDEX_AUTH_PROXY_URL` | URL of the external authorization proxy | — |
| `YANDEX_OAUTH_AUTHORIZE_URL`| Yandex OAuth authorization endpoint | `https://oauth.yandex.com/authorize` |
| `YANDEX_OAUTH_TOKEN_URL` | Yandex OAuth token exchange endpoint | `https://oauth.yandex.com/token` |
| `RUST_LOG` | Logging level for the application | `info` |

---

## Development & Documentation

* **Architecture**: [ARCHITECTURE.md](ARCHITECTURE.md) · [ARCHITECTURE.ru.md](ARCHITECTURE.ru.md)
* **Development**: [DEVELOPMENT.md](DEVELOPMENT.md) · [DEVELOPMENT.ru.md](DEVELOPMENT.ru.md)
* **Security**: [SECURITY.md](SECURITY.md) · [SECURITY.ru.md](SECURITY.ru.md)
* **API**: [API.md](API.md) · [API.ru.md](API.ru.md)
* **Install**: [INSTALL.md](INSTALL.md) · [INSTALL.ru.md](INSTALL.ru.md)
* **Test plan**: [TESTPLAN.md](TESTPLAN.md) · [TESTPLAN.ru.md](TESTPLAN.ru.md)
* **Changelog**: [CHANGELOG.md](CHANGELOG.md) · [CHANGELOG.ru.md](CHANGELOG.ru.md)
* **Roadmap**: [ROADMAP.md](ROADMAP.md) · [ROADMAP.ru.md](ROADMAP.ru.md)
* **Detailed roadmap**: [ROADMAP_DETAILED.md](ROADMAP_DETAILED.md) · [ROADMAP_DETAILED.ru.md](ROADMAP_DETAILED.ru.md)
* **Contributing**: [CONTRIBUTING.md](CONTRIBUTING.md) · [CONTRIBUTING.ru.md](CONTRIBUTING.ru.md)

## 🗺️ Roadmap

Detailed plan: [ROADMAP.md](ROADMAP.md). See the [open issues](https://github.com/Bezooom/yandex-messenger-native/issues) for the full list of ideas and known problems.

## 🤝 Contributing

Contributions are welcome. If you have an idea that would make this better, open an issue or a pull request:

1. Fork the repository
2. Create a branch (`git checkout -b feature/my-idea`)
3. Commit (`git commit -m 'feat: my idea'`)
4. Push (`git push origin feature/my-idea`)
5. Open a pull request

Detailed guidelines: [CONTRIBUTING.md](CONTRIBUTING.md).

## License

This project is licensed under the MIT License. See [LICENSE](LICENSE) for details.

## 📬 Contact

**Pavel Naumov** (Bezooom) — [naumovai.ru](https://naumovai.ru) · [X @Bezooooom](https://x.com/Bezooooom) · naumov122@gmail.com

Project: [https://github.com/Bezooom/yandex-messenger-native](https://github.com/Bezooom/yandex-messenger-native)

## 🙏 Acknowledgments

- [gtk-rs](https://gtk-rs.org) — Rust bindings for GTK4
- [Libadwaita](https://gnome.pages.gitlab.gnome.org/libadwaita/)
- [nheko](https://github.com/Nheko-Reborn/nheko) and Telegram Desktop — UI inspiration
- [Best-README-Template](https://github.com/othneildrew/Best-README-Template)
- [Shields.io](https://shields.io)

<p align="right">(<a href="#readme-top">back to top</a>)</p>
