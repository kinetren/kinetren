#!/usr/bin/env bash
# Installation of Kinetren for the current user (~/.local).
# Run from the unpacked tarball: ./install.sh
set -euo pipefail
cd "$(dirname "$0")"

BIN="$HOME/.local/bin"
APPS="$HOME/.local/share/applications"
ICONS="$HOME/.local/share/icons/hicolor/scalable/apps"
MIME="$HOME/.local/share/mime/packages"

mkdir -p "$BIN" "$APPS" "$ICONS" "$MIME"
# the program was called Kinotrain before 2026-10-21: remove what an older
# install left, or the desktop would show two entries and a stale MIME type
# the GUI binary was called kt-app before 2026-10-22: remove it, or `kt-app`
# in a terminal would keep starting the old build
rm -f "$BIN/kt-app"  # former binary name
rm -f "$APPS/kinotrain.desktop" "$ICONS/kinotrain.svg" "$MIME/kinotrain-mime.xml"  # former name
for s in 16 22 24 32 48 64 128 256; do
    rm -f "$HOME/.local/share/icons/hicolor/${s}x${s}/apps/kinotrain.png"  # former name
done
install -m 755 kinetren kt-cli "$BIN/"
install -m 644 kinetren.svg "$ICONS/kinetren.svg"
# icons: some panels/taskbars scale SVG to small sizes poorly
for s in 16 22 24 32 48 64 128 256; do
    d="$HOME/.local/share/icons/hicolor/${s}x${s}/apps"
    mkdir -p "$d"
    [ -f "icons/$s/kinetren.png" ] && install -m 644 "icons/$s/kinetren.png" "$d/kinetren.png"
done
install -m 644 kinetren-mime.xml "$MIME/kinetren-mime.xml"
# Exec without a path: we rely on ~/.local/bin in PATH; if it is not there —
# we substitute the absolute path.
if command -v kinetren >/dev/null 2>&1; then
    install -m 644 kinetren.desktop "$APPS/kinetren.desktop"
else
    sed "s|^Exec=kinetren|Exec=$BIN/kinetren|" kinetren.desktop > "$APPS/kinetren.desktop"
    chmod 644 "$APPS/kinetren.desktop"
fi

command -v update-mime-database >/dev/null 2>&1 && update-mime-database "$HOME/.local/share/mime" || true
command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database "$APPS" || true
command -v gtk-update-icon-cache >/dev/null 2>&1 && gtk-update-icon-cache -q "$HOME/.local/share/icons/hicolor" || true

echo "Installed: $BIN/kinetren, $BIN/kt-cli"
echo "Kinetren needs the system FFmpeg 6, 7 or 8 (libavcodec and friends); it is loaded at run time."
