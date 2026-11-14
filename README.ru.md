<a id="readme-top"></a>
<!-- best-readme-template -->
<div align="center">

  <a href="https://github.com/Bezooom/yandex-messenger-native">
    <img src="assets/icon.png" alt="Logo" width="120" height="120">
  </a>
<h1 align="center">Yandex Messenger Native</h1>

<p align="center">
  Неофициальный лёгкий нативный клиент Яндекс Мессенджера для Linux — Rust, GTK4, Libadwaita.
  <br />
  <br />
  <a href="https://github.com/Bezooom/yandex-messenger-native/tree/HEAD/docs"><strong>Документация »</strong></a> · <a href="https://github.com/Bezooom/yandex-messenger-native/issues/new?labels=bug">Сообщить об ошибке</a> · <a href="https://github.com/Bezooom/yandex-messenger-native/issues/new?labels=enhancement">Предложить улучшение</a> · <a href="README.md">English</a>
</p>

<p align="center">
  <a href="https://github.com/Bezooom/yandex-messenger-native/actions/workflows/ci.yml"><img src="https://img.shields.io/github/actions/workflow/status/Bezooom/yandex-messenger-native/ci.yml?style=for-the-badge&label=CI&logo=githubactions&logoColor=white" alt="CI" /></a>
  <a href="https://github.com/Bezooom/yandex-messenger-native/releases/latest"><img src="https://img.shields.io/github/v/release/Bezooom/yandex-messenger-native?style=for-the-badge" alt="Release" /></a>
  <a href="https://github.com/Bezooom/yandex-messenger-native/blob/HEAD/LICENSE"><img src="https://img.shields.io/badge/License-MIT-blue?style=for-the-badge" alt="License" /></a>
  <a href="https://github.com/Bezooom/yandex-messenger-native/stargazers"><img src="https://img.shields.io/github/stars/Bezooom/yandex-messenger-native?style=for-the-badge" alt="Stars" /></a>
  <a href="https://github.com/Bezooom/yandex-messenger-native/commits"><img src="https://img.shields.io/github/last-commit/Bezooom/yandex-messenger-native?style=for-the-badge" alt="Last commit" /></a>
  <a href="https://github.com/Bezooom/yandex-messenger-native/issues"><img src="https://img.shields.io/github/issues/Bezooom/yandex-messenger-native?style=for-the-badge" alt="Issues" /></a>
</p>

</div>

<details open>
<summary><b>📑 Содержание</b></summary>

