# Анализ Android-клиента Telemost v2

[English version](telemost-android-analysis.md)

Заметки по реверсу APK `ru.yandex.telemost` **3.12.0.138**. Сам APK/XAPK в репозиторий **не** входит. Desktop-клиент 2.173.0 по-прежнему использует оболочку WebView.

## Отпечаток APK

- Package: `ru.yandex.telemost`
- Version: `3.12.0.138`
- Min SDK: 26, Target SDK: 35
- DEX: classes.dex (main), classes2.dex (meetings/cloudapi), classes3.dex (config), classes4.dex (protobuf)
- Native: `libjingle_peerconnection_so.so`, `libquasar_daemons.so`, `librnnoise.so`

## Архитектура

- **Call Framework**: `ru.yandex.goloom` (Goloom) — на базе WebRTC
- **Signaling**: Protobuf over WebSocket
- **HTTP**: Ktor Client + OkHttp
- **JSON**: Moshi
- **Protobuf**: Square Wire + custom protobuf
- **WebView SDK**: `com.yandex.messenger.websdk.api`
- **Auth**: Yandex Passport SDK + OAuth2

Полные таблицы эндпоинтов, signaling-классов и нативных библиотек — в английской версии (исходный разбор). План внедрения: [TELEMOST_IMPLEMENTATION_PLAN.md](TELEMOST_IMPLEMENTATION_PLAN.md).
