# План развития мессенджера

## Спринт 1: Enterprise UI (тема, чат-лист, чат)
- **1.1** Глобальная тема и CSS — вынести `theme.css`, анимации, hover-эффекты
- **1.2** Chat List Panel — hover, skeleton, контекстное меню, drag-to-reorder
- **1.3** Chat View — улучшенные bubble, hover-actions, inline image viewer
- **1.4** Message Input — toolbar (жирный, курсив, код, эмодзи), emoji picker, auto-resize
- **1.5** Settings Panel — SettingsWindow с категориями

## Спринт 2: Real-time и отправка сообщений
- **2.1** WebSocket — подключить subscribe, reconnect, heartbeat
- **2.2** Typing Indicator — индикатор "печатает..."
- **2.3** Online Status — real-time обновление статусов
- **2.4** Message Sending — session cookies, retry, file upload, queue
- **2.5** Reply & Edit — UI для ответов и редактирования

## Спринт 3: Media, уведомления, полировка
- **3.1** Image Viewer — zoom overlay, download
- **3.2** Voice Recording — GStreamer, waveform
- **3.3** Notifications — GIO/libnotify
- **3.4** System Tray — иконка, minimize
- **3.5** Search — within chat, highlight
- **3.6** Keyboard — shortcuts, navigation
- **3.7** Performance — virtualization, lazy loading
