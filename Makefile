.PHONY: help all dev server ui db db-setup db-drop seed check install

# ── Config ─────────────────────────────────────────────────────────────────────
PG_BIN    := /opt/homebrew/opt/postgresql@17/bin
PSQL      := $(PG_BIN)/psql
CREATEDB  := $(PG_BIN)/createdb
CREATEUSER := $(PG_BIN)/createuser
DROPDB    := $(PG_BIN)/dropdb
PG_READY  := $(PG_BIN)/pg_isready

DB_USER   := burst
DB_PASS   := burst
DB_NAME   := burst
DB_URL    := postgres://$(DB_USER):$(DB_PASS)@localhost:5432/$(DB_NAME)

# ── Help ───────────────────────────────────────────────────────────────────────
help: ## Show this help
	@awk 'BEGIN {FS = ":.*##"; printf "\nUsage:\n  make \033[36m<target>\033[0m\n\nTargets:\n"} \
	/^[a-zA-Z_-]+:.*?##/ { printf "  \033[36m%-12s\033[0m %s\n", $$1, $$2 }' $(MAKEFILE_LIST)

# ── Combined ───────────────────────────────────────────────────────────────────
all: ## Start server + UI via overmind (requires: make install, make db-setup)
	overmind start

dev: ## Print instructions for running server + ui in separate terminals
	@echo ""
	@echo "  Open two terminals and run:"
	@echo ""
	@echo "    make server    # Burst API on :3000"
	@echo "    make ui        # Vite dev server on :5173"
	@echo ""
	@echo "  Then open http://localhost:5173"

# ── Backend ────────────────────────────────────────────────────────────────────
server: ## Run the Burst API server
	RUST_LOG=info,burst=debug,burst_server=debug cargo run --bin burst -- burst.toml

server-release: ## Run with release build
	cargo build --release && RUST_LOG=info ./target/release/burst burst.toml

# ── Frontend ───────────────────────────────────────────────────────────────────
ui: ## Run the Vite dev server (proxies API to localhost:3000)
	cd ui && npm run dev

# ── Database ───────────────────────────────────────────────────────────────────
db-setup: ## Create the burst role and database in local postgres (run once)
	@$(PG_READY) -q || (echo "ERROR: local postgres is not running (brew services start postgresql@17)"; exit 1)
	@$(PSQL) -d postgres -tAc "SELECT 1 FROM pg_roles WHERE rolname='$(DB_USER)'" | grep -q 1 \
		|| $(CREATEUSER) --createdb $(DB_USER) && echo "Created role: $(DB_USER)"
	@$(PSQL) -U $(DB_USER) -d postgres -tAc "ALTER USER $(DB_USER) WITH PASSWORD '$(DB_PASS)'" > /dev/null
	@$(PSQL) -d postgres -tAc "SELECT 1 FROM pg_database WHERE datname='$(DB_NAME)'" | grep -q 1 \
		|| $(CREATEDB) -O $(DB_USER) $(DB_NAME) && echo "Created database: $(DB_NAME)"
	@echo "Database ready at $(DB_URL)"

db-drop: ## Drop the burst database (destructive!)
	$(DROPDB) --if-exists $(DB_NAME)
	@echo "Dropped database: $(DB_NAME)"

db: ## Open a psql shell on the burst database
	$(PSQL) $(DB_URL)

seed: ## Seed the database with a test user
	cargo run --example seed -- $(DB_URL)

# ── Quality ────────────────────────────────────────────────────────────────────
check: ## Run fmt, clippy, and tests
	cargo fmt --all
	cargo clippy --all-targets -- -D warnings
	cargo test

# ── Tooling ────────────────────────────────────────────────────────────────────
install: ## Install dev tooling (overmind)
	brew install overmind
