.PHONY: help all dev stop restart require-engine require-procfile-runner services-up server ui gateway gateway-compile services db seed lint-spec check smoke smoke-s3 e2e install

# ── Config ─────────────────────────────────────────────────────────────────────
DB_URL    := postgres://burst:burst@localhost:5432/burst

# Container engine. Docker when present, podman otherwise; both accept the
# `compose` subcommand. Override with CONTAINER_ENGINE=... for anything else.
CONTAINER_ENGINE ?= $(shell command -v docker 2>/dev/null || command -v podman 2>/dev/null)
COMPOSE          := $(CONTAINER_ENGINE) compose

# Procfile runner. Overmind when present, hivemind otherwise; they read the same
# Procfile but are invoked differently, and only overmind can stop a running set
# from another shell.
OVERMIND := $(shell command -v overmind 2>/dev/null)
HIVEMIND := $(shell command -v hivemind 2>/dev/null)
ifneq ($(OVERMIND),)
  PROC_START := $(OVERMIND) start
  PROC_STOP  := $(OVERMIND) quit 2>/dev/null || true
else
  PROC_START := $(HIVEMIND)
  PROC_STOP  := true
endif

BARBACANE_VERSION ?= 0.12.2
# Downloaded from the release by default. Point it at a local build to run
# ahead of a release, which works while the change stays out of the plugins
# and the artifact format:
#   BARBACANE_BIN=../barbacane/target/release/barbacane make gateway
BARBACANE_BIN     ?= .barbacane/bin/barbacane-$(BARBACANE_VERSION)
# The Procfile reads it too, and overmind inherits this process's environment,
# so export it for `make BARBACANE_BIN=... all` as well as the env form.
export BARBACANE_BIN
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
all: require-procfile-runner .env burst.toml ui/node_modules services-up gateway-compile ## Start gateway + server + UI
	$(PROC_START)

stop: ## Stop the process set and free ports (containers stay running)
	@$(PROC_STOP)
	@for port in 3000 5173 8080; do \
		pid=$$(lsof -ti :$$port 2>/dev/null); \
		[ -n "$$pid" ] && kill $$pid 2>/dev/null && echo "killed pid $$pid on :$$port"; \
	done; true

restart: require-procfile-runner .env burst.toml ui/node_modules gateway-compile stop services-up ## Recompile gateway, stop everything, then start fresh
	$(PROC_START)

# Overmind loads .env for the process set, but a target run on its own does
# not get it, and the gateway resolves its env:// references from there.
# Sourced in the recipe so quoting behaves as the shell does.
LOAD_ENV := set -a; . ./.env; set +a;

# The Procfile runs `burst burst.toml`, and the file is gitignored. The example
# already matches the dev stack: the same database, and the local gateway's
# address in trusted_proxies.
burst.toml:
	@cp burst.toml.example burst.toml
	@echo "Created burst.toml from burst.toml.example."

# The Procfile runs `npm run dev`, which needs the UI dependencies present.
# Keyed on the lockfile so it reinstalls when that changes.
ui/node_modules: ui/package-lock.json
	@cd ui && npm ci
	@touch ui/node_modules

# Overmind reads .env from the working directory, and the gateway resolves its
# env:// references from there. Seed it rather than failing after the compile.
.env:
	@cp .env.example .env
	@echo "Created .env from .env.example. Edit it if your local ports differ."

require-procfile-runner:
	@test -n "$(PROC_START)" || { \
		echo "No Procfile runner found. Run 'make install', or install overmind"; \
		echo "(https://github.com/DarthSim/overmind) or hivemind."; \
		exit 1; \
	}

require-engine:
	@test -n "$(CONTAINER_ENGINE)" || { \
		echo "No container engine found. Install docker or podman, or set CONTAINER_ENGINE=<path>."; \
		exit 1; \
	}

services-up: require-engine ## Ensure the container services are running and healthy
	@$(COMPOSE) -f docker-compose.dev.yml up -d
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

gateway: $(BARBACANE_BIN) .env ## Run the Barbacane gateway, recompiling on spec changes
	@$(LOAD_ENV) BARBACANE_ALLOW_INTERNAL_EGRESS=true $(BARBACANE_BIN) dev \
		--spec specs/burst-api.yaml \
		--manifest barbacane.yaml \
		--listen 0.0.0.0:8080 \
		--max-body-size 10485760 \
		--log-format pretty

# ── Dev Services ──────────────────────────────────────────────────────────
services: require-engine ## Run PostgreSQL + mock OIDC server
	$(COMPOSE) -f docker-compose.dev.yml up

# ── Backend ────────────────────────────────────────────────────────────────────
server: burst.toml ## Run the Burst API server
	RUST_LOG=info,burst=debug,burst_server=debug cargo run --bin burst -- burst.toml

server-release: ## Run with release build
	cargo build --release && RUST_LOG=info ./target/release/burst burst.toml

# ── Frontend ───────────────────────────────────────────────────────────────────
ui: ui/node_modules ## Run the Vite dev server (proxies API to localhost:8080)
	cd ui && npm run dev

# ── Database ───────────────────────────────────────────────────────────────────
db: ## Open a psql shell on the burst database
	$(COMPOSE) -f docker-compose.dev.yml exec postgres psql -U burst burst

seed: ## Seed the database with sample users
	cargo run --example seed -- $(DB_URL)

BARBACANE_RULESET_URL := https://docs.barbacane.dev/rulesets

# Downloads the Barbacane ruleset and every custom function it references.
specs/functions/.barbacane-fetched:
	@mkdir -p .barbacane/rulesets
	@curl -fsSL "$(BARBACANE_RULESET_URL)/barbacane.yaml" -o .barbacane/rulesets/barbacane.yaml
	@for f in $$(grep -E '^[[:space:]]*function:[[:space:]]*barbacane-' .barbacane/rulesets/barbacane.yaml \
			| sed -E 's/.*function:[[:space:]]*//' | sort -u); do \
		curl -fsSL "$(BARBACANE_RULESET_URL)/functions/$${f}.js" \
			-o "specs/functions/$${f}.js"; \
	done
	@touch $@

lint-spec: specs/functions/.barbacane-fetched ## Lint OpenAPI spec with vacuum
	vacuum lint -f specs/functions specs/burst-api.yaml -r specs/.vacuum.yaml --ignore-file specs/.vacuum-ignore.yaml

# ── Quality ────────────────────────────────────────────────────────────────────
check: ## Run fmt, clippy, and tests
	cargo fmt --all
	cargo clippy --all-targets -- -D warnings
	DATABASE_URL=$(DB_URL) cargo test

smoke: ## Run k6 smoke tests (requires: make all running in another terminal)
	k6 run tests/http/smoke.js

smoke-s3: ## Run k6 S3 storage smoke test (requires: RustFS + Barbacane S3 dispatcher)
	k6 run tests/http/smoke-s3.js

# Playwright downloads its browsers separately from npm, so a fresh checkout
# has the runner without anything to drive.
ui/node_modules/.playwright-browsers: ui/node_modules
	@cd ui && npx playwright install chromium
	@touch ui/node_modules/.playwright-browsers

e2e: ui/node_modules/.playwright-browsers ## Run Playwright E2E tests (requires: make all running in another terminal)
	cd ui && npx playwright test

# ── Tooling ────────────────────────────────────────────────────────────────────
install: ## Install dev tooling (the Procfile runner)
	brew install overmind
