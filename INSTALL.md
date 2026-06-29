# Installation Guide

## Requirements

- Ubuntu 22.04+ or Debian 11+
- Rust toolchain (`cargo`, `rustc`)
- GTK4 development packages

## Install dependencies

```bash
sudo apt update
sudo apt install -y \
  build-essential pkg-config cargo devscripts debhelper librsvg2-bin \
  libgtk-4-dev libsqlite3-dev libssl-dev libnotify-dev
```

## Build

```bash
make build
```

## Run

```bash
make run
```

## Install system-wide

```bash
sudo make install
```

## Uninstall

```bash
sudo make uninstall
```

## Debian package

```bash
debuild -us -uc
```

## Build distributable artifacts

```bash
make dist
```
