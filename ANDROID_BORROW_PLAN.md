# План заимствования фич из Android APK

> **Источник:** `ru.yandex.telemost_25559178_rs.apk` (~250MB)
> **Цель:** Rust + GTK4 десктопный клиент Yandex Messenger
> **Дата:** 2026-04-25

---

## 0. Методология

Каждая фича оценивается по 3 параметрам:

| Критерий | Описание |
|----------|----------|
| **P** (Priority) | K1 — критично для UX, K2 — важно, K3 — nice-to-have |
| **D** (Dependency) | От чего зависит фича |
| **E** (Effort) | S — 1-2 дня, M — 3-5 дней, L — 1-2 недели, XL — 2+ недели |

---

## 1. Критичные фичи (Phase 1: MVP parity)

### 1.1 Threads / Replies (вложенные разговоры)

**P: K1 | D: API + Models | E: L**

Из APK: `threads_view`, `threads_messages_view`, `is_thread`, `parent_internal_id`

**Что берём:**
- Модель `Thread` — вложенный разговор к сообщению
- Поле `thread_id` в `Message`
- API-метод `get_thread_messages(thread_id)`
- UI: список сообщений thread-а с breadcrumb-навигацией

**Модели (новые):**
```rust
// src/models/mod.rs — добавить:
pub struct Thread {
    pub thread_id: String,
    pub chat_id: String,
    pub parent_message_id: String,
    pub reply_count: u32,
    pub last_reply_at: Option<DateTime<Utc>>,
    pub is_muted: bool,
}
```

**API (новые методы в `HttpClient`):**
```rust
// src/api/mod.rs — добавить:
async fn get_thread_messages(&self, thread_id: &str, offset: usize, limit: usize) -> Result<Vec<Message>, String>
async fn send_thread_message(&self, thread_id: &str, text: &str) -> Result<Message, String>
async fn get_thread_summary(&self, thread_id: &str) -> Result<Thread, String>
```

**UI (новые компоненты):**
- `ThreadView` — отдельный компонент для отображения thread-а
- Breadcrumb: `Chat > Thread > Message`

---

### 1.2 Stories (истории)

**P: K1 | D: API + Media | E: L**

Из APK: `stories`, `story_views`, `story_reactions`, `is_story_visible`

**Что берём:**
- Модель `Story` — временный контент с TTL
- Горизонтальный рилл-бар в верхней части чат-листа
- Просмотрщик историй в отдельном окне
- API: `get_stories`, `view_story`, `add_story_reaction`

**Модели (новые):**
```rust
pub struct Story {
    pub story_id: String,
    pub author_id: String,
    pub media_urls: Vec<String>,
    pub viewed: bool,
    pub expires_at: DateTime<Utc>,
    pub view_count: u32,
    pub reaction_count: u32,
}
```

**API (новые методы):**
```rust
async fn get_stories(&self, friend_ids: &[String]) -> Result<Vec<Story>, String>
async fn view_story(&self, story_id: &str) -> Result<(), String>
async fn add_story_reaction(&self, story_id: &str, emoji: &str) -> Result<(), String>
async fn upload_story(&self, media_url: &str, caption: &str) -> Result<Story, String>
```

---

### 1.3 Voice Messages с транскрипцией

**P: K1 | D: Audio + WS | E: M**

Из APK: `voice_record_api`, `speechkit`, `OggOpusEncoderWrapper`, `Recognizer`, `ManualStartStopAudioSource`

**Что берём:**
- Запись голосовых сообщений через `gstreamer` (Rust-экосистема)
- Отправка как `MessageType::VoiceMessage` с MIME `audio/ogg; codecs=opus`
- Бэкенд-транскрипция (SpeechKit) — текст сообщения в `transcribed_text`

**Модели (обновить):**
```rust
// MessageType::VoiceMessage — уже есть, дополнить:
pub struct VoiceMessage {
    pub url: String,
    pub duration: f64,        // секунды
    pub waveform: Vec<f32>,   // визуализация звуковой волны
    pub transcribed_text: Option<String>,
    pub is_transcribing: bool,
}
```

