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

# 2 c testsrc 25 fps H.264 (п.14 р.14: fps источника ≠ fps проекта —
# только на такой фикстуре видна разница floor/ceil при выборке кадра;
# на 30 fps обе ветки вырождаются в один и тот же кадр)
gen testsrc_25fps.mp4 -f lavfi -i "testsrc=size=320x180:rate=25:duration=2" \
    -c:v libx264 -preset veryfast -g 13 -pix_fmt yuv420p -f mp4

# 2 c testsrc VP9 (threads=1 путь)
gen testsrc_vp9.webm -f lavfi -i "testsrc=size=320x180:rate=30:duration=2" \
    -c:v libvpx-vp9 -cpu-used 8 -g 15 -f webm

# Р.67 (t796): ОДИНОЧНАЯ КАРТИНКА — JPEG и PNG с альфой. Стиллы не были
# покрыты ни одной фикстурой: «статичные картинки не отображаются, вообще..
# там вечный Loading...» (владелец). Детерминированные генераторы lavfi.
gen still.jpg -f lavfi -i "testsrc=size=320x180:rate=1:duration=1" -frames:v 1 -q:v 3 -f image2
gen still_alpha.png -f lavfi -i "color=c=red@0.5:size=320x180:rate=1:duration=1,format=rgba" \
    -frames:v 1 -f image2

# Р.62 (t711): ОДНИ пиксели 640×480, РАЗНЫЙ SAR (1:1 и 2:1) — вид 4:3
# против 8:3; кадры декода при равном Quality обязаны совпасть байт-в-байт,
# различие живёт только в MediaInfo.sar. Один проход энкодера на оба
# файла невозможен (SAR пишется в SPS), поэтому — одинаковые детерминированные
# генераторы и одинаковые параметры x264.
gen testsrc_sar11.mp4 -f lavfi -i "testsrc=size=640x480:rate=25:duration=1" \
    -vf "setsar=1/1" -c:v libx264 -preset veryfast -g 25 -pix_fmt yuv420p -f mp4
gen testsrc_sar21.mp4 -f lavfi -i "testsrc=size=640x480:rate=25:duration=1" \
    -vf "setsar=2/1" -c:v libx264 -preset veryfast -g 25 -pix_fmt yuv420p -f mp4

# 2 c муксованный A/V (h264 + aac 48k stereo)
gen muxed_av.mp4 \
    -f lavfi -i "testsrc=size=320x180:rate=30:duration=2" \
    -f lavfi -i "sine=frequency=440:sample_rate=48000:duration=2" \
    -af "pan=stereo|c0=c0|c1=c0" \
    -c:v libx264 -preset veryfast -g 15 -pix_fmt yuv420p -c:a aac -shortest -f mp4

# 2 c синус 330 Гц ogg/vorbis + ОБЛОЖКА через METADATA_BLOCK_PICTURE
# (как в реальных файлах). Регрессия п.26: libavformat выставляет обложку
# отдельным attached_pic-потоком; чужой stream_index на ленивом
# ogg-демуксере бил av_assert0 (stream_index < ogg->nstreams) → abort.
if [ ! -f out/cover_art.ogg ]; then
    ffmpeg -v error -y -f lavfi -i "testsrc=size=64x64:rate=1:duration=1" \
        -frames:v 1 -q:v 5 out/.tmp.cover.jpg
    ffmpeg -v error -y -f lavfi -i "sine=frequency=330:sample_rate=48000:duration=2" \
        -c:a libvorbis -f ogg out/.tmp.plain.ogg
    python3 - <<'PY'
import base64, struct
jpg = open('out/.tmp.cover.jpg','rb').read()
mime = b'image/jpeg'
# FLAC METADATA_BLOCK_PICTURE: type=3 (front cover), BE-поля
blk = struct.pack('>I', 3)
blk += struct.pack('>I', len(mime)) + mime
blk += struct.pack('>I', 0)                     # описание пустое
blk += struct.pack('>IIII', 64, 64, 24, 0)      # w, h, depth, colors
blk += struct.pack('>I', len(jpg)) + jpg
open('out/.tmp.mbp.txt','w').write(base64.b64encode(blk).decode())
PY
    ffmpeg -v error -y -i out/.tmp.plain.ogg -c copy \
        -metadata METADATA_BLOCK_PICTURE="$(cat out/.tmp.mbp.txt)" \
        -f ogg out/.tmp.cover_art.ogg
    rm -f out/.tmp.cover.jpg out/.tmp.plain.ogg out/.tmp.mbp.txt
    mv out/.tmp.cover_art.ogg out/cover_art.ogg
fi

echo ok
