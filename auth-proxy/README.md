# Yandex Auth Proxy

Минимальный backend proxy для централизованной OAuth-авторизации.

## Зачем

- `client_secret` хранится на сервере, а не на клиентских машинах.
- Пользователям desktop-клиента не нужно регистрировать OAuth app вручную.

## Environment

- `YANDEX_CLIENT_ID` (required)
- `YANDEX_CLIENT_SECRET` (required)
- `YANDEX_REDIRECT_URI` (required) — callback URL, который зарегистрирован в Yandex OAuth для вашего app.
- `AUTH_PROXY_BIND` (optional, default `127.0.0.1:8080`)
- `YANDEX_OAUTH_SCOPES` (optional)

## Run

```bash
cd auth-proxy
export YANDEX_CLIENT_ID='...'
export YANDEX_CLIENT_SECRET='...'
export YANDEX_REDIRECT_URI='https://your-domain.example/oauth/callback'
cargo run --release
```

## Endpoints

- `GET /health`
- `GET /oauth/start` — redirect на Yandex OAuth authorize
- `GET /oauth/callback` — показывает код подтверждения для вставки в desktop app
- `POST /oauth/exchange` — обмен code на token

Body:

```json
{"code":"<authorization_code>"}
```
