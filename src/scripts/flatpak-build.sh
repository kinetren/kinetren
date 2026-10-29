#!/usr/bin/env bash
# Build, lint and check the Flatpak (change flatpak-flathub, D9): `make flatpak`, the CI job
# `flatpak`, and — with --bundle — the single-file bundle the release attaches. Needs flatpak and
# flatpak-builder from the distribution; the runtime, SDK extensions and the linter come from
# Flathub. Nothing is uploaded.
#
#   src/scripts/flatpak-build.sh            # build, install for the user, lint, check
#   src/scripts/flatpak-build.sh --bundle   # … and write src/target/dist-out/kinetren-<version>-linux-x86_64.flatpak
set -euo pipefail
cd "$(dirname "$0")/../.."  # the repository root: the manifest's source is the working tree

ID=com.kinetren.Kinetren
MANIFEST="src/packaging/flatpak/$ID.yml"
REPO=src/target/flatpak-repo
BUILD=src/target/flatpak-build

for tool in flatpak flatpak-builder appstreamcli; do
    command -v "$tool" > /dev/null || { echo "flatpak-build.sh: $tool is required" >&2; exit 1; }
done
flatpak remote-add --user --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo

flatpak-builder --user --install-deps-from=flathub --force-clean --repo="$REPO" --install "$BUILD" "$MANIFEST"

# the Flathub linter on the manifest and the built repository, the metainfo strictly
flatpak install --user -y --noninteractive flathub org.flatpak.Builder
flatpak run --command=flatpak-builder-lint org.flatpak.Builder manifest "$MANIFEST"
flatpak run --command=flatpak-builder-lint org.flatpak.Builder repo "$REPO"
appstreamcli validate --strict --no-net "src/packaging/$ID.metainfo.xml"

# the sandbox's software path: the runtime's FFmpeg, a libx264 + AAC export that decodes
src/scripts/flatpak-check.sh

if [ "${1:-}" = "--bundle" ]; then
    out=src/target/dist-out
    # the version of the tarball built before (release-build.sh writes LATEST)
    version=$(head -1 "$out/LATEST")
    name="kinetren-$version-linux-x86_64.flatpak"
    flatpak build-bundle "$REPO" "$out/$name" "$ID"
    (cd "$out" && sha256sum "$name" > "$name.sha256")
    echo "Done: $out/$name"
fi
