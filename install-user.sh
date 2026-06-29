#!/usr/bin/env bash
# Install Yandex Messenger to user-level ~/.local directory without requiring sudo/root password.
# Usage: ./install-user.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
BIN_DEST="$HOME/.local/bin"
DESKTOP_DEST="$HOME/.local/share/applications"
ICONS_DEST="$HOME/.local/share/icons/hicolor"

echo "=== Локальная установка Yandex Messenger (без sudo) ==="

# 1. Создание необходимых директорий
mkdir -p "$BIN_DEST"
mkdir -p "$DESKTOP_DEST"
mkdir -p "$ICONS_DEST"

# 2. Копирование бинарного файла
if [ -f "$SCRIPT_DIR/target/release/yandex-messenger" ]; then
    echo "Копирование бинарного файла в $BIN_DEST/..."
    cp "$SCRIPT_DIR/target/release/yandex-messenger" "$BIN_DEST/yandex-messenger"
    chmod +x "$BIN_DEST/yandex-messenger"
else
    echo "Ошибка: target/release/yandex-messenger не найден. Сначала выполните сборку: cargo build --release"
    exit 1
fi

# 3. Подготовка и копирование иконок
echo "Копирование иконок приложения..."
if [ -d "$SCRIPT_DIR/icons/hicolor" ]; then
    # Копирование векторной иконки
    mkdir -p "$ICONS_DEST/scalable/apps"
    cp "$SCRIPT_DIR/icons/hicolor/scalable/apps/yandex-messenger.svg" "$ICONS_DEST/scalable/apps/yandex-messenger.svg"

    # Копирование растровых иконок
    for size in 16 24 32 48 64 128 256; do
        if [ -d "$SCRIPT_DIR/icons/hicolor/${size}x${size}/apps" ]; then
            mkdir -p "$ICONS_DEST/${size}x${size}/apps"
            cp "$SCRIPT_DIR/icons/hicolor/${size}x${size}/apps/yandex-messenger.png" "$ICONS_DEST/${size}x${size}/apps/yandex-messenger.png"
        fi
    done
else
    echo "Предупреждение: директория icons/hicolor не найдена. Пропуск установки иконок."
fi

# 4. Настройка и копирование .desktop файла
echo "Создание ярлыка запуска..."
DESKTOP_FILE="$SCRIPT_DIR/yandex-messenger.desktop"
if [ -f "$DESKTOP_FILE" ]; then
    # Заменяем Exec на абсолютный путь к локальному бинарнику
    sed -e "s|^Exec=yandex-messenger|Exec=$BIN_DEST/yandex-messenger|g" "$DESKTOP_FILE" > "$DESKTOP_DEST/yandex-messenger.desktop"
    chmod +x "$DESKTOP_DEST/yandex-messenger.desktop"
else
    echo "Предупреждение: yandex-messenger.desktop не найден, ярлык не создан."
fi

# 5. Обновление базы данных приложений
echo "Обновление системной базы ярлыков..."
if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database "$DESKTOP_DEST" || true
fi

echo "=== Установка успешно завершена! ==="
echo "Новая версия Yandex Messenger установлена в: $BIN_DEST/yandex-messenger"
echo "Она автоматически переопределит старую системную версию в меню приложений."
echo "Запуск: yandex-messenger (убедитесь, что $BIN_DEST есть в вашей переменной PATH)"
echo "или через меню приложений: Yandex Messenger"
