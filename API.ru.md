# API Яндекс Мессенджера — Спецификация Реверс-Инжиниринга

В данном документе собрана техническая спецификация API Яндекс Мессенджера (включая веб-версию и мобильное приложение Телемост `ru.yandex.telemost`), полученная в результате реверс-инжиниринга и анализа сетевого трафика.

---

## 1. Анализ APK и Структура Приложения

Анализ мобильного приложения проводился на основе APK-файла `ru.yandex.telemost`.

### Результаты декомпиляции
1. **Java/Kotlin (jadx)**: Исходный код извлечен в директорию `decompile_out`. Позволяет анализировать логику работы приложения, алгоритмы авторизации, бизнес-логику взаимодействия с API и структуру данных.
2. **Smali (apktool)**: Байткод и ресурсы извлечены в `apktool_out`. Используется для внесения изменений (патчинга) непосредственно в smali-код с последующей пересборкой.

### Структура пакетов
* **Имя пакета**: `ru.yandex.telemost`
* **Application Class**: `com.yandex.messenger.MainApplication`
* **Основная Activity**: `com.yandex.messenger.LauncherActivity` / `com.yandex.messaging.activity.MessengerActivity`
* **Ключевые модули**:
  - `com.yandex.messaging` — логика чатов, WebSocket-клиент, кэширование.
  - `com.yandex.passport` — модуль интеграции с Яндекс ID (аутентификация).

---

## 2. Базовые Эндпоинты API

Приложение взаимодействует со следующими хостами Яндекса:

| Сервис | URL | Описание |
|---|---|---|
| **Messenger API** | `https://api.messenger.yandex.net` | Основное REST API для работы с чатами и пользователями |
| **Messenger Registry** | `https://yandex.ru/messenger/api/registry/api/` | Базовый URL для некоторых веб-клиентов |
| **Uniproxy WS** | `wss://uniproxy.messenger.yandex.ru/uni.ws` | WebSocket-сервер реального времени (Xiva) |
| **Yandex Files** | `https://files.messenger.yandex.net` | Хранилище медиафайлов и документов |
| **Yandex Passport** | `https://mobileproxy.passport.yandex.net` | Аутентификация и работа с токенами |
| **Yandex Telemost** | `https://telemost.yandex.ru` | Видеозвонки и конференции |

---

## 3. Протокол HTTP API

Все REST-запросы требуют авторизации и соблюдения правил безопасности Яндекса.

### 3.1. Аутентификация и Заголовки
Все запросы к API мессенджера должны содержать заголовок авторизации OAuth:
```http
Authorization: OAuth <access_token>
```

Для выполнения мутирующих запросов (отправка сообщений, создание чатов) предварительно требуется получить CSRF-токен:
```http
GET /csrf-token/
```
Полученный токен передается в заголовке `X-CSRF-Token`.

### 3.2. Основные HTTP-методы

#### Получение списка чатов
```http
GET /api/get_chat_list?offset=<offset>&limit=<limit>
```
* **Параметры**:
  - `offset` (int) — смещение для пагинации.
  - `limit` (int) — количество возвращаемых чатов.
* **Формат ответа**:
```json
{
  "chats": [
    {
      "id": "chat-uuid",
      "title": "Название чата",
      "type": "private", 
      "unreadCount": 0,
      "pinned": false,
      "lastMessage": {
        "id": "msg-uuid",
        "text": "Текст сообщения",
        "timestamp": 1717012345
      }
    }
  ]
}
```

#### Получение истории сообщений
```http
GET /api/get_history?chatId=<chatId>&offset=<offset>&limit=<limit>
```
* **Параметры**:
  - `chatId` (string) — уникальный идентификатор чата.
  - `offset` (int) — смещение.
  - `limit` (int) — лимит сообщений.
