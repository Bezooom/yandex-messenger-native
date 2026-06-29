# Yandex Messenger Native — Development

## 1. Цель

Собрать production-ready Linux-клиент Yandex Messenger на Rust + GTK4 с:
- стабильным chat core;
- realtime и файловыми потоками;
- базовой интеграцией звонков;
- desktop-интеграцией;
- воспроизводимым процессом сборки и поставки.

## 2. Текущее состояние архитектуры

### Слои

- `UI` — виджеты и события пользователя (`src/ui/*`)
- `Core` — orchestration и state (`src/core.rs`)
- `API` — OAuth, HTTP, WS (`src/api/*`)
- `Models` — доменные структуры (`src/models/mod.rs`)
- `Packaging/CI` — `debian/*`, `.github/workflows/ci.yml`

### Основной runtime flow

1. `main.rs` поднимает GTK-приложение и авторизацию.
2. Создаётся `AppController` с `HttpClient` и `WebSocketClient`.
3. Загружается список чатов, выбор чата запрашивает историю.
4. Отправка сообщения и вложений идёт через `AppController`.
5. Call action открывает окно `TelemostWindow`.

## 3. Architecture Deep Dive

### UI Layer (`src/ui/`)

Каждый виджет — standalone GTK widget с собственной логикой рендеринга.
`ChatListPanel` отображает список чатов с превью и счётчиками непрочитанных.
`ChatView` рендерит сообщения, содержит input-поле и кнопки attachments/call.
`AuthDialog` управляет OAuth-флоу через встроенный WebView (feature `in_app_webview`).
`TelemostWindow` embed-обёртка для Yandex Telemost.
`settings.rs` — персистентные настройки через JSON (тёмная тема, минимизация в трей).
Общая тема — CSS-провайдер GTK (`theme.css`) с поддержкой dark mode.

### Core Layer (`src/core.rs`)

`AppController` — единственный orchestrator приложения. Содержит `AppState` (`Arc<Mutex<AppState>>`) как shared state между UI и API.
Методы контроллера асинхронные, выполняются через Tokio runtime.
`AppState` хранит: список чатов, selected chat ID, историю сообщений по чатам.
Все мутации состояния проходят через `Arc<Mutex<AppState>>`, UI подписывается на события через callback-замыкания в `main.rs`.

### API Layer (`src/api/`)

`AuthManager` — OAuth2 client: генерация auth URL, exchange code → token, refresh token, save/load to disk.
`HttpClient` — REST client на `reqwest` с TLS, автоматическим `OAuth` заголовком, CSRF-токенами.
`WebSocketClient` — каркас WS-клиента с seq-счётчиком, callback-системой для сообщений и state-событий.
HTTP-клиент поддерживает два формата ответа (flat list и wrapped `ListResponse`) для обратной совместимости.

### Models (`src/models/mod.rs`)

Доменные структуры сериализуются через `serde`. Ключевые типы: `Chat`, `Message`, `User`, `WSMessage`, `WSResponse`.
`MessageType` и `MediaType` — exhaustive enum'ы для всех типов контента.
`Message` поддерживает reply, forward, reactions, pinned, edited, media entities.

## 4. Coding Standards

- **Async/Await**: все I/O-операции асинхронные. `block_on` используется только в UI-потоке через `Arc<Runtime>`.
- **Error handling**: Result-типы с текстовыми ошибками (не custom error types yet).
- **Naming**: Rust conventions (`snake_case` для функций/переменных, `PascalCase` для типов).
- **Imports**: группировка — std, external, internal (`crate::`).
- **Config**: все хардкод-значения в `config.rs`, не в коде.
- **Arc<Mutex<T>>**: shared state только через Arc+Mutex, clone Arc при передаче в замыкания.
- **Comments**: doc comments для public функций, inline-комментарии для неочевидной логики.

## 5. Testing

### Unit-тесты
```bash
cargo test
```
Текущие тесты: проверка констант конфигурации (`src/api/mod.rs::tests`).

### Integration (ручные)
1. Запуск с `cargo run --release` → проверка авторизации.
2. Отправка сообщения → проверка отображения.
3. Загрузка файла → проверка upload/download.
4. Переключение тем → проверка dark/light mode.
5. Закрытие/восстановление окна → проверка tray behavior.

