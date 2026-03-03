SHELL := /bin/bash

# Detect host triple for cross-compilation support.
# Override: make build TARGET=x86_64-unknown-linux-gnu
HOST_TRIPLE := $(shell rustc -vV 2>/dev/null | awk '/^host:/{print $$2}')
TARGET      ?= $(HOST_TRIPLE)

ifeq ($(TARGET),$(HOST_TRIPLE))
  DEBUG_DIR   := target/debug
  RELEASE_DIR := target/release
else
  DEBUG_DIR   := target/$(TARGET)/debug
  RELEASE_DIR := target/$(TARGET)/release
endif

.PHONY: help build release build-linux build-all run dev info list check test clean install link link-release docker-build docker-shell docker-cp docker-release

LINUX_TARGET := x86_64-unknown-linux-gnu

# ---------------------------------------------------------------------------
# Help
# ---------------------------------------------------------------------------

help: ## Show this help
	@echo "postlab commands:"; echo
	@grep -E '^[a-zA-Z0-9_-]+:.*?## .*$$' $(MAKEFILE_LIST) \
		| awk 'BEGIN {FS=":.*?## "}; {printf "  \033[36m%-16s\033[0m %s\n", $$1, $$2}' \
		| sort

# ---------------------------------------------------------------------------
# Build
# ---------------------------------------------------------------------------

build: ## Dev build
	cargo build -p postlab $(if $(filter-out $(HOST_TRIPLE),$(TARGET)),--target $(TARGET))

release: ## Release build (stripped, LTO, ~8–15 MB)
	cargo build -p postlab --release $(if $(filter-out $(HOST_TRIPLE),$(TARGET)),--target $(TARGET))
	@echo "  Binary: $(RELEASE_DIR)/postlab"
	@ls -lh $(RELEASE_DIR)/postlab

build-linux: ## Release build for x86_64 Linux via zigbuild (requires: cargo install cargo-zigbuild)
	cargo zigbuild -p postlab --release --target $(LINUX_TARGET)
	@mkdir -p binaries/$(LINUX_TARGET)
	@rm -f binaries/$(LINUX_TARGET)/postlab
	@cp -rf ./target/$(LINUX_TARGET)/release/postlab binaries/$(LINUX_TARGET)/postlab
	@echo "  binaries/$(LINUX_TARGET)/postlab"
	@ls -lh binaries/$(LINUX_TARGET)/postlab

build-all: ## Build release binaries for all targets (native + x86_64 Linux)
	$(MAKE) release
	$(MAKE) build-linux
	@echo; echo "  Binaries:"
	@cp $(RELEASE_DIR)/postlab binaries/$(LINUX_TARGET)/postlab

link: ## Symlink dev binary → binaries/<triple>/postlab
	@mkdir -p binaries/$(TARGET)
	@ln -sf ../../$(DEBUG_DIR)/postlab binaries/$(TARGET)/postlab

link-release: ## Symlink release binary → binaries/<triple>/postlab
	@mkdir -p binaries/$(TARGET)
	@ln -sf ../../$(RELEASE_DIR)/postlab binaries/$(TARGET)/postlab

# ---------------------------------------------------------------------------
# Run
# ---------------------------------------------------------------------------

run: ## Launch the TUI (dev build)
	cargo run -p postlab

dev: ## Watch + auto-restart on file changes (requires: cargo install cargo-watch)
	sudo cargo watch -x 'run -p postlab'

info: ## Print system info (no TUI)
	cargo run -p postlab -- info

list: ## Print installed packages (no TUI)
	cargo run -p postlab -- list

# ---------------------------------------------------------------------------
# Quality
# ---------------------------------------------------------------------------

check: ## Type-check + clippy
	cargo check -p postlab
	cargo clippy -p postlab -- -D warnings

test: ## Run unit tests
	cargo test -p postlab

# ---------------------------------------------------------------------------
# Install
# ---------------------------------------------------------------------------

install: release ## Install postlab binary to /usr/local/bin
	@echo "Installing $(RELEASE_DIR)/postlab → /usr/local/bin/postlab"
	install -m 0755 $(RELEASE_DIR)/postlab /usr/local/bin/postlab
	@echo "Done. Run: postlab"

# ---------------------------------------------------------------------------
# Docker — Fedora dev/test container
# ---------------------------------------------------------------------------

docker-build: ## Build Fedora dev image
	docker compose -f docker/docker-compose.yml build

docker-shell: ## Shell into running Fedora container
	docker exec -it postlab-fedora-dev bash

docker-cp: ## Copy Linux binary → running Fedora container (/usr/local/bin/postlab)
	docker cp target/$(LINUX_TARGET)/release/postlab postlab-fedora-dev:/usr/local/bin/postlab

docker-release: ## Build release Linux/amd64 binary inside Fedora container
	docker compose -f docker/docker-compose.yml up -d
	docker exec postlab-fedora-dev bash -c \
		"cd /workspace && cargo build -p postlab --release 2>&1"
	@mkdir -p binaries/x86_64-unknown-linux-gnu
	docker exec postlab-fedora-dev bash -c \
		"cp /workspace/target/release/postlab /workspace/binaries/x86_64-unknown-linux-gnu/"
	@echo "  binaries/x86_64-unknown-linux-gnu/postlab"
	@ls -lh binaries/x86_64-unknown-linux-gnu/postlab

# ---------------------------------------------------------------------------
# Maintenance
# ---------------------------------------------------------------------------

clean: ## Remove Rust build artifacts
	cargo clean

# End of Makefile
