# The one build container of Kinetren (change github-ci-docker-build).
#
# Ubuntu 22.04 = glibc 2.35: a binary built here runs on any system with glibc
# >= 2.35 and the system FFmpeg 6/7/8 (loaded at run time, invariant 10 — no
# libav* linking, so no FFmpeg packages are needed to build).
#
# Used by src/scripts/release-build.sh, the root Makefile and the release
# workflow — never copy this recipe anywhere else. RUST_TOOLCHAIN must equal
# the value in .github/workflows/ci.yml; src/scripts/dist-check.py (t834)
# keeps the two equal. Bump both together.
#
# Stages (change windows-build): `base` is the Linux build; `windows` adds the
# cross toolchain for x86_64-pc-windows-gnu (mingw-w64, clang for bindgen with
# the Windows target, wine for running the Windows tests, zip); the last stage
# `linux` is what a plain `docker build` produces, so the Linux build is
# unchanged. Select the Windows image with `--target windows`.
FROM ubuntu:22.04 AS base
ARG RUST_TOOLCHAIN=1.97.1
ENV DEBIAN_FRONTEND=noninteractive
# binutils: the delivery checks (ldd, objdump -T) run inside this container,
# so the host needs nothing but Docker
RUN apt-get update && apt-get install -y --no-install-recommends \
    curl ca-certificates build-essential pkg-config clang libclang-dev \
    libasound2-dev binutils \
    && rm -rf /var/lib/apt/lists/*
ENV RUSTUP_HOME=/usr/local/rustup CARGO_HOME=/usr/local/cargo \
    PATH=/usr/local/cargo/bin:$PATH
RUN curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal \
    --default-toolchain "$RUST_TOOLCHAIN" --no-modify-path \
    && chmod -R a+w /usr/local/rustup /usr/local/cargo

FROM base AS windows
# mingw-w64 links the exe and compiles the bundled SQLite; the PE delivery check
# uses x86_64-w64-mingw32-objdump. Wine 6.0 of jammy is too old for current std
# and nextest, so wine comes from WineHQ's repository for jammy.
RUN dpkg --add-architecture i386 && apt-get update \
    && apt-get install -y --no-install-recommends \
       gcc-mingw-w64-x86-64 binutils-mingw-w64-x86-64 zip unzip p7zip-full gnupg \
    && mkdir -pm755 /etc/apt/keyrings \
    && curl -fsSL https://dl.winehq.org/wine-builds/winehq.key \
       | gpg --dearmor -o /etc/apt/keyrings/winehq-archive.key \
    && curl -fsSL -o /etc/apt/sources.list.d/winehq-jammy.sources \
       https://dl.winehq.org/wine-builds/ubuntu/dists/jammy/winehq-jammy.sources \
    && apt-get update && apt-get install -y --install-recommends winehq-stable \
    && rm -rf /var/lib/apt/lists/*
# the default toolchain is the pinned one installed by the base stage (an ARG
# does not cross stages, so it is not named again here)
RUN rustup target add x86_64-pc-windows-gnu \
    && chmod -R a+w /usr/local/rustup /usr/local/cargo
# nextest runs the cross-built tests under wine (`make windows-test`) and packs the archive
# the windows-latest CI job runs; the same version as the host's finish.sh
RUN curl -fsSL https://github.com/nextest-rs/nextest/releases/download/cargo-nextest-0.9.146/cargo-nextest-0.9.146-x86_64-unknown-linux-gnu.tar.gz \
    | tar xzf - -C /usr/local/cargo/bin && cargo nextest --version
ENV CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=x86_64-w64-mingw32-gcc \
    CC_x86_64_pc_windows_gnu=x86_64-w64-mingw32-gcc \
    AR_x86_64_pc_windows_gnu=x86_64-w64-mingw32-ar \
    CARGO_TARGET_X86_64_PC_WINDOWS_GNU_RUNNER=wine \
    WINEDEBUG=-all

FROM base AS linux
