# Инвентарь APK 3.12.0.138: экраны / сервисы / пуши

Источник строк: бинарный `AndroidManifest.xml` (`strings -el`) + `classes*.dex`.
Квадратные скобки — роль компонента. Маппинг на наш код — последний столбец.

## Точка входа и приложение

| Компонент | Роль | У нас |
|---|---|---|
| `com.yandex.messenger.LauncherActivity` | LAUNCHER | `src/main.rs` — окно сразу чаты, без лаунчера |
| `com.yandex.messenger.MainApplication` | Application | `src/main.rs` |
| `com.yandex.messenger.emoji.MessengerEmojiInitializer` | init emoji | `src/ui/emoji_picker.rs` (статичный набор) |

## Мессенджер: чаты и навигация

| Компонент | Роль | У нас |
|---|---|---|
| `com.yandex.messaging.activity.MessengerActivity` | главный экран чатов | `src/ui/chat_list.rs` + `src/ui/chat_view.rs` |
| `com.yandex.messaging.activity.ChatOpenAlias` | deep-link открытия чата | нет (нет `ychat://`/x-scheme handler) |
| `com.yandex.messaging.DeepLinkActivityAlias` | внешние ссылки | нет |
| `com.yandex.messaging.ShareAlias` | share-target из других app | нет (нет desktop share-target) |
| `com.yandex.messaging.UniversalLinkTelemostActivityAlias` | universal links Телемоста | нет |
| `InviteLink{Alpha,Prod,ProdLegal,ProdTeam}ActivityAlias` | 4 флейвора invite-ссылок | нет invite-ссылок |
| `com.yandex.messaging.ui.auth.AuthorizeActivity` | экран входа | `src/ui/auth_dialog.rs` |
| `com.yandex.messaging.ui.auth.ProxyPassportActivity` | вход через прокси Паспорта | `auth-proxy/` + `YANDEX_AUTH_PROXY_URL` (задел есть) |
| `com.yandex.messaging.ui.migration.MigrationEnvironmentActivity` | миграция окружения (alpha/prod) | нет (есть только alpha-хост в строках, не в коде) |

## Нотификации и пуши

| Компонент | Роль | У нас |
|---|---|---|
| `com.yandex.messenger.push.FcmService` | Firebase push | нет (desktop: WS-poll; FCM невозможен — нужен свой poll) |
| `com.yandex.messaging.activity.MessengerNotificationActivity` | экран по тапу на пуш | частично: `send_notification_for_chat` без click-action |
| `com.yandex.messaging.NotificationActionService` | экшены уведомлений | нет |
| `...notifications.autocancel.CancelNotificationBroadcastReceiver` | автоснятие уведомлений | нет |
| Интенты `Chat.{OPEN,MARK_AS_READ,DISMISS,NOTIFICATION_CLICK}`, `ChatSummary.{OPEN,DISMISS}`, `Sharing.OPEN` | контракт нотификаций | нет эквивалента — тосты без кнопок |

## Медиа и вложения

| Компонент | Роль | У нас |
|---|---|---|
| `com.yandex.messaging.mediaviewer.MediaViewerActivity` | просмотр медиа | `src/ui/image_viewer.rs` (фото ок) |
| `com.yandex.messaging.ui.imageviewer.ImageViewerActivity` | второй вьювер картинок | см. выше |
| `com.yandex.messaging.video.activity.MessengerVideoPlayerActivity` | видеоплеер | **нет** (`chat_view.rs:3064 TODO`) |
| `com.yandex.attachments.activity.ChooserActivity` + `ChooserFileProvider` | системный выбор файлов | частично: GTK file chooser + DnD/paste |
| `com.yandex.messaging.MessagingFileProvider` + `files.SharingFileProvider` | расшаривание файлов | частично: `xdg-open`, `~/Downloads` |
| `com.yandex.mail360.camera.doc.scanner.*` | скан документов | нет (out of scope) |
| `com.yandex.messaging.video` / `audio` (6/3 класса) | запись/кодирование | `src/core/voice_recorder.rs` (pipeline без съема семплов — чинить) |