**API (новые методы):**
```rust
// src/api/mod.rs — добавить:
async fn upload_voice_message(&self, chat_id: &str, audio_data: &[u8]) -> Result<Message, String>
async fn get_transcription(&self, message_id: &str) -> Result<Option<String>, String>
```

**UI (новые компоненты):**
- `VoiceMessagePlayer` — плеер с waveform-визуализацией
- Кнопка записи в input-баре (long-press)

---

### 1.4 Расширенные реакции (Extended Reactions)

**P: K1 | D: WS + Models | E: S**

Из APK: `UserReaction`, `PublicReactionsConfig`, `ListReactionsRequest`, `extendedReactionsConfig`, `restrictions`

**Что берём:**
- Система кастомных реакций (не только эмодзи)
- Backend-конфиг `PublicReactionsConfig` определяет разрешённые реакции
- Группировка реакций по пользователям

**Модели (обновить):**
```rust
// src/models/mod.rs — обновить Reaction:
pub struct Reaction {
    pub emoji: String,
    pub count: u32,
    pub selected: bool,
    pub user_ids: Vec<String>,  // rename from 'users'
    pub is_extended: bool,      // кастомная реакция vs стандартный эмодзи
}

// Добавить:
pub struct ExtendedReactionsConfig {
    pub reactions: Vec<ExtendedReaction>,
    pub restrictions: ReactionRestrictions,
}

pub struct ExtendedReaction {
    pub id: String,
    pub emoji: String,
    pub category: String,
    pub is_animated: bool,
}

pub struct ReactionRestrictions {
    pub max_reactions_per_message: u32,
    pub allow_custom_emojis: bool,
    pub allow_extended: bool,
}
```

**API (новые методы):**
```rust
async fn get_reactions_config(&self) -> Result<ExtendedReactionsConfig, String>
async fn add_reaction(&self, message_id: &str, emoji: &str) -> Result<Reaction, String>
async fn remove_reaction(&self, message_id: &str, emoji: &str) -> Result<(), String>
async fn get_message_reactions(&self, message_id: &str) -> Result<Vec<Reaction>, String>
```

---

## 2. Важные фичи (Phase 2: Feature parity)

### 2.1 Chat Folders (папки чатов)

**P: K2 | D: API + Models + UI | E: M**

Из APK: `folders`, `folder_filter`, `folder_filter_cross_ref`, `MainFolderEntity`, `CustomFolderInfo`

**Что берём:**
- Систему папок чатов (встроенные + пользовательские)
- Drag & drop чатов между папками
- Фильтрация чат-листа по папкам

**Модели (новые):**
```rust
pub enum FolderType { Main, Custom }

pub struct ChatFolder {
    pub folder_id: String,
    pub name: String,
    pub folder_type: FolderType,
    pub organization_id: Option<u64>,
    pub chat_ids: Vec<String>,
    pub filters: Vec<FolderFilter>,
}

pub struct FolderFilter {
    pub filter_id: String,
    pub chat_type: Option<ChatType>,
    pub is_muted: Option<bool>,
    pub is_unread: Option<bool>,
}
```

**API (новые методы):**
```rust
async fn get_folders(&self) -> Result<Vec<ChatFolder>, String>
async fn create_folder(&self, name: &str, filters: &[FolderFilter]) -> Result<ChatFolder, String>
async fn update_folder(&self, folder_id: &str, name: &str, filters: &[FolderFilter]) -> Result<ChatFolder, String>
async fn delete_folder(&self, folder_id: &str) -> Result<(), String>
async fn move_chat_to_folder(&self, chat_id: &str, folder_id: &str) -> Result<(), String>
async fn remove_chat_from_folder(&self, chat_id: &str, folder_id: &str) -> Result<(), String>
```

**UI (новые компоненты):**
- Sidebar с разделом "Папки"
- Dropdown для создания папки с фильтром

---

### 2.2 Polls & Quizzes (опросы)

**P: K2 | D: API + Models + UI | E: M**

Из APK: `PlainMessage.Poll`, `PollInfoRequest`, `PollInfoResponse`, `pending_poll_votes`, `PollMessagesConfig`

