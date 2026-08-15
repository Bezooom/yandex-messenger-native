# Руководство по установке

[English version](INSTALL.md)

Текущий релиз: **2.173.0**.

## Требования

- Ubuntu 22.04+ или Debian 11+ (рекомендуется Ubuntu 24.04+ для GTK 4.12 / Libadwaita)
- Инструментарий Rust (`cargo`, `rustc`) через [rustup](https://rustup.rs/)
- Пакеты разработки GTK4, Libadwaita, SQLite и WebKitGTK

## Зависимости

```bash
sudo apt update
sudo apt install -y \
  build-essential pkg-config cargo devscripts debhelper librsvg2-bin \
  libgtk-4-dev libadwaita-1-dev libsqlite3-dev libssl-dev libnotify-dev \
  libdbus-1-dev libwebkitgtk-6.0-dev
```

Для записи голоса (опционально, UI всё ещё заглушка):

```bash
sudo apt install -y libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev
```

## Сборка

```bash
make build
```

Бинарник: `target/release/yandex-messenger`.

## Запуск

```bash
make run
```

После входа через WebView клиент сохраняет `~/.config/yandex-messenger-native/session.json` (права `0600`) и `token.json`.

## Установка в систему

```bash
sudo make install
```

Локальная установка без root:

```bash
./install-user.sh
```

## Удаление

```bash
sudo make uninstall
```

## Пакет Debian

```bash
debuild -us -uc
```

или

```bash
make dist
sudo apt install -y ./dist/yandex-messenger-native_2.173.0-*_amd64.deb
```

## Окружение

Таблица переменных — в [README.ru.md](README.ru.md). Флаги: `YM_ENABLE_VOICE`, `YM_ENABLE_TELEMOST_UI` (оба выключены по умолчанию).
