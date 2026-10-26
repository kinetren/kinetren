#!/usr/bin/env bash
# The workspace tests for Windows, under wine, in the `windows` stage of the build
# container (change windows-build): `make windows-test`.
#
# wine is the fast local loop, not the verdict — the windows-latest CI job runs the same
# tests on real Windows (mandatory locks, rename semantics, WASAPI, the CRT's math).
# Tests that cannot run there skip themselves with the reason printed (GPU, Xvfb, the
# repository guards that call python3/bash).
#
# The fixtures (src/fixtures/gen.sh) are generated on the host — the container has no
# ffmpeg binary; without them the fixture tests are skipped or fail with the reason.
#
# Usage:  scripts/windows-test.sh [nextest filter args…]
set -euo pipefail
cd "$(dirname "$0")/.." # src/

if ! command -v docker > /dev/null; then
    echo "windows-test.sh: Docker is required" >&2
    exit 1
fi
IMAGE=kinetren-build:windows
docker build --network=host --target windows -t "$IMAGE" -f packaging/Dockerfile packaging

if [ ! -d fixtures/out ] && command -v ffmpeg > /dev/null; then
    bash fixtures/gen.sh
fi

UID_GID="$(id -u):$(id -g)"
REGISTRY="$HOME/.cargo/registry"
[ -d "$REGISTRY" ] || { REGISTRY=kinetren-cargo-registry; docker volume create "$REGISTRY" > /dev/null; }
# The REPOSITORY is mounted at /kt and the build runs in /kt/src: compile-time paths
# (env!("CARGO_MANIFEST_DIR")) become /kt/src/…, so tests reaching the repository root
# (../../..) find it, and on the Windows CI runner a junction D:\kt → the checkout makes the
# same paths valid there.
docker run --rm --network=host -u "$UID_GID" -v "$PWD/..:/kt" -w /kt/src \
    -v "$REGISTRY:/usr/local/cargo/registry" \
    -e CARGO_TARGET_DIR=/kt/src/target/win-container -e RUSTFLAGS= \
    -e WINEPREFIX=/kt/src/target/wineprefix -e HOME=/tmp \
    -e KT_FFMPEG_WIN_DIR=/kt/src/target/ffmpeg-win/9.0.2 \
    -e 'KINETREN_FFMPEG_DIR=Z:\kt\src\target\ffmpeg-win\9.0.2' \
    "$IMAGE" bash -c "set -euo pipefail
scripts/fetch-ffmpeg-win.sh
cargo nextest run --target x86_64-pc-windows-gnu --workspace --no-fail-fast $*"
