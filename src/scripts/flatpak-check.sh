#!/usr/bin/env bash
# The Flatpak's software path, inside the sandbox (change flatpak-flathub, t971): the FFmpeg it
# loads is the runtime's major, the export with libx264 and AAC finishes, and our own decoder reads
# the file to its last frame. The CI runner has no GPU: hardware encoding stays a manual check
# (docs/flatpak.md, tasks 4.1). Run after the app is installed from the built repository:
#
#   src/scripts/flatpak-check.sh            # (make flatpak runs it)
set -euo pipefail
cd "$(dirname "$0")/.."  # src/

ID=com.kinetren.Kinetren
MANIFEST="packaging/flatpak/$ID.yml"
run() { flatpak run --command=kt-cli "$ID" "$@"; }

# 1. the FFmpeg the sandbox loads is the runtime's major (runtime-ffmpeg.txt)
branch=$(sed -n "s/^runtime-version: '\{0,1\}\([0-9.]*\)'\{0,1\}$/\1/p" "$MANIFEST")
want=$(awk -v b="$branch" '$1 == b { print $2 }' packaging/flatpak/runtime-ffmpeg.txt)
report=$(run ffmpeg)
echo "$report" | head -3
got=$(echo "$report" | sed -n 's/^loaded: FFmpeg \([0-9]*\) .*/\1/p')
if [ -z "$got" ] || [ "$got" != "$want" ]; then
    echo "flatpak-check: the sandbox loaded FFmpeg '${got:-none}', the runtime $branch has $want" >&2
    exit 1
fi

# 2. an export with libx264 (and AAC, which every export uses) of the committed fixture finishes,
# 3. and our decoder reads the file to its last frame — inside the sandbox
work=$(mktemp -d -p "$HOME" kt-flatpak-check.XXXXXX)
trap 'rm -rf "$work"' EXIT
cp packaging/flatpak/check.kt "$work/check.kt"
run export "$work/check.kt" "$work/check.mp4" libx264
run decode "$work/check.mp4" | tee "$work/decode.txt"
grep -q "^decoded 60 video frames" "$work/decode.txt" || {
    echo "flatpak-check: the export does not decode to its 60th frame" >&2
    exit 1
}
echo "flatpak-check: FFmpeg $got of runtime $branch, libx264 + AAC export decoded to its last frame"
