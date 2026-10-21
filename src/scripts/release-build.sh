#!/usr/bin/env bash
# Релизная сборка Kinetren в контейнере Ubuntu 22.04 (glibc 2.35):
# бинарь работает на любой системе с glibc >= 2.35 и системным FFmpeg 6/7/8
# (грузится в рантайме, инвариант №10 — линковки с libav* нет).
#
# Использование:  scripts/release-build.sh
# Результат:      target/dist-out/kinetren-<версия>-linux-x86_64.tar.gz
set -euo pipefail
cd "$(dirname "$0")/.." # src/

IMAGE=kinetren-build:ubuntu2204
# Версия считается ЗДЕСЬ, на хосте (п.6 р.12): внутри контейнера нет ни
# git, ни каталога .git — монтируется только src/. Формат vYY.MM.DD-hash
# по дате КОММИТА; build.rs крейта kt-version возьмёт её из окружения.
KT_VERSION_FULL="${KT_VERSION_FULL:-$(git log -1 --format=%cd --date=format:%y.%m.%d 2>/dev/null | sed 's/^/kt/')-$(git rev-parse --short=8 HEAD 2>/dev/null)}"
[ "$KT_VERSION_FULL" = "kt-" ] && KT_VERSION_FULL="kt00.00.00-nogit"
KT_VERSION_SHORT="${KT_VERSION_SHORT:-kt${KT_VERSION_FULL:2:5}}"
export KT_VERSION_FULL KT_VERSION_SHORT
VERSION="${KT_VERSION_FULL#kt}"
ARCH=$(uname -m)
OUT="target/dist-out"
STAGE="$OUT/kinetren-$VERSION-linux-$ARCH"

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
# RUSTFLAGS= гасит .cargo/config.toml хоста (Р.68): там стоит mold, которого
# в контейнере Ubuntu 22.04 нет (и gcc 11 не знает `-fuse-ld=mold`). Пустая
# переменная сильнее конфига — cargo берёт её. Ускоритель разработки не имеет
# права стать условием сборки того, что уезжает людям.
docker run --rm --network=host -u "$(id -u):$(id -g)" \
    -v "$PWD:/work" -w /work "${CACHE_ARGS[@]}" \
    -e CARGO_TARGET_DIR=/work/target/dist-container \
    -e RUSTFLAGS= \
    -e KT_VERSION_FULL="$KT_VERSION_FULL" -e KT_VERSION_SHORT="$KT_VERSION_SHORT" \
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
# чистим ВЕСЬ выхлоп: раньше старые тарболы и распакованные каталоги
# копились вечно, и пользователь мог взять не ту сборку (п.7 р.12)
rm -rf "$OUT"
mkdir -p "$STAGE"
cp "$BIN_DIR/kt-app" "$BIN_DIR/kt-cli" "$STAGE/"
cp packaging/kinetren.desktop packaging/kinetren.svg \
    packaging/kinetren-mime.xml packaging/install.sh "$STAGE/"
cp -r packaging/icons "$STAGE/"
chmod +x "$STAGE/install.sh"
tar -C "$OUT" -czf "$STAGE.tar.gz" "$(basename "$STAGE")"
printf '%s\n%s\n' "$KT_VERSION_FULL" "$STAGE.tar.gz" > "$OUT/LATEST"
echo "Готово: $STAGE.tar.gz (версия $KT_VERSION_FULL)"
