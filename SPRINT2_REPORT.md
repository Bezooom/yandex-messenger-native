# Отчёт Спринт 2: Voice Messages

**Дата:** 2026-04-25  
**Приоритет:** K1 — критично для UX  
**Статус:** ✅ Завершён

---

## Выполнено

### 1. Модели (src/models/)

| Файл | Описание |
|------|----------|
| `models/voice_message.rs` | `VoiceMessage`, `TranscribeStatus`, `VoiceRecordParams`, `VoiceFormat` |
| `models/mod.rs` | Экспорт VoiceMessage, добавлено `waveform: Option<Vec<f32>>` в MediaAttachment |

### 2. API (src/api/mod.rs)

| Метод | Описание |
|-------|----------|
| `upload_voice_message()` | POST на `api/upload_voice` с audio data + waveform |
| `get_transcription()` | GET на `api/get_transcription?messageId=...` |

### 3. Core (src/core/)

| Файл | Описание |
|------|----------|
| `core/voice_recorder.rs` | VoiceRecorder — stub с таймером, waveform, duration tracking |

### 4. UI (src/ui/)

| Файл | Описание |
|------|----------|
| `ui/voice_message_player.rs` | VoiceMessagePlayer — play/pause, progress bar, waveform, transcription, download, reply |
| `ui/chat_view.rs` | Интеграция VoiceMessagePlayer в render_message(), VoiceRecorder в struct |
| `ui/mod.rs` | Экспорт VoiceRecordParams |
| `ui/theme.css` | CSS: .voice-player, .voice-play-btn, .voice-progress, .waveform-container, .transcription-box, dark mode |

### 5. Конфигурация (src/config.rs)

| Константа | Значение |
|-----------|----------|
| `MAX_VOICE_DURATION` | 600 секунд (10 минут) |
| `VOICE_SAMPLE_RATE` | 16000 Hz |
| `VOICE_BITRATE` | 64000 kbps |
| `VOICE_MAX_FILE_SIZE` | ~5MB |

---

## Файлы изменены/созданы

| Файл | Действие |
|------|----------|
| `src/models/voice_message.rs` | Создан |
| `src/models/mod.rs` | Обновлён (waveform в MediaAttachment) |
| `src/api/mod.rs` | Обновлён (upload_voice, get_transcription) |
| `src/core/voice_recorder.rs` | Создан (stub, no GStreamer dependency) |
| `src/ui/voice_message_player.rs` | Создан |
| `src/ui/chat_view.rs` | Обновлён (VoiceMessagePlayer integration) |
| `src/ui/mod.rs` | Обновлён |
| `src/ui/theme.css` | Обновлён |
| `src/config.rs` | Обновлён |
| `Cargo.toml` | Обновлён (uuid 1.10, indexmap pin, getrandom 0.2) |

---

## Компиляция

- **Новых ошибок:** 0
- **Предсуществующих ошибок:** 22 (chat_list.rs, auth.rs, tray.rs) — не затронуты
- **Предупреждения:** 7 (unused imports api/mod.rs — будут устранены при активном использовании)

---

## Следующий спринт: Polls

- Модель Poll с вариантами ответов
- API: create_poll, vote_poll, get_results
- UI: PollCreator, PollRenderer
- WS: subscribe_poll_updates
