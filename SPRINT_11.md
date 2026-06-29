# Sprint 11 — Advanced Features

**Дата формирования:** 2026-05-05  
**Текущая версия:** 2.159.0  
**Статус:** 🔄 В процессе (thread management, pin messages — ✅)

---

## Обзор Sprint 11

Реализуем 4 ключевых фичи:

| # | Фича | Приоритет | Статус |
|---|------|-----------|--------|
| 1 | Chat groups / channels | K1 | 🔲 |
| 2 | Bot support | K1 | 🔲 |
| 3 | Message scheduling | K2 | 🔲 |
| 4 | Saved messages | K3 | 🔲 |

---

## 1. Chat groups / channels

### Текущее состояние
- `ChatType` enum уже имеет варианты `Group` и `Channel` (models/mod.rs:24-29)
- `Chat.participants: Vec<Participant>` уже есть
- `Participant` имеет `status`, `last_seen`, `name`, `avatar`
- Контекстное меню чатов в chat_list.rs имеет "Закреить", "Архивировать"

### Что нужно создать

#### Модели (src/models/group.rs — новый файл)
```rust
pub struct GroupSettings {
    pub chat_id: String,
    pub admin_ids: Vec<String>,
    pub join_policy: JoinPolicy,  // Open, Request, InviteOnly
    pub invite_link: Option<String>,
    pub member_count: u32,
    pub description: Option<String>,
}

pub enum JoinPolicy {
    Open,
    Request,
    InviteOnly,
}

pub struct ChannelSettings {
    pub chat_id: String,
    pub subscribers: Vec<String>,
    pub subscriber_count: u32,
    pub is_signatory: bool,
    pub join_to_send: bool,
}

pub struct GroupMember {
    pub user_id: String,
    pub role: MemberRole,  // Member, Admin, Creator
    pub joined_at: DateTime<Utc>,
}

pub enum MemberRole {
    Member,
    Admin,
    Creator,
}
```

#### API (src/api/group.rs — новый файл)
```rust
// Методы для групп:
pub async fn create_group(&self, name: &str, members: Vec<String>)
pub async fn get_group_info(&self, chat_id: &str)
pub async fn add_member(&self, chat_id: &str, user_id: &str)
pub async fn remove_member(&self, chat_id: &str, user_id: &str)
pub async fn update_group_settings(&self, chat_id: &str, settings: GroupSettings)
pub async fn get_group_members(&self, chat_id: &str)
pub async fn join_channel(&self, chat_id: &str)
pub async fn leave_group(&self, chat_id: &str)
pub async fn invite_to_group(&self, chat_id: &str, invite_link: &str)
```

#### UI (src/ui/group_panel.rs — новый файл)
- `GroupPanel` — sidebar с информацией о группе (участники, настройки)
- `GroupMemberRow` — строка участника (имя, аватар, роль, статус)
- `CreateGroupDialog` — диалог создания группы
- `ChannelSettingsView` — настройки канала

#### UI изменения (chat_list.rs)
- Добавить иконку типа чата (👥 для групп, 📢 для каналов) в строку чата
- Обновить контекстное меню с опциями: "Создать группу", "Создать канал", "Открыть настройки"
- Фильтрация чатов по типу в search (добавить "Все", "Личные", "Группы", "Каналы")

#### WebSocket (api/mod.rs)
- Новые WS-события: `group_member_added`, `group_member_removed`, `group_updated`

### Зависимости
- Зависит от `Chat.participants` (уже есть)
- Зависит от `ChatType::Group` и `ChatType::Channel` (уже есть)

---

## 2. Bot support

### Текущее состояние
- `ChatType::Bot` уже есть
- `User.is_bot: bool` уже есть (models/mod.rs:314)
- `MessageType` имеет варианты для ботов (Reply, Forward, System)

### Что нужно создать

#### Модели (src/models/bot.rs — новый файл)
```rust
pub struct BotInfo {
    pub bot_id: String,
    pub username: String,
    pub description: Option<String>,
    pub avatar_id: Option<String>,
    pub commands: Vec<BotCommand>,
    pub can_reply: bool,
    pub inline_modes: Vec<InlineMode>,
}

pub struct BotCommand {
    pub command: String,
    pub description: String,
}

pub enum InlineMode {
    Empty,
    OnlyInPM,
    Everywhere,
}

pub struct BotMessage {
    pub message_id: String,
    pub bot_id: String,
    pub text: String,
    pub reply_markup: Option<ReplyMarkup>,
}

pub struct ReplyMarkup {
    pub keyboard: Vec<Vec<KeyboardButton>>,
    pub inline_keyboard: Vec<Vec<InlineKeyboardButton>>,
}
```

