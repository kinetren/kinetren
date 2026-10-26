#!/usr/bin/env bash
# Release build of Kinetren in an Ubuntu 22.04 container (glibc 2.35):
# the binary runs on any system with glibc >= 2.35 and a system FFmpeg 6/7/8/9
# (loaded at runtime, invariant No. 10 — there is no linking with libav*).
#
# The container recipe is src/packaging/Dockerfile (github-ci-docker-build):
# this script, the root Makefile and the release workflow all build through
# it. The host needs only Docker and git (git only for the version string).
#
# Usage:  scripts/release-build.sh [--target linux|windows]
# Result:      target/dist-out/kinetren-<version>-linux-x86_64.tar.gz
#                 + .sha256 next to it + LATEST (version and tarball path)
#   --target windows (change windows-build): a cross-build for x86_64-pc-windows-gnu in
#   the `windows` stage of the same Dockerfile →
#              target/dist-out/kinetren-<version>-windows-x86_64.zip + .sha256
#              + LATEST-windows. No FFmpeg DLL is ever packed: the program finds the
#              user's FFmpeg or downloads gyan.dev's build on a click.
set -euo pipefail
cd "$(dirname "$0")/.." # src/

TARGET=linux
while [ $# -gt 0 ]; do
    case "$1" in
        --target) TARGET="${2:-}"; shift 2 ;;
        --target=*) TARGET="${1#--target=}"; shift ;;
        *) echo "release-build.sh: unknown argument $1" >&2; exit 2 ;;
    esac
done
case "$TARGET" in
    linux | windows) ;;
    *) echo "release-build.sh: --target must be linux or windows" >&2; exit 2 ;;
esac

if ! command -v docker > /dev/null; then
    echo "release-build.sh: Docker is required: https://docs.docker.com/engine/install/" >&2
    exit 1
fi

IMAGE=kinetren-build:ubuntu2204
DOCKER_STAGE=linux
if [ "$TARGET" = windows ]; then
    IMAGE=kinetren-build:windows
    DOCKER_STAGE=windows
fi
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
STAGE="$OUT/kinetren-$VERSION-$TARGET-$ARCH"
# the GUI is `kinetren` on disk (owner, 2026-10-22); kt-cli keeps its name
BINS="kinetren kt-cli"
[ "$TARGET" = windows ] && BINS="kinetren.exe kt-cli.exe"

# --network=host: the user's docker config may force a proxy on 127.0.0.1 onto containers —
# in the host's network namespace it is reachable.
docker build --network=host --target "$DOCKER_STAGE" -t "$IMAGE" -f packaging/Dockerfile packaging

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

# Invariant No. 10, Windows form (windows-build, design D10): the PE import table of
# each exe names only Windows system DLLs — no av*/sw* (FFmpeg is loaded at run time
# from the folder the program finds) and no mingw runtime DLL (libgcc_s, libstdc++,
# libwinpthread), which a clean Windows does not have. The allow-list grows only
# deliberately, when a dependency legitimately imports another system DLL.
pe_checks() { # pe_checks <dir> <exe...>
    local dir="$1"; shift
    local b dll lc bad=0
    local allow=" kernel32.dll user32.dll gdi32.dll advapi32.dll shell32.dll ole32.dll
        oleaut32.dll ws2_32.dll bcrypt.dll bcryptprimitives.dll ntdll.dll msvcrt.dll
        d3d12.dll dxgi.dll d3dcompiler_47.dll dwmapi.dll uxtheme.dll imm32.dll winmm.dll
        comctl32.dll comdlg32.dll propsys.dll userenv.dll crypt32.dll secur32.dll
        opengl32.dll ksuser.dll avrt.dll mmdevapi.dll cfgmgr32.dll setupapi.dll
        version.dll dbghelp.dll synchronization.dll powrprof.dll shlwapi.dll
        hid.dll winhttp.dll iphlpapi.dll ncrypt.dll combase.dll uiautomationcore.dll "
    # one space between names, whatever the line breaks above
    allow=" $(echo $allow | tr 'A-Z' 'a-z') "
    for b in "$@"; do
        echo "$b imports:"
        for dll in $(x86_64-w64-mingw32-objdump -p "$dir/$b" | sed -n 's/^\s*DLL Name: //p'); do
            lc=$(printf '%s' "$dll" | tr 'A-Z' 'a-z')
            echo "  $dll"
            case "$lc" in
                av*.dll | sw*.dll | postproc*.dll)
                    echo "ERROR: $b imports $dll — FFmpeg must be loaded at run time (invariant No. 10)" >&2
                    bad=1; continue ;;
                libgcc_s*.dll | libstdc++*.dll | libwinpthread*.dll)
                    echo "ERROR: $b imports the mingw runtime $dll, absent on a clean Windows" >&2
                    bad=1; continue ;;
                api-ms-win-*.dll) continue ;;
            esac
            case "$allow" in
                *" $lc "*) ;;
                *) echo "ERROR: $b imports $dll, which is not in the Windows system allow-list" >&2
                   bad=1 ;;
            esac
        done
        echo "$b: size $(du -h "$dir/$b" | cut -f1)"
    done
    return $bad
}

