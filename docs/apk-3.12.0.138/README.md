# APK-эталон: ru.yandex.telemost 3.12.0.138

Источник: `ru.yandex.telemost_3.12.0.138.xapk` (103М) в корне репозитория.
Дата извлечения: 2026-09-06. Метод: `unzip` + `strings` по `classes*.dex` и бинарному `AndroidManifest.xml`
(без jadx/apktool — только проверяемые строковые факты, без декомпиляции).

## Состав xapk

| Файл | Размер | Содержимое |
|---|---|---|
| `ru.yandex.telemost.apk` | 53М | base: код + ресурсы, `classes.dex…classes4.dex` |
| `config.arm64_v8a.apk` | 47М | нативные `.so` под arm64 |
| `config.xxhdpi.apk` | 3.5М | ресурсы под xxhdpi |
| `manifest.json` | 4К | `package=ru.yandex.telemost`, `versionCode=25559178`, `versionName=3.12.0.138` |

`minSdk=26`, `targetSdk=35`.

## Нативные библиотеки (config.arm64_v8a)

| .so | Размер | Роль |
|---|---|---|
| `libjingle_peerconnection_so.so` | 16М | WebRTC media stack (libwebrtc) |
| `libquasar_daemons.so` | 16М | фоновые демоны (звонки/телеметрия) |
| `librnnoise.so` | 129К | шумоподавление микрофона |
| `libsearchapp-arcadia.so` | 13М | общий поисковый рантайм Яндекса |
| `libappmetrica-*.so` | ~2М | аналитика AppMetrica |

Вывод для десктопа: media-путь эталона — **libwebrtc + RNNoise**. Наш аналог —
GStreamer `webrtcbin` + `rnnoise`/`webrtcdsp`, либо `webrtc-rs`.

## Сеть эталона (строки из DEX)

* REST: `https://api.messenger.yandex.net/api/`, `https://api.messenger.yandex.net/logout_client/`,
  `https://backend.messenger.yandex.net/unread_count`, alpha-хост `https://api.messenger.alpha.yandex.net`
* Telemost-ссылки: `telemost.yandex.ru/j/`, `/c/`, `/link`, DSP/DST-хосты (см. `telemost-android-analysis.md`)
* Сигналинг: `wss://goloom.strm.yandex.net/join` (primary),
  `wss://uniproxy.alice.yandex.net/uni.ws` + `wss://beta.uniproxy.alice.yandex.net/uni.ws` (fallback)
* REST мессенджера: `dialog_history/api/{list_dialogs,create_dialog_with_messages,read_dialog,remove_dialog,remove_messages}`
* Telemost Cloud API: `create_personal_meeting`, `start_meeting_call`, `end_personal_meeting`, `meeting_info[s]`

## Ключевой факт: мессенджер встроен целиком

* Лаунчер пакета — мессенджерный: `com.yandex.messenger.LauncherActivity` (`MAIN/LAUNCHER`),
  `com.yandex.messenger.MainApplication`.
* По числу классов в DEX: `com/yandex/messaging` — 2330 (`core` 1498, `internal` 314, `action` 99,
  `input` 84, `ui` 74, `telemost` 63, `mediaviewer` 49…), `com/yandex/telemost` — 467,
  `com/yandex/passport` — 5682, `com/yandex/messenger` — 33.
* Полный инвентарь экранов/сервисов/пушей — в [`INVENTORY.md`](INVENTORY.md).
* Protobuf-схемы сигналинга лежат в APK открыто — [`proto/`](proto/):
  `signaling.proto` (45К, пакет `videoplatform.speakerroom.common.signaling`),
  `capabilities.proto` (пакет `...common.capabilities`),
  `events.proto` (пакет `...common.telemetry`).
  Rust-порт: `src/api/goloom.rs` (prost-типы + конвертер handshake).

## Rust-порт: статус

* `src/api/goloom.rs` — prost-типы handshake/media-negotiation (28 capabilities,
  Hello/ServerHello/SDP/ICE/слоты/качество, политика `StatusCode`), 6 тестов.
* `src/api/goloom_client.rs` — WS-клиент: Hello-first, ServerHello (ICE/secret/
  ping-периоды), proto Ping⇄Ack heartbeat с таймаутом, экспоненциальный
  реконнект по политике close-кодов, события в канал (`Connected/Roster/Slots/
  SDP/ICE/...`). 3 теста, включая loopback против локального WS-сервера
  (handshake → Ping→Ack → roster → terminal close; drop → переподключение).
* `src/api/goloom_media.rs` + `src/api/goloom_call.rs` — двух-PC дизайн как в
  протоколе: `CallController` владеет `publish` (LocalOffer →
  `PublisherSdpOffer`, входящий `PublisherSdpAnswer` → движок) и `subscribe`
  (входящий `SubscriberSdpOffer` → движок → `LocalAnswer` с тем же `pc_seq`)
  движками; ICE тегируется/роутится по `target`, mute — pad-probe + `update_me`.
  3 теста, включая полный signaling-mesh против mock-сервера.
