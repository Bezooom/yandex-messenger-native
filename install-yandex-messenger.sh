#!/bin/bash
# Install Yandex Messenger .deb package
# Usage: sudo ./install-yandex-messenger.sh

set -e

DEB_FILE="yandex-messenger-native_2.162.0-1_amd64.deb"

# Resolve script directory (works even with sudo which changes $PWD)
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

# .deb is in parent directory (dpkg-buildpackage outputs to ..)
DEB_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
DEB_PATH="${DEB_DIR}/${DEB_FILE}"

if [ ! -f "$DEB_PATH" ]; then
    echo "Ошибка: файл $DEB_FILE не найден в ${DEB_DIR}"
    exit 1
fi

echo "Установка Yandex Messenger..."
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
