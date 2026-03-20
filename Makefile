.PHONY: help all dev stop restart services-up server ui gateway gateway-compile services db seed lint-spec check smoke smoke-s3 e2e install

# ── Config ─────────────────────────────────────────────────────────────────────
DB_URL    := postgres://burst:burst@localhost:5432/burst

BARBACANE_VERSION := 0.5.1
BARBACANE_BIN     := .barbacane/bin/barbacane
BURST_BCA         := burst-api.bca

# Detect platform for binary download
UNAME_S := $(shell uname -s)
UNAME_M := $(shell uname -m)
# macOS reports arm64, release assets use aarch64
ifeq ($(UNAME_M),arm64)
  ARCH := aarch64
else
  ARCH := $(UNAME_M)
endif
ifeq ($(UNAME_S),Darwin)
  BARBACANE_TARGET := $(ARCH)-apple-darwin
else
  BARBACANE_TARGET := $(ARCH)-unknown-linux-gnu
endif

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
	@mkdir -p .barbacane/bin
	@echo "Downloading barbacane v$(BARBACANE_VERSION) ($(BARBACANE_TARGET))..."
	@curl -fSL -o $(BARBACANE_BIN) \
		https://github.com/barbacane-dev/barbacane/releases/download/v$(BARBACANE_VERSION)/barbacane-$(BARBACANE_TARGET)
	@chmod +x $(BARBACANE_BIN)
	@echo "Installed $(BARBACANE_BIN)"

gateway-compile: $(BARBACANE_BIN) ## Compile the Burst OpenAPI spec into Barbacane artifacts
	$(BARBACANE_BIN) compile \
		--spec specs/burst-api.yaml \
		--manifest barbacane.yaml \
		--output $(BURST_BCA) \
		--allow-plaintext
	@echo "Compiled $(BURST_BCA)"
	$(BARBACANE_BIN) compile \
		--spec specs/burst-s3.yaml \
		--manifest barbacane-s3.yaml \
		--output burst-s3.bca \
		--allow-plaintext
	@echo "Compiled burst-s3.bca"

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

BARBACANE_FUNCTIONS := barbacane-auth-opt-out barbacane-no-duplicate-middlewares \
	barbacane-no-plaintext-upstream barbacane-no-unknown-extensions \
	barbacane-valid-path-params barbacane-valid-secret-refs \
	barbacane-validate-dispatch-config barbacane-validate-middleware-config

specs/functions/.barbacane-fetched:
	@for f in $(BARBACANE_FUNCTIONS); do \
		curl -fsSL "https://docs.barbacane.dev/rulesets/functions/$${f}.js" \
			-o "specs/functions/$${f}.js"; \
	done
	@touch $@

lint-spec: specs/functions/.barbacane-fetched ## Lint OpenAPI spec with vacuum
	vacuum lint -f specs/functions specs/burst-api.yaml -r specs/.vacuum.yaml

# ── Quality ────────────────────────────────────────────────────────────────────
check: ## Run fmt, clippy, and tests
	cargo fmt --all
	cargo clippy --all-targets -- -D warnings
	DATABASE_URL=$(DB_URL) cargo test

smoke: ## Run k6 smoke tests (requires: make all running in another terminal)
	k6 run tests/http/smoke.js

smoke-s3: ## Run k6 S3 storage smoke test (requires: RustFS + Barbacane S3 dispatcher)
	k6 run tests/http/smoke-s3.js

e2e: ## Run Playwright E2E tests (requires: make all running in another terminal)
	cd ui && npx playwright test

# ── Tooling ────────────────────────────────────────────────────────────────────
install: ## Install dev tooling (overmind)
	brew install overmind