**Что берём:**
- Создание опросов с несколькими вариантами ответа
- Анонимные/неанонимные голосования
- Результаты опроса в реальном времени
- Quiz-режим с правильным ответом

**Модели (новые):**
```rust
pub struct Poll {
    pub poll_id: String,
    pub question: String,
    pub answers: Vec<PollAnswer>,
    pub total_voters: u32,
    pub is_anonymous: bool,
    pub is_multi_select: bool,
    pub quiz_mode: bool,
    pub correct_answer_id: Option<String>,
    pub created_by: String,
    pub expires_at: Option<DateTime<Utc>>,
}

pub struct PollAnswer {
    pub answer_id: String,
    pub text: String,
    pub votes: u32,
    pub is_correct: bool,
    pub is_selected: bool,
}
```

**API (новые методы):**
```rust
async fn create_poll(&self, chat_id: &str, poll: &Poll) -> Result<Poll, String>
async fn vote_poll(&self, poll_id: &str, answer_ids: &[String]) -> Result<Poll, String>
async fn get_poll_results(&self, poll_id: &str) -> Result<Poll, String>
```

**UI (новые компоненты):**
- `PollCreator` — форма создания опроса
- `PollRenderer` — отображение опроса с результатами
- Input-bar с кнопкой "Добавить опрос"

---

### 2.3 Sticker Packs (паки стикеров)

**P: K2 | D: API + Models + UI | E: M**

Из APK: `sticker_pack_list`, `sticker_user_packs`, `sticker_panel_sticker_view`, `TextStickerPayload`

**Что берём:**
- Систему стикер-паков (каталог + пользовательские)
- Панель стикеров в input-баре
- Text stickers (стикеры из текста)
- Inline stickers в сообщениях

**Модели (новые):**
```rust
pub struct StickerPack {
    pub pack_id: String,
    pub title: String,
    pub stickers: Vec<Sticker>,
    pub is_installed: bool,
    pub is_featured: bool,
    pub category: String,
}

pub struct Sticker {
    pub sticker_id: String,
    pub pack_id: String,
    pub file_url: String,
    pub file_size: u64,
    pub emoji: String,
    pub width: u32,
    pub height: u32,
}
```

**API (новые методы):**
```rust
async fn get_sticker_catalog(&self) -> Result<Vec<StickerPack>, String>
async fn search_stickers(&self, query: &str) -> Result<Vec<StickerPack>, String>
async fn install_pack(&self, pack_id: &str) -> Result<(), String>
async fn get_sticker(&self, sticker_id: &str) -> Result<Sticker, String>
async fn send_sticker(&self, chat_id: &str, sticker_id: &str, caption: Option<&str>) -> Result<Message, String>
```

**UI (новые компоненты):**
- `StickerPanel` — панель стикеров (слайд-панель справа)
- `StickerGrid` — сетка стикеров в паке
- Sticker picker в input-баре

---

### 2.4 Bots с Inline Keyboard

**P: K2 | D: API + Models + UI | E: M**

Из APK: `AiBotAction`, `InlineKeyboardButton`, `bot_command`, `GeneratedJsonAdapter(BotRequest)`

**Что берём:**
- Bot-команды `/start`, `/help`
- Inline keyboard buttons с callback_data
- Bot actions (кнопки действий в сообщениях)
- Bot как участник чата

**Модели (обновить):**
```rust
// MessageType::Bot — дополнить:
pub struct InlineKeyboardButton {
    pub text: String,
    pub callback_data: Option<String>,
    pub url: Option<String>,
    pub web_app: Option<WebAppInfo>,
}

pub struct WebAppInfo {
    pub url: String,
}

pub struct BotCommand {
    pub command: String,
    pub description: String,
}
```

**API (новые методы):**
```rust
async fn get_bot_commands(&self, bot_id: &str) -> Result<Vec<BotCommand>, String>
async fn send_bot_callback(&self, callback_query_id: &str, text: &str) -> Result<(), String>
async fn send_bot_action(&self, chat_id: &str, bot_id: &str, action: &str, data: &str) -> Result<Message, String>
```

---

### 2.5 Custom Status с Emoji