#### API (src/api/bot.rs — новый файл)
```rust
pub async fn get_bot_info(&self, bot_id: &str)
pub async fn send_bot_command(&self, bot_id: &str, command: &str, params: serde_json::Value)
pub async fn get_bot_commands(&self, bot_id: &str)
pub async fn start_bot(&self, bot_id: &str, start_param: &str)
```

#### UI (src/ui/bot_panel.rs — новый файл)
- `BotPanel` — отображение информации о боте
- `CommandList` — список команд бота
- `BotMessageBubble` — специальный стиль для сообщений от ботов
- Кнопка "Команды" в header при выборе чата с ботом

#### Изменения в chat_view.rs
- Отрисовка сообщений от ботов с иконкой бота
- Показ reply_markup (клавиатуры) в сообщении
- Inline-кнопки (reply_markup.buttons)
- Отправка команд через reply

#### Изменения в chat_list.rs
- Иконка бота (🤖) в строке чата
- Отдельная вкладка "Боты" в sidebar

#### WebSocket
- WS-событие `bot_message`
- WS-событие `bot_keyboard_update`

### Зависимости
- Зависит от `User.is_bot` (уже есть)
- Зависит от `MessageType` (уже есть)
- Зависит от `chat_view.rs` для рендера сообщений

---

## 3. Message scheduling

### Текущее состояние
- `Message.created: DateTime<Utc>` есть
- Нет поля `scheduled_at`
- Нет UI для выбора даты/времени
- `send_text_message` в api/mod.rs отправляет сразу

### Что нужно создать

#### Модели (src/models/scheduled_message.rs — новый файл)
```rust
pub struct ScheduledMessage {
    pub message_id: String,
    pub chat_id: String,
    pub scheduled_at: DateTime<Utc>,
    pub status: ScheduledStatus,  // Pending, Sending, Sent, Failed
    pub original_message: Message,
}

pub enum ScheduledStatus {
    Pending,
    Sending,
    Sent,
    Failed,
}

pub struct SendAtPicker {
    pub selected_date: Option<DateTime<Utc>>,
    pub selected_time: Option<DateTime<Utc>>,
    pub quick_presets: Vec<(String, Duration)>,  // "1 мин", "1 час", "Сегодня 18:00"
}
```

#### API (src/api/scheduled.rs — новый файл)
```rust
pub async fn schedule_message(&self, chat_id: &str, message: &str, scheduled_at: DateTime<Utc>)
pub async fn get_scheduled_messages(&self, chat_id: &str)
pub async fn cancel_scheduled_message(&self, message_id: &str)
pub async fn update_scheduled_time(&self, message_id: &str, scheduled_at: DateTime<Utc>)
```

#### UI (src/ui/scheduled_panel.rs — новый файл)
- `SendAtPopover` — popover с выбором даты и времени (вместо кнопки отправки)
- `SchedulableButton` — кнопка с двумя состояниями: "Отправить" / "Отложить"
- `ScheduledMessageList` — список запланированных сообщений

#### Изменения в chat_view.rs
- Добавить кнопку "Отложить" рядом с кнопкой отправки
- Popover с календарём и временем
- Пресеты: "Через 5 минут", "Через час", "Через день", "Завтра в 09:00"

#### Core (core.rs)
- `AppController` получает метод `schedule_message()`
- Background job (glib::timeout / tokio::spawn) для отправки запланированных
- Кэширование запланированных сообщений

#### WebSocket
- WS-событие `message_scheduled`
- WS-событие `message_send_now` (когда приходит время)

### Зависимости
- Зависит от `Message` (уже есть)
- Зависит от `chrono` (уже в Cargo.toml)
- Зависит от `chat_view.rs` для UI

---

## 4. Saved messages

### Текущее состояние
- "Сохранить в Избранное" есть в popover chat_view.rs (строка 854-860)
- Но нет отдельного чата "Saved Messages"
- Нет модели для saved messages
- Нет UI для просмотра сохранённых сообщений

