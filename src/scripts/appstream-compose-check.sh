#!/usr/bin/env bash
# The AppStream compose of the Flatpak, in seconds and before the build (change ci-red-again, D3).
# flatpak-builder runs `appstreamcli compose` on the HOST after a ten-minute build; CI of 9e1f1a45
# died there because gdk-pixbuf had no SVG loader (librsvg2-common) to read our icon. This script
# installs the files of the manifest's own `install … /app/share/…` lines into a temporary folder
# and composes it with flatpak-builder 1.4's arguments. flatpak-build.sh runs it first; t1381.
#
#   src/scripts/appstream-compose-check.sh [repository-root]
set -euo pipefail
ROOT="$(cd "${1:-$(dirname "$0")/../..}" && pwd)"
ID=com.kinetren.Kinetren
MANIFEST="$ROOT/src/packaging/flatpak/$ID.yml"

# appstream-compose is a separate package; flatpak-builder depends on it and fails there anyway
if ! appstreamcli compose --help > /dev/null 2>&1; then
    echo "appstream-compose-check.sh: not run — no \`appstreamcli compose\` (package appstream-compose)" >&2
    exit 0
fi

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
FILES="$TMP/files"
mkdir -p "$FILES"

# the build commands that install into /app/share, as written in the manifest, into $FILES
lines="$(sed -n 's/^ *- \(.*install -Dm644 src\/packaging\/.* \/app\/share\/.*\)$/\1/p' "$MANIFEST")"
[ -n "$lines" ] || { echo "appstream-compose-check.sh: no install lines in $MANIFEST" >&2; exit 1; }
(cd "$ROOT" && while IFS= read -r line; do eval "${line//\/app\//$FILES/}"; done <<< "$lines")

# the arguments flatpak-builder 1.4.2 passes (no screenshot mirror, so no media folder)
set +e
(cd "$TMP" && appstreamcli compose --no-color --prefix=/ --origin="$ID" --result-root="$FILES" \
    --data-dir="$FILES/share/app-info/xmls" --icons-dir="$FILES/share/app-info/icons/flatpak" \
    --hints-dir="$TMP/hints" "--components=$ID,$ID.desktop" "$FILES")
rc=$?
set -e
if [ "$rc" -ne 0 ] || [ ! -s "$FILES/share/app-info/xmls/$ID.xml.gz" ]; then
    echo "appstream-compose-check.sh: compose failed; its hints:" >&2
    # the component's own hints (the tag legend before `- Unit:` is the same for every run)
    sed -n '/^- Unit:/,$p' "$TMP/hints/$ID.hints.yaml" >&2 2> /dev/null || true
    exit 1
fi
echo "appstream-compose-check.sh: $ID composes"
