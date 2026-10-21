#!/usr/bin/env bash
# Установка Kinetren для текущего пользователя (~/.local).
# Запускать из распакованного тарбола: ./install.sh
set -euo pipefail
cd "$(dirname "$0")"

BIN="$HOME/.local/bin"
APPS="$HOME/.local/share/applications"
ICONS="$HOME/.local/share/icons/hicolor/scalable/apps"
MIME="$HOME/.local/share/mime/packages"

mkdir -p "$BIN" "$APPS" "$ICONS" "$MIME"
# the program was called Kinotrain before 2026-10-21: remove what an older
# install left, or the desktop would show two entries and a stale MIME type
rm -f "$APPS/kinotrain.desktop" "$ICONS/kinotrain.svg" "$MIME/kinotrain-mime.xml"  # former name
for s in 16 22 24 32 48 64 128 256; do
    rm -f "$HOME/.local/share/icons/hicolor/${s}x${s}/apps/kinotrain.png"  # former name
done
install -m 755 kt-app kt-cli "$BIN/"
install -m 644 kinetren.svg "$ICONS/kinetren.svg"
# растры: часть панелей/таскбаров плохо масштабирует SVG в мелкие размеры
for s in 16 22 24 32 48 64 128 256; do
    d="$HOME/.local/share/icons/hicolor/${s}x${s}/apps"
    mkdir -p "$d"
    [ -f "icons/$s/kinetren.png" ] && install -m 644 "icons/$s/kinetren.png" "$d/kinetren.png"
done
install -m 644 kinetren-mime.xml "$MIME/kinetren-mime.xml"
# Exec без пути: полагаемся на ~/.local/bin в PATH; если его там нет —
# подставляем абсолютный путь.
if command -v kt-app >/dev/null 2>&1; then
    install -m 644 kinetren.desktop "$APPS/kinetren.desktop"
else
    sed "s|^Exec=kt-app|Exec=$BIN/kt-app|" kinetren.desktop > "$APPS/kinetren.desktop"
    chmod 644 "$APPS/kinetren.desktop"
fi

command -v update-mime-database >/dev/null 2>&1 && update-mime-database "$HOME/.local/share/mime" || true
command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database "$APPS" || true
command -v gtk-update-icon-cache >/dev/null 2>&1 && gtk-update-icon-cache -q "$HOME/.local/share/icons/hicolor" || true

echo "Установлено: $BIN/kt-app, $BIN/kt-cli"
echo "Требуется системный FFmpeg 6/7/8 (libavcodec и др.) — приложение грузит его в рантайме."
