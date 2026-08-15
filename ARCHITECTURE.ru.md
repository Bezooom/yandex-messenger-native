# Yandex Messenger Native — Архитектура

[English version](ARCHITECTURE.md)

Текущий релиз: **2.173.0**.

## Обзор

Нативный Linux-клиент для Яндекс Мессенджера, разработанный на Rust и GTK4.
Приложение построено на основе слоистой архитектуры с четким разделением между интерфейсом (UI), бизнес-логикой и сетевым взаимодействием (API).

```mermaid
flowchart TD
    %% Styling
    classDef user fill:#e1f5fe,stroke:#01579b,stroke-width:2px;
    classDef ui fill:#e8f5e9,stroke:#2e7d32,stroke-width:2px;
    classDef core fill:#fff8e1,stroke:#f57f17,stroke-width:2px;
    classDef api fill:#f3e5f5,stroke:#6a1b9a,stroke-width:2px;
    classDef ext fill:#ffebee,stroke:#c62828,stroke-width:2px;

    subgraph UserGroup ["Пространство пользователя"]
        User["Пользователь (Desktop)"]:::user
    end

    subgraph UILayer ["Слой UI (src/ui/*)"]
        direction LR
        AuthDialog["AuthDialog"]
        ChatListPanel["ChatListPanel"]
        ChatView["ChatView"]
        TelemostWindow["TelemostWindow"]
        TrayHandle["TrayHandle"]
        Notifications["Notifications"]
        Settings["Settings"]
        Theme["Тема (CSS)"]
    end
    class UILayer ui;

    subgraph CoreLayer ["Слой Core (src/core.rs)"]
        AppController["AppController"]
        AppState["AppState (Arc&lt;Mutex&gt;)"]
        AppEvent["AppEvent (enum)"]
    end
    class CoreLayer core;

    subgraph APILayer ["Слой API (src/api/*)"]
        direction LR
        AuthManager["AuthManager (OAuth2)"]
        HttpClient["HttpClient (reqwest)"]
        WebSocketClient["WebSocketClient"]
    end
    class APILayer api;

    subgraph External ["Внешние сервисы"]
        OAuth["OAuth эндпоинты"]
        MessengerAPI["Messenger API (REST)"]
        UniproxyWS["Uniproxy WS"]
        Disk["Файлы (Yandex Files)"]
    end
    class External ext;

    %% Connections
    User -->|"GTK4 События / Системные вызовы"| UILayer
    UILayer -->|"Коллбеки / Shared State"| CoreLayer
    CoreLayer -->|"Асинхронные вызовы API"| APILayer
    APILayer -->|"HTTP / WS / Диск"| External
```

## Описание слоев

### Слой UI

Слой UI состоит из изолированных виджетов GTK4, каждый из которых управляет собственным рендерингом и взаимодействием с пользователем. Виджеты общаются со слоем Core через замыкания (коллбеки) и разделяемое состояние.

- **AuthDialog** (`auth_dialog.rs`): Диалог входа OAuth. Поддерживает открытие внешнего браузера и встроенный WebView (feature `in_app_webview`). Извлекает токен из фрагмента URL.
- **ChatListPanel** (`chat_list.rs`): Боковая панель со списком чатов, отображающая название, аватар, превью последнего сообщения, счетчик непрочитанных и статус закрепления чата. Генерирует сигнал `chat_selected`.
- **ChatView** (`chat_view.rs`): Главная область сообщений с виртуализированным MessageList, полем ввода MessageInput (текст + вложения) и кнопкой звонка. Обрабатывает отправку, ответы, пересылку и реакции.
- **TelemostWindow** (`telemost.rs`): Окно-обертка для видеозвонков Яндекс Телемост. Поддерживает отключение звука/видео и завершение звонка.
- **TrayHandle** (`tray.rs`): Иконка системного трея с контекстным меню (открыть, настройки, выход).
- **Notifications** (`notifications.rs`): Системные уведомления на рабочем столе с использованием библиотеки `notify-rust`.
- **Settings** (`settings.rs`): Сохраняемые настройки в формате JSON (тёмная тема, сворачивание в трей, уведомления, уменьшить анимации). Файл: `~/.config/yandex-messenger-native/settings.json`.
- **Тема** (`theme.css`): токены Telegram Desktop night, плотный список в стиле nheko, пузыри `msgIn` / `msgOut`, адаптивный сайдбар.

### Слой Core

Класс `AppController` (`src/core.rs`) является центральным оркестратором приложения. Он владеет:

