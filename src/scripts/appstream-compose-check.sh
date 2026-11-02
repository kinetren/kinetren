#!/usr/bin/env bash
# The AppStream compose of the Flatpak, in seconds and before the build (change ci-red-again, D3).
# flatpak-builder runs `appstreamcli compose` on the HOST after a ten-minute build; CI of 9e1f1a45
# died there because gdk-pixbuf had no SVG loader (librsvg2-common) to read our icon. This script
# installs the files of the manifest's own `install … /app/share/…` lines into a temporary folder
# and composes it with flatpak-builder 1.4's arguments. flatpak-build.sh runs it first; t1381.
# The compose mirrors the screenshots as flatpak-build.sh's flatpak-builder does: CI of 21505ad2
# built and installed, then Flathub's linter found no screenshot in the catalogue, because gdk-pixbuf
# had no WebP loader and compose dropped all four (change ci-flatpak-screenshots, t1410). So the
# script also fails unless every screenshot of the listing reaches the catalogue under Flathub's
# media URL, stored as a PNG in the media folder. Fetching them needs the network.
#
#   src/scripts/appstream-compose-check.sh [repository-root]
set -euo pipefail
ROOT="$(cd "${1:-$(dirname "$0")/../..}" && pwd)"
ID=com.kinetren.Kinetren
MANIFEST="$ROOT/src/packaging/flatpak/$ID.yml"
MEDIA_URL=https://dl.flathub.org/media

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

# the arguments flatpak-builder 1.4.6+ passes with flatpak-build.sh's --mirror-screenshots-url and
# --compose-url-policy=full
MEDIA="$FILES/share/app-info/media"
CATALOGUE="$FILES/share/app-info/xmls/$ID.xml.gz"
set +e
(cd "$TMP" && appstreamcli compose --no-color --no-partial-urls --prefix=/ --origin="$ID" \
    --media-baseurl="$MEDIA_URL" --media-dir="$MEDIA" --result-root="$FILES" \
    --data-dir="$FILES/share/app-info/xmls" --icons-dir="$FILES/share/app-info/icons/flatpak" \
    --hints-dir="$TMP/hints" "--components=$ID,$ID.desktop" "$FILES")
rc=$?
set -e
hints() {
    # the component's own hints (the tag legend before `- Unit:` is the same for every run)
    sed -n '/^- Unit:/,$p' "$TMP/hints/$ID.hints.yaml" >&2 2> /dev/null || true
}
if [ "$rc" -ne 0 ] || [ ! -s "$CATALOGUE" ]; then
    echo "appstream-compose-check.sh: compose failed; its hints:" >&2
    hints
    exit 1
fi

# every screenshot of the listing is in the catalogue, by Flathub's media URL, and its file is a
# PNG in the media folder: what flatpak-builder-lint's `repo` check reads from the catalogue and
# from the `screenshots/<arch>` ref the build commits out of that folder
listed=$(grep -c '<screenshot[ >]' "$FILES/share/metainfo/$ID.metainfo.xml" || true)
composed=$(zcat "$CATALOGUE" | grep -c '<screenshot[ >]' || true)
bad=""
while IFS= read -r url; do
    [ -n "$url" ] || continue
    case "$url" in
        "$MEDIA_URL"/*.png) [ -s "$MEDIA/${url#"$MEDIA_URL"/}" ] || bad="$bad\n  not stored: $url" ;;
        *) bad="$bad\n  not mirrored: $url" ;;
    esac
done <<< "$(zcat "$CATALOGUE" | sed -n 's/.*<image[^>]*>\([^<]*\)<\/image>.*/\1/p')"
if [ "$composed" -ne "$listed" ] || [ -n "$bad" ]; then
    printf 'appstream-compose-check.sh: %s of %s screenshots reached the catalogue%b\n' "$composed" "$listed" "$bad" >&2
    echo "  (a WebP screenshot needs gdk-pixbuf's WebP loader, package webp-pixbuf-loader); hints:" >&2
    hints
    exit 1
fi
echo "appstream-compose-check.sh: $ID composes, $listed screenshots mirrored"
