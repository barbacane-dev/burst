# Burst — Claude Code Instructions

Burst is an open-source team messaging app built on [Barbacane](https://github.com/barbacane-dev/barbacane) API gateway.

## Architecture

3 crates + React frontend:

| Crate | Role |
|-------|------|
| `burst` | Binary — CLI entry point, config loading |
| `burst-core` | Domain types, events, pure logic (no I/O) |
| `burst-server` | Axum handlers, DB (sqlx), WebSocket, file storage |

Frontend lives in `ui/` (React 19, Vite, Tailwind, TanStack Query).

## Rust Conventions

- Never `unwrap()` or `panic!()` in production code
- Use `.expect("reason")` only for provably infallible operations
- `thiserror` for library error types, `anyhow` for binary errors
- Error URNs follow `urn:burst:error:<type>` pattern (RFC 9457 ProblemDetails)
- UUIDv7 primary keys with `usr_`, `ch_`, `msg_`, `wh_` prefixes

## Before Pushing

Always run before pushing code:

```bash
# 1. Format
cargo fmt --all

# 2. Lint Rust
cargo clippy --all-targets

# 3. Run tests
cargo test

# 4. Security audit
cargo deny check advisories

# 5. Lint OpenAPI spec (MUST pass — CI gate)
vacuum lint -f specs/functions specs/burst-api.yaml -r specs/.vacuum.yaml
```

The vacuum lint step is a **hard gate** in CI. Any errors will fail the pipeline.

To debug vacuum errors, use details mode and filter for error markers:

```bash
vacuum lint -f specs/functions specs/burst-api.yaml -r specs/.vacuum.yaml --no-banner -d -q 2>&1 | grep "✗"
```

## OpenAPI Spec

- Single source of truth: `specs/burst-api.yaml`
- Also configures Barbacane gateway (dispatch + middleware blocks)
- Ruleset: `specs/.vacuum.yaml` (extends Vacuum recommended + OWASP + Barbacane rules)
- After spec changes, recompile gateway artifact: `make gateway-compile`

## Testing

- **Unit tests**: `cargo test` (burst-core is pure, no DB)
- **Integration tests**: `crates/burst-server/tests/` (use `#[sqlx::test]` with real DB)
- **Frontend unit tests**: `cd ui && npm test`
- **E2E tests**: `make e2e` (Playwright, requires full stack running)

## Deployment

Two deployment options:

- **All-in-one** (`docker/docker-compose.all-in-one.yaml`): single container with Barbacane + Burst, SPA served from RustFS via S3 dispatcher. No nginx, no s6-overlay.
- **Multi-service** (`docker/docker-compose.yaml`): separate containers for nginx, Barbacane, Burst. Zero-trust topology.

## Barbacane Integration

- Gateway manifest: `barbacane.yaml` (plugin paths)
- Plugins used: `oidc-auth`, `acl`, `rate-limit`, `ws-upstream`, `http-upstream`, `s3`, `mock`
- `groups_claim: "roles"` maps JWT roles to `x-auth-consumer-groups`
- Admin routes have gateway-level ACL (`allow: [admin]`) + backend `AdminUser` extractor
- Integration routes (webhooks, bots) use ACL (`allow: [admin, integrator]`) + `IntegrationUser` extractor
- Incoming webhook trigger endpoint uses `x-barbacane-middlewares: []` to bypass OIDC (token-based auth)
- SPA served via S3 dispatcher with `fallback_key: index.html` (all-in-one)
- OIDC configuration is runtime-injected via `/env.js` (no rebuild to change provider)
