#!/usr/bin/env bash
# The crates.io registry cache a build container mounts at /usr/local/cargo/registry, printed
# on stdout: the host's ~/.cargo/registry when it exists (the owner's build), otherwise the named
# volume `kinetren-cargo-registry`, so a second run on a machine without Rust does not download
# everything again.
#
# A fresh volume belongs to root, and the containers run as the calling user (-u UID:GID): the
# volume is handed to the user here, every time (cheap, and a volume an older script made stays
# usable). Without it cargo died on "failed to create directory /usr/local/cargo/registry/cache"
# (the ffmpeg9 CI job of 94e36e9e — the runner has no ~/.cargo/registry). One body for every
# script that mounts the registry: release-build.sh, windows-test.sh, ffmpeg-test.sh.
#
# Usage:  REGISTRY="$(scripts/cargo-registry.sh <image>)"
set -euo pipefail
IMAGE="$1"
if [ -d "$HOME/.cargo/registry" ]; then
    echo "$HOME/.cargo/registry"
    exit 0
fi
REGISTRY=kinetren-cargo-registry
docker volume create "$REGISTRY" > /dev/null
docker run --rm -u 0 -v "$REGISTRY:/usr/local/cargo/registry" "$IMAGE" \
    chown "$(id -u):$(id -g)" /usr/local/cargo/registry
echo "$REGISTRY"
