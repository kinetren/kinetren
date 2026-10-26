# Kinetren — build in Docker, run on the host (change github-ci-docker-build).
#
# The build itself lives in src/scripts/release-build.sh and the container
# recipe in src/packaging/Dockerfile; these targets only call them, so
# `make`, `finish.sh --release` and the release workflow cannot drift apart.
# Building needs Docker, make and git; running needs the system FFmpeg 6/7/8/9.

RUST     := src
DIST     := $(RUST)/target/dist-out
# the unpacked tarball directory of the last build (there is only one:
# release-build.sh clears dist-out before packing)
STAGEDIR  = $(firstword $(patsubst %/kinetren,%,$(wildcard $(DIST)/kinetren-*-linux-*/kinetren)))

.PHONY: all build install run test clean help docker-check windows windows-test ffmpeg-test

all: build ## the default: same as `make build`

help: ## list the targets
	@echo "Kinetren — make targets:"
	@grep -E '^[a-z-]+:.*## ' $(MAKEFILE_LIST) | sed -E 's/^([a-z-]+):.*## /  make \1\t/' | expand -t 20

build: docker-check ## build the release tarball in Docker (src/target/dist-out)
	$(RUST)/scripts/release-build.sh

windows: docker-check ## cross-build the Windows zip in Docker (src/target/dist-out)
	$(RUST)/scripts/release-build.sh --target windows

windows-test: docker-check ## developers: the workspace tests for Windows under wine, in Docker
	$(RUST)/scripts/windows-test.sh

ffmpeg-test: docker-check ## developers: the FFmpeg tests against FFmpeg 9 built from source, in Docker
	$(RUST)/scripts/ffmpeg-test.sh

docker-check:
	@command -v docker > /dev/null || { \
	  echo "make: Docker is required to build Kinetren: https://docs.docker.com/engine/install/"; \
	  exit 1; }

install: ## build if needed, then install into ~/.local (kinetren, kt-cli, menu entry)
	@[ -n "$(STAGEDIR)" ] || $(MAKE) --no-print-directory build
	cd $(DIST)/kinetren-*-linux-*/ && ./install.sh

run: ## start the built kinetren without installing it
	@[ -n "$(STAGEDIR)" ] || { echo "make: nothing built yet — run \`make\` first"; exit 1; }
	"$(STAGEDIR)/kinetren"

test: ## developers: cargo test on the host (needs Rust, clang, libasound2-dev, ffmpeg)
	@command -v cargo > /dev/null || { \
	  echo "make: \`make test\` runs on the host and needs a Rust toolchain, clang/libclang,"; \
	  echo "      pkg-config, libasound2-dev and ffmpeg — see CONTRIBUTING.md"; \
	  exit 1; }
	cd $(RUST) && cargo test --workspace

clean: ## remove the tarball and the container's build directory (keeps the crate cache)
	rm -rf $(DIST) $(RUST)/target/dist-container
