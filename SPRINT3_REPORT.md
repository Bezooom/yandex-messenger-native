# Отчёт Спринт 3: Polls

**Дата:** 2026-04-27  
**Приоритет:** K1 — критично для UX  
**Статус:** ✅ Завершён

---

## Выполнено

### 1. Модели (src/models/)

| Файл | Описание |
|------|----------|
| `models/poll.rs` | `Poll` с `PollAnswer` — вопросы, варианты, голоса, quiz mode, multi-select |
| `models/mod.rs` | Экспорт Poll, PollAnswer, добавлено `poll: Option<Poll>` в Message |

### 2. API (src/api/mod.rs)

| Метод | Описание |
|-------|----------|
| `create_poll()` | POST на `api/create_poll` с question, answers, settings |
| `vote_poll()` | POST на `api/vote_poll` с poll_id, answer_ids |
| `get_poll_results()` | GET на `api/poll_results?pollId=...` |
| `subscribe_poll_updates()` | WS подписка на обновления опроса |
| `send_poll_vote_ws()` | WS отправка голоса |

### 3. UI (src/ui/)

| Файл | Описание |
|------|----------|
| `ui/poll_creator.rs` | PollCreator — форма создания (вопрос, варианты, quiz mode, anonymity) |
| `ui/poll_renderer.rs` | PollRenderer — отображение опроса с прогресс-барами и кнопками голосования |
| `ui/chat_view.rs` | Интеграция: poll_btn, poll_popover, render_poll_message, update_poll |
| `ui/mod.rs` | Экспорт PollCreator, PollRenderer |
| `ui/theme.css` | CSS: .poll-creator, .poll-renderer, .poll-question, .poll-answer-row, .poll-progress-bar, .poll-quiz-correct/wrong, dark mode |

### 4. CSS стили

- `.poll-creator` — контейнер формы создания
- `.poll-question` — заголовок опроса
- `.poll-answer-row` — строка варианта ответа
- `.poll-progress-bar` — прогресс-бар с процентами
- `.poll-quiz-correct` / `.poll-quiz-wrong` — подсветка в режиме викторины
- `.poll-vote-btn` — кнопка голосования
- Dark mode для всех компонентов

---

## Файлы изменены/созданы

| Файл | Действие |
|------|----------|
| `src/models/poll.rs` | Создан |
| `src/models/mod.rs` | Обновлён (Poll, PollAnswer) |
| `src/api/mod.rs` | Обновлён (create_poll, vote_poll, get_poll_results, WS подписки) |
| `src/ui/poll_creator.rs` | Создан (полная реализация) |
| `src/ui/poll_renderer.rs` | Создан (полная реализация + poll_id()) |
| `src/ui/chat_view.rs` | Обновлён (poll_btn, poll_popover, render_poll_message, update_poll) |
| `src/ui/mod.rs` | Обновлён |
| `src/ui/theme.css` | Обновлён (poll CSS) |

---

## Компиляция

- **Новых ошибок:** 0
- **Предсуществующих ошибок:** 22 (chat_list.rs, auth.rs, tray.rs) — не затронуты
- **Замечания:** `render_messages()` вызывается из `add_message()` но не определена (pre-existing)

---

## Следующий спринт: Stickers (Sprint 4)

- Модель Sticker + StickerPack
- API: get_stickers, download_sticker
- UI: StickerPackList, StickerPanel (popover), animation
- CSS: sticker animations, dark mode
