# Burst

Open-source team messaging built on [Barbacane](https://github.com/barbacane-dev/barbacane).

## Prerequisites

- Rust (stable)
- Node.js 20+
- Docker
- [k6](https://grafana.com/docs/k6/) (for smoke tests)

## Quick start

```bash
# 1. Start PostgreSQL + mock OIDC server
make services

# 2. Compile the gateway artifact (first time + after spec changes)
make gateway-compile

# 3. Seed the database (first time only)
make seed

# 4. Start the three remaining processes (each in its own terminal)
make gateway    # Barbacane on :8080
make server     # Burst API on :3000
make ui         # Vite on :5173
```

Open http://localhost:5173 and sign in with `alice` (any password).

## OpenAPI linting

The API spec is linted with [vacuum](https://quobix.com/vacuum/) using Burst-specific rules (ADR-005) and the [Barbacane ruleset](https://docs.barbacane.dev/guide/vacuum.html).

```bash
# Download the Barbacane ruleset and the custom functions it references,
# then lint. The function list comes from the ruleset, so it tracks what
# docs.barbacane.dev publishes.
make lint-spec
```

CI runs the same download and lint on each run.

## Architecture

**Development** (Vite proxies API + WS to Barbacane):

```
Browser (:5173) → Vite ─┬─ /api/* → Barbacane (:8080) → Burst (:3000) → PostgreSQL (:5432)
                         ├─ /ws   → Barbacane (:8080) → Burst
                         └─ /*    → SPA (HMR)                    ↓
                                    Mock OIDC (:9099)    Barbacane-S3 (:8081) → RustFS (:9000)
```

**Production — all-in-one** (single container, SPA served from S3):

```
Browser → Barbacane API (:8080) ─┬─ /api/* → Burst (:3000) → PostgreSQL
                                 ├─ /ws    → Burst (:3000)
                                 └─ /*     → RustFS (SPA via S3 dispatcher)
                                 Barbacane S3 (:8081) → RustFS (file storage)
```

**Production — multi-service** (nginx + separate containers):

```
Browser → nginx (:8080) ─┬─ /*     → SPA static files
                          ├─ /api/* → Barbacane (internal) → Burst → PostgreSQL
                          └─ /ws   → Barbacane (internal) → Burst
                                          ↓
                                   Barbacane S3 (internal) → RustFS
```

Two Barbacane instances in both topologies (zero-trust, minimal attack surface):

- **Public gateway** — oidc-auth, acl, rate-limit, http-upstream, ws-upstream, s3. Handles all API, WebSocket, and SPA traffic.
- **S3 sidecar** — s3, apikey-auth only. Internal, serves Burst's file storage operations.

Barbacane validates JWTs (oidc-auth plugin) and sets `X-Auth-Consumer` / `X-Auth-Consumer-Groups` before forwarding to Burst. JIT user provisioning maps `preferred_username`, `name`, `email` from OIDC claims, and `admin` group to admin role. WebSocket auth uses `?access_token=` query param (RFC 6750 §2.3).

### Production deployment

**All-in-one** (recommended — no gateway compilation needed):

```bash
cp docker/.env.example docker/.env   # Configure secrets + OIDC
docker compose -f docker/docker-compose.all-in-one.yaml up --build
```

**Multi-service** (zero-trust topology with nginx):

```bash
cp docker/.env.example docker/.env   # Configure secrets
make gateway-compile                  # Compile Barbacane artifacts
docker compose -f docker/docker-compose.yaml up --build
```

## Make targets

Run `make help` for the full list. Key targets:

| Target | Description |
|--------|-------------|
| `make services` | PostgreSQL + mock OIDC + RustFS (Docker) |
| `make gateway-compile` | Compile both Barbacane artifacts (API + S3 sidecar) |
| `make gateway` | Run Barbacane gateway |
| `make server` | Run Burst API server |
| `make ui` | Run Vite dev server |
| `make seed` | Seed database with test users |
| `make db` | Open psql shell |
| `make all` | Compile gateway + start everything via overmind |
| `make stop` | Stop all overmind processes |
| `make restart` | Recompile gateway and restart everything |
| `make check` | Format, lint, and test |
| `make smoke` | Run k6 smoke tests (106 checks) |
| `make smoke-s3` | Run S3 storage smoke test (Burst → Barbacane → RustFS) |
| `make e2e` | Run Playwright E2E tests (requires stack running) |

## Test users

| Username | Role | Password |
|----------|------|----------|
| `alice` | admin | anything |
| `bob` | member | anything |
