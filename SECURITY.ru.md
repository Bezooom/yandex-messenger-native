# Yandex Messenger Native — Безопасность

[English version](SECURITY.md)

Текущий релиз: **2.173.0**.

## Обзор

Документ описывает соображения безопасности нативного desktop-клиента:
аутентификацию, хранение токенов и сессии, сетевое взаимодействие, файлы и WebSocket.

## Аутентификация

### OAuth2 Authorization Code Flow

Приложение использует OAuth2 Authorization Code Flow для desktop:

1. Открывается `/authorize` с `client_id`, `state`, `device_id`.
2. Пользователь входит в браузере или во встроенном WebView (`in_app_webview`).
3. Callback на локальный loopback-сервер; токен парсится из фрагмента URL или обменивается по коду.
4. Access token обновляется через refresh token за 5 минут до истечения.

Альтернатива — корпоративный `YANDEX_AUTH_PROXY_URL`, чтобы `client_secret` не жил на клиенте.

### Захват сессии в WebView (2.167+)

После логина клиент сохраняет cookies Паспорта (`Session_id`) и CSRF в
`~/.config/yandex-messenger-native/session.json`. Без этой сессии history / WS / файлы
деградируют. Fallback: `scripts/login_browser.py`.

## Хранение секретов

| Файл | Содержимое | Ожидаемые права |
|---|---|---|
| `~/.config/yandex-messenger-native/token.json` | access / refresh token | `0600` |
| `~/.config/yandex-messenger-native/session.json` | Passport cookies + CSRF | `0600` |
| `~/.config/yandex-messenger-native/settings.json` | UI-настройки, не секреты | обычные |
| `~/.local/share/yandex-messenger-native/cache.db` | кэш чатов/сообщений | пользовательский каталог |

- Токен в памяти: `Arc<Mutex<Option<OAuthToken>>>`.
- Проверка истечения: буфер 300 секунд, затем silent refresh.
- Logout должен удалять и память, и файлы на диске.

## HTTPS / TLS

- `reqwest` + `rustls-tls` (без системного OpenSSL).
- Полная проверка цепочки сертификатов rustls.
- TLS 1.2+ для HTTP; WSS для Uniproxy.

## CSRF и мутации

Перед изменяющими запросами клиент берёт CSRF-токен. Session cookies и CSRF
применяются вместе через `reload_session()` / `apply_session()`.

## Файлы

- Загрузка через Yandex Files (`files.messenger.yandex.net`).
- Лимит размера (порядка 50 МБ) на стороне клиента.
- Скачивание в `~/Downloads`; открытие через `xdg-open`.
- DnD и Ctrl+V идут в тот же attach pipeline.

## Поверхность атаки

1. **Браузер / WebView** — разбор OAuth redirect и XSS на странице Telemost.
2. **Диск** — `token.json`, `session.json`, SQLite-кэш переписки.
3. **Сеть** — HTTP API + WSS.
4. **Окружение** — `YANDEX_CLIENT_ID`, secret, proxy URL.
5. **UI** — WebView при `in_app_webview`.

## Угрозы и смягчение

| Угроза | Влияние | Смягчение |
|---|---|---|
| Кража токена/сессии с диска | Действия от имени пользователя | config dir, chmod 600; шифрование at rest — в планах |
| Replay токена | Отправка до истечения | Короткоживущий access token, auto-refresh |
| MITM | Перехват сообщений | TLS 1.2+ |
| CSRF | Подстановка сообщений | CSRF на сессию |
| Перехват OAuth redirect | Кража кода | проверка `state` |
| Переполнение загрузкой | DoS | лимит размера файла |

## Планы

- [ ] Шифровать `token.json` и `session.json` ключом устройства
- [ ] Certificate pinning для API
- [ ] Валидация схем WebSocket
- [ ] Rate limit загрузок
- [ ] Ротация refresh token
