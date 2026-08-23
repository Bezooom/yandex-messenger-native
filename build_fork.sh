#!/bin/bash

# Скрипт для сборки и подписи форка Yandex Messenger

# 1. Сборка через apktool
echo "=> Собираем APK с помощью apktool..."
java -jar apktool.jar b apktool_out -o fork_unsigned.apk

if [ $? -ne 0 ]; then
    echo "Ошибка сборки apktool!"
    exit 1
fi

# 2. Выравнивание APK (zipalign)
echo "=> Выравниваем APK..."
# Пытаемся найти zipalign в системе
ZIPALIGN=$(command -v zipalign)
if [ -z "$ZIPALIGN" ]; then
    # Если zipalign не установлен, попробуем использовать его из Android SDK, если он там есть
    # Но для скрипта просто установим заглушку или попросим пользователя установить
    echo "Внимание: zipalign не найден в PATH. Установите zipalign (sudo apt install zipalign)."
    echo "Пропускаем шаг zipalign (может не установиться на новые версии Android)."
    cp fork_unsigned.apk fork_aligned.apk
else
    $ZIPALIGN -p -f 4 fork_unsigned.apk fork_aligned.apk
fi

# 3. Подпись APK (apksigner)
# Keystore НЕ хранится в репозитории. Создайте свой:
#   keytool -genkeypair -v -keystore fork_keystore.jks -alias fork_alias \
#     -keyalg RSA -keysize 2048 -validity 10000
echo "=> Подписываем APK..."
if [ ! -f fork_keystore.jks ]; then
    echo "Ошибка: fork_keystore.jks не найден. Сгенерируйте локально (см. комментарий выше) и держите вне git."
    exit 1
fi
read -rsp "Пароль keystore: " KS_PASS; echo
APKSIGNER=$(command -v apksigner)
if [ -z "$APKSIGNER" ]; then
    echo "Внимание: apksigner не найден в PATH. Установите apksigner (sudo apt install apksigner)."
    echo "Пытаемся использовать jarsigner как запасной вариант..."
    jarsigner -verbose -sigalg SHA256withRSA -digestalg SHA-256 -keystore fork_keystore.jks -storepass "$KS_PASS" -keypass "$KS_PASS" fork_aligned.apk fork_alias
    mv fork_aligned.apk fork_signed.apk
else
    $APKSIGNER sign --ks fork_keystore.jks --ks-pass pass:"$KS_PASS" --key-pass pass:"$KS_PASS" --out fork_signed.apk fork_aligned.apk
fi

echo "=> Готово! Ваш форк собран: fork_signed.apk"
