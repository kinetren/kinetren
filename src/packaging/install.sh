#!/usr/bin/env bash
# Установка Kinotrain для текущего пользователя (~/.local).
# Запускать из распакованного тарбола: ./install.sh
set -euo pipefail
cd "$(dirname "$0")"

BIN="$HOME/.local/bin"
APPS="$HOME/.local/share/applications"
ICONS="$HOME/.local/share/icons/hicolor/scalable/apps"
MIME="$HOME/.local/share/mime/packages"

mkdir -p "$BIN" "$APPS" "$ICONS" "$MIME"
install -m 755 kt-app kt-cli "$BIN/"
install -m 644 kinotrain.svg "$ICONS/kinotrain.svg"
install -m 644 kinotrain-mime.xml "$MIME/kinotrain-mime.xml"
# Exec без пути: полагаемся на ~/.local/bin в PATH; если его там нет —
# подставляем абсолютный путь.
if command -v kt-app >/dev/null 2>&1; then
    install -m 644 kinotrain.desktop "$APPS/kinotrain.desktop"
else
    sed "s|^Exec=kt-app|Exec=$BIN/kt-app|" kinotrain.desktop > "$APPS/kinotrain.desktop"
    chmod 644 "$APPS/kinotrain.desktop"
fi

command -v update-mime-database >/dev/null 2>&1 && update-mime-database "$HOME/.local/share/mime" || true
command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database "$APPS" || true
command -v gtk-update-icon-cache >/dev/null 2>&1 && gtk-update-icon-cache -q "$HOME/.local/share/icons/hicolor" || true

echo "Установлено: $BIN/kt-app, $BIN/kt-cli"
echo "Требуется системный FFmpeg 6/7/8 (libavcodec и др.) — приложение грузит его в рантайме."
