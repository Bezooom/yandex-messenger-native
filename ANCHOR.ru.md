# Якорное резюме — yandex-messenger-native

> Обновлено: 2026-11-14 — релиз **2.175.1**.  
> [English version](ANCHOR.md)

## Цель

Рабочий неофициальный Linux-клиент Яндекс Мессенджера (Rust + GTK4 + Libadwaita) с честными документами: фича либо работает end-to-end, либо скрыта.

## Ограничения и предпочтения

- Статус в README/CHANGELOG должен совпадать с кодом.
- Заглушки прячутся флагами: `YM_ENABLE_VOICE`, `YM_ENABLE_TELEMOST_UI` (по умолчанию выкл.).
- Проверка: `cargo build` и `cargo test --all-targets`.

## Прогресс (на 2.175.1)

### Сделано

- Список чатов из bootstrap: `members_count`, `private`, бакеты pin/mute, `members` → `Participant` с именами.
- Реальные метки времени из `create_timestamp` / `version`; превью больше не схлопываются в «сейчас».
- Непрочитанные в состоянии приложения (заголовок окна, трей и список согласованы); своё эхо игнорируется.
- Домашний экран Телемоста (новый чат / звонок / подключение по ссылке) + фильтры Все / Личные / Новые с пустыми состояниями.
- Заголовок окна как у официального клиента: «Яндекс Телемост — N новое сообщение».
- Эмодзи и стикеры — в правом доке с вкладками; просмотрщик вписывает фото в окно и реально зумит.
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

- Голос, видео и медиа Телемоста требуют фичу `gstreamer`; default и CI-бинарь её не компилируют.
- HTTP-пути встреч Телемоста — best-effort против живого сервера.
- Имена RPC действий чата — best-effort.
- Полный паритет групп/каналов, a11y, i18n шире RU-превью.

## Уровень проверки

- `cargo check --bin yandex-messenger` и `cargo test --bin yandex-messenger` проходят (65 тестов, 1 ignored).
- На этой машине есть GTK4 runtime, но нет `gtk4.pc`/заголовков; проверка шла на pkg-config из распакованных Ubuntu `.deb` в `/tmp/gtk-prefix` и `-L /tmp/ym-liblinks`. На обычной dev-машине нужен `apt install libgtk-4-dev libadwaita-1-dev libwebkitgtk-6.0-dev`.

## Дальше

- Gate B/C из [`ROADMAP_DETAILED.ru.md`](ROADMAP_DETAILED.ru.md): ежедневные медиа, затем голос/видео.
- Нативный сигналинг Telemost ([`TELEMOST_IMPLEMENTATION_PLAN.md`](TELEMOST_IMPLEMENTATION_PLAN.md)).

## Ключевые файлы

- `README.md` / `README.ru.md` — лендинги (шаблон Best-README-Template)
- `CHANGELOG.md` / `CHANGELOG.ru.md` — 2.165–2.175
- `src/api/session_store.rs`, `src/core/outbox.rs`, `src/core/drafts.rs`, `src/core/db.rs`
- `src/ui/theme.css`, `src/ui/chat_view.rs`, `src/ui/chat_list.rs`
