#!/usr/bin/env bash
# Вендоринг ПУБЛИЧНЫХ заголовков FFmpeg для мажоров 6/7/8.
# Нужны только типы структур (layout) — их bindgen превращает в версионные
# модули kt-ffmpeg. Скачивается один раз и коммитится: сборка не требует сети.
set -eo pipefail
cd "$(dirname "$0")/.."
DEST=libs/kt-ffmpeg/vendor
LIBS="libavutil libavcodec libavformat libswscale libswresample"

fetch() { # fetch <major> <version>
    local major="$1"
    local ver="$2"
    local out="$DEST/ffmpeg-$major/include"
    [ -f "$out/libavcodec/avcodec.h" ] && { echo "ffmpeg-$major: уже есть"; return; }
    echo "== FFmpeg $ver -> $out"
    local tmp; tmp=$(mktemp -d)
    curl -sSL "https://ffmpeg.org/releases/ffmpeg-$ver.tar.xz" -o "$tmp/ff.tar.xz"
    # только *.h нужных библиотек
    tar -xJf "$tmp/ff.tar.xz" -C "$tmp" --wildcards \
        $(for l in $LIBS; do echo "ffmpeg-$ver/$l/*.h"; done)
    mkdir -p "$out"
    for l in $LIBS; do
        mkdir -p "$out/$l"
        cp "$tmp/ffmpeg-$ver/$l"/*.h "$out/$l/"
    done
    # avconfig.h/ffversion.h генерируются configure — кладём минимальные
    cat > "$out/libavutil/avconfig.h" <<'AVC'
/* сгенерировано вручную: значения одинаковы для всех целевых платформ */
#ifndef AVUTIL_AVCONFIG_H
#define AVUTIL_AVCONFIG_H
#define AV_HAVE_BIGENDIAN 0
#define AV_HAVE_FAST_UNALIGNED 1
#endif
AVC
    rm -rf "$tmp"
    echo "   ok: $(find "$out" -name '*.h' | wc -l) заголовков"
}

fetch 6 6.1.1
fetch 7 7.1.1
fetch 8 8.0
echo "готово: $DEST"