**P: K2 | D: API + WS + UI | E: S**

Из APK: `custom_statuses`, `CustomStatusPreset`, `CustomStatusMessage`, `availability`, `notificationMode`

**Что берём:**
- Кастомный статус с emoji + текст
- Отображение статуса под аватаром пользователя
- Уведомления при смене статуса (notification_mode)

**Модели (обновить):**
```rust
// User — дополнить:
pub struct CustomStatus {
    pub status_id: String,
    pub emoji: String,
    pub text: String,
    pub availability: StatusAvailability, // online, away, offline
    pub notification_mode: NotificationMode,
    pub expires_at: Option<DateTime<Utc>>,
}

pub enum StatusAvailability { Online, Away, Offline }
pub enum NotificationMode { Normal, Quiet }
```

**API (новые методы):**
```rust
async fn get_custom_status(&self) -> Result<Option<CustomStatus>, String>
async fn set_custom_status(&self, emoji: &str, text: &str) -> Result<(), String>
async fn clear_custom_status(&self) -> Result<(), String>
```

---

### 2.6 Message Translation (перевод в чате)

**P: K2 | D: API + UI | E: S**

Из APK: `translated_lang`, `translated_text`, `forcedTranslatedText`, `canTranslate`, `TRANSLATOR_FORCE_TRANSLATE_CHAT`

**Что берём:**
- Кнопка "Перевести" на сообщении
- Автоперевод (если включено в настройках)
- `translated_text` — бэкенд-перевод в модели сообщения

**Модели (обновить):**
```rust
// Message — дополнить:
pub struct Message {
    // ... existing fields ...
    pub translated_text: Option<String>,
    pub original_lang: Option<String>,
    pub translated_lang: String,
}
```

**API (новые методы):**
```rust
async fn translate_message(&self, message_id: &str, target_lang: &str) -> Result<String, String>
async fn get_translation_config(&self) -> Result<TranslationConfig, String>
async fn set_translation_config(&self, auto_translate: bool, target_lang: &str) -> Result<(), String>
```

---

### 2.7 Calendar Integration (события в чате)

**P: K2 | D: API + Models + UI | E: M**

Из APK: `CalendarEventNotification`, `CalendarSearchEventApiDTO`, `MeetingRoomInfo`, `telemost_schedule_meeting`

**Что берём:**
- Создание событий календаря из чата
- Отображение событий в контексте чата
- Интеграция с Telemost (групповые звонки)

**Модели (новые):**
```rust
pub struct CalendarEvent {
    pub event_id: String,
    pub title: String,
    pub description: Option<String>,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub organizer_id: String,
    pub attendees: Vec<Attendee>,
    pub chat_id: Option<String>,
    pub telemost_link: Option<String>,
}

pub struct Attendee {
    pub user_id: String,
    pub status: AttendeeStatus,
}

pub enum AttendeeStatus { Pending, Accepted, Declined, Tentative }
```

**API (новые методы):**
```rust
async fn create_calendar_event(&self, event: &CalendarEvent) -> Result<CalendarEvent, String>
async fn get_calendar_events(&self, chat_id: &str) -> Result<Vec<CalendarEvent>, String>
async fn schedule_meeting(&self, chat_id: &str, start_time: DateTime<Utc>, duration_min: u32) -> Result<CalendarEvent, String>
```

---

## 3. Nice-to-have фичи (Phase 3: Polish)

### 3.1 Smart Suggestions / AI Reply

**P: K3 | D: ML (Quasar) + API | E: L**

Из APK: `smart_suggestions`, `AI_ASSISTANT`, `auto_complete`, `reply_suggestions`

**Что берём:**
- Подсказки для ответа при вводе текста
- Автозаполнение на основе контекста чата
- AI-powered suggestions

---

### 3.2 Screen Sharing (в звонках)

**P: K3 | D: Telemost (WebRTC) | E: M**

Из APK: `screen_share`, `screen_recording`, `On dirait qu'il n'y a pas d'Internet`, `Somebody else is sharing their screen`

**Что берём:**
- Screen sharing в Telemost-звонках
- Индикатор "кто-то делится экраном"
- Кнопка screen share в панели звонка