- `AuthManager` — жизненный цикл токенов OAuth
- `HttpClient` — REST API-клиент (session cookies + CSRF)
- `WebSocketClient` — WebSocket-клиент реального времени
- `AppState` — общее изменяемое состояние приложения (Shared mutable state)
- SQLite-кэш (`src/core/db.rs`), черновики (`drafts.rs`), outbox (`outbox.rs`)
- Session store (`src/api/session_store.rs`) — cookies Паспорта после входа в WebView

Структура `AppState` содержит:
```rust
pub struct AppState {
    pub chats: Vec<Chat>,                    // Кэшированный список чатов
    pub selected_chat_id: Option<String>,    // Идентификатор активного чата
    pub messages_by_chat: HashMap<String, Vec<Message>>, // История сообщений по чатам
}
```

Контроллер предоставляет асинхронные методы для выполнения всех операций. Код UI вызывает их через `tokio::Runtime::block_on` в главном потоке GTK.

### Слой API

#### AuthManager (`src/api/auth.rs`)
Обеспечивает полную поддержку OAuth2 Authorization Code Flow:
1. Генерирует URL авторизации с параметрами `response_type=code`, `client_id`, `state`, `device_id`.
2. Пользователь авторизуется в браузере и перенаправляется на страницу с `#access_token=...`.
3. Токен парсится из фрагмента URL и сохраняется в `~/.config/yandex-messenger-native/token.json`.
4. При запуске загружает токен с диска и проверяет его валидность (с буфером в 300 секунд).
5. При истечении срока действия использует `refresh_token` для получения нового токена.
6. Поддерживает режим auth-proxy для централизованной авторизации.

Токены хранятся с использованием блокировки файлов (через `fs::write`) в формате JSON, содержащем:
- `access_token`, `refresh_token`, `expires_in`, `token_type`, `user_id`.

#### HttpClient (`src/api/mod.rs`)
REST-клиент на базе библиотеки `reqwest` с поддержкой `rustls-tls`:
- **Авторизация**: Заголовок `Authorization: OAuth <token>` добавляется ко всем запросам.
- **CSRF**: Запрашивает CSRF-токен с пути `/csrf-token/` перед выполнением мутирующих запросов.
- **Список чатов**: `GET /api/get_chat_list?offset=&limit=`
- **История**: `GET /api/get_history?chatId=&offset=&limit=`
- **Отправка сообщения**: `POST /api/send_text` с JSON-телом.
- **Загрузка**: `PUT /media_upload/<chatId>/<filename>?<uuid>` на сервера Yandex Files.
- **Скачивание**: `GET /file_shortterm/<fileId>` из Yandex Files.
- **Профиль пользователя**: `GET /api/get_profile`
- **Ссылка Телемост**: Создает URL вида `https://telemost.yandex.ru/?chatId=<id>`

Парсинг ответов поддерживает как плоские списки, так и обернутые ответы (`ListResponse<T>` с полями `items`/`chats`/`messages`).

#### WebSocketClient (`src/api/mod.rs`)
WebSocket-клиент для работы с Yandex Uniproxy (`wss://uniproxy.messenger.yandex.ru/uni.ws`):
- **Счетчик последовательности (Sequence)**: Каждое исходящее сообщение получает инкрементируемый `seq`.
- **Формат сообщений**: `{"seq": N, "message": {"method": "...", "params": {...}}}`
- **Методы**: `subscribe`, `unsubscribe`, `bootstrap`.
- **Коллбеки**: `on_message()` для входящих сообщений, `on_state_change()` для событий состояния подключения (Disconnected → Connecting → Connected → Reconnecting).
- **Текущий статус**: Базовый каркас подключения и отправки (разработка полного транспортного цикла находится в процессе).

### Модели данных (`src/models/mod.rs`)
Доменные типы данных с поддержкой сериализации `serde`:
- **Chat**: id, название, тип чата (Private/Group/Channel/Bot), участники, счетчик непрочитанных, последнее сообщение, флаги закрепления/архивации/выключения звука.
- **Message**: id, ID чата, ID отправителя, тип, текст, разметка (entities), ответ на сообщение (reply_to), пересланное сообщение (forward), медиафайлы, реакции, статус доставки и прочтения.
- **User**: id, телефон, email, имя, никнейм (username), ID аватара, статус в сети, флаги бота и премиума.
- **WSMessage/WSResponse**: Конверты протокола WebSocket с seq, result и error.
- **MediaAttachment**: Метаданные файлов с миниатюрами, разрешением, размером и длительностью.
- **TelemostCall**: Жизненный цикл звонка с участниками и статусами.

---

## Диаграммы потоков данных

### Процесс WebSocket/HTTP взаимодействия (Interaction Flow)