* **Формат ответа**:
```json
{
  "messages": [
    {
      "id": "msg-uuid",
      "chatId": "chat-uuid",
      "fromId": "user-uuid",
      "text": "Привет!",
      "timestamp": 1717012300,
      "status": "read"
    }
  ]
}
```

#### Отправка текстового сообщения
```http
POST /api/send_text
Content-Type: application/json

{
  "chatId": "chat-uuid",
  "text": "Текст сообщения"
}
```
* **Формат ответа**: Объект созданного сообщения с UUID и меткой времени.

#### Загрузка медиафайлов
Загрузка файлов происходит напрямую на сервера Yandex Files:
```http
PUT /media_upload/<chatId>/<filename>?<uuid>
Authorization: OAuth <token>
Content-Type: <mime-type>

[file_bytes]
```
* **Ответ**:
```json
{
  "fileId": "unique-file-identifier",
  "size": 12345,
  "mimeType": "image/png"
}
```

#### Скачивание файлов
```http
GET /file_shortterm/<fileId>
Authorization: OAuth <token>
```
Возвращает бинарное содержимое файла с соответствующими заголовками `Content-Type` и `Content-Length`.

---

## 4. WebSocket-протокол (Uniproxy / Xiva)

Для обеспечения работы в реальном времени (получение уведомлений о новых сообщениях, статусах прочтения, процессе набора текста) используется WebSocket-соединение с сервисом Uniproxy.

* **Эндпоинт**: `wss://uniproxy.messenger.yandex.ru/uni.ws`

### 4.1. Формат исходящих сообщений
Каждое сообщение, отправляемое клиентом, оборачивается в конверт с уникальным монотонно возрастающим идентификатором `seq`:
```json
{
  "seq": 1,
  "message": {
    "method": "METHOD_NAME",
    "params": {
      "param1": "value1"
    }
  }
}
```

### 4.2. Основные методы WebSocket

#### Подписка на обновления чата (`subscribe`)
Позволяет получать сообщения, статусы набора текста и реакции в реальном времени.
```json
{
  "seq": 2,
  "message": {
    "method": "subscribe",
    "params": {
      "chatId": "chat-uuid"
    }
  }
}
```

#### Отмена подписки (`unsubscribe`)
```json
{
  "seq": 3,
  "message": {
    "method": "unsubscribe",
    "params": {
      "chatId": "chat-uuid"
    }
  }
}
```

#### Первичная синхронизация (`bootstrap`)
Запрос состояния при подключении.
```json
{
  "seq": 4,
  "message": {
    "method": "bootstrap",
    "params": {
      "with_deleted": true,
      "compact": false
    }
  }
}
```

### 4.3. Формат входящих сообщений

#### Ответ на запрос клиента
```json
{
  "seq": 2,
  "result": {
    "status": "ok"
  },
  "error": null
}
```

#### Асинхронное событие от сервера (Event/Notification)
```json
{
  "seq": 0,
  "message": {
    "method": "message",
    "params": {
      "chatId": "chat-uuid",
      "message": {
        "id": "msg-uuid",
        "fromId": "user-uuid",
        "text": "Новое сообщение в реальном времени",
        "timestamp": 1717012400
      }
    }
  }
}
```

---

## 5. Векторы модификации приложения (Форк)

При создании форков или анализе безопасности приложения Яндекса используются следующие подходы:

1. **Перенаправление API (API Redirection)**: Поиск базовых доменных имен (например, `api.messenger.yandex.net`) в Smali-коде и их замена на кастомные адреса серверов для работы с собственной инфраструктурой.
2. **Отключение аналитики**: Локализация и удаление вызовов SDK AppMetrica и Amplitude для повышения приватности пользователей.
3. **Обход SSL Pinning**: Модификация файла конфигурации сетевой безопасности (`network_security_config.xml` в ресурсах `res/xml/`) для разрешения доверия к пользовательским (пользовательским CA) сертификатам, что позволяет перехватывать трафик через прокси (например, Charles, Burp Suite).
