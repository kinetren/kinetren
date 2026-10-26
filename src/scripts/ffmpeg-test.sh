#!/usr/bin/env bash
# The FFmpeg tests against FFmpeg 9 on Linux, in the `ffmpeg-test` stage of the build
# container (change ffmpeg-9, design D5): `make ffmpeg-test`, and the `ffmpeg9` CI job.
#
# The stage builds shared FFmpeg 9.0.2 from the official source tarball into /opt/ffmpeg and
# registers it with ldconfig: the tests find it as a distribution's FFmpeg 9 is found, by
# SONAME, and KT_EXPECT_FFMPEG_MAJOR=9 makes t865 fail unless FFmpeg 9 is what actually
# loaded — a run that silently fell back to another FFmpeg (or skipped) is not an FFmpeg 9
# proof. The tests run INSIDE the container: the libraries link the container's libx264.
#
# The fixtures (src/fixtures/gen.sh) are generated on the host — the container has no
# ffmpeg binary.
#
# Usage:  scripts/ffmpeg-test.sh [nextest filter args…]
set -euo pipefail
cd "$(dirname "$0")/.." # src/

if ! command -v docker > /dev/null; then
    echo "ffmpeg-test.sh: Docker is required" >&2
    exit 1
fi
IMAGE=kinetren-build:ffmpeg-test
# the CI job loads the image from its cache (keyed by the Dockerfile) and says so
if [ -z "${KT_FFMPEG_TEST_IMAGE_READY:-}" ]; then
    docker build --network=host --target ffmpeg-test -t "$IMAGE" -f packaging/Dockerfile packaging
fi
EXPECT="${KT_EXPECT_FFMPEG_MAJOR:-9}"

if [ ! -d fixtures/out ] && command -v ffmpeg > /dev/null; then
    bash fixtures/gen.sh
fi

UID_GID="$(id -u):$(id -g)"
REGISTRY="$HOME/.cargo/registry"
[ -d "$REGISTRY" ] || { REGISTRY=kinetren-cargo-registry; docker volume create "$REGISTRY" > /dev/null; }
# the repository at /kt and the build in /kt/src, as for the Windows tests: tests that reach
# the repository root (../../..) find it
docker run --rm --network=host -u "$UID_GID" -v "$PWD/..:/kt" -w /kt/src \
    -v "$REGISTRY:/usr/local/cargo/registry" \
    -e CARGO_TARGET_DIR=/kt/src/target/ffmpeg-test-container -e RUSTFLAGS= \
    -e CARGO_PROFILE_DEV_DEBUG=0 -e HOME=/tmp \
    -e KT_EXPECT_FFMPEG_MAJOR="$EXPECT" \
    "$IMAGE" \
    cargo nextest run -p kt-ffmpeg -p kt-media -p kt-engine -p kt-cli --no-fail-fast "$@"
