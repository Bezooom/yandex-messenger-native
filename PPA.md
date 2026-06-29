# PPA Release Notes

Проект подготовлен к публикации в PPA. Рекомендуемый процесс:

1. Обновить `debian/changelog` под release-версию.
2. Прогнать pre-release проверки:
   - `cargo fmt --check`
   - `cargo clippy --all-targets`
   - `cargo test --all-targets`
3. Собрать source package:
   - `debuild -S -sa`
4. Загрузить в Launchpad:
   - `dput ppa:<team>/<ppa-name> ../yandex-messenger-native_<version>_source.changes`
5. Проверить публикацию и smoke-install на Ubuntu LTS.
6. Обновить `CHANGELOG.md` и релизные заметки.
