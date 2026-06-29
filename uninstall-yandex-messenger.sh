#!/bin/bash
# Uninstall Yandex Messenger .deb package
# Usage: sudo ./uninstall-yandex-messenger.sh

set -e

echo "Удаление Yandex Messenger..."
sudo dpkg --purge yandex-messenger-native

# Remove leftover files not tracked by dpkg
echo "Очистка оставшихся файлов..."
sudo rm -rf /usr/share/yandex-messenger-native/
sudo rm -f /usr/share/applications/yandex-messenger.desktop
sudo rm -f /usr/share/icons/hicolor/scalable/apps/yandex-messenger.svg
sudo rm -f /usr/share/icons/hicolor/128x128/apps/yandex-messenger.png
sudo rm -f /usr/share/man/man1/yandex-messenger.1

# Clean up icons cache
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
    gtk-update-icon-cache -q /usr/share/icons/hicolor || true
fi

# Clean up desktop database
if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database -q || true
fi

echo "Удаление завершено!"
