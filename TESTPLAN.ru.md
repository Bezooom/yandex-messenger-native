# План тестирования

[English version](TESTPLAN.md)

## Текущий статус (2.173.0)

| Фаза | Статус |
|---|---|
| 0 — Sedimentation | Завершена |
| 1 — API Integration | Завершена (session login, файлы, reply/edit) |
| 2 — Звонки / Telemost | Только оболочка — WebRTC всё ещё заглушка |
| Desktop trust | Уведомления, трей, настройки, тики |

**Тесты:** `cargo test --all-targets` (13 unit/smoke на последней проверке).

---

## 1. Статические проверки

```bash
cargo fmt --check
cargo clippy --all-targets
cargo test --all-targets
```

## 2. Дымовой запуск

```bash
make run
```

Проверить:

- открытие окна с night-темой и разделением в стиле nheko;
- авторизацию через OAuth (WebView): после входа появляется `~/.config/yandex-messenger-native/session.json` с `Session_id`;
- загрузку списка чатов; выбор чата показывает историю и автоскролл к последнему сообщению;
- outbox: отключить сеть → отправить текст → bubble pending → сеть обратно → сообщение уходит (или после reconnect);
- пагинация: длинный чат → scroll up → подгружается старая история; виден индикатор «Загрузка истории…»;
- drafts: набрать текст → сменить чат → вернуться → текст на месте;
- DnD: перетащить файл в чат → отправка; Ctrl+V картинки;
- вложение: кнопки «Скачать» / «Открыть» → файл в Downloads;
- cold start offline: список чатов/история из SQLite после предыдущей сессии;
- UX: при старте skeleton → list; без выбранного чата — welcome; пустой чат — empty conversation;
- поиск без результатов — empty «Ничего не найдено»;
- реакция → chip с pop-in; настройки → «Уменьшить анимации» гасит shimmer/pop-in;
- открытие чата прокручивает к последнему сообщению.

## 3. Сообщения

- отправка текста через кнопку и Enter;
- отображение отправленного сообщения в списке;
- отсутствие падений при переключении чатов;
- reply: peer видит quote (session/WS);
- edit: peer видит изменённый текст;
- mark as read: badge падает при открытии чата;
- context menu: mute/pin/archive обновляют UI (API best-effort).

## 4. Файлы

- attach action вызывается и уходит в upload pipeline;
- download API возвращает байты;
- «Скачать» / «Открыть» работают через `xdg-open`;
- при ошибке выводится корректное уведомление.

## 5. Звонки

- клик по call action открывает окно Telemost;
- кнопка End закрывает окно без ошибок;
- встроенный WebView (feature `in_app_webview`) загружает страницу Telemost;
- кнопки Mute / Video переключают состояние и визуал;
- fallback: без `in_app_webview` открывается диалог с кнопкой «Open in browser»;
- нативный WebRTC в 2.173.0 **не ожидается**.

## 6. Поведение на рабочем столе

- уведомления через `notify-rust` (не в активном том же чате; mute уважается);
- night-тема применяется при `dark_theme = true`;
- close behavior: при `minimize_to_tray` окно скрывается, трей остаётся;
- tray: «Показать» / «Выход», tooltip с unread;
- настройки: уведомления / трей / тёмная тема / уменьшить анимации.

## 7. Пакеты и CI

- `debuild -us -uc` завершается успешно;
- workflow `.github/workflows/ci.yml` проходит на чистом runner;
- `make dist` собирает `yandex-messenger-native_2.173.0-*`.