### CI
GitHub Actions (`/.github/workflows/ci.yml`):
- `cargo fmt --check`
- `cargo clippy`
- `cargo test`
- `cargo build --release`

### Recommended next tests
- Mock HTTP responses для unit-тестов API-слоя.
- Integration-тесты WS-подписки.
- Тесты OAuth-флоу с mock OAuth-сервером.

## 6. Deployment

### Локальная сборка
```bash
make build        # cargo build --release
make run          # cargo run --release
make dist         # release artifacts + deb
make icons        # generate icon set
```

### Debian пакет
```bash
debuild -us -uc -b
```
Требует `debhelper`, `dh-sequence-gtk4`, `dh-cargo`.

### PPA
См. [PPA.md](PPA.md) — процесс публикации в Personal Package Archive.

### Docker (для CI)
```bash
docker build -t yandex-messenger -f Dockerfile .
```

## 7. Статус по фазам

### Фаза 1 — Фундамент
- [x] Базовая структура проекта
- [x] AuthManager и хранение токена
- [x] Базовые модели чатов/сообщений/пользователя
- [x] HTTP/WS клиенты

### Фаза 2 — Базовый UI
- [x] ChatView
- [x] MessageList
- [x] MessageInput
- [x] Переключение чатов
- [x] Базовая стилизация

### Фаза 3 — Real-time
- [x] Subscribe/unsubscribe hooks
- [x] Отправка текстовых сообщений
- [x] Контракты под incoming/typing/read/unread
- [x] Статусные обновления на уровне UI/Core

### Фаза 4 — Файлы
- [x] Upload flow (API + controller)
- [x] Download flow (API + controller)
- [x] Attach action в UI
- [x] Базовые file preview hooks

### Фаза 5 — Telemost
- [x] Окно звонка (`src/ui/telemost.rs`)
- [x] Базовые call controls (mute/video/end)
- [x] Интеграция запуска звонка из чата

### Фаза 6 — Полировка
- [x] Уведомления (`notify-rust`)
- [x] Персистентные настройки
- [x] Shortcut action
- [x] Dark theme настройка
- [x] Desktop entry

### Фаза 7 — Упаковка
- [x] Debian metadata и rules
- [x] CI workflow
- [x] LICENSE
- [x] Man page
- [x] PPA release notes

## 8. Current Sprint

### Задачи
- [ ] Реализация полноценного WS transport loop (receive loop, pong/heartbeat)
- [ ] Статусы чтения и онлайн-статусы участников
- [ ] Unit-тесты API-слоя с mock-ответами
- [ ] Поиск по сообщениям
- [ ] Обработка ошибок сети с retry-политикой
- [ ] Форматирование текста (bold, italic, links) в MessageList

### Задокументировано
- [x] ARCHITECTURE.md
- [x] SECURITY.md
- [x] Обновлённый README.md
- [x] Обновлённый DEVELOPMENT.md

## 9. Ограничения текущей реализации

- WebSocket-слой в текущем baseline реализован как безопасный контрактный каркас, а не полный продакшн transport loop со всеми событиями сервера.
- Telemost реализован как интеграционный scaffolding-уровень (окно и call flow wiring), без полнофункционального embed-браузера.
- Для финального production rollout нужен обязательный прогон `cargo check/test/clippy` в окружении с установленным Rust toolchain.

## 10. Acceptance checklist

Проект считается готовым к релизу при выполнении:
- [x] Все фазы 2–7 отражены в кодовой базе
- [x] Документация синхронизирована с фактической структурой
- [x] Пакетные и release-файлы присутствуют
- [x] CI конфигурация добавлена
- [ ] Подтверждён полный runtime smoke на целевом окружении
- [ ] Подтверждена сборка `.deb` на чистом Debian/Ubuntu runner

## 11. Следующий технический шаг

1. Прогнать локально `make check && make test && make build`.
2. Проверить сборку пакета через `debuild -us -uc`.
3. Довести WS transport до полноценной двунаправленной обработки событий.
