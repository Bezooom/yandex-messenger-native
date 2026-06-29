# Sprint 6 Checklist — Yandex Messenger

**Спринт:** 6  
**Период:** май 2026  
**Приоритет:** K1  
**Статус:** ⏳ В процессе

---

## ✅ Реализовано

### 1. ImageViewer (`src/ui/image_viewer.rs`)
- [x] ImageViewer struct с ScrolledWindow
- [x] show(url: &str) — загрузка по URL
- [x] zoom_in/zoom_out/reset_zoom
- [x] controls_popover с кнопками (zoom, close)
- [x] close/is_closed
- [x] Rc-based для безопасных closures

### 2. Inline Image Preview
- [x] MediaType::Image detection в message
- [x] Thumbnail отображение в bubble
- [x] GestureClick для клика
- [x] CSS: `.inline-image` с hover

### 3. Translate Button
- [x] Hover visibility
- [x] on_translate callback
- [x] Использует существующий translate_message API

### 4. FolderSidebar Integration
- [x] В main.rs
- [x] get_folders API call
- [x] set_folders callback

### 5. Status Indicators
- [x] set_typing(user)
- [x] set_online()
- [x] set_status_text(text)

### 6. CSS Styles
- [x] `.inline-image` — max-width: 400px
- [x] `.image-viewer` — overlay background
- [x] `.image-controls` — buttons hover/active
- [x] `.viewer-image`

### 7. Bug Fixes
- [x] SelectionModelExt — GTK4 v4_12
- [x] Cargo.toml — gtk v4_12 feature
- [x] ImageViewer — borrow errors

---

## 🔄 В процессе

### Voice Recording (GStreamer)
- [x] GStreamer pipeline setup
- [x] Real-time waveform
- [x] Voice playback with waveform
- [x] Voice transcription (SpeechKit)

### Image Enhancements
- [x] Image download
- [x] Swipe navigation
- [x] Image compression

---

## 📊 Метрики спринта

| Метрика | Значение |
|---------|----------|
| Новых файлов | 2 (image_viewer.rs, обновлён chat_view.rs) |
| Обновлённых файлов | 2 (main.rs, theme.css) |
| API методов | 1 (get_folders) |
| CSS классов | 5+ |
| Ошибок исправлено | 3 (SelectionModelExt, borrow, types) |
| Статус | 100% завершено |

---

## 🚀 Следующие шаги

1. GStreamer integration для голосовых сообщений
2. Image download functionality
3. Swipe navigation между изображениями
4. Polish: hover effects, transitions
5. Testing & QA
