#!/usr/bin/env bash
# Release build of Kinetren in an Ubuntu 22.04 container (glibc 2.35):
# the binary runs on any system with glibc >= 2.35 and a system FFmpeg 6/7/8
# (loaded at runtime, invariant No. 10 — there is no linking with libav*).
#
# The container recipe is src/packaging/Dockerfile (github-ci-docker-build):
# this script, the root Makefile and the release workflow all build through
# it. The host needs only Docker and git (git only for the version string).
#
# Usage:  scripts/release-build.sh
# Result:      target/dist-out/kinetren-<version>-linux-x86_64.tar.gz
#                 + .sha256 next to it + LATEST (version and tarball path)
set -euo pipefail
cd "$(dirname "$0")/.." # src/

if ! command -v docker > /dev/null; then
    echo "release-build.sh: Docker is required: https://docs.docker.com/engine/install/" >&2
    exit 1
fi

IMAGE=kinetren-build:ubuntu2204
# The version is computed HERE, on the host (item 6 round 12): inside the container there is neither
# git nor a .git directory — only src/ is mounted. Format vYY.MM.DD-hash
# by the COMMIT date; build.rs of the kt-version crate takes it from the environment.
# Without git (or outside a clone) the version is kt00.00.00-nogit.
if [ -z "${KT_VERSION_FULL:-}" ]; then
    KT_DATE=$(git log -1 --format=%cd --date=format:%y.%m.%d 2>/dev/null || true)
    KT_HASH=$(git rev-parse --short=8 HEAD 2>/dev/null || true)
    if [ -n "$KT_DATE" ] && [ -n "$KT_HASH" ]; then
        KT_VERSION_FULL="kt$KT_DATE-$KT_HASH"
    else
        KT_VERSION_FULL="kt00.00.00-nogit"
    fi
fi
KT_VERSION_SHORT="${KT_VERSION_SHORT:-kt${KT_VERSION_FULL:2:5}}"
export KT_VERSION_FULL KT_VERSION_SHORT
VERSION="${KT_VERSION_FULL#kt}"
ARCH=$(uname -m)
OUT="target/dist-out"
STAGE="$OUT/kinetren-$VERSION-linux-$ARCH"
# the GUI is `kinetren` on disk (owner, 2026-10-22); kt-cli keeps its name
BINS="kinetren kt-cli"

# --network=host: the user's docker config may force a proxy on 127.0.0.1 onto containers —
# in the host's network namespace it is reachable.
docker build --network=host -t "$IMAGE" -f packaging/Dockerfile packaging

# crates.io registry cache: from the host if present (the owner's build); otherwise
# a named volume, so that a second `make` on a machine without Rust does not download everything
# again. A fresh volume belongs to root — we hand it to the user once.
UID_GID="$(id -u):$(id -g)"
if [ -d "$HOME/.cargo/registry" ]; then
    REGISTRY="$HOME/.cargo/registry"
else
    REGISTRY=kinetren-cargo-registry
    docker volume create "$REGISTRY" > /dev/null
    docker run --rm -u 0 -v "$REGISTRY:/usr/local/cargo/registry" "$IMAGE" \
        chown "$UID_GID" /usr/local/cargo/registry
fi

# --- Delivery checks ---------------------------------------------------
# Run INSIDE the container (binutils are in the image), so the host needs
# no ldd/objdump and every builder checks with the same tools.
delivery_checks() { # delivery_checks <dir> <bin...>
    local dir="$1"; shift
    local b need ver
    # 1. Invariant No. 10: no linking with libav*.
    for b in "$@"; do
        if ldd "$dir/$b" | grep -q libav; then
            echo "ERROR: $b is linked with libav* — violation of invariant No. 10" >&2
            ldd "$dir/$b" | grep libav >&2
            return 1
        fi
    done
    # 2. Ceiling of the required glibc (informational + a guard against building outside the container).
    for b in "$@"; do
        need=$(objdump -T "$dir/$b" | grep -o 'GLIBC_[0-9.]*' | sort -uV | tail -1)
        echo "$b: requires $need, size $(du -h "$dir/$b" | cut -f1)"
        ver=${need#GLIBC_}
        if [ "$(printf '%s\n' "$ver" 2.35 | sort -V | tail -1)" != "2.35" ]; then
            echo "ERROR: $b requires glibc $ver > 2.35 — built outside the container?" >&2
            return 1
        fi
    done
}

# A separate target inside the workspace (does not overlap with host builds).
# RUSTFLAGS= disables the host's .cargo/config.toml (R.68): it sets mold, which
# is absent in the Ubuntu 22.04 container (and gcc 11 does not know `-fuse-ld=mold`). An empty
# variable beats the config — cargo takes it. A development accelerator must not have
# the right to become a condition for building what ships to people.
BIN_DIR="target/dist-container/dist"
# Anonymous usage telemetry (change telemetry-posthog): the public, write-only PostHog
# project key enters ONLY the release binary (option_env! in telemetry.rs); cargo run, tests,
# finish.sh and the live scenarios build without it and never report. An empty or missing
# file gives a release that sends nothing.
KINETREN_POSTHOG_KEY=$(tr -d '[:space:]' < packaging/posthog-key 2>/dev/null || true)

if ! docker run --rm --network=host -u "$UID_GID" \
    -v "$PWD:/work" -w /work -v "$REGISTRY:/usr/local/cargo/registry" \
    -e CARGO_TARGET_DIR=/work/target/dist-container \
    -e RUSTFLAGS= \
    -e KT_VERSION_FULL="$KT_VERSION_FULL" -e KT_VERSION_SHORT="$KT_VERSION_SHORT" \
    -e KINETREN_POSTHOG_KEY="$KINETREN_POSTHOG_KEY" \
    "$IMAGE" bash -c "set -euo pipefail
$(declare -f delivery_checks)
cargo build --profile dist -p kt-app -p kt-cli
delivery_checks $BIN_DIR $BINS"; then
    # a failed build or check must not leave a tarball that looks good
    rm -rf "$OUT"
    echo "release-build.sh: build or delivery check failed, no tarball" >&2
    exit 1
fi

# --- Tarball ---------------------------------------------------------------
# we clean ALL the output: before, old tarballs and unpacked directories
# piled up forever, and the user could take the wrong build (item 7 round 12)
rm -rf "$OUT"
mkdir -p "$STAGE"
for b in $BINS; do cp "$BIN_DIR/$b" "$STAGE/"; done
cp packaging/kinetren.desktop packaging/kinetren.svg \
    packaging/kinetren-mime.xml packaging/install.sh "$STAGE/"
cp -r packaging/icons "$STAGE/"
chmod +x "$STAGE/install.sh"
tar -C "$OUT" -czf "$STAGE.tar.gz" "$(basename "$STAGE")"
(cd "$OUT" && sha256sum "$(basename "$STAGE").tar.gz" > "$(basename "$STAGE").tar.gz.sha256")
printf '%s\n%s\n' "$KT_VERSION_FULL" "$STAGE.tar.gz" > "$OUT/LATEST"
echo "Done: $STAGE.tar.gz (version $KT_VERSION_FULL)"
