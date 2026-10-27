#!/usr/bin/env bash
# A screenshot of the Windows build under wine (change wine-frame-and-dark-theme, design D5).
#
# Runs kinetren.exe in the `windows` stage of the build container on a virtual screen
# (Xvfb + openbox, installed into the throwaway container) and saves the whole screen to
# target/wine-out/<name>.png. The picture is LOOKED AT, not compared: wine's fonts and
# openbox's theme are not ours to pin. Real Windows stays a manual check.
#
# Usage:  scripts/wine-shot.sh [--name N] [--undecorated] [--exe PATH] [-- VAR=VALUE…]
#   --name N       the screenshot's name (default: default)
#   --undecorated  wine's `HKCU\Software\Wine\X11 Driver\Decorated=N` in the throwaway prefix
#   --exe PATH     the exe, relative to src/ (default: the `make windows` output)
#   VAR=VALUE…     the program's environment, e.g. KT_DECORATIONS=0
set -euo pipefail
cd "$(dirname "$0")/.." # src/

NAME=default
UNDECORATED=0
EXE=target/dist-container/x86_64-pc-windows-gnu/dist/kinetren.exe
while [ $# -gt 0 ]; do
    case "$1" in
        --name) NAME=$2; shift 2 ;;
        --undecorated) UNDECORATED=1; shift ;;
        --exe) EXE=$2; shift 2 ;;
        --) shift; break ;;
        *) echo "wine-shot.sh: unknown argument $1" >&2; exit 2 ;;
    esac
done
APP_ENV="$*"

if ! command -v docker > /dev/null; then
    echo "wine-shot.sh: Docker is required" >&2
    exit 1
fi
if [ ! -f "$EXE" ]; then
    echo "wine-shot.sh: $EXE not found — build it with: make windows" >&2
    exit 1
fi
IMAGE=kinetren-build:windows
docker build --network=host --target windows -t "$IMAGE" -f packaging/Dockerfile packaging > /dev/null 2>&1

OUT=target/wine-out
mkdir -p "$OUT"
rm -f "$OUT/$NAME.png" "$OUT/$NAME.log"
# root inside: apt-get installs the X tools; the files are handed back to the user at the end
docker run --rm --network=host -v "$PWD:/w" -w /w \
    -e NAME="$NAME" -e UNDECORATED="$UNDECORATED" -e EXE="$EXE" -e APP_ENV="$APP_ENV" \
    -e HOST_UID="$(id -u)" -e HOST_GID="$(id -g)" \
    "$IMAGE" bash -c '
set -euo pipefail
apt-get update -qq > /dev/null 2>&1
apt-get install -y -qq --no-install-recommends xvfb openbox imagemagick x11-utils > /dev/null 2>&1
# no Mono/Gecko prompt: it would cover the window on the first start of a prefix
export WINEPREFIX=/tmp/wp HOME=/tmp WINEDEBUG=-all DISPLAY=:99 WINEDLLOVERRIDES="mscoree,mshtml="
Xvfb :99 -screen 0 1600x1000x24 > /dev/null 2>&1 &
sleep 2
openbox > /dev/null 2>&1 &
sleep 1
wineboot -i > /dev/null 2>&1; wineserver -w
if [ "$UNDECORATED" = 1 ]; then
    wine reg add "HKCU\\Software\\Wine\\X11 Driver" /v Decorated /d N /f > /dev/null 2>&1
    wineserver -w
fi
env $APP_ENV wine "$EXE" > "target/wine-out/$NAME.log" 2>&1 &
# wait for the FIRST DRAWN frame (the screen stops being one colour), not a fixed sleep: under
# wine the window appears long before wgpu draws into it; then let the frames settle
for _ in $(seq 120); do
    import -window root "target/wine-out/$NAME.png" 2> /dev/null || true
    spread=$(convert "target/wine-out/$NAME.png" -format "%[fx:standard_deviation]" info: 2> /dev/null || echo 0)
    awk "BEGIN { exit !($spread > 0.02) }" && break
    sleep 1
done
sleep 10
import -window root "target/wine-out/$NAME.png"
chown "$HOST_UID:$HOST_GID" "target/wine-out/$NAME.png" "target/wine-out/$NAME.log"
'
echo "wine-shot.sh: $OUT/$NAME.png"
