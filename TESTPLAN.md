# Test Plan

## 1. Static checks

```bash
cargo fmt --check
cargo clippy --all-targets
cargo test --all-targets
```

## 2. Smoke run

```bash
make run
```

Проверить:
- открытие окна приложения;
- авторизацию через OAuth URL;
- загрузку списка чатов;
- выбор чата и отображение истории.

## 3. Messaging flow

- отправка текста через кнопку и Enter;
- отображение отправленного сообщения в списке;
- отсутствие падений при переключении чатов.

## 4. File flow

- attach action вызывается и уходит в upload pipeline;
- download API возвращает байты;
- при ошибке выводится корректное уведомление.

## 5. Calls

- клик по call action открывает окно Telemost;
- кнопка End закрывает окно без ошибок.

## 6. Desktop behavior

- уведомления показываются через `notify-rust`;
- dark theme применяется при `dark_theme = true`;
- close behavior соответствует `minimize_to_tray`.

## 7. Packaging and CI

- `debuild -us -uc` завершается успешно;
- workflow `.github/workflows/ci.yml` проходит на чистом runner.
