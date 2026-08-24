#!/usr/bin/env bash
# Релизная сборка Kinotrain в контейнере Ubuntu 22.04 (glibc 2.35):
# бинарь работает на любой системе с glibc >= 2.35 и системным FFmpeg 6/7/8
# (грузится в рантайме, инвариант №10 — линковки с libav* нет).
#
# Использование:  scripts/release-build.sh
# Результат:      target/dist-out/kinotrain-<версия>-linux-x86_64.tar.gz
set -euo pipefail
cd "$(dirname "$0")/.." # src/

IMAGE=kinotrain-build:ubuntu2204
VERSION=$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)
ARCH=$(uname -m)
OUT="target/dist-out"
STAGE="$OUT/kinotrain-$VERSION-linux-$ARCH"

# --network=host: docker-конфиг пользователя может навязывать контейнерам
# прокси на 127.0.0.1 — в сетевом неймспейсе хоста он достижим.
docker build --network=host -t "$IMAGE" - <<'EOF'
FROM ubuntu:22.04
ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update && apt-get install -y --no-install-recommends \
    curl ca-certificates build-essential pkg-config clang libclang-dev \
    libasound2-dev binutils \
    && rm -rf /var/lib/apt/lists/*
ENV RUSTUP_HOME=/usr/local/rustup CARGO_HOME=/usr/local/cargo \
    PATH=/usr/local/cargo/bin:$PATH
RUN curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal \
    --default-toolchain stable --no-modify-path \
    && chmod -R a+w /usr/local/rustup /usr/local/cargo
EOF

# Отдельный target внутри workspace (не пересекается с хостовыми сборками);
# кэш реестра crates.io — с хоста, если есть.
CACHE_ARGS=()
if [ -d "$HOME/.cargo/registry" ]; then
    CACHE_ARGS=(-v "$HOME/.cargo/registry:/usr/local/cargo/registry")
fi
docker run --rm --network=host -u "$(id -u):$(id -g)" \
    -v "$PWD:/work" -w /work "${CACHE_ARGS[@]}" \
    -e CARGO_TARGET_DIR=/work/target/dist-container \
    "$IMAGE" cargo build --profile dist -p kt-app -p kt-cli

BIN_DIR="target/dist-container/dist"

# --- Проверки поставки ---------------------------------------------------
# 1. Инвариант №10: никакой линковки с libav*.
for b in kt-app kt-cli; do
    if ldd "$BIN_DIR/$b" | grep -q libav; then
        echo "ОШИБКА: $b слинкован с libav* — нарушение инварианта №10" >&2
        ldd "$BIN_DIR/$b" | grep libav >&2
        exit 1
    fi
done
# 2. Потолок требуемого glibc (информативно + защита от сборки вне контейнера).
for b in kt-app kt-cli; do
    need=$(objdump -T "$BIN_DIR/$b" | grep -o 'GLIBC_[0-9.]*' | sort -uV | tail -1)
    echo "$b: требует $need, размер $(du -h "$BIN_DIR/$b" | cut -f1)"
    ver=${need#GLIBC_}
    if [ "$(printf '%s\n' "$ver" 2.35 | sort -V | tail -1)" != "2.35" ]; then
        echo "ОШИБКА: $b требует glibc $ver > 2.35 — собрано не в контейнере?" >&2
        exit 1
    fi
done

# --- Тарбол --------------------------------------------------------------
rm -rf "$STAGE"
mkdir -p "$STAGE"
cp "$BIN_DIR/kt-app" "$BIN_DIR/kt-cli" "$STAGE/"
cp packaging/kinotrain.desktop packaging/kinotrain.svg \
    packaging/kinotrain-mime.xml packaging/install.sh "$STAGE/"
chmod +x "$STAGE/install.sh"
tar -C "$OUT" -czf "$STAGE.tar.gz" "$(basename "$STAGE")"
echo "Готово: $STAGE.tar.gz"