## Опросы, боты, миниаппы

| Компонент | Роль | У нас |
|---|---|---|
| `ui.createpoll.CreatePollActivity`, `ui.pollinfo.PollInfoActivity`, `ui.polloptioninfo.PollOptionInfoActivity` | создание + результаты опросов | `poll_creator.rs`/`poll_renderer.rs`, live-результатов нет |
| `com/yandex/messaging` `bot` (890), `miniapps` (85), `calendar` (2) | боты/миниаппы/календарь | `bot_panel.rs` без callbacks E2E; миниаппов/календаря нет |

## Звонки (Телемост newarch)

| Компонент | Роль | У нас |
|---|---|---|
| `messaging.telemost.ui.MessengerTelemostActivity` | экран звонка | `src/ui/telemost_window.rs` (плейсхолдер без media) |
| `...ui.MessengerTelemostStarterActivity` | стартер звонка из чата | частично: кнопка звонка за `YM_ENABLE_TELEMOST_UI` |
| `...ui.incoming.MessengerTelemostRingingActivity` | входящий звонок | нет |
| `...telemost.ringing.MessengerTelemostRingingService` | foreground ringing-сервис | нет (desktop-аналог: portal + call window) |
| `...ui.invite.MessengerInviteToMeetingActivity` | приглашение на встречу | нет |
| `activity.calls.MessengerCallActivity`, `MessengerCallConfirmActivity`, `MessengerCallFeedbackActivity` | звонок/подтверждение/фидбек | нет (только `End` с пустым id) |
| `telemost.newarch.screensharing.ScreenSharingService` | шаринг экрана | нет (путь: `xdg-desktop-portal ScreenCast`) |
| `telemost.core.waitingroom.WaitService` | зал ожидания | нет |
| `telemost.ui.telemsngr.callscreen.*` (`ChatState/MediaDevicesState/StatusesState`), `slots.GridMode`, `MoreMenuSectionParams.{Broadcast,Cloud,MediaSettings,Privacy,Top}` | Compose-экран звонка newarch | нет — ориентир для редизайна `telemost_window.rs` |

## Аккаунты и Passport

| Компонент | Роль | У нас |
|---|---|---|
| `passport.AuthSdk`, `AuthenticationService`, `SsoContentProvider`, `SsoAnnouncingReceiver`, `SyncProvider/SyncService` | SSO, аккаунты, синк | `src/api/auth.rs` (OAuth) + `account_dropdown.rs`; SSO/синка нет |
| `passport` соц-auth (`Google/FB/VK/Esia`), `SmsRetrieverReceiver`, CredentialManager | способы входа | out of scope (только OAuth/WebView) |
| `FetchExperimentsService` | эксперименты/фичи-флаги сервера | нет (только локальные `YM_ENABLE_*`) |

## Разрешения, релевантные десктопу

Микрофон/камера (`RECORD_AUDIO`, `CAMERA`), нотификации (`POST_NOTIFICATIONS`), точные алармы
(`SCHEDULE_EXACT_ALARM` — отложенные сообщения), `SYSTEM_ALERT_WINDOW` (поверх окон — входящий звонок),
`USE_FULL_SCREEN_INTENT` (полноэкранный входящий), `FOREGROUND_SERVICE_{PHONE_CALL,MICROPHONE,MEDIA_PROJECTION}`
(ringing/шаринг), контакты (`READ_CONTACTS` — инвайты), `RECEIVE_BOOT_COMPLETED` (автозапуск).
Наш чеклист: portal-микрофон/камера/скринкаст, autostart `.desktop`, полноэкранный ringing, точные таймеры outbox/scheduled.

## Что осознанно out of scope

Windows/macOS/mobile, Firebase push как есть (замена — WS-poll), скан документов Mail360,
соц-входы Passport, полный admin Yandex 360.
