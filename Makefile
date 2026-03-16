.PHONY: help all dev stop restart services-up server ui gateway gateway-compile services db seed check install

# ── Config ─────────────────────────────────────────────────────────────────────
DB_URL    := postgres://burst:burst@localhost:5432/burst

BARBACANE_DIR := ../Barbacane
BARBACANE_BIN := $(BARBACANE_DIR)/target/release/barbacane
BURST_BCA     := burst-api.bca

# ── Help ───────────────────────────────────────────────────────────────────────
help: ## Show this help
	@awk 'BEGIN {FS = ":.*##"; printf "\nUsage:\n  make \033[36m<target>\033[0m\n\nTargets:\n"} \
	/^[a-zA-Z_-]+:.*?##/ { printf "  \033[36m%-12s\033[0m %s\n", $$1, $$2 }' $(MAKEFILE_LIST)

# ── Combined ───────────────────────────────────────────────────────────────────
all: services-up gateway-compile ## Start gateway + server + UI via overmind
	overmind start

stop: ## Stop overmind processes and free ports (Docker stays running)
	overmind quit 2>/dev/null || true
	@for port in 3000 5173 8080; do \
		pid=$$(lsof -ti :$$port 2>/dev/null); \
		[ -n "$$pid" ] && kill $$pid 2>/dev/null && echo "killed pid $$pid on :$$port"; \
	done; true

restart: gateway-compile stop services-up ## Recompile gateway, stop everything, then start fresh
	overmind start

services-up: ## Ensure Docker services are running and healthy
	@docker compose -f docker-compose.dev.yml up -d
	@until curl -sf http://localhost:9099/burst/.well-known/openid-configuration >/dev/null 2>&1; do \
		sleep 0.5; \
	done
	@echo "Services ready"

dev: ## Print instructions for running the full stack
	@echo ""
	@echo "  First, compile the gateway artifact:"
	@echo ""
	@echo "    make gateway-compile"
	@echo ""
	@echo "  Then open four terminals and run:"
	@echo ""
	@echo "    make services    # PostgreSQL + mock OIDC (Docker)"
	@echo "    make server      # Burst API on :3000"
	@echo "    make gateway     # Barbacane gateway on :8080"
	@echo "    make ui          # Vite dev server on :5173"
	@echo ""
	@echo "  Then open http://localhost:5173"

# ── Gateway ───────────────────────────────────────────────────────────────────
$(BARBACANE_BIN):
	cargo build --release --manifest-path $(BARBACANE_DIR)/Cargo.toml

gateway-compile: $(BARBACANE_BIN) ## Compile the Burst OpenAPI spec into a Barbacane artifact
	$(BARBACANE_BIN) compile \
		--spec specs/burst-api.yaml \
		--manifest barbacane.yaml \
		--output $(BURST_BCA) \
		--allow-plaintext
	@echo "Compiled $(BURST_BCA)"

gateway: $(BURST_BCA) ## Run the Barbacane gateway (requires: make gateway-compile)
	$(BARBACANE_BIN) serve \
		--artifact $(BURST_BCA) \
		--listen 0.0.0.0:8080 \
		--dev \
		--allow-plaintext-upstream \
		--max-body-size 10485760 \
		--log-format pretty

# ── Dev Services ──────────────────────────────────────────────────────────
services: ## Run PostgreSQL + mock OIDC server (Docker)
	docker compose -f docker-compose.dev.yml up

# ── Backend ────────────────────────────────────────────────────────────────────
server: ## Run the Burst API server
	RUST_LOG=info,burst=debug,burst_server=debug cargo run --bin burst -- burst.toml

server-release: ## Run with release build
	cargo build --release && RUST_LOG=info ./target/release/burst burst.toml

# ── Frontend ───────────────────────────────────────────────────────────────────
ui: ## Run the Vite dev server (proxies API to localhost:8080)
	cd ui && npm run dev

# ── Database ───────────────────────────────────────────────────────────────────
db: ## Open a psql shell on the burst database
	docker compose -f docker-compose.dev.yml exec postgres psql -U burst burst

seed: ## Seed the database with sample users
	cargo run --example seed -- $(DB_URL)

# ── Quality ────────────────────────────────────────────────────────────────────
check: ## Run fmt, clippy, and tests
	cargo fmt --all
	cargo clippy --all-targets -- -D warnings
	DATABASE_URL=$(DB_URL) cargo test

# ── Tooling ────────────────────────────────────────────────────────────────────
install: ## Install dev tooling (overmind)
	brew install overmind
