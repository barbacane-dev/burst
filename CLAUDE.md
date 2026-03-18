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
- UUIDv7 primary keys with `usr_`, `ch_`, `msg_` prefixes

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
cargo audit

# 5. Lint OpenAPI spec (MUST pass — CI gate)
vacuum lint -f .barbacane/rulesets/functions specs/burst-api.yaml -r specs/.vacuum.yaml
```

The vacuum lint step is a **hard gate** in CI. Any errors will fail the pipeline.

To debug vacuum errors, use details mode and filter for error markers:

```bash
vacuum lint -f .barbacane/rulesets/functions specs/burst-api.yaml -r specs/.vacuum.yaml --no-banner -d -q 2>&1 | grep "✗"
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

## Barbacane Integration

- Gateway manifest: `barbacane.yaml` (plugin paths)
- Plugins used: `oidc-auth`, `acl`, `rate-limit`, `ws-upstream`, `http-upstream`
- `groups_claim: "roles"` maps JWT roles to `x-auth-consumer-groups`
- Admin routes have gateway-level ACL (`allow: [admin]`) + backend `AdminUser` extractor
