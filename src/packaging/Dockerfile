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
FROM ubuntu:22.04
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
