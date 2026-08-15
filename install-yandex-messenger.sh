#!/bin/bash
# Install Yandex Messenger .deb package
# Usage: sudo ./install-yandex-messenger.sh

set -e

# Prefer newest packaged deb under dist/, fall back to parent dir (debuild output)
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
DEB_PATH=""
if ls "$SCRIPT_DIR"/dist/yandex-messenger-native_*_amd64.deb >/dev/null 2>&1; then
    DEB_PATH="$(ls -1t "$SCRIPT_DIR"/dist/yandex-messenger-native_*_amd64.deb | head -1)"
elif ls "$SCRIPT_DIR"/../yandex-messenger-native_*_amd64.deb >/dev/null 2>&1; then
    DEB_PATH="$(ls -1t "$SCRIPT_DIR"/../yandex-messenger-native_*_amd64.deb | head -1)"
fi

if [ -z "$DEB_PATH" ] || [ ! -f "$DEB_PATH" ]; then
    echo "Ошибка: .deb не найден. Соберите пакет или положите его в dist/"
    exit 1
fi

echo "Установка Yandex Messenger из:"
echo "  $DEB_PATH"
sudo dpkg -i "$DEB_PATH"

# Fix any missing dependencies
if sudo apt-get install -f -y 2>/dev/null; then
    echo "Зависимости установлены."
else
    echo "Внимание: не удалось автоматически установить зависимости."
    echo "Выполните: sudo apt-get install -f"
fi

echo "Установка завершена!"
echo ""
echo "Запуск: yandex-messenger"
echo "или через меню приложений: Yandex Messenger"
