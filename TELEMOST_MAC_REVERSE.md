# Реверс официального desktop-клиента Telemost (macOS)

> Дата: 2026-11-25. Объект: `Yandex.Telemost.app` **3.6.4 (10616)**, `ru.yandex.desktop.telemost`,
> dmg 387 МБ с `https://telemost.yandex.ru/download-desktop?platform=mac` (нужен Mac User-Agent).
> Метод: `7z` (dmg = HFS+, вскрывается без macOS), `strings` по thin-x86_64 Mach-O, пути исходников из бинаря.
> Артефакты: `telemost-mac/` в этом репо (`YTelemost.dmg`, распакованный бандл `extracted/`, thin-бинари `telemost_0x*.bin`).

## Главный вывод

Официальный desktop-клиент — **не нативный мессенджер**. Это Qt6-оболочка, в которой:

- **чат/мессенджер рендерится веб-приложением** через встроенный Chromium (`libQt6WebEngineCore` — 371 МБ);
  нативный слой лишь мостится к вебву через JSON-протокол (`core/Messenger/Protocol/In|Out`);
- **звонки — нативные**: свой Goloom-клиент + форк WebRTC (`sdk/`), а не вебзвонки.

Значит цель «функции как в оф. приложении и не хуже» для нас = **паритет web-мессенджера**
(его API мы уже долбим) **+ нативный звонок уровня desktop Goloom**. Десктоп-клиент не открывает
никаких закрытых messaging-API, которых нет у web/mobile.

## Что видно внутри (пути исходников из бинаря, всего 254)

Префикс: `arcadia/yandex360/desktop/clients/src/Applications/Telemost/`.

### Goloom (звонки) — `goloom/`

`session.cpp`, `signaling/signaling.cpp`, `api_messaging/ws_api_messaging_service.cpp`,
`api_messaging/mediator.cpp`, `room_connection/room_connection.cpp`, `publisher/publisher.cpp`,
`subscriber/subscriber.cpp`, `subscriber/participants_service.cpp`,
`peer_connection/peer_connection_controller.cpp`, `screen_capture/screen_capturer.cpp`,
`telemetry/stats_reporter.cpp`.

Словарь сообщений (совпадает с Android-protobuf из APK, классы видны в символах):
`HelloMessage`, `ServerHelloMessage`, `PublisherSdpOffer/Answer`, `SubscriberSdpOffer/Answer`,
`WebrtcIceCandidateMessage`, `SlotsConfigMessage`, `SlotsMetaMessage`, `SetSlots`,
`UpsertDescriptionMessage`, `RemoveDescriptionMessage`, `UpdateDescriptionMessage`,
`NotificationMessage`, `Status`, `PingMessage`, `VideoLayersConfigurationMessage`,
`TelemostAllowConfigureSimulcastMessage`, quality-report (`SelfQualityReport`,
`UpsertParticipantsQualityReport`).
Поля: `peerId`, `roomId`, `sessionId`, `credentials`, `iceServers`, `sdp`, `candidate`, `participantId`.
В UI есть **GoloomDebugPanel** — протокол печатается текстом; в символах есть `SubscriberSdpAnswerMessage::toJSON`.
**Транспорт — JSON-over-WSS (доказано):** в бинаре лежат строковые ключи кадров
`type`, `payload`, `sendAudio`, `sendVideo`, `sendSharing`, `capabilitiesOffer`, `iceServers` —
т.е. protobuf-схемы Android'а здесь сериализуются в JSON с camelCase-ключами.
Наш `goloom_ws` можно писать сразу на serde_json, protobuf не нужен.

### Медиа-SDK — `sdk/`

Форк WebRTC (`sdk/webrtc/...`, VP9 decoder), `AudioModule`, `AudioDeviceMac`, `RNNoiseDenoiser`
(+ `libRNNoise*.dylib`), `VirtualBackground` (+ **ONNX Runtime 1.18.1**, 40 МБ — сегментация фона),
`ScreencastCapturer` (ScreenCaptureKit: `SckWindowCapturer.mm`), `LocalRecorderEngine` (локальная запись),
`SummaryControllerImpl` (саммаризация/Alice), `WaitingRoom/` (админ, admit, access-checker).

### Фичи, которые и есть «уровень оф. приложения» в звонках

Screen share (+ звук вкладки, HUD-окно), виртуальный фон, шумоподавление RNNoise, локальная запись,
waiting room, модерация участников (`GUI/Moderation/`), broadcast, раскладка спикера/мини-окно,
входящий кол-модал (`ShowAcceptCallModalData`), баннеры уведомлений, статистика RTC-панель.

### Messaging-мост — `core/Messenger/`

In: `SetCallStatus`, `SetMute`, `HandleNotificationBannerAction`.
Out: `MeetingConnect/DisconnectRequest`, `OpenChatRequest`, `OpenDeepLinkRequest`,
`SetSettingRequest`, `SetPrivacySettingRequest`, `HandleDownloadWebRequest`, `HandleMuteWebRequest`.
Common: `UnreadCounter`, `NotificationData`, `MediaDevices`, `PrivacySettings`, `HotKeys`,
`CreateBroadcastData`, `JoinMeetingData`, `UpdateCurrentUserStatus`.
Это всё события моста web<->native, не новые API.

### Прочее

Endpoints: `https://api.messenger.yandex.ru/api/` (+ alpha/test/deploy), `wss://goloom[-prestable].strm.yandex.net/join`,
`wss://push.yandex.ru`. Автообновления: `Services/UpdateService`. Трейн: `Services/TrayService`.
Мультаязычность: `en/ru/tr/uk.lproj`. Мин. macOS 12, universal (x86_64 + arm64).

## Что это меняет в нашем плане

1. `TELEMOST_IMPLEMENTATION_PLAN.md` подтверждается: тот же Goloom, что в APK. Добавить: транспорт —
   JSON (проверить на живой сессии через логирование WS), словарь сообщений выше, `SlotsConfig/SlotsMeta`
   обязательны для раскладки.
2. Чеклист паритета звонков (P0→P2): аудио (Opus + RNNoise) → видео (VP8/VP9) → screen share (PipeWire,
   звук вкладки — P2) → слоты/раскладка → mute/модерация → waiting room → запись → саммаризация.
3. Для «мессенджер-фич не хуже» ориентир — web-версия (тот же UI у оф. клиента), не экзотика mac-клиента.

## Не проверено / дальше

- Точный wire-format (JSON vs protobuf) — бинарь не доказывает, нужна живая Goloom-сессия.
- Ghidra 12.2 установлена (`~/storage/Tools/ghidra_12.2_PUBLIC`) — можно декомпилировать конкретные
  функции (`room_connection`, `signaling`) для полей сообщений.
- Не вскрывал win-версию (тот же код, но легче декомпилируется PE) — если пригодится.
