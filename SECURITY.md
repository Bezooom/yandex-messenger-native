# Yandex Messenger Native — Security

## Overview

This document covers security considerations for the Yandex Messenger Native
desktop client, including authentication, token storage, API communication,
file handling, and WebSocket protocol.

## Authentication

### OAuth2 Authorization Code Flow

The application uses OAuth2 Authorization Code Flow for desktop applications:

```
Client                          Yandex OAuth                    Browser
  │                               │                              │
  │── GET /authorize?             │                              │
│   response_type=code          │                              │
│   + client_id=...             │                              │
│   + state=<uuid>              │                              │
│   + device_id=<uuid>          │                              │
│   + device_name=...           │                              │
│   + force_confirm=yes         │                              │
│──▶                              │                              │
│                               │──▶ User authenticates        │
│                               │◀──┤                          │
│                               │                              │
│◀── Redirect (302) ───────────│                              │
│   #access_token=...           │                              │
│   + refresh_token=...         │                              │
│   + expires_in=3600           │                              │
│                               │                              │
  │── Parse fragment → OAuthToken                              │
  │── POST /token (if code flow)                               │
  │   grant_type=refresh_token                                 │
  │   + client_id + refresh_token                              │
  │◀── New access_token ──────────────────────────────────────│
```

### Security Properties

- **State parameter**: UUID generated per auth request, prevents CSRF on auth
- **Device ID**: UUID sent to identify the client device
- **force_confirm=yes**: Requires explicit user confirmation
- **No scope sent**: Avoids invalid_scope errors; permissions controlled by OAuth app
- **Two-token strategy**: access_token (short-lived) + refresh_token (long-lived)

### Token Exchange

The client supports two token exchange strategies, tried in order:

1. **Basic Auth** (preferred): `Authorization: Basic <base64(client_id:client_secret)>`
2. **Form body**: `client_id` + `client_secret` in POST body

Both strategies iterate over configured token URLs, supporting failover between
`oauth.yandex.com` and `oauth.yandex.ru`.

### Auth Proxy Mode

When `YANDEX_AUTH_PROXY_URL` is set:

```
Client         Auth Proxy              Yandex OAuth
  │                │                      │
  │── GET /oauth/start ──▶ Redirect to OAuth ──▶
  │◀── OAuth callback ──────┤                      │
  │── POST /oauth/exchange ──┤                     │
│   {"code": "..."}         │                      │
│◀── {"access_token":...} ──┤                      │
```

The proxy handles token storage, reducing client-side secrets exposure.

## Token Storage

### Location and Format

- **Path**: `~/.config/yandex-messenger-native/token.json`
- **Format**: Pretty-printed JSON via `serde_json::to_string_pretty`
- **Content**:
  ```json
  {
    "access_token": "yaA...",
    "refresh_token": "AQ...",
    "expires_in": 3600,
    "token_type": "Bearer",
    "user_id": "..."
  }
  ```

### Security Measures

- **Memory caching**: Token kept in `Arc<Mutex<Option<OAuthToken>>>` after loading
- **Expiry check**: Tokens considered expired 5 minutes before actual expiry (300s)
- **Auto-refresh**: Silent refresh using refresh_token when near expiry
- **On-disk write**: `fs::write` for atomic token persistence
- **Cleanup on logout**: Both in-memory and disk token removed

### In-Memory Protection

```
AuthManager {
    token: Arc<Mutex<Option<OAuthToken>>>  // Protected by tokio Mutex
    data_dir: PathBuf                       // Config directory path
}
```

The tokio Mutex ensures thread-safe access from both async API methods and
sync UI callbacks (via `block_on`).

## HTTPS / TLS Communication

- **Library**: `reqwest` with `rustls-tls` feature (no OpenSSL dependency)
- **Certificate validation**: Full rustls certificate chain verification
- **HSTS**: Implicit via rustls default trusted root store
- **Protocol**: TLS 1.2+ for all HTTP endpoints
- **WebSocket**: WSS (WebSocket Secure) for Uniproxy connection

## CSRF Protection

The client fetches a CSRF token from the API before state-changing operations:

```
GET /csrf-token/ → {"token": "csrf-string"}
POST /api/send_text
    Headers:
      Authorization: OAuth <token>
      Content-Type: application/json
    Body: {"chatId": "...", "text": "..."}
```

