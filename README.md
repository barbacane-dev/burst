# Burst

Open-source team messaging built on [Barbacane](https://github.com/barbacane-dev/barbacane).

## Prerequisites

- Rust (stable)
- Node.js 20+
- Docker
- [Barbacane](https://github.com/barbacane-dev/barbacane) cloned at `../Barbacane` with plugins built (`make plugins && make release`)

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
# Download the Barbacane ruleset and custom functions (first time only)
mkdir -p .barbacane/rulesets/functions
curl -fsSL https://docs.barbacane.dev/rulesets/barbacane.yaml \
  -o .barbacane/rulesets/barbacane.yaml
for f in barbacane-auth-opt-out barbacane-no-duplicate-middlewares \
         barbacane-no-plaintext-upstream barbacane-no-unknown-extensions \
         barbacane-valid-secret-refs barbacane-validate-dispatch-config \
         barbacane-validate-middleware-config; do
  curl -fsSL "https://docs.barbacane.dev/rulesets/functions/${f}.js" \
    -o ".barbacane/rulesets/functions/${f}.js"
done

# Lint
vacuum lint -f .barbacane/rulesets/functions specs/burst-api.yaml -r specs/.vacuum.yaml
```

CI downloads the ruleset automatically on each run.

## Architecture

```
Browser (:5173) → Vite → Barbacane (:8080) → Burst (:3000)
                    ↘ Mock OIDC (:9099)          ↘ PostgreSQL (:5432)
```

Barbacane validates JWTs (oidc-auth plugin) and sets `X-Auth-Consumer` / `X-Auth-Consumer-Groups` before forwarding to Burst. Admin routes are protected by the ACL plugin at the gateway level (`allow: [admin]`), with defense-in-depth via the `AdminUser` extractor on the backend. WebSocket auth uses `?access_token=` query param (RFC 6750 §2.3).

## Make targets

Run `make help` for the full list. Key targets:

| Target | Description |
|--------|-------------|
| `make services` | PostgreSQL + mock OIDC (Docker) |
| `make gateway-compile` | Compile OpenAPI spec into Barbacane artifact |
| `make gateway` | Run Barbacane gateway |
| `make server` | Run Burst API server |
| `make ui` | Run Vite dev server |
| `make seed` | Seed database with test users |
| `make db` | Open psql shell |
| `make all` | Compile gateway + start everything via overmind |
| `make stop` | Stop all overmind processes |
| `make restart` | Recompile gateway and restart everything |
| `make check` | Format, lint, and test |
| `make e2e` | Run Playwright E2E tests (requires stack running) |

## Test users

| Username | Role | Password |
|----------|------|----------|
| `alice` | admin | anything |
| `bob` | member | anything |