* `src/api/gst_webrtc.rs` (фича `gstreamer`) — настоящий `webrtcbin`:
  publish-конвейер (capture → opus/vp8 → RTP), subscribe-конвейер
  (`add-transceiver` RECVONLY + `decodebin`), promise-цепочки SDP, ICE trickle,
  STUN/TURN из `rtc_configuration`, pad-probe mute, опрос `connection-state`,
  RGBA-кадры через appsink (`RemoteSink::Frames`, ноль новых зависимостей).
  P2P full-mesh тест двух реальных пайплайнов: офферы/ответы в обе стороны,
  ICE, кадры в обе стороны за ~2с, 0 ошибок. Требуется системный
  `gstreamer1.0-nice`, иначе webrtcbin не создаёт транспорты.
* `src/api/telemost.rs` — Cloud API встреч по именам методов из DEX
  (`create_personal_meeting` / `start_meeting_call` / `end_personal_meeting` /
  `meeting_info[s]`, толерантный парсинг + `UserErrors` → `Err`); HTTP-пути —
  best-effort, подтвердить live. `CallParams::from_meeting` отдаёт
  room/credentials в Goloom Hello.
* `src/ui/telemost_window.rs` — живое окно на `CallController`: статусы,
  таймер, ростер, mute/end/share, копирование ссылки, fallback «в браузере»,
  ringing-панель (`show_incoming` + `on_accept`), видео через `MemoryTexture`
  на `Picture`, PiP-превью камеры. Кнопка звонка в чате создаёт встречу →
  контроллер → окно. Входящие звонки поднимаются из WS-трафика
  (`extract_invite`: прямые методы + Telemost-ссылки с call-маркерами;
  голая ссылка без маркеров не звонит).
* Хвосты: HTTP-пути встреч — best-effort до живого сервера (включены
  debug-дампы ответов).
* Рядом закрыто: настоящие голосовые (запись/плеер/voice-тип), инлайн
  видеоплеер в чате, светлая Яндекс-тема + тёмная night с переключателем,
  портал screen-cast (Wayland) с фолбэком на ximagesrc, фетчинг транскрипций
  по кнопке, WCAG-контраст токенов в тестах.
* Рядом закрыто: настоящие голосовые (запись/плеер/voice-тип), инлайн
  видеоплеер в чате, светлая Яндекс-тема + тёмная night с переключателем.

## Сверка с кодом 2.173 (что эталон меняет)

Проверено `grep` по `src/`:

* `GOLOOM_WS_URL` объявлен в `src/config.rs:33`, но **нигде не используется** —
  теперь к нему есть типы handshake: `src/api/goloom.rs` (`hello_message`,
  `CapabilitiesOffer` на все 28 полей, `ServerHello` с `rtc_configuration`,
  `StatusCode::{expects_reconnect,is_terminal}`).
* `dialog_history/api/{list_dialogs,create_dialog_with_messages,read_dialog,…}`
  в коде **нет** — история ходит dual-path (session RPC + OAuth search).
  Следующий шаг: сверить `get_chat_list/get_history` с `list_dialogs/read_dialog`.
* `backend.messenger.yandex.net/unread_count` **не используется** — анрид
  считается локальной суммой (`main.rs:1092`, `core.rs:823`). Кандидат на серверный счетчик.
* `src/api/telemost.rs:47,86,119,143` ходит на `/v1/telemost/conferences*`,
  а эталон знает `create_personal_meeting/start_meeting_call/end_personal_meeting/meeting_info[s]`
  на `api.messenger.yandex.net/api/` — Cloud-клиент надо пересадить на методы APK.
* Нотификации (`FcmService`/`NotificationActionService`/интенты `Chat.*`) —
  у нас тосты без экшенов; ringing-сервиса и полноэкранного входящего нет.

## Домены фич (частота строк в DEX, ориентир scope паритета)

`video` 3262, `call` 2320, `media` 1633, `file` 1589, `auth` 1476, `account` 1176, `calend` 1152,
`notif` 1117, `push` 1072, `participant` 950, `bot` 890, `contact` 641, `attach` 494, `confer` 416,
`folder` 400, `voice` 390, `search` 353, `schedul` 320, `stick` 313, `reaction` 230, `poll` 208,
`broadcast` 232, `summariz` 113, `miniapp` 85, `thread` 718, `draft` 32, `transcri` 2.

Примечательно: транскрипции войсов в эталоне почти нет (2 упоминания) — гнаться за ней не нужно;
календарь (`calend` 1152) наоборот силен — учесть при планировании встреч.