The CSRF token is included in the `X-CSRF-Token` header (standard Yandex API convention).

## File Upload Security

### Upload Process

1. File data read into memory (`Vec<u8>`)
2. UUID appended to upload URL for uniqueness
3. PUT request with `Content-Type: application/octet-stream`
4. Response contains `fileId` for reference in messages

### Limits

- **Max file size**: 50 MB (`MAX_FILE_SIZE` constant)
- **Max files per message**: 30 (`MAX_FILE_UPLOAD_COUNT` constant)
- **Max message length**: 4096 characters (`MAX_MESSAGE_LENGTH` constant)

### Download Security

- Signed short-term URLs: `https://files.messenger.yandex.net/file_shortterm/<fileId>`
- Authorization header included for authenticated access
- Response read as raw bytes, no format validation

## WebSocket Security

### Connection

- **Endpoint**: `wss://uniproxy.messenger.yandex.ru/uni.ws`
- **Protocol**: WSS (WebSocket over TLS)
- **Authentication**: Token validated by server on connection (via prior HTTP auth)

### Message Integrity

- **Sequence numbers**: Monotonically increasing counter prevents duplicate processing
- **Method validation**: Only known methods accepted (`subscribe`, `unsubscribe`, `bootstrap`)
- **Error responses**: Server errors include `code`, `message`, and optional `details`

### Reconnection

- **Max attempts**: 10 (`WS_MAX_RECONNECT_ATTEMPTS`)
- **Interval**: 11 seconds (`WS_RECONNECT_INTERVAL`)
- **State tracking**: `WSState` enum with `Reconnecting(n)` counter

## Environment Variables Security

| Variable | Sensitive | Stored | Default |
|---|---|---|---|
| `YANDEX_CLIENT_ID` | Low | Env | `<YOUR_YANDEX_CLIENT_ID>` |
| `YANDEX_CLIENT_SECRET` | High | Env | — |
| `YANDEX_REDIRECT_URI` | Low | Env | `""` |
| `YANDEX_AUTH_PROXY_URL` | Medium | Env | — |
| `YANDEX_OAUTH_AUTHORIZE_URL` | Low | Env | `https://oauth.yandex.com/authorize` |
| `YANDEX_OAUTH_TOKEN_URL` | Low | Env | `https://oauth.yandex.com/token` |
| `YANDEX_FORCE_AUTH` | Low | Env | `""` |
| `RUST_LOG` | Low | Env | `info` |

**Sensitive variables** (`CLIENT_SECRET`) are never written to disk.

## Threat Model Summary

### Assets

| Asset | Location | Protection |
|---|---|---|
| Access token | Memory + disk | Expiry check, auto-refresh |
| Refresh token | Memory + disk | Stored with access token |
| OAuth secret | Environment | Never persisted |
| Chat messages | Memory (HashMap) | In-process only |
| User settings | Disk (JSON) | Standard file permissions |
| WebSocket connection | Network (WSS) | TLS encryption |

### Threats and Mitigations

| Threat | Impact | Mitigation |
|---|---|---|
| Token theft from disk | Send messages as user | Stored in config dir (chmod 600), encrypted at rest pending |
| Token replay | Send messages until expiry | Short-lived access tokens (1h), auto-refresh |
| Man-in-the-middle | Intercept messages | TLS 1.2+ for all connections |
| CSRF | Inject messages | CSRF token per session |
| OAuth redirect hijack | Steal auth code | state parameter validation |
| WS hijack | Receive messages from other user | Token-bound session |
| File upload overflow | DoS via large file | 50 MB limit enforced |
| Memory exposure | Token in /proc/<pid>/maps | Short-lived tokens, cleared on logout |

### Attack Surface

1. **Browser**: OAuth redirect URL parsing (fragment extraction)
2. **Disk**: `token.json` file
3. **Network**: HTTP API + WSS endpoints
4. **Environment**: Client ID, secret, proxy URL
5. **UI**: WebView (in_app_webview feature) — potential XSS from Telemost

### Future Improvements

- [ ] Encrypt token.json with device-specific key
- [ ] Pin TLS certificates for API endpoints
- [ ] Validate all WebSocket message schemas
- [ ] Implement rate limiting for file uploads
- [ ] Add certificate transparency validation
- [ ] Sanitize file types on upload
- [ ] Implement token rotation on each refresh