---

### 3.3 Doc Scanner / QR

**P: K3 | D: Camera + UI | E: M**

Из APK: `DocScannerTrackerResultView`, `DocScannerImageScanContainer`, `qr_code_scan`, `mt_doc_scanner_image_zoomable_layout`

**Что берём:**
- Сканирование QR-кодов через веб-камеру
- Сканирование документов
- Отправка отсканированных данных в чат

---

### 3.4 Yandex Disk Integration

**P: K3 | D: API (Disk API) | E: M**

Из APK: `https://disk.yandex.{TLD}/client`, `mt_doc_scanner_image_save_ydisk_item`

**Что берём:**
- Выбор файлов из Yandex Disk в чате
- Отправка файлов через Yandex Disk
- Превью файлов из Disk

---

### 3.5 Reminders в чате

**P: K3 | D: API + UI | E: S**

Из APK: `RemindersSyncPayload`, `alice_reminders_notification_channel`, `RemindersBroadcastReceiver`

**Что берём:**
- Создание напоминаний из чата
- Remind me об этой сообщении
- Уведомления по таймеру

---

### 3.6 Animated Emoji (AEO)

**P: K3 | D: Media + UI | E: S**

Из APK: `AnimatedEmoji`, `emoji2-bundled`, `EmojiDrawable`, `customEmoji`

**Что берём:**
- Рендеринг анимированных эмодзи в сообщениях
- Inline анимированные эмодзи (не стикеры)

---

## 4. WebSocket-сообщения (из APK)

### Новые типы WS-сообщений для подписки:

| Метод | Описание |
|-------|----------|
| `subscribe_thread` | Подписка на thread |
| `subscribe_story_views` | Подписка на просмотры историй |
| `subscribe_reaction_updates` | Обновления реакций |
| `subscribe_typing_enhanced` | Расширенные индикаторы набора |
| `subscribe_poll_updates` | Обновления опросов |
| `subscribe_calendar` | События календаря |
| `subscribe_custom_status` | Смена статусов |

### Новые типы WS-сообщений для отправки:

| Метод | Описание |
|-------|----------|
| `add_reaction` | Добавить реакцию |
| `remove_reaction` | Убрать реакцию |
| `send_voice` | Отправить голосовое |
| `send_sticker` | Отправить стикер |
| `send_poll_vote` | Голос в опросе |
| `send_thread_message` | Сообщение в thread |
| `view_story` | Просмотр истории |
| `set_custom_status` | Установить кастомный статус |
| `send_inline_callback` | Callback inline-кнопки |
| `send_typing_enhanced` | Расширенный индикатор набора |

---

## 5. Конфигурация (из APK)

### Добавить в `src/config.rs`:

```rust
// Stories
pub const MAX_STORY_DURATION: u32 = 60; // seconds
pub const STORY_TTL_HOURS: u32 = 24;
pub const MAX_STORIES_PER_AUTHOR: u32 = 100;

// Voice messages
pub const MAX_VOICE_DURATION: u32 = 600; // 10 minutes
pub const VOICE_SAMPLE_RATE: u32 = 16000;
pub const VOICE_BITRATE: u32 = 128000;

// Stickers
pub const MAX_STICKER_SIZE: u64 = 5_242_880; // 5MB
pub const MAX_STICKER_DIMENSION: u32 = 512;

// Reactions
pub const MAX_REACTIONS_PER_MESSAGE: u32 = 10;

// Translation
pub const SUPPORTED_LANGUAGES: &[&str] = &[
    "ru", "en", "de", "fr", "es", "it", "tr", "zh", "ja", "ko", "ar", "hi"
];

// Calendar
pub const TELEMOST_MEETING_DURATION_DEFAULT: u32 = 60; // minutes
```

---

## 6. План реализации (по спринтам)

### Спринт 1: Threads + Reactions (2 недели)
- [ ] Модели Thread, ExtendedReaction
- [ ] API: get_thread_messages, send_thread_message, add_reaction
- [ ] UI: ThreadView, ReactionPanel
- [ ] WS: subscribe_thread, add_reaction