### Что нужно создать

#### Модели (src/models/saved.rs — новый файл)
```rust
pub struct SavedMessage {
    pub message_id: String,
    pub chat_id: String,
    pub saved_at: DateTime<Utc>,
    pub message: Message,
    pub note: Option<String>,
}

pub enum SavedFilter {
    All,
    Text,
    Images,
    Links,
    Files,
}
```

#### API (src/api/saved.rs — новый файл)
```rust
pub async fn save_message(&self, chat_id: &str, message_id: &str, note: Option<String>)
pub async fn get_saved_messages(&self, limit: usize, filter: SavedFilter)
pub async fn unsave_message(&self, message_id: &str)
pub async fn search_saved(&self, query: &str)
pub async fn get_saved_count(&self)
pub async fn move_to_saved(&self, chat_id: &str, message_id: &str)
```

#### UI (src/ui/saved_panel.rs — новый файл)
- `SavedPanel` — основной вид сохранённых сообщений
- `SavedMessageRow` — строка с превью
- `SavedMessageView` — полноэкранное отображение сохранённого сообщения
- `SavedMessagePopover` — popover с заметкой и действиями
- Кнопка "Избранное" в sidebar

#### Изменения в chat_list.rs
- Добавить чат "Saved Messages" (special chat with id = "saved_messages")
- Отдельная иконка (📌 или 📋)
- Возможность filter по типу контента

#### Изменения в chat_view.rs
- "Сохранить в Избранное" → перемещает в SavedMessages chat
- Добавить option "Добавить заметку" при сохранении

### Зависимости
- Зависит от `Chat` (уже есть, нужно создать chat "saved_messages")
- Зависит от `Message` (уже есть)
- Зависит от chat_list.rs для sidebar

---

## Зависимости между фичами

```
┌─────────────────────────────────────────────────────────────────┐
│                        Sprint 11                                 │
│                                                                  │
│  [1] Groups/Channels     [2] Bots                              │
│       │                       │                                  │
│       └───────┬───────────────┘                                  │
│               ▼                                                  │
│         [3] Message Scheduling                                   │
│               │                                                  │
│               ▼                                                  │
│         [4] Saved Messages                                       │
│                                                                  │
│  Базовые модели (Chat, Message, ChatType) — уже есть            │
│  Базовые API (send_message, get_messages) — уже есть            │
└─────────────────────────────────────────────────────────────────┘
```

- **1 → 3**: Группы могут иметь отложенные сообщения (настройки групп)
- **2 → 3**: Боты могут принимать scheduled messages
- **1 + 2 → 4**: Saved messages может содержать сообщения из групп и от ботов
- **3 → 4**: Scheduled messages могут быть сохранены

---

## Рекомендуемый порядок реализации

### Этап 1: Saved messages (самая простая фича)
1. Создать модель `SavedMessage`
2. Создать `api/saved.rs`
3. Добавить чат "Saved Messages" в chat_list
4. Обновить "Сохранить в Избранное" в popover
5. Создать `ui/saved_panel.rs`

### Этап 2: Bot support
1. Создать модель `BotInfo`, `BotCommand`
2. Создать `api/bot.rs`
3. Обновить chat_view.rs для сообщений от ботов
4. Создать `ui/bot_panel.rs`

### Этап 3: Message scheduling
1. Создать модель `ScheduledMessage`
2. Создать `api/scheduled.rs`
3. Добавить выбор даты/времени в chat_view
4. Создать `ui/scheduled_panel.rs`
5. Background sending job

### Этап 4: Chat groups / channels (самая сложная фича)
1. Создать модели `GroupSettings`, `ChannelSettings`, `GroupMember`
2. Создать `api/group.rs`
3. Обновить chat_list.rs (иконки типов, контекстное меню)
4. Создать `ui/group_panel.rs`
5. Создать `ui/create_group_dialog.rs`
6. WS-события

---

## Детальный чек-лист

### [1] Chat groups / channels

#### Модели (models/group.rs)
- [ ] `GroupSettings` struct
- [ ] `ChannelSettings` struct
- [ ] `GroupMember` struct
- [ ] `JoinPolicy` enum
- [ ] `MemberRole` enum
- [ ] Расширить `ChatType` с `Group` и `Channel`

