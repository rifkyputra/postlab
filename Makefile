# Postlab — development shortcuts
# Usage: make help

SHELL := /bin/bash
DB_URL ?= sqlite://postlab.db?mode=rwc

.PHONY: help \
        dev \
        build build-release check test \
        server cli \
        web-install web-dev web-build web-test \
        db-reset \
        docker-keygen docker-up docker-down docker-ssh-ubuntu docker-ssh-fedora \
        harden-perms \
        clean

DOCKER_KEY   := docker/test_key
DOCKER_COMPOSE := docker compose -f docker/docker-compose.yml

# ---------------------------------------------------------------------------
# Help
# ---------------------------------------------------------------------------

help: ## Show this help
	@echo "Postlab commands:"; echo
	@grep -E '^[a-zA-Z0-9_-]+:.*?## .*$$' $(MAKEFILE_LIST) \
		| awk 'BEGIN {FS=":.*?## "}; {printf "  \033[36m%-18s\033[0m %s\n", $$1, $$2}' \
		| sort

# ---------------------------------------------------------------------------
# Dev (full stack)
# ---------------------------------------------------------------------------

dev: ## Start API server + web dev server concurrently (Ctrl-C stops both)
	@trap 'kill %1 %2 2>/dev/null; exit 0' INT TERM; \
	DATABASE_URL=$(DB_URL) cargo run -p postlab-server & \
	(cd web && npm run dev) & \
	wait

# ---------------------------------------------------------------------------
# Rust
# ---------------------------------------------------------------------------

build: ## Build all crates (dev)
	cargo build --workspace

build-release: ## Build all crates (release)
	cargo build --workspace --release

check: ## Type-check + lint without building binaries
	cargo check --workspace
	cargo clippy --workspace -- -D warnings

test: ## Run all Rust unit tests
	cargo test --workspace

# ---------------------------------------------------------------------------
# Run
# ---------------------------------------------------------------------------

server: ## Start the API server (port 3000, auto-migrates DB)
	DATABASE_URL=$(DB_URL) cargo run -p postlab-server

cli: ## Launch the interactive CLI wizard
	cargo run -p postlab-cli

# ---------------------------------------------------------------------------
# Web
# ---------------------------------------------------------------------------

web-install: ## Install web dependencies
	cd web && npm install

web-dev: ## Start the SvelteKit dev server (proxies /api → :3000)
	cd web && npm run dev

web-build: ## Production build of the web frontend
	cd web && npm run build

web-test: ## Run Vitest frontend tests
	cd web && npm test

# ---------------------------------------------------------------------------
# Database
# ---------------------------------------------------------------------------

db-reset: ## Delete the local SQLite database (will be re-created on next server start)
	@echo "Removing postlab.db …"
	rm -f postlab.db postlab.db-shm postlab.db-wal
	@echo "Done. Start the server to re-apply migrations."

# ---------------------------------------------------------------------------
# Docker test containers
# ---------------------------------------------------------------------------

docker-keygen: ## Generate SSH test keypair for Docker containers (run once)
	@if [ -f $(DOCKER_KEY) ]; then \
		echo "$(DOCKER_KEY) already exists — delete it first to regenerate."; \
	else \
		ssh-keygen -t ed25519 -f $(DOCKER_KEY) -N "" -C "postlab-test"; \
		echo "Created: $(DOCKER_KEY) and $(DOCKER_KEY).pub"; \
		echo "Now run: make docker-up"; \
	fi

docker-up: ## Build and start Ubuntu (:2222) + Fedora (:2223) SSH containers
	@if [ ! -f $(DOCKER_KEY).pub ]; then \
		echo "No test key found. Run: make docker-keygen"; exit 1; \
	fi
	$(DOCKER_COMPOSE) up -d --build
	@echo ""
	@echo "Containers ready:"
	@echo "  Ubuntu  → localhost:2222  (user: postlab, key: $(DOCKER_KEY))"
	@echo "  Fedora  → localhost:2223  (user: postlab, key: $(DOCKER_KEY))"
	@echo ""
	@echo "Add to Postlab:"
	@echo "  cargo run -p postlab-cli -- server add --name ubuntu-test --host localhost --port 2222 --user postlab --key $(CURDIR)/$(DOCKER_KEY)"
	@echo "  cargo run -p postlab-cli -- server add --name fedora-test --host localhost --port 2223 --user postlab --key $(CURDIR)/$(DOCKER_KEY)"

docker-down: ## Stop and remove test containers
	$(DOCKER_COMPOSE) down

docker-ssh-ubuntu: ## Open a shell in the Ubuntu test container
	ssh -i $(DOCKER_KEY) -p 2222 \
		-o StrictHostKeyChecking=no \
		-o UserKnownHostsFile=/dev/null \
		postlab@localhost

docker-ssh-fedora: ## Open a shell in the Fedora test container
	ssh -i $(DOCKER_KEY) -p 2223 \
		-o StrictHostKeyChecking=no \
		-o UserKnownHostsFile=/dev/null \
		postlab@localhost

# ---------------------------------------------------------------------------
# Harden scripts
# ---------------------------------------------------------------------------

harden-perms: ## Make all harden-security scripts executable
	chmod +x harden-security/*.sh harden-security/lib/*.sh

# ---------------------------------------------------------------------------
# Maintenance
# ---------------------------------------------------------------------------

clean: ## Remove Rust build artifacts and web build output
	cargo clean
	rm -rf web/.svelte-kit web/build

# End of Makefile