### Спринт 2: Voice Messages (1.5 недели)
- [ ] Модель VoiceMessage с waveform
- [ ] API: upload_voice_message
- [ ] UI: VoiceMessagePlayer, record button
- [ ] GStreamer интеграция для записи

### Спринт 3: Polls (1 неделя)
- [ ] Модель Poll
- [ ] API: create_poll, vote_poll, get_results
- [ ] UI: PollCreator, PollRenderer
- [ ] WS: subscribe_poll_updates

### Спринт 4: Stickers (1.5 недели)
- [ ] Модель StickerPack, Sticker
- [ ] API: get_catalog, search, send_sticker
- [ ] UI: StickerPanel, StickerGrid
- [ ] Inline stickers в сообщениях

### Спринт 5: Folders + Translation (1.5 недели)
- [ ] Модель ChatFolder, FolderFilter
- [ ] API: get_folders, create_folder, move_chat
- [ ] UI: Folder sidebar, drag-drop
- [ ] API: translate_message, set_config
- [ ] UI: Translate button in messages

### Спринт 6: Bots + Custom Status + Calendar (2 недели)
- [ ] Inline keyboard model + UI
- [ ] CustomStatus model + UI
- [ ] CalendarEvent model + API
- [ ] Meeting scheduler

### Спринт 7: Polish (Animated Emoji, Screen Share, Scanner)
- [ ] Animated Emoji renderer
- [ ] Screen share in Telemost
- [ ] QR/Doc scanner
- [ ] Disk integration

---

## 7. Зависимости (Cargo.toml)

### Новые зависимости:
```toml
# Voice recording
gstreamer = "0.22"
gstreamer-audio = "0.22"
gstreamer-app = "0.22"

# Image processing (sticker rendering)
image = "0.25"

# QR code scanning
qrcode = "0.14"

# Audio waveform generation
rubato = "0.15"  # Resampling for waveform
```

---

## 8. Структура файлов (новые)

```
src/
├── models/
│   ├── thread.rs       # Thread model
│   ├── story.rs        # Story model
│   ├── poll.rs         # Poll model
│   ├── sticker.rs      # Sticker pack models
│   ├── calendar.rs     # Calendar event model
│   └── mod.rs          # Обновить
├── api/
│   ├── threads.rs      # Thread API methods
│   ├── stories.rs      # Story API methods
│   ├── polls.rs        # Poll API methods
│   ├── stickers.rs     # Sticker API methods
│   ├── translation.rs  # Translation API methods
│   ├── calendar.rs     # Calendar API methods
│   └── mod.rs          # Обновить
└── ui/
    ├── thread_view.rs  # Thread view component
    ├── sticker_panel.rs # Sticker picker panel
    ├── poll_creator.rs  # Poll creation form
    ├── poll_renderer.rs # Poll display component
    ├── story_viewer.rs  # Story viewer window
    ├── folder_sidebar.rs # Folders sidebar
    └── mod.rs           # Обновить
```

---

## 9. Итого

| Фича | Приоритет | Зависимость | Сложность | Спринт |
|------|-----------|-------------|-----------|--------|
| Threads/Replies | K1 | API | L | 1 |
| Stories | K1 | API + Media | L | 1 |
| Voice Messages | K1 | Audio | M | 2 |
| Extended Reactions | K1 | WS + Models | S | 1 |
| Chat Folders | K2 | API + UI | M | 5 |
| Polls | K2 | API + UI | M | 3 |
| Stickers | K2 | API + UI | M | 4 |
| Bots (Inline) | K2 | API + UI | M | 6 |
| Custom Status | K2 | API + WS + UI | S | 6 |
| Translation | K2 | API + UI | S | 5 |
| Calendar | K2 | API + UI | M | 6 |
| Smart Suggestions | K3 | ML + API | L | — |
| Screen Share | K3 | Telemost | M | 7 |
| Doc Scanner/QR | K3 | Camera | M | 7 |
| Disk Integration | K3 | API | M | 7 |
| Reminders | K3 | API + UI | S | 7 |
| Animated Emoji | K3 | Media + UI | S | 7 |

**Итого:** ~10 недель на полную реализацию Phase 1-3.
