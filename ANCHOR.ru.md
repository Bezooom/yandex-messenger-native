# Якорное резюме — yandex-messenger-native

> Обновлено: 2026-08-15 — релиз **2.173.0**.  
> [English version](ANCHOR.md)

## Цель

Рабочий неофициальный Linux-клиент Яндекс Мессенджера (Rust + GTK4 + Libadwaita) с честными документами: фича либо работает end-to-end, либо скрыта.

## Ограничения и предпочтения

- Статус в README/CHANGELOG должен совпадать с кодом.
- Заглушки прячутся флагами: `YM_ENABLE_VOICE`, `YM_ENABLE_TELEMOST_UI` (по умолчанию выкл.).
- Проверка: `cargo build` и `cargo test --all-targets`.

## Прогресс (на 2.173.0)

### Сделано

- Session внутри login (WebView → `session.json` + CSRF), Python на happy path не нужен.
- Outbox (`outbox.json`) и черновики (`drafts.json`).
- Пагинация истории, SQLite-кэш (`cache.db`) с cold start.
- Файлы: upload/send, Скачать/Открыть, DnD, Ctrl+V картинок.
- Reply/edit, тики доставки/прочтения, mark-as-read, mute/pin/archive/delete в UI.
- Системные уведомления (`notify-rust`) и трей (`ksni`).
- Настройки: уведомления, трей, тёмная тема, уменьшить анимации.
- UX: skeleton, empty states, pop-in реакций, loader пагинации.
- Night-тема (токены Telegram Desktop) + плотный список в стиле nheko.

### Ещё заглушки / открыто

- Голосовые сообщения и видеоплеер.
- Telemost WebRTC (только оболочка + WebView).
- Имена RPC действий чата — best-effort.
- Полный паритет групп/каналов, a11y, i18n шире RU-превью.

## Дальше

- Gate B/C из [`ROADMAP_DETAILED.ru.md`](ROADMAP_DETAILED.ru.md): ежедневные медиа, затем голос/видео.
- Нативный сигналинг Telemost ([`TELEMOST_IMPLEMENTATION_PLAN.md`](TELEMOST_IMPLEMENTATION_PLAN.md)).

## Ключевые файлы

- `README.md` / `README.ru.md` — матрица функций
- `CHANGELOG.md` / `CHANGELOG.ru.md` — 2.165–2.173
- `src/api/session_store.rs`, `src/core/outbox.rs`, `src/core/drafts.rs`, `src/core/db.rs`
- `src/ui/theme.css`, `src/ui/chat_view.rs`, `src/ui/chat_list.rs`
