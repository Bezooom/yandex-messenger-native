# PITFALLS

Формат: `симптом | корень | как проверить заранее`.

- Стикер уходит JSON-текстом в официальный клиент | `Sticker{FileInfo}` в любом составе сервер молча выбрасывает, единственный рабочий формат – `Image{FileInfo{Id2: "stickers/images/…"}}` | зонд `send_sticker_message` + скриншот официального клиента; не изобретать формат по docs.
- Разбитые картинки стикеров в перечитанной истории | `shortterm_url` строит `/file_shortterm/stickers/…`, а `stickers/`-id – прямой путь CDN (`{host}/{id}?size=large`), отдаёт 404 | перед построением URL проверять префикс id: `stickers/` vs `disk/`.
- Фоновые процессы инструмента получают SIGKILL (137) на загрузке тяжёлых весов | cgroup инструмента ограничен 4 ГБ | запускать тяжёлое через `systemd-run --user --unit=… -p MemoryMax=infinity`.
- Финальная сборка deb не находит `libsoup-3.0`/`webkit2gtk-5.0` при `make dist` | pkg-config пути только в `/tmp/gtk-prefix` | экспортировать `PKG_CONFIG_PATH=/tmp/gtk-prefix/usr/lib/x86_64-linux-gnu/pkgconfig RUSTFLAGS="-L /tmp/ym-liblinks"` (проверено live).