#### API (api/group.rs)
- [ ] `create_group()`
- [ ] `get_group_info()`
- [ ] `add_member()`
- [ ] `remove_member()`
- [ ] `update_group_settings()`
- [ ] `get_group_members()`
- [ ] `join_channel()`
- [ ] `leave_group()`
- [ ] `invite_to_group()`

#### UI (ui/group_panel.rs, ui/create_group_dialog.rs)
- [ ] `GroupPanel` — sidebar с информацией
- [ ] `GroupMemberRow` — строка участника
- [ ] `CreateGroupDialog` — диалог создания
- [ ] `ChannelSettingsView`
- [ ] Иконки типов чатов в chat_list.rs
- [ ] Обновить контекстное меню чатов

#### WebSocket
- [ ] `group_member_added`
- [ ] `group_member_removed`
- [ ] `group_updated`

---

### [2] Bot support

#### Модели (models/bot.rs)
- [ ] `BotInfo` struct
- [ ] `BotCommand` struct
- [ ] `InlineMode` enum
- [ ] `BotMessage` struct
- [ ] `ReplyMarkup` struct

#### API (api/bot.rs)
- [ ] `get_bot_info()`
- [ ] `send_bot_command()`
- [ ] `get_bot_commands()`
- [ ] `start_bot()`

#### UI (ui/bot_panel.rs)
- [ ] `BotPanel`
- [ ] `CommandList`
- [ ] `BotMessageBubble`
- [ ] Иконка бота в chat_list.rs
- [ ] Вкладка "Боты" в sidebar

#### Изменения в chat_view.rs
- [ ] Отрисовка сообщений от ботов
- [ ] Reply markup (клавиатура)
- [ ] Inline-кнопки

#### WebSocket
- [ ] `bot_message`
- [ ] `bot_keyboard_update`

---

### [3] Message scheduling

#### Модели (models/scheduled_message.rs)
- [ ] `ScheduledMessage` struct
- [ ] `SendAtPicker` struct
- [ ] `ScheduledStatus` enum

#### API (api/scheduled.rs)
- [ ] `schedule_message()`
- [ ] `get_scheduled_messages()`
- [ ] `cancel_scheduled_message()`
- [ ] `update_scheduled_time()`

#### UI (ui/scheduled_panel.rs)
- [ ] `SendAtPopover`
- [ ] `SchedulableButton`
- [ ] `ScheduledMessageList`
- [ ] Пресеты: 5 мин, 1 час, 1 день, завтра

#### Core (core.rs)
- [ ] `AppController::schedule_message()`
- [ ] Background sending job (tokio::spawn)
- [ ] Кэширование запланированных сообщений

#### Изменения в chat_view.rs
- [ ] Кнопка "Отложить"
- [ ] Выбор даты/времени

---

### [4] Saved messages

#### Модели (models/saved.rs)
- [ ] `SavedMessage` struct
- [ ] `SavedFilter` enum

#### API (api/saved.rs)
- [ ] `save_message()`
- [ ] `get_saved_messages()`
- [ ] `unsave_message()`
- [ ] `search_saved()`
- [ ] `get_saved_count()`
- [ ] `move_to_saved()`

#### UI (ui/saved_panel.rs)
- [ ] `SavedPanel`
- [ ] `SavedMessageRow`
- [ ] `SavedMessageView`
- [ ] `SavedMessagePopover`
- [ ] Кнопка "Избранное" в sidebar
- [ ] Чат "Saved Messages" в chat_list.rs

#### Изменения в chat_view.rs
- [ ] Обновить "Сохранить в Избранное"
- [ ] Добавить заметку при сохранении

---

## Технические детали

### Новые файлы
| Файл | Описание |
|------|----------|
| `src/models/group.rs` | Модели для групп и каналов |
| `src/models/bot.rs` | Модели для ботов |
| `src/models/scheduled_message.rs` | Модели для отложенных сообщений |
| `src/models/saved.rs` | Модели для сохранённых сообщений |
| `src/api/group.rs` | API для групп/каналов |
| `src/api/bot.rs` | API для ботов |
| `src/api/scheduled.rs` | API для отложенных сообщений |
| `src/api/saved.rs` | API для сохранённых сообщений |
| `src/ui/group_panel.rs` | Панель групп |
| `src/ui/create_group_dialog.rs` | Диалог создания группы |
| `src/ui/bot_panel.rs` | Панель ботов |
| `src/ui/scheduled_panel.rs` | Панель отложенных сообщений |
| `src/ui/saved_panel.rs` | Панель сохранённых сообщений |

