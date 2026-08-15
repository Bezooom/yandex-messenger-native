# Yandex Messenger API — Reverse Engineering Specification

[Русская версия](API.ru.md)

This document contains the technical specification of the Yandex Messenger API (including the web client and the Telemost mobile application `ru.yandex.telemost`), obtained through reverse engineering and network traffic analysis. Client release **2.173.0** uses these endpoints via session cookies + OAuth.

---

## 1. APK Analysis & Application Structure

The mobile application analysis was performed on the APK file `ru.yandex.telemost`.

### Decompilation Results
1. **Java/Kotlin (jadx)**: Source code decompiled into the `decompile_out` directory. This allows analyzing application flow, authentication algorithms, business logic, and API data structures.
2. **Smali (apktool)**: Bytecode and resources extracted to `apktool_out`. Used to apply changes (patching) directly in smali code and reassemble the APK.

### Package Structure
* **Package Name**: `ru.yandex.telemost`
* **Application Class**: `com.yandex.messenger.MainApplication`
* **Main Activity**: `com.yandex.messenger.LauncherActivity` / `com.yandex.messaging.activity.MessengerActivity`
* **Key Modules**:
  - `com.yandex.messaging` — chat logic, WebSocket client, caching.
  - `com.yandex.passport` — Yandex ID integration module (authentication).

---

## 2. Base API Endpoints

The application communicates with the following Yandex hosts:

| Service | URL | Description |
|---|---|---|
| **Messenger API** | `https://api.messenger.yandex.net` | Main REST API for chats and users |
| **Messenger Registry** | `https://yandex.ru/messenger/api/registry/api/` | Base URL for some web clients |
| **Uniproxy WS** | `wss://uniproxy.messenger.yandex.ru/uni.ws` | Real-time WebSocket server (Xiva) |
| **Yandex Files** | `https://files.messenger.yandex.net` | Storage for media files and attachments |
| **Yandex Passport** | `https://mobileproxy.passport.yandex.net` | Authentication and token operations |
| **Yandex Telemost** | `https://telemost.yandex.ru` | Video calls and conferences |

---

## 3. HTTP API Protocol

All REST requests require authentication and compliance with Yandex security mechanisms.

### 3.1. Authentication and Headers
All requests to the Messenger API must contain the OAuth authorization header:
```http
Authorization: OAuth <access_token>
```

For mutating requests (sending messages, creating chats), a CSRF token must be obtained beforehand:
```http
GET /csrf-token/
```
The token received is passed in the `X-CSRF-Token` header.

### 3.2. Main HTTP Methods

#### Get Chat List
```http
GET /api/get_chat_list?offset=<offset>&limit=<limit>
```
* **Parameters**:
  - `offset` (int) — offset for pagination.
  - `limit` (int) — number of chats to return.
* **Response Format**:
```json
{
  "chats": [
    {
      "id": "chat-uuid",
      "title": "Chat Title",
      "type": "private", 
      "unreadCount": 0,
      "pinned": false,
      "lastMessage": {
        "id": "msg-uuid",
        "text": "Message text",
        "timestamp": 1717012345
      }
    }
  ]
}
```

#### Get Message History
```http
GET /api/get_history?chatId=<chatId>&offset=<offset>&limit=<limit>
```
* **Parameters**:
  - `chatId` (string) — unique chat identifier.
  - `offset` (int) — offset.
  - `limit` (int) — message limit.
* **Response Format**:
```json
{
  "messages": [
    {
      "id": "msg-uuid",
      "chatId": "chat-uuid",
      "fromId": "user-uuid",
      "text": "Hello!",
      "timestamp": 1717012300,
      "status": "read"
    }
  ]
}
```

#### Send Text Message
```http
POST /api/send_text
Content-Type: application/json

{
  "chatId": "chat-uuid",
  "text": "Message text"
}
```
* **Response Format**: Created message object with UUID and timestamp.

#### Media Upload
Files are uploaded directly to Yandex Files servers:
```http
PUT /media_upload/<chatId>/<filename>?<uuid>
Authorization: OAuth <token>
Content-Type: <mime-type>

[file_bytes]
```
* **Response**:
```json
{
  "fileId": "unique-file-identifier",
  "size": 12345,
  "mimeType": "image/png"
}
```

#### Download File
```http
GET /file_shortterm/<fileId>
Authorization: OAuth <token>
```
Returns the binary content of the file with the corresponding `Content-Type` and `Content-Length` headers.

---

## 4. WebSocket Protocol (Uniproxy / Xiva)

To support real-time features (receiving new messages, read status updates, typing indicators, reactions), a WebSocket connection is established with the Uniproxy service.

* **Endpoint**: `wss://uniproxy.messenger.yandex.ru/uni.ws`

### 4.1. Outgoing Message Format
Every message sent by the client is wrapped in an envelope containing a unique, monotonically increasing `seq` identifier:
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

### 4.2. Main WebSocket Methods

#### Subscribe to Chat Updates (`subscribe`)
Enables receiving messages, typing status, and reactions in real-time.
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

#### Unsubscribe (`unsubscribe`)
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

#### Initial Sync (`bootstrap`)
Request application state upon connection.
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

### 4.3. Incoming Message Format

#### Client Request Response
```json
{
  "seq": 2,
  "result": {
    "status": "ok"
  },
  "error": null
}
```

#### Server Event / Notification
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
        "text": "New message in real-time",
        "timestamp": 1717012400
      }
    }
  }
}
```

---

## 5. Application Modification (Forking Vectors)

When creating forks or analyzing Yandex app security, the following approaches are commonly used:

1. **API Redirection**: Searching for base domain names (e.g., `api.messenger.yandex.net`) in the Smali code and replacing them with custom server addresses to work with your own backend infrastructure.
2. **Disabling Analytics**: Locating and removing SDK calls to AppMetrica and Amplitude to increase user privacy.
3. **Bypassing SSL Pinning**: Modifying the network security configuration file (`network_security_config.xml` in resources `res/xml/`) to trust user-defined CA certificates, enabling traffic interception via proxy tools (e.g., Charles, Burp Suite).
