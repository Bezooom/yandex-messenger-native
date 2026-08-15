# План реализации Telemost в Yandex Messenger Native

[English version](TELEMOST_IMPLEMENTATION_PLAN.en.md)

> Основано на реинжиниринге APK `ru.yandex.telemost` v3.12.0.138  
> Актуально для релиза **2.173.0**: нативного WebRTC нет, только WebView/browser shell.

## Текущее состояние

| Компонент | Статус | Путь |
|-----------|--------|------|
| `TelemostWindow` | Stub (WebView/browser) | `src/ui/telemost.rs` |
| `start_call` / `end_call` / `get_call_status` | REST stub | `src/api/mod.rs:2786+` |
| `subscribe_call_updates` / `send_call_event_ws` | WebSocket stub | `src/api/mod.rs:1084+` |
| `TelemostCall`, `CallStatus`, `CallParticipant` | Модели есть | `src/models/mod.rs` |
| `TELEMOST_URL` | `https://telemost.yandex.ru` | `src/config.rs` |
| `YM_ENABLE_TELEMOST_UI` | Feature flag | `src/config.rs` |

## Найденные в APK данные

| Находка | Значение / формат |
|---------|-------------------|
| Goloom WS | `wss://goloom.strm.yandex.net/join` |
| Uniproxy WS | `wss://uniproxy.messenger.yandex.ru/uni.ws` |
| Cloud API | `https://api.messenger.yandex.net` |
| Signaling | 245 protobuf-классов (`ru.yandex.goloom.lib.model.signaling`) |
| Capabilities | 27 типов (AUDIO, VIDEO, SCREEN_SHARE, BROADCAST…) |
| ConferenceParams | `wsUri`, `roomId`, `peerId`, `sessionId`, `credentials`, `clientConfig` |
| Hello поля | `capabilities_offer`, `credentials`, `send_audio`, `send_video`, `send_sharing` |
| Нативная WebRTC | `libjingle_peerconnection_so.so` (16 MB) |
| Шумоподавление | `librnnoise.so` |
| Quasar | `libquasar_daemons.so` (интеграция с Алисой) |

## Цели

### P0 (минимально жизнеспособный звонок)
1. Создание конференции через Cloud API → получить `ConferenceParams`
2. Подключение к Goloom WebSocket с signaling
3. Базовый аудио-звонок (WebRTC)
4. UI: окно звонка с mute/hangup

### P1 (полноценный звонок)
5. Видео-потоки
6. Демонстрация экрана
7. UI: layout участников, индикаторы

### P2 (дополнительно)
8. Входящие звонки (ringing service)
9. Запись звонка
10. Трансляция (broadcast)
11. ИИ-функции (саммаризация, Alice Pro)

## Детальный план

### Phase 1: Foundation (недели 1-2)

#### 1.1 Конфигурация
- [ ] Добавить в `config.rs`:
  - `GOLOOM_WS_URL`: `wss://goloom.strm.yandex.net/join`
  - `UNIPROXY_WS_URL`: `wss://uniproxy.messenger.yandex.ru/uni.ws`
  - `CLOUD_API_BASE`: `https://api.messenger.yandex.net`
  - `TELEMOST_API_PATH`: `/v1/telemost` (предположительно)

#### 1.2 Модели данных
- [ ] Добавить в `src/models/telemost.rs`:
  - `ConferenceParams` — параметры конференции из Cloud API
  - `ConferenceState` — состояние конференции
  - `AccessLevel` — уровни доступа
  - `Capabilities` — capabilities offer/answer
  - `SignalingMessage` — enum для всех типов signaling-сообщений
  - `HelloMessage` — приветствие при подключении к Goloom

#### 1.3 Cloud API методы
- [ ] Добавить в `HttpClient`:
  - `create_conference(chat_id, options)` → `ConferenceParams`
  - `get_conference(conference_id)` → `ConferenceState`
  - `end_conference(conference_id)`
  - `update_participant(conference_id, params)`