### Обновлённые файлы
| Файл | Изменения |
|------|-----------|
| `src/models/mod.rs` | Добавить `pub use` для новых моделей |
| `src/api/mod.rs` | Добавить модули и WS-события |
| `src/ui/mod.rs` | Добавить новые UI компоненты |
| `src/ui/chat_list.rs` | Иконки типов, фильтры, saved chat |
| `src/ui/chat_view.rs` | Боты, scheduling, saved |
| `src/core.rs` | `schedule_message()` |
| `src/ui/theme.css` | Новые CSS-классы |

### CSS-классы для новых компонентов
```css
/* Groups/Channels */
.group-panel { ... }
.group-member-row { ... }
.channel-settings { ... }
.group-icon { ... }

/* Bots */
.bot-panel { ... }
.bot-message { ... }
.bot-command { ... }
.bot-icon { ... }
.reply-keyboard { ... }
.inline-button { ... }

/* Scheduling */
.scheduled-panel { ... }
.scheduled-message-row { ... }
.send-at-popover { ... }
.send-schedule-btn { ... }
.quick-preset { ... }

/* Saved */
.saved-panel { ... }
.saved-message-row { ... }
.saved-message-view { ... }
.saved-filter-bar { ... }
```

### Оценка сложности
| Фича | Сложность | Файлов |
|------|-----------|--------|
| Saved messages | 🟢 Низкая | 2 новых + 2 изменённых |
| Bot support | 🟡 Средняя | 2 новых + 2 изменённых |
| Message scheduling | 🟡 Средняя | 2 новых + 2 изменённых |
| Groups/Channels | 🔴 Высокая | 3 новых + 3 изменённых |

### Оценка времени
| Этап | Дней |
|------|------|
| Saved messages | 0.5 |
| Bot support | 1 |
| Message scheduling | 1.5 |
| Groups/Channels | 2 |
| **Итого** | **5** |

---

## Критерии приёмки

### [1] Groups/Channels
- [ ] Можно создать группу с названием и участниками
- [ ] Можно создать канал
- [ ] Можно добавлять/удалять участников группы
- [ ] Можно вступить/покинуть канал
- [ ] Видна информация о группе (участники, настройки)
- [ ] Иконки групп/каналов в chat list
- [ ] Отдельная вкладка "Группы" и "Каналы"

### [2] Bot support
- [ ] Сообщения от ботов отображаются с иконкой
- [ ] Можно отправить команду боту
- [ ] Отображается клавиатура (reply_markup)
- [ ] Inline-кнопки работают
- [ ] Панель бота с командным списком
- [ ] Отдельная вкладка "Боты"

### [3] Message scheduling
- [ ] Можно запланировать сообщение (дата + время)
- [ ] Пресеты: 5 мин, 1 час, 1 день, завтра
- [ ] Запланированные сообщения отображаются в списке
- [ ] Можно отменить запланированное сообщение
- [ ] Можно изменить время отправки
- [ ] Сообщения отправляются автоматически

### [4] Saved messages
- [ ] Можно сохранить сообщение в избранное
- [ ] Можно добавить заметку при сохранении
- [ ] Можно просмотреть все сохранённые сообщения
- [ ] Можно удалить сохранённое сообщение
- [ ] Поиск по сохранённым сообщениям
- [ ] Фильтр по типу (текст, изображения, ссылки, файлы)

---

## Риски

1. **WebSocket-события** — нужно убедиться, что Yandex API поддерживает новые события (группы, боты, scheduling)
2. **Session cookies** — методы групп могут требовать session cookies (как send_message)
3. **CSRF-токен** — обновление и кэширование
4. **GTK ListView** — виртуализация при большом количестве участников группы
5. **Таймзон** — scheduling должен учитывать часовой пояс пользователя

---

## Связанные документы

- `ROADMAP.md` — дорожная карта проекта
- `ARCHITECTURE.md` — архитектура
- `CHANGELOG.md` — история изменений