# A separate target inside the workspace (does not overlap with host builds).
# RUSTFLAGS= disables the host's .cargo/config.toml (R.68): it sets mold, which
# is absent in the Ubuntu 22.04 container (and gcc 11 does not know `-fuse-ld=mold`). An empty
# variable beats the config — cargo takes it. A development accelerator must not have
# the right to become a condition for building what ships to people.
BIN_DIR="target/dist-container/dist"
CARGO_TARGET_ARGS=""
CHECKS="delivery_checks"
if [ "$TARGET" = windows ]; then
    BIN_DIR="target/dist-container/x86_64-pc-windows-gnu/dist"
    CARGO_TARGET_ARGS="--target x86_64-pc-windows-gnu"
    CHECKS="pe_checks"
fi
# Anonymous usage telemetry (change telemetry-posthog): the public, write-only PostHog
# project key enters ONLY the release binary (option_env! in telemetry.rs); cargo run, tests,
# finish.sh and the live scenarios build without it and never report. An empty or missing
# file gives a release that sends nothing.
# Each target cleans ONLY its own packages: the release workflow builds both into one
# dist-out, and a Windows build must not delete the Linux tarball next to it.
clean_out() {
    mkdir -p "$OUT"
    find "$OUT" -maxdepth 1 -name "kinetren-*-$TARGET-*" -exec rm -rf {} +
    if [ "$TARGET" = windows ]; then rm -f "$OUT/LATEST-windows"; else rm -f "$OUT/LATEST"; fi
}

KINETREN_POSTHOG_KEY=$(tr -d '[:space:]' < packaging/posthog-key 2>/dev/null || true)

if ! docker run --rm --network=host -u "$UID_GID" \
    -v "$PWD:/work" -w /work -v "$REGISTRY:/usr/local/cargo/registry" \
    -e CARGO_TARGET_DIR=/work/target/dist-container \
    -e RUSTFLAGS= \
    -e KT_VERSION_FULL="$KT_VERSION_FULL" -e KT_VERSION_SHORT="$KT_VERSION_SHORT" \
    -e KINETREN_POSTHOG_KEY="$KINETREN_POSTHOG_KEY" \
    "$IMAGE" bash -c "set -euo pipefail
$(declare -f delivery_checks pe_checks)
cargo build --profile dist $CARGO_TARGET_ARGS -p kt-app -p kt-cli
$CHECKS $BIN_DIR $BINS"; then
    # a failed build or check must not leave a package that looks good
    clean_out
    echo "release-build.sh: build or delivery check failed, no package" >&2
    exit 1
fi

# we clean the output of this target: before, old tarballs and unpacked directories
# piled up forever, and the user could take the wrong build (item 7 round 12)
clean_out

if [ "$TARGET" = windows ]; then
    # --- Zip (Windows) -----------------------------------------------------
    # The licence lives at the repository root, outside the mounted src/: it is copied
    # on the host, and the zip is made inside the container (zip is in the image).
    mkdir -p "$STAGE"
    for b in $BINS; do cp "$BIN_DIR/$b" "$STAGE/"; done
    cp packaging/README-windows.txt "$STAGE/"
    cp ../LICENSE "$STAGE/LICENSE.txt"
    case " $(ls "$STAGE" | tr 'A-Z' 'a-z' | tr '\n' ' ') " in
        *.dll*) echo "release-build.sh: a DLL got into the zip stage" >&2; clean_out; exit 1 ;;
    esac
    NAME=$(basename "$STAGE")
    docker run --rm -u "$UID_GID" -v "$PWD:/work" -w "/work/$OUT" "$IMAGE" bash -c \
        "set -euo pipefail; zip -qr '$NAME.zip' '$NAME'; sha256sum '$NAME.zip' > '$NAME.zip.sha256'"
    printf '%s\n%s\n' "$KT_VERSION_FULL" "$STAGE.zip" > "$OUT/LATEST-windows"
    echo "Done: $STAGE.zip (version $KT_VERSION_FULL)"
    exit 0
fi

# --- Tarball ---------------------------------------------------------------
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