- [📖 О проекте](#-о-проекте)
- [Матрица статуса функций](#матрица-статуса-функций)
- [Возможности](#возможности)
- [Требования к системе](#требования-к-системе)
- [Сборка и установка](#сборка-и-установка)
- [Переменные окружения](#переменные-окружения)
- [Разработка и документация](#разработка-и-документация)
- [🗺️ Дорожная карта](#-дорожная-карта)
- [🤝 Участие в разработке](#-участие-в-разработке)
- [Лицензия](#лицензия)
- [🧱 Технологии (Built With)](#-технологии-built-with)
- [📬 Контакты](#-контакты)
- [🙏 Благодарности](#-благодарности)

</details>

## 📖 О проекте


[![CI](https://github.com/Bezooom/yandex-messenger-native/actions/workflows/ci.yml/badge.svg)](https://github.com/Bezooom/yandex-messenger-native/actions/workflows/ci.yml) [![Release](https://img.shields.io/github/v/release/Bezooom/yandex-messenger-native)](https://github.com/Bezooom/yandex-messenger-native/releases/latest) [![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

**Версия 2.175.3** — нативный Linux-клиент Яндекс Мессенджера (чаты Телемоста) на Rust, GTK4 и Libadwaita. Быстрый, лёгкий, с night-темой Telegram Desktop и плотным списком чатов в стиле nheko.

[English version](README.md)

> ⚠️ **Статус проекта: В процессе разработки / Незавершенный**
> Это открытый (open-source) проект, который на данный момент не завершен и находится в стадии активной разработки. В то время как основная логика работы с API и отправки сообщений полностью функционирует, пользовательский интерфейс (UI) требует значительной полировки, доработки и устранения визуальных багов.
> 
> Мы будем очень рады вашей помощи! Если вы интересуетесь Rust, GTK4, Libadwaita или реверс-инжинирингом API, мы с радостью примем любые вклады, сообщения об ошибках и пулл-реквесты. Ознакомьтесь с нашим [Руководством по контрибьютингу](CONTRIBUTING.ru.md) для быстрого старта!
> 
> **Важный дисклеймер**: Данный проект является **неофициальным клиентом**, созданным сообществом для Яндекс Мессенджера. Он никак не связан, не поддерживается и не ассоциирован с ООО «Яндекс» или его дочерними компаниями. Разработчики проекта не заявляют никаких прав на товарные знаки, брендинг или медиа-ресурсы Яндекса. Используйте это программное обеспечение на свой страх и риск.

> ✅ **Проверено на этом дереве (2.175.3):** `cargo check --bin yandex-messenger` чистый; `cargo test --bin yandex-messenger` → **68 passed, 1 ignored**; `cargo build --release` → бинарь 16 МБ, все `NEEDED`-библиотеки находятся. GUI и живые сетевые сценарии проверяются руками.

### Что изменилось в 2.175.3

- **Миниатюры стикеров держат размер**: асинхронный загрузчик больше не перебивает размеры чипов (32 px) и сетки (52 px), полоса паков перестала обрезаться.
- **Док реально ужимается**: вкладки категорий эмодзи прокручиваются по горизонтали, минимум `.emoji-picker` – 280 px, поэтому диапазон 300–460 px работает, а не откатывается к 360.
- **Определение своих сообщений под тестом**: `models::is_own_sender` принимает GUID или legacy numeric account id; оптимистичные правки и outbox тоже несут GUID.

### Что изменилось в 2.175.2

- **Свои сообщения остаются справа**: оптимистичная отправка теперь несёт GUID аккаунта, а пузырь сравнивает отправителя и с GUID, и с account id. Сообщения из этого клиента больше не падают в колонку входящих.
- **Док стикеров исправлен**: паки — горизонтальная полоса над сеткой, CSS-минимум 480 px убран, ячейки 64 px, сетка переносится по ширине дока, а не растягивает панель.
- **Ширина дока следует за окном** (31%, 300–460 px) вместо фиксированных 360 px.

### Что изменилось в 2.175.1

- **Эмодзи и стикеры — правый док** с вкладками «Эмодзи / Стикеры», поиском и сеткой категорий, как в Telegram, вместо поповера над композером. Открывается по `Ctrl+Shift+E`.
- **Просмотрщик картинок исправлен**: `gtk::Picture` с `ContentFit::Contain` заполняет окно вместо миниатюры; кнопки зума реально масштабируют фото.
- **Окно по умолчанию** 1180×780, чтобы список, чат и док помещались вместе.

### Что изменилось в 2.175.0

- **Список чатов из реального bootstrap**: читаются `members_count`, `private`, `pinned_chats` и `chat_mutings`; группы больше не пишут «0 участников», а `chats[].members` наполняет список участников именами и аватарами.
- **Стабильный порядок**: метки превью берутся из `create_timestamp` / `version`, а не из `Utc::now()`.
- **Один источник непрочитанных**: входящее сообщение увеличивает `state.chats[].unread_count`; заголовок окна, трей и список согласованы, эхо своих отправок игнорируется.
- **Домашний экран Телемоста**: карточки «Новый чат / звонок / подключиться по ссылке», фильтры Все / Личные / Новые с пустыми состояниями, заголовок окна «Яндекс Телемост — N новое сообщение».
- **Приватность**: полный дамп bootstrap включается только через `YM_DUMP_BOOTSTRAP=/path`.

## Матрица статуса функций

| Функция | Статус | Описание |
|---|---|---|
| OAuth / Вход | Beta | OAuth + **автозахват session cookies** в WebView (без `login_browser.py`) |
| Список чатов | Beta | Плотный список в стиле nheko, реальные счётчики участников и pin/mute из bootstrap, фильтры Все / Личные / Новые |
| Текстовые сообщения | Beta | Отправка + reply/edit + outbox + черновики + пагинация + **delivery/read ticks** |
| Файлы и вложения | Beta | Upload→send; **Скачать / Открыть**; **DnD** файлов; **Ctrl+V** картинок |
| Голосовые сообщения | Нет в default-сборке | Нужны `--features gstreamer` и `YM_ENABLE_VOICE`; default и CI-бинарь её не компилируют |
| Воспроизведение видео | Нет в default-сборке | Инлайн-плеер требует фичу `gstreamer`; в default и CI-бинаре её нет |
| Звонки (Телемост) | Частично | Сигналинг Goloom + `webrtcbin` в дереве; медиа в звонке требует `gstreamer`, `gstreamer1.0-nice` и `YM_ENABLE_TELEMOST_UI`; в default-бинаре WebRTC нет |
| Системные уведомления | Beta | `notify-rust`, mute + настройки |
| Системный трей | Beta | StatusNotifierItem (`ksni`), close→tray, badge unread |
| Настройки | Beta | Уведомления / трей / тёмная тема / уменьшить анимации |
| Оффлайн-кэш | Beta | SQLite (`cache.db`) upsert + JSON L2; cold start из SQLite |
| Тема | Beta | Светлая Яндекс-тема по умолчанию + тёмная night, переключатель в настройках |

### Флаги фич (Feature Flags)

Две новые переменные окружения управляют видимостью элементов интерфейса:

| Флаг | По умолчанию | Описание |
|---|---|---|
| `YM_ENABLE_VOICE` | `false` | Показать интерфейс голосовых сообщений (волна, кнопки воспроизведения/записи) |
| `YM_ENABLE_TELEMOST_UI` | `false` | Показать элементы интерфейса Телемоста (видеозвонки) в заголовке чата |

Когда флаг выключен, соответствующие элементы-заглушки скрываются из интерфейса, чтобы пользователь видел чистый интерфейс до полной реализации функции.

### Известные ограничения

- **Default-сборка**: голос, видео и медиа звонков вообще не компилируются; нужна фича `gstreamer`
- **Session cookies**: сохраняются при входе через WebView; fallback-скрипт `scripts/login_browser.py` всё ещё работает
- **Голосовые**: запись/плеер за `YM_ENABLE_VOICE` (нужна сборка с фичей `gstreamer`; транскрипция — только отображение, без запроса)
- **Телемост**: живой звонок за `YM_ENABLE_TELEMOST_UI` (нужна сборка с фичей `gstreamer` + пакет `gstreamer1.0-nice`); REST встреч и часть SDP-политик — best-effort, требуют live-проверки
- **Видеоплеер**: инлайн в чате (play/pause/скраб); нужен `gstreamer`
- **Chat actions API**: best-effort reverse RPC — имена методов могут отличаться на стороне сервера
- **WS status events**: зависят от payload сервера; history flags парсятся best-effort

## Возможности

### Авторизация и безопасность
* **OAuth2 протокол**: Поддержка Authorization Code и Implicit Grant flows с локальным callback-сервером.
* **Неблокирующий UI**: Интерактивный диалог входа с анимацией ожидания (`Spinner`) и таймаутом в 180 секунд.
* **Безопасное хранение**: Токены сохраняются по пути `~/.config/yandex-messenger-native/token.json` с правами доступа `0600`.
* **Автопродление**: Автоматическое обновление токенов доступа (refresh token) за 5 минут до истечения срока действия.
* **Ввод токена**: Возможность ручного ввода Access Token при необходимости.
* **Сеть и прокси**: Поддержка шифрования через `rustls` (без внешнего OpenSSL) и поддержка корпоративного режима авторизации через `YANDEX_AUTH_PROXY_URL`.

### Интерфейс пользователя (UI)
* **Адаптивный макет**: Использование `gtk::Paned` в качестве разделителя с поддержкой компактного режима сайдбара (ширина < 180px).
* **Списки чатов**: Сортировка (закрепленные чаты всегда вверху, остальные — по времени последнего сообщения), индикатор закрепления 📌 и отображение аватаров с помощью `adw::Avatar`.
* **Быстродействие**: Асинхронное кэширование аватаров в памяти (`AVATAR_CACHE`), мгновенный рендеринг сообщений из локального L2-кэша (`load_cache_l2_async`) при выборе чата, и фоновое обновление истории с сервера.
* **Night-тема**: токены Telegram Desktop night (`dialogsBg`, `msgIn` / `msgOut`), плотный сайдбар в стиле nheko, сплошное выделение, тонкие скроллбары.

### Сообщения и интерактивность
* **Редактор ввода**: Многострочное поле ввода (`TextView`) с автоматической высотой (до 120px). Отправка по нажатию `Enter`, новая строка по `Shift+Enter`.
* **Быстрое редактирование**: Нажатие стрелки `Up` при пустом поле ввода мгновенно открывает режим редактирования последнего отправленного сообщения.
* **Разделители дат**: Логические разделители сообщений по дням («Сегодня», «Вчера», даты).
* **Специальные форматы**: Отображение стикеров (128x128px) и режим "только эмодзи" (увеличенный размер без баббла при отправке 1-3 эмодзи).
* **Контекстное меню**: По клику правой кнопкой мыши доступны действия (ответ, копирование, удаление, редактирование).
* **Вложения**: выбор файла, drag-and-drop в чат, Ctrl+V для картинки из буфера, «Скачать» / «Открыть» через `xdg-open`.
* **Черновики и outbox**: текст черновика живёт при смене чата; неотправленные сообщения повторяются после reconnect.
* **Просмотр медиа**: Галерея картинок с поддержкой свайпов и скачивания в папку `Downloads`.
* **Голосовые сообщения**: Запись аудио и отрисовка waveform с использованием опционального пайплайна GStreamer.
* **Глобальный поиск**: Быстрый вызов по `Ctrl+K` с фильтрацией результатов.
* **Расширенные сущности**: Поддержка ботов (инлайн-кнопки, клавиатуры), отложенных сообщений (Message Scheduling), раздела «Избранное» (Saved Messages), групповых чатов и каналов.

---

## Требования к системе

Для сборки и запуска в Linux (Ubuntu/Debian) требуются следующие системные библиотеки:

```bash
sudo apt update
sudo apt install -y \
  build-essential pkg-config cargo \
  libgtk-4-dev libadwaita-1-dev libsqlite3-dev \
  libssl-dev libnotify-dev libdbus-1-dev libwebkitgtk-6.0-dev
```

*Примечание: для поддержки записи голосовых сообщений рекомендуется установить пакеты GStreamer (`libgstreamer1.0-dev`, `libgstreamer-plugins-base1.0-dev`). Для звонков (фича `gstreamer`: `webrtcbin`) обязателен ICE-плагин `gstreamer1.0-nice` — без него webrtcbin не создаёт транспорты и линковка `! webrtc.` падает.*

---

## Сборка и установка

### 1. Компиляция
Сборка проекта в режиме оптимизации (релизе):

```bash
cargo build --release
```

Бинарный файл будет расположен по пути `target/release/yandex-messenger`.

### 2. Сборка через Makefile (упрощенная)
Собрать и запустить проект в один шаг:

```bash
make build
make run
```

### 3. Установка в систему
Чтобы установить исполняемый файл, иконки и desktop-файл в системные директории:

```bash
sudo make install
```

Сборка локального `.deb` пакета для дистрибутивов Debian/Ubuntu:

```bash
make dist
sudo apt install -y ./dist/yandex-messenger-native_*_amd64.deb
```

---

## Переменные окружения

Клиент можно конфигурировать через переменные окружения:

| Переменная | Описание | По умолчанию |
|---|---|---|
| `YANDEX_CLIENT_ID` | OAuth client ID приложения | `<YOUR_YANDEX_CLIENT_ID>` |
| `YANDEX_CLIENT_SECRET` | OAuth client secret приложения | — |
| `YANDEX_REDIRECT_URI` | Redirect URI для callback-сервера | Автоопределение (локальный порт) |
| `YANDEX_AUTH_PROXY_URL` | URL внешнего auth-proxy | — |
| `YANDEX_OAUTH_AUTHORIZE_URL` | URL страницы авторизации | `https://oauth.yandex.com/authorize` |
| `YANDEX_OAUTH_TOKEN_URL` | URL обмена авторизационного кода | `https://oauth.yandex.com/token` |
| `RUST_LOG` | Уровень логирования приложения | `info` |

---

## Разработка и документация

* **Архитектура**: [ARCHITECTURE.ru.md](ARCHITECTURE.ru.md) · [ARCHITECTURE.md](ARCHITECTURE.md)
* **Разработка**: [DEVELOPMENT.ru.md](DEVELOPMENT.ru.md) · [DEVELOPMENT.md](DEVELOPMENT.md)
* **Безопасность**: [SECURITY.ru.md](SECURITY.ru.md) · [SECURITY.md](SECURITY.md)
* **API**: [API.ru.md](API.ru.md) · [API.md](API.md)
* **Установка**: [INSTALL.ru.md](INSTALL.ru.md) · [INSTALL.md](INSTALL.md)
* **План тестов**: [TESTPLAN.ru.md](TESTPLAN.ru.md) · [TESTPLAN.md](TESTPLAN.md)
* **История изменений**: [CHANGELOG.ru.md](CHANGELOG.ru.md) · [CHANGELOG.md](CHANGELOG.md)
* **Дорожная карта**: [ROADMAP.ru.md](ROADMAP.ru.md) · [ROADMAP.md](ROADMAP.md)
* **Детальная дорожная карта**: [ROADMAP_DETAILED.ru.md](ROADMAP_DETAILED.ru.md) · [ROADMAP_DETAILED.md](ROADMAP_DETAILED.md)
* **Контрибьютинг**: [CONTRIBUTING.ru.md](CONTRIBUTING.ru.md) · [CONTRIBUTING.md](CONTRIBUTING.md)

## 🗺️ Дорожная карта

Подробный план: [ROADMAP.ru.md](ROADMAP.ru.md). Полный список задач и идей — в [issues](https://github.com/Bezooom/yandex-messenger-native/issues).

## 🤝 Участие в разработке

Вклад в проект приветствуется. Если есть идея, как сделать лучше, откройте issue или pull request:

1. Сделайте форк репозитория
2. Создайте ветку (`git checkout -b feature/my-idea`)
3. Сделайте коммит (`git commit -m 'feat: my idea'`)
4. Отправьте ветку (`git push origin feature/my-idea`)
5. Откройте pull request

Подробные правила: [CONTRIBUTING.md](CONTRIBUTING.md).

## Лицензия

Проект распространяется под лицензией MIT. Подробнее см. [LICENSE](LICENSE).

## 🧱 Технологии (Built With)

<p>
<img src="https://img.shields.io/badge/Rust-stable-000000?style=for-the-badge&logo=rust&logoColor=white" alt="Rust" />
<img src="https://img.shields.io/badge/GTK-4-4A86CF?style=for-the-badge&logo=gtk&logoColor=white" alt="GTK" />
<img src="https://img.shields.io/badge/Libadwaita-GNOME-4A86CF?style=for-the-badge&logo=gnome&logoColor=white" alt="Libadwaita" />
<img src="https://img.shields.io/badge/GStreamer-media-FF3131?style=for-the-badge" alt="GStreamer" />
<img src="https://img.shields.io/badge/SQLite-cache-003B57?style=for-the-badge&logo=sqlite&logoColor=white" alt="SQLite" />
</p>

## 📬 Контакты

**Павел Наумов** (Bezooom) — [naumovai.ru](https://naumovai.ru) · [X @Bezooooom](https://x.com/Bezooooom) · naumov122@gmail.com

Проект: [https://github.com/Bezooom/yandex-messenger-native](https://github.com/Bezooom/yandex-messenger-native)

## 🙏 Благодарности

- [gtk-rs](https://gtk-rs.org) — Rust-привязки к GTK4
- [Libadwaita](https://gnome.pages.gitlab.gnome.org/libadwaita/)
- [nheko](https://github.com/Nheko-Reborn/nheko) и Telegram Desktop — вдохновение для интерфейса
- [Best-README-Template](https://github.com/othneildrew/Best-README-Template)
- [Shields.io](https://shields.io)

<p align="right">(<a href="#readme-top">наверх</a>)</p>
