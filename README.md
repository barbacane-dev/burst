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

Open http://localhost:5173 and sign in with `alice@example.com` (any password).

## Architecture

```
Browser (:5173) → Vite → Barbacane (:8080) → Burst (:3000)
                    ↘ Mock OIDC (:9099)          ↘ PostgreSQL (:5432)
```

Barbacane validates JWTs (oidc-auth plugin) and sets `X-Auth-Consumer` before forwarding to Burst. WebSocket auth uses `?access_token=` query param (RFC 6750 §2.3).

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

## Test users

| Email | Role | Password |
|-------|------|----------|
| `alice@example.com` | admin | anything |
| `bob@example.com` | member | anything |
