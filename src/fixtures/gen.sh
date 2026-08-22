#!/usr/bin/env bash
# Генерация тестовых фикстур (conformance-тест 10, golden-тест 11).
# Детерминированно: фиксированные генераторы ffmpeg, без времени/рандома.
# Запись атомарна (tmp + mv) — параллельный читатель не увидит недописанный файл.
set -euo pipefail
cd "$(dirname "$0")"
mkdir -p out

gen() { # gen <target> <ffmpeg args...>
    local target="$1"; shift
    [ -f "out/$target" ] && return 0
    ffmpeg -v error -y "$@" "out/.tmp.$target"
    mv "out/.tmp.$target" "out/$target"
}

# 2 c синус 440 Гц, 44.1k mono AAC (проверка ресемпла и лосси-якоря)
gen sine44k_mono.m4a -f lavfi -i "sine=frequency=440:sample_rate=44100:duration=2" \
    -c:a aac -b:a 128k -f mp4

# 2 c синус 440 Гц, 48k stereo FLAC (lossless: sample-accuracy проверяется точно)
gen sine48k_stereo.flac -f lavfi -i "sine=frequency=440:sample_rate=48000:duration=2" \
    -af "pan=stereo|c0=c0|c1=c0" -c:a flac -f flac

# 2 c testsrc 30 fps H.264 (MT-декодер, backward seek)
gen testsrc_h264.mp4 -f lavfi -i "testsrc=size=320x180:rate=30:duration=2" \
    -c:v libx264 -preset veryfast -g 15 -pix_fmt yuv420p -f mp4

# 2 c testsrc VP9 (threads=1 путь)
gen testsrc_vp9.webm -f lavfi -i "testsrc=size=320x180:rate=30:duration=2" \
    -c:v libvpx-vp9 -cpu-used 8 -g 15 -f webm

# 2 c муксованный A/V (h264 + aac 48k stereo)
gen muxed_av.mp4 \
    -f lavfi -i "testsrc=size=320x180:rate=30:duration=2" \
    -f lavfi -i "sine=frequency=440:sample_rate=48000:duration=2" \
    -af "pan=stereo|c0=c0|c1=c0" \
    -c:v libx264 -preset veryfast -g 15 -pix_fmt yuv420p -c:a aac -shortest -f mp4

echo ok