```mermaid
sequenceDiagram
    autonumber
    actor User as Пользователь/Приложение
    participant Auth as AuthManager
    participant REST as HttpClient (REST)
    participant WS as WebSocketClient
    participant Uniproxy as Yandex Uniproxy

    User->>Auth: Загрузка или запрос токена
    Note over Auth: Проверка срока действия / Обновление при необходимости
    Auth-->>User: Токен валиден

    User->>REST: GET /api/get_chat_list (с OAuth токеном)
    REST->>User: Возврат Vec<Chat> (Заполнение UI)

    User->>WS: Установка соединения
    WS->>Uniproxy: Подключение (wss://uni.ws)
    Uniproxy-->>WS: Соединение установлено

    User->>WS: Подписка на обновления чата
    WS->>Uniproxy: Отправка {"method":"subscribe", "params":{"chatId":"..."}}
    Uniproxy-->>WS: Подписка подтверждена

    loop Обновления в реальном времени
        Uniproxy->>WS: Входящее событие (сообщение)
        WS->>User: Вызов коллбека on_message()
    end
```

### Выбор чата (Chat Selection Flow)

```mermaid
flowchart TD
    Click["Пользователь выбирает чат в ChatListPanel"] --> Emit["ChatListPanel генерирует сигнал chat_selected"]
    Emit --> Controller["Коллбек в main.rs: controller.select_chat(chat_id)"]
    
    Controller --> WSSub["WebSocketClient.subscribe(chat_id)"]
    WSSub --> WSSend["Отправка WS {'method':'subscribe','params':{'chatId':'...'}}"]
    WSSend --> WSWait["Ожидание входящих сообщений WS (в процессе)"]
    
    Controller --> HTTPGet["HttpClient.get_messages(chat_id, limit=50)"]
    HTTPGet --> HTTPReq["GET /api/get_history?chatId=...&offset=0&limit=50"]
    HTTPReq --> Parse["Парсинг ответа → Vec&lt;Message&gt;"]
    Parse --> UpdateState["Обновление AppState.messages_by_chat[chat_id]"]
    UpdateState --> SetMsgs["ChatView.set_messages(messages)"]
    SetMsgs --> Render["Рендеринг списка сообщений в интерфейсе"]
```

### Отправка сообщения (Message Send Flow)

```mermaid
flowchart TD
    SendClick["Пользователь вводит текст и нажимает отправить"] --> EmitSend["ChatView генерирует событие send_message(chat_id, text)"]
    EmitSend --> ControllerSend["Коллбек в main.rs: controller.send_text_message(chat_id, text)"]
    ControllerSend --> HTTPSend["HttpClient.send_message(chat_id, text)"]
    HTTPSend --> HTTPPost["POST /api/send_text {'chatId':'...','text':'...'}"]
    HTTPPost --> ParseMsg["Парсинг ответа → Message"]
    ParseMsg --> UpdateStateMsg["Обновление AppState.messages_by_chat[chat_id].push(message)"]
    UpdateStateMsg --> AppendUI["ChatView.append_message(message)"]
    AppendUI --> RenderBottom["Отображение нового сообщения внизу списка"]
    RenderBottom --> Notify["Отправка системного уведомления на рабочий стол"]
```

### Авторизация OAuth (OAuth Flow)

```mermaid
flowchart TD
    Start["Запуск приложения"] --> LoadToken["AuthManager.load_token()"]
    LoadToken --> CheckToken{"Токен существует и не истек?"}
    
    CheckToken -- Да --> ControllerNew["AppController.new(auth, token)"]
    CheckToken -- Нет --> ShowDialog["AuthDialog.show()"]
    
    ShowDialog --> OpenBrowser["Открытие браузера → Страница авторизации Yandex OAuth"]
    OpenBrowser --> UserAuth["Пользователь вводит учетные данные и подтверждает вход"]
    UserAuth --> Redirect["Перенаправление на redirect_uri с #access_token=..."]
    Redirect --> ParseToken["AuthDialog.parse_token_from_url()"]
    ParseToken --> SaveToken["AuthManager.save_token(token)"]
    
    SaveToken --> ControllerNew
    ControllerNew --> ConnectRT["AppController.connect_realtime()"]
    ConnectRT --> WSConnect["WebSocketClient.connect() → WSState.Connected"]
    WSConnect --> LoadChats["AppController.load_chats()"]
    LoadChats --> RenderUI["Отрисовка главного окна интерфейса"]
```

### Обновление токена (Token Refresh Flow)

