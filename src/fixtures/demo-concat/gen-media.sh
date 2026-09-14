#!/usr/bin/env bash
# Медиа демо-проектов конкатенации (Р.44/Р.45): генерируется СИСТЕМНЫМ
# ffmpeg, в репозиторий не коммитится. testsrc2 — мелкая фактура (текст,
# клетки, градиенты), на которой мыло/резкость видны сразу.
set -euo pipefail
cd "$(dirname "$0")"
ffmpeg -y -v error -f lavfi -i testsrc2=size=1280x720:rate=30 -t 12 \
  -pix_fmt yuv420p -c:v libx264 -crf 16 detail.mp4
# правая половина — чистый зелёный (lime = 00FF00) под greenscreen
ffmpeg -y -v error -f lavfi -i testsrc2=size=1280x720:rate=30 -t 12 \
  -vf "drawbox=x=640:y=0:w=640:h=720:color=lime@1:t=fill" \
  -pix_fmt yuv420p -c:v libx264 -crf 16 green.mp4
# Р.46: тот же материал в 2560x1440 — вчетверо больше пикселей, чем в
# проекте 1280x720. На нём видно ступень декода: зум за 100 % берёт
# детали ФАЙЛА, а не мыло из кадра, ужатого в Quality ещё при декоде.
ffmpeg -y -v error -f lavfi -i testsrc2=size=2560x1440:rate=30 -t 12 \
  -pix_fmt yuv420p -c:v libx264 -crf 16 detail-hi.mp4
echo "ok: detail.mp4 green.mp4 detail-hi.mp4"
