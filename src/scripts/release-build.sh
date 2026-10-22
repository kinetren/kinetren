#!/usr/bin/env bash
# Релизная сборка Kinetren в контейнере Ubuntu 22.04 (glibc 2.35):
# бинарь работает на любой системе с glibc >= 2.35 и системным FFmpeg 6/7/8
# (грузится в рантайме, инвариант №10 — линковки с libav* нет).
#
# The container recipe is src/packaging/Dockerfile (github-ci-docker-build):
# this script, the root Makefile and the release workflow all build through
# it. The host needs only Docker and git (git only for the version string).
#
# Использование:  scripts/release-build.sh
# Результат:      target/dist-out/kinetren-<версия>-linux-x86_64.tar.gz
#                 + .sha256 next to it + LATEST (version and tarball path)
set -euo pipefail
cd "$(dirname "$0")/.." # src/

if ! command -v docker > /dev/null; then
    echo "release-build.sh: Docker is required: https://docs.docker.com/engine/install/" >&2
    exit 1
fi

IMAGE=kinetren-build:ubuntu2204
# Версия считается ЗДЕСЬ, на хосте (п.6 р.12): внутри контейнера нет ни
# git, ни каталога .git — монтируется только src/. Формат vYY.MM.DD-hash
# по дате КОММИТА; build.rs крейта kt-version возьмёт её из окружения.
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

# --network=host: docker-конфиг пользователя может навязывать контейнерам
# прокси на 127.0.0.1 — в сетевом неймспейсе хоста он достижим.
docker build --network=host -t "$IMAGE" -f packaging/Dockerfile packaging

# Кэш реестра crates.io: с хоста, если есть (сборка владельца); иначе —
# именованный том, чтобы второй `make` у человека без Rust не качал всё
# заново. Свежий том принадлежит root — отдаём его пользователю один раз.
UID_GID="$(id -u):$(id -g)"
if [ -d "$HOME/.cargo/registry" ]; then
    REGISTRY="$HOME/.cargo/registry"
else
    REGISTRY=kinetren-cargo-registry
    docker volume create "$REGISTRY" > /dev/null
    docker run --rm -u 0 -v "$REGISTRY:/usr/local/cargo/registry" "$IMAGE" \
        chown "$UID_GID" /usr/local/cargo/registry
fi

# --- Проверки поставки ---------------------------------------------------
# Run INSIDE the container (binutils are in the image), so the host needs
# no ldd/objdump and every builder checks with the same tools.
delivery_checks() { # delivery_checks <dir> <bin...>
    local dir="$1"; shift
    local b need ver
    # 1. Инвариант №10: никакой линковки с libav*.
    for b in "$@"; do
        if ldd "$dir/$b" | grep -q libav; then
            echo "ОШИБКА: $b слинкован с libav* — нарушение инварианта №10" >&2
            ldd "$dir/$b" | grep libav >&2
            return 1
        fi
    done
    # 2. Потолок требуемого glibc (информативно + защита от сборки вне контейнера).
    for b in "$@"; do
        need=$(objdump -T "$dir/$b" | grep -o 'GLIBC_[0-9.]*' | sort -uV | tail -1)
        echo "$b: требует $need, размер $(du -h "$dir/$b" | cut -f1)"
        ver=${need#GLIBC_}
        if [ "$(printf '%s\n' "$ver" 2.35 | sort -V | tail -1)" != "2.35" ]; then
            echo "ОШИБКА: $b требует glibc $ver > 2.35 — собрано не в контейнере?" >&2
            return 1
        fi
    done
}

# Отдельный target внутри workspace (не пересекается с хостовыми сборками).
# RUSTFLAGS= гасит .cargo/config.toml хоста (Р.68): там стоит mold, которого
# в контейнере Ubuntu 22.04 нет (и gcc 11 не знает `-fuse-ld=mold`). Пустая
# переменная сильнее конфига — cargo берёт её. Ускоритель разработки не имеет
# права стать условием сборки того, что уезжает людям.
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

# --- Тарбол --------------------------------------------------------------
# чистим ВЕСЬ выхлоп: раньше старые тарболы и распакованные каталоги
# копились вечно, и пользователь мог взять не ту сборку (п.7 р.12)
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
echo "Готово: $STAGE.tar.gz (версия $KT_VERSION_FULL)"