```mermaid
flowchart TD
    Expire["Срок действия AccessToken истекает (expires_in &lt;= 300)"] --> Refresh["AuthManager.refresh_token(refresh_token)"]
    Refresh --> POST["POST /token {grant_type: 'refresh_token', client_id, refresh_token}"]
    POST --> CheckSuccess{"Обновление успешно?"}
    
    CheckSuccess -- Да --> SaveDisk["Новый токен сохраняется на диск"]
    SaveDisk --> UpdateMem["Обновление AppState в памяти"]
    
    CheckSuccess -- Нет --> ShowDialog["Повторное отображение AuthDialog"]
    ShowDialog --> Reauth["Пользователь проходит авторизацию заново"]
```

### Загрузка файлов (File Upload Flow)

```mermaid
flowchart TD
    Attach["Пользователь прикрепляет файл в ChatView"] --> EmitUpload["ChatView генерирует событие upload_file(chat_id, bytes, filename)"]
    EmitUpload --> ControllerUpload["AppController.upload_file(chat_id, bytes, filename)"]
    ControllerUpload --> HTTPUpload["HttpClient.upload_file()"]
    HTTPUpload --> PUTReq["PUT https://files.messenger.yandex.net/media_upload/&lt;chatId&gt;/&lt;filename&gt;?&lt;uuid&gt;<br/>Заголовки: Authorization: OAuth &lt;token&gt;<br/>Тело: байты файла"]
    PUTReq --> ParseFileId["Парсинг ответа → fileId"]
    ParseFileId --> SendMsgFile["Отправка сообщения со ссылкой на файл через send_message()"]
    SendMsgFile --> NotifyFile["Системное уведомление: 'Файл успешно загружен'"]
```

## Управление состоянием (State Management)

В приложении используется паттерн единого разделяемого состояния (Single Shared State):

```
AppController (владеет)
    └── Arc<Mutex<AppState>>
            ├── chats: Vec<Chat>
            ├── selected_chat_id: Option<String>
            └── messages_by_chat: HashMap<String, Vec<Message>>
```

Компоненты UI получают клонированные ссылки `Arc<Mutex<AppState>>` и блокируют их для чтения при отрисовке. Изменения состояния происходят внутри асинхронных методов `AppController`, после чего UI обновляется с помощью коллбеков, зарегистрированных в `main.rs`.

Этот подход позволяет обойтись без сложных библиотек управления состоянием и обеспечивает потокобезопасный доступ благодаря `Mutex` из библиотеки Tokio.

---

## WebSocket-протокол

Клиент общается с Uniproxy Яндекса по следующему протоколу:

### Формат исходящих сообщений
```json
{
  "seq": 0,
  "message": {
    "method": "subscribe",
    "params": {"chatId": "chat-uuid"}
  }
}
```

### Поддерживаемые методы
- `subscribe` — подписка на обновления чата.
- `unsubscribe` — отписка от обновлений чата.
- `bootstrap` — запрос начального состояния приложения.

### Формат входящих сообщений
```json
{
  "seq": 0,
  "result": {...},
  "error": null
}
```

Либо в виде событий от сервера:
```json
{
  "seq": 0,
  "message": {
    "method": "message",
    "params": {"chatId": "...", "message": {...}}
  }
}
```

### Состояния подключения

```mermaid
stateDiagram-v2
    [*] --> Disconnected
    Disconnected --> Connecting : Подключение
    Connecting --> Connected : Соединение установлено
    Connecting --> Disconnected : Ошибка подключения
    Connected --> Reconnecting : Соединение потеряно
    Reconnecting --> Connected : Восстановление успешно
    Reconnecting --> Disconnected : Восстановление не удалось / Превышен лимит попыток
    Connected --> Disconnected : Закрытие / Выход из системы
```

---

## Константы конфигурации (`src/config.rs`)

| Константа | Значение | Описание |
|---|---|---|
| `OAUTH_CLIENT_ID` | `<YOUR_YANDEX_CLIENT_ID>` | OAuth Client ID приложения |
| `API_BASE_URL` | `https://yandex.ru/messenger/api/registry/api/` | Базовый REST API |
| `UNIPROXY_URL` | `wss://uniproxy.messenger.yandex.ru/uni.ws` | Точка подключения WebSocket |
| `FILE_PUBLIC_HOST` | `https://files.messenger.yandex.net` | Сервис работы с файлами |
| `TELEMOST_URL` | `https://telemost.yandex.ru` | Видеозвонки Телемост |
| `MAX_MESSAGE_LENGTH` | 4096 | Максимальная длина сообщения |
| `MAX_FILE_SIZE` | 50MB | Максимальный размер загрузки |
| `HISTORY_CHUNK_SIZE` | 50 | Сообщений на страницу истории |
| `WS_HEARTBEAT_INTERVAL` | 30s | Интервал пинга WebSocket |
| `MAX_MEMBERS_COUNT` | 1000 | Лимит участников группы |
