# Installation Guide

[Русская версия](INSTALL.ru.md)

Current release: **2.173.0**.

## Requirements

- Ubuntu 22.04+ or Debian 11+ (Ubuntu 24.04+ recommended for GTK 4.12 / Libadwaita)
- Rust toolchain (`cargo`, `rustc`) via [rustup](https://rustup.rs/)
- GTK4, Libadwaita, SQLite, and WebKitGTK development packages

## Install dependencies

```bash
sudo apt update
sudo apt install -y \
  build-essential pkg-config cargo devscripts debhelper librsvg2-bin \
  libgtk-4-dev libadwaita-1-dev libsqlite3-dev libssl-dev libnotify-dev \
  libdbus-1-dev libwebkitgtk-6.0-dev
```

For voice recording (optional, still a stub UI):

```bash
sudo apt install -y libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev
```

## Build

```bash
make build
```

The binary is `target/release/yandex-messenger`.

## Run

```bash
make run
```

After WebView login the client stores `~/.config/yandex-messenger-native/session.json` (mode `0600`) plus `token.json`.

## Install system-wide

```bash
sudo make install
```

User-local install (no root):

```bash
./install-user.sh
```

## Uninstall

```bash
sudo make uninstall
```

## Debian package

```bash
debuild -us -uc
```

or

```bash
make dist
sudo apt install -y ./dist/yandex-messenger-native_2.173.0-*_amd64.deb
```

## Environment

See the table in [README.md](README.md). Feature flags: `YM_ENABLE_VOICE`, `YM_ENABLE_TELEMOST_UI` (both off by default).