#### 1.4 Зависимости
- [ ] Добавить в `Cargo.toml`:
  - `prost` + `prost-types` — protobuf
  - `bytes` — для бинарных данных
  - `webrtc` — WebRTC (или обёртка над FFI)
  - `pipewire` — для захвата экрана/аудио на Linux (опционально)

### Phase 2: Signaling (недели 3-4)

#### 2.1 Goloom WebSocket клиент
- [ ] Создать `src/api/goloom_ws.rs`:
  - Подключение к `wss://goloom.strm.yandex.net/join`
  - Отправка `Hello` с capabilities
  - Обработка `ServerHello`, `SFUHello`
  - State machine: `Disconnected → Connecting → Connected → InCall`

#### 2.2 Signaling парсер
- [ ] Реализовать десериализацию protobuf-сообщений
  - Первый вариант: JSON-over-WS (если сервер поддерживает)
  - Второй вариант: полный protobuf через `prost`
- [ ] Обработка ключевых сообщений:
  - `PublisherSdpOffer` / `PublisherSdpAnswer`
  - `SubscriberSdpOffer` / `SubscriberSdpAnswer`
  - `WebrtcIceCandidate`
  - `RequestSubscription`
  - `SetSlots`
  - `Status` / `Notification`

#### 2.3 WebRTC PeerConnection
- [ ] Интегрировать `webrtc` crate:
  - `RTCPeerConnection` для аудио/видео
  - `MediaStream` для локальных/удалённых потоков
  - ICE candidate exchange через signaling
  - SDP offer/answer negotiation
- [ ] Аудио:
  - Захват через `pipewire` или `pulseaudio`
  - Кодирование Opus
  - Отправка в PeerConnection
- [ ] Видео:
  - Захват через `pipewire` или `v4l2`
  - Кодирование VP8/VP9
  - Отправка в PeerConnection

### Phase 3: UI (недели 5-6)

#### 3.1 TelemostWindow v2
- [ ] Заменить WebView на GTK-виджеты:
  - `GtkVideo` (или `GtkGLArea`) для удалённых видео
  - `GtkPicture` для локального превью
  - Панель управления: mute, video, screen share, end call
- [ ] Индикаторы:
  - Speaking indicator (VAD)
  - Muted indicator
  - Connection quality

#### 3.2 Интеграция с ChatView
- [ ] Кнопка звонка в `ChatView`
- [ ] Incoming call notification
- [ ] Call history в чате (system messages)

#### 3.3 Настройки
- [ ] Audio input/output selection
- [ ] Video input selection
- [ ] Screen share source selection

### Phase 4: Polish (неделя 7+)

#### 4.1 Тестирование
- [ ] Unit-тесты для signaling
- [ ] Integration-тесты WebRTC (loopback)
- [ ] E2E тесты UI

#### 4.2 Оптимизация
- [ ] Адаптивный битрейт
- [ ] Энергосбережение (отключение видео при сворачивании)
- [ ] Обработка сетевых ошибок

#### 4.3 Документация
- [ ] Архитектура звонков
- [ ] Protocol docs (signaling flow)
- [ ] Troubleshooting guide

## Риски и ограничения

| Риск | Митигация |
|------|-----------|
| Protobuf-схемы неизвестны | Обратный инжиниринг из APK, JSON fallback |
| WebRTC в Rust сложен | Использовать `webrtc` crate, начать с аудио |
| Screen share на Linux | PipeWire требует настройку, fallback на X11 |
| Производительность | Профилирование, оптимизация пулов потоков |
| Обратная совместимость | Feature flag `YM_ENABLE_TELEMOST_UI` |

## Следующие шаги

1. Согласовать план с командой
2. Начать с Phase 1.1 (конфиги)
3. Реализовать `create_conference` API
4. Подключиться к Goloom WS и получить `Hello`
