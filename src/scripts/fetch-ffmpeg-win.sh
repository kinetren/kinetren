#!/usr/bin/env bash
# The pinned Windows FFmpeg build for the Windows TESTS (change windows-build): t846 under
# wine, `make windows-test` and the windows-latest CI job load real DLLs from here.
#
# The same archive, URL and SHA-256 as the program's Download button
# (src/apps/kt-app/src/ffmpeg_setup.rs) — keep the two equal. The GitHub mirror serves a
# byte-identical file (checked 2026-10-24), so one hash covers both URLs.
#
# Usage:  scripts/fetch-ffmpeg-win.sh [dest]      (default: target/ffmpeg-win/8.1.2)
# Needs:  curl, sha256sum, 7z (all in the `windows` stage of packaging/Dockerfile)
set -euo pipefail
cd "$(dirname "$0")/.." # src/

VERSION=8.1.2
NAME="ffmpeg-$VERSION-full_build-shared.7z"
SHA256=cba748035c21ce1431d0823c7a3a711f38616f89f87a265dceddf9b7f6749d2d
URLS="https://www.gyan.dev/ffmpeg/builds/packages/$NAME
https://github.com/GyanD/codexffmpeg/releases/download/$VERSION/$NAME"
DEST="${1:-target/ffmpeg-win/$VERSION}"
CACHE="target/ffmpeg-win/$NAME"

if [ -f "$DEST/avcodec-62.dll" ]; then
    echo "fetch-ffmpeg-win: $DEST is ready"
    exit 0
fi
mkdir -p "$(dirname "$CACHE")"
if ! echo "$SHA256  $CACHE" | sha256sum -c --status 2>/dev/null; then
    rm -f "$CACHE"
    for url in $URLS; do
        echo "fetch-ffmpeg-win: downloading $url"
        if curl -fsSL --retry 3 -o "$CACHE.part" "$url" \
            && echo "$SHA256  $CACHE.part" | sha256sum -c --status; then
            mv "$CACHE.part" "$CACHE"
            break
        fi
        echo "fetch-ffmpeg-win: $url failed or its hash differs" >&2
        rm -f "$CACHE.part"
    done
fi
echo "$SHA256  $CACHE" | sha256sum -c --status || { echo "fetch-ffmpeg-win: no verified archive" >&2; exit 1; }

# only the DLLs and the licence, flat — the layout of the program's own download folder
TMP="$DEST.partial-$$"
rm -rf "$TMP"
mkdir -p "$TMP"
7z e -y -bd -o"$TMP" "$CACHE" '*/bin/*.dll' '*/LICENSE' > /dev/null
rm -rf "$DEST"
mv "$TMP" "$DEST"
echo "fetch-ffmpeg-win: $DEST: $(ls "$DEST" | tr '\n' ' ')"
