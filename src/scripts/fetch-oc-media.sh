#!/usr/bin/env bash
# Real film for cross-render (test 179, the user’s suggestion:
# “take some film whose source files are smaller and also render it
# with two programs”).
#
# Netflix Open Content “Sol Levante”, an animatic: 63 cuts on one
# track, 23.976 fps, timecode starts at the hour, 162 s, one H.264 source.
# Exactly the case that was missing in synthetic data: NTSC rate, real
# decode, real shot lengths (7…347 frames).
#
# NOTHING IS COMMITTED: the media is 320 MB, and the repository with .otio has no license
# (checked via the GitHub API). So both files are downloaded here, the directory is in
# .gitignore, and the test silently skips without them.
#
# Licenses:
#   video  — Netflix Open Content, CC BY 4.0 (https://opencontent.netflix.com/)
#   .otio  — darbyjohnston/otio-oc-examples, no license stated; the file is
#   used locally as test input and does not go into the repository.
set -euo pipefail

dir="$(cd "$(dirname "$0")/.." && pwd)/fixtures/otio-oc"
mkdir -p "$dir"

OTIO_URL=https://raw.githubusercontent.com/darbyjohnston/otio-oc-examples/main/SolLevante_Animatics_ver1_H264.otio
MOV_URL=https://download.opencontent.netflix.com/SolLevante/working_assets/SolLevante_Animatics_ver1_H264.mov
otio="$dir/SolLevante_Animatics_ver1_H264.otio"
mov="$dir/SolLevante_Animatics_ver1_H264.mov"

if [ ! -s "$otio" ]; then
    echo "downloading the edit (~105 KB)…"
    curl -fL --progress-bar "$OTIO_URL" -o "$otio.tmp" && mv "$otio.tmp" "$otio"
fi

# an already downloaded file can simply be passed as an argument — 320 MB is not copied,
# we put a symlink (the test reads it as a regular file)
if [ $# -ge 1 ] && [ -s "$1" ] && [ ! -e "$mov" ]; then
    ln -s "$(cd "$(dirname "$1")" && pwd)/$(basename "$1")" "$mov"
    echo "source taken from link: $1"
fi

if [ ! -s "$mov" ]; then
    echo "downloading the source (~320 MB)…"
    if ! curl -fL --progress-bar "$MOV_URL" -o "$mov.tmp"; then
        rm -f "$mov.tmp"
        echo "direct link unavailable; trying aws cli…" >&2
        aws s3 cp --no-sign-request \
            "s3://download.opencontent.netflix.com/SolLevante/working_assets/SolLevante_Animatics_ver1_H264.mov" \
            "$mov.tmp"
    fi
    mv "$mov.tmp" "$mov"
fi

ls -lh "$otio" "$mov"
echo
echo "Sol Levante © Netflix, CC BY 4.0 — https://opencontent.netflix.com/"
echo "done: cargo test -p kt-cli --test oc_render"
