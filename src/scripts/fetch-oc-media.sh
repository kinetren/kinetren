#!/usr/bin/env bash
# Настоящий фильм для кросс-рендера (тест 179, предложение пользователя:
# «взять какой-то фильм у которого исходники поменьше весят и тоже
# прорендерить двумя программами»).
#
# Netflix Open Content «Sol Levante», аниматик: 63 склейки на одной
# дорожке, 23.976 fps, старт таймкода с часа, 162 с, один исходник H.264.
# Ровно тот случай, которого не было в синтетике: NTSC-частота, настоящий
# декод, реальные длины планов (7…347 кадров).
#
# НИЧЕГО НЕ КОММИТИТСЯ: медиа 320 МБ, а у репозитория с .otio нет лицензии
# (проверено через GitHub API). Поэтому оба файла качаются сюда, каталог в
# .gitignore, а тест без них молча пропускается.
#
# Лицензии:
#   видео  — Netflix Open Content, CC BY 4.0 (https://opencontent.netflix.com/)
#   .otio  — darbyjohnston/otio-oc-examples, лицензия не указана; файл
#            используется локально как вход теста и в репозиторий не попадает.
set -euo pipefail

dir="$(cd "$(dirname "$0")/.." && pwd)/fixtures/otio-oc"
mkdir -p "$dir"

OTIO_URL=https://raw.githubusercontent.com/darbyjohnston/otio-oc-examples/main/SolLevante_Animatics_ver1_H264.otio
MOV_URL=https://download.opencontent.netflix.com/SolLevante/working_assets/SolLevante_Animatics_ver1_H264.mov
otio="$dir/SolLevante_Animatics_ver1_H264.otio"
mov="$dir/SolLevante_Animatics_ver1_H264.mov"

if [ ! -s "$otio" ]; then
    echo "качаю монтаж (~105 КБ)…"
    curl -fL --progress-bar "$OTIO_URL" -o "$otio.tmp" && mv "$otio.tmp" "$otio"
fi

# уже скачанный файл можно просто указать аргументом — 320 МБ не копируем,
# кладём симлинк (тест читает его как обычный файл)
if [ $# -ge 1 ] && [ -s "$1" ] && [ ! -e "$mov" ]; then
    ln -s "$(cd "$(dirname "$1")" && pwd)/$(basename "$1")" "$mov"
    echo "исходник взят по ссылке: $1"
fi

if [ ! -s "$mov" ]; then
    echo "качаю исходник (~320 МБ)…"
    if ! curl -fL --progress-bar "$MOV_URL" -o "$mov.tmp"; then
        rm -f "$mov.tmp"
        echo "прямая ссылка недоступна; пробую aws cli…" >&2
        aws s3 cp --no-sign-request \
            "s3://download.opencontent.netflix.com/SolLevante/working_assets/SolLevante_Animatics_ver1_H264.mov" \
            "$mov.tmp"
    fi
    mv "$mov.tmp" "$mov"
fi

ls -lh "$otio" "$mov"
echo
echo "Sol Levante © Netflix, CC BY 4.0 — https://opencontent.netflix.com/"
echo "готово: cargo test -p kt-cli --test oc_render"
