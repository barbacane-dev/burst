# ADR-012: Testing Strategy

**Status:** Accepted
**Date:** 2026-03-06

## Context

Per [ADR-008](008-crate-architecture.md), Burst has 3 crates: `burst-core` (pure domain logic), `burst-server` (all I/O), and `burst` (binary entry point). Per [ADR-003](003-technology-stack.md), the frontend is React/TypeScript built with Vite. We need a testing strategy that covers domain logic, database queries, API endpoints, WebSocket behaviour, and the UI — while keeping CI fast and the developer experience frictionless.

Barbacane's CI pipeline provides a proven blueprint: change detection, parallel jobs, separate unit/integration/UI stages, and benchmark regression checks. We follow the same structure, adapted for Burst's simpler architecture.

## Decision

### Test layers

#### 1. Unit tests (fast, no I/O)

**Crate: `burst-core`** — the primary unit test target.

Domain logic is pure and testable without any infrastructure:

- Permission checks: can this role perform this action?
- Channel membership rules: who can join, who can invite?
- ID generation and validation (UUIDv7).
- Markdown processing and sanitisation.
- Event envelope construction.
- ProblemDetails error types.

```bash
cargo test -p burst-core
```

These tests run in milliseconds with no setup. They are the first line of defence and should cover all business rules exhaustively.

**Crate: `burst-server`** — pure logic within the server crate (input validation, pagination helpers, response construction) is also unit-tested where it doesn't require a database.

#### 2. Database integration tests (require PostgreSQL)

**Crate: `burst-server`** — all database query tests.

sqlx provides compile-time query checking, but we still need runtime tests for:

- Query correctness: do cursors paginate correctly? Does search return expected results?
- Migration integrity: do all migrations apply cleanly on a fresh database?
- Constraint enforcement: unique violations, foreign key cascades, soft deletes.
- Concurrent access: race conditions on channel membership, duplicate reactions.

**Test database management:** Each test gets an isolated database using sqlx's `#[sqlx::test]` macro, which creates a temporary database with migrations applied and drops it after the test. No shared state between tests, no cleanup needed.

```bash
# Requires a running PostgreSQL instance
DATABASE_URL=postgres://burst:burst@localhost:5432/burst cargo test -p burst-server
```

#### 3. API integration tests (require PostgreSQL)

**Crate: `burst-server`** — HTTP handler tests using Axum's `TestClient`.

These tests exercise the full request/response cycle without starting a real server:

- Route handlers: correct status codes, response bodies, error formats.
- Authentication: `X-Auth-Consumer` header extraction and role checking.
- Authorization: channel membership checks, resource ownership validation.
- Pagination: cursor-based pagination with correct `Link` headers.
- File upload: multipart form processing, storage backend interaction.

Axum's built-in test utilities (`tower::ServiceExt`, `axum::body::Body`) allow testing handlers directly without HTTP overhead. The test harness provides a `TestApp` struct that sets up a database, runs migrations, and provides request helpers — similar to Barbacane's `TestGateway` pattern.

#### 4. WebSocket tests (require PostgreSQL)

**Crate: `burst-server`** — WebSocket lifecycle and real-time event tests.

- Connection: auth via first frame, subscription to channels.
- Message flow: send message → persist → broadcast → receive on connected clients.
- Presence: connect/disconnect → presence events.
- Gap-fill: reconnect with last event ID → receive missed events.
- Typing indicators: start/stop typing → broadcast to channel members.

These tests start a real Axum server on a random port and connect WebSocket clients. They are inherently slower than unit tests but exercise the real async flow.

#### 5. Frontend tests

**Unit tests (Vitest):**

- Component rendering and behaviour.
- State management logic.
- WebSocket client reconnection logic.
- Message formatting and Markdown rendering.

```bash
cd ui && npm test
```

**E2E tests (Playwright):**

- Full user flows: login, browse channels, send messages, upload files.
- WebSocket integration: real-time message delivery, typing indicators.
- Responsive layout: desktop and mobile viewports.

```bash
cd ui && npx playwright test
```

E2E tests run against a real Burst server with a test database. The test harness starts the server, seeds test data, and tears down after the suite.

### CI pipeline

Following Barbacane's proven structure — change detection, parallel jobs, fast feedback:

```yaml
jobs:
  changes:        # Detect what changed (code, ui, both)

  fmt:            # cargo fmt --all -- --check
  clippy:         # cargo clippy --workspace --all-targets -- -D warnings
  audit:          # cargo audit (non-blocking, continue-on-error)

  unit-tests:     # cargo test -p burst-core (no DB needed)
    needs: [fmt, clippy]

  db-tests:       # cargo test -p burst-server (PostgreSQL service container)
    needs: [fmt, clippy]
    services:
      postgres: postgres:16

  ui-tests:       # npm test (Vitest)
    needs: [changes]

  ui-e2e:         # npx playwright test
    needs: [ui-tests]

  openapi-lint:   # Vacuum/Spectral lint on specs/ (per ADR-005)
    needs: [changes]
```

Key design choices:

- **Change detection** via `dorny/paths-filter` — skip backend jobs when only UI changes, and vice versa. Same approach as Barbacane.
- **fmt and clippy run first** — fast feedback, gate everything else.
- **Unit tests and DB tests run in parallel** — `burst-core` tests don't need PostgreSQL, so they run independently.
- **PostgreSQL as a GitHub Actions service container** — same pattern as Barbacane's control plane tests.
- **OpenAPI linting as a CI gate** — per [ADR-005](005-api-design-conventions.md), the spec is linted on every PR.
- **Security audit is non-blocking** — `continue-on-error: true`, same as Barbacane. Known low-severity issues don't block PRs.

### Pre-push checklist

Consistent with Barbacane and the global Claude Code instructions:

```bash
cargo fmt --all
cargo clippy --all-targets
cargo test
cargo audit
```

### sqlx compile-time checking

sqlx checks SQL queries at compile time against a cached schema (`sqlx-data.json` or `.sqlx/` directory). This requires:

- **Offline mode for CI** — the `sqlx-data.json` file is committed to the repo. CI builds use `SQLX_OFFLINE=true` so compilation doesn't need a live database.
- **Developer workflow** — developers run with a live database connection (`DATABASE_URL` set) for immediate query validation. Run `cargo sqlx prepare` to update the offline cache before committing query changes.

### Test data seeding

A shared `test_helpers` module in `burst-server` provides:

- `TestApp` — sets up an Axum app with a test database, runs migrations, provides request helpers.
- `seed_user()`, `seed_channel()`, `seed_message()` — create test fixtures with sensible defaults.
- `assert_problem_details()` — verify RFC 9457 error responses.

This module is `#[cfg(test)]` only — it's not published or included in the production binary.

### What we deliberately avoided

- **No separate test crate.** Barbacane has `barbacane-test` because its integration tests need to compile specs and boot a gateway binary. Burst's tests are simpler — they test Axum handlers directly. A separate crate adds overhead without benefit.
- **No benchmarks in v1.** Barbacane runs criterion benchmarks for its hot path (routing, validation, WASM execution). Burst's hot path is PostgreSQL queries and WebSocket fan-out — profiling these is better done with realistic load tests than microbenchmarks. We'll add benchmarks if a specific performance question arises.
- **No contract testing against Barbacane.** Burst trusts Barbacane headers — there's no runtime contract to test between the two. The integration point is the OpenAPI spec, which is linted in CI.

## Consequences

- **`burst-core` is the easiest crate to test** — pure functions, no mocks, no setup. This reinforces the architectural decision ([ADR-008](008-crate-architecture.md)) to keep domain logic in a zero-dependency crate.
- **Database tests use real PostgreSQL, not mocks.** sqlx queries are the core of the application. Testing against a real database catches issues that mocks hide (query syntax, constraint violations, index behaviour). The trade-off is that these tests are slower and require a running PostgreSQL — mitigated by sqlx's per-test database isolation.
- **CI mirrors Barbacane's structure.** Contributors working across both projects see the same patterns: change detection, parallel jobs, same pre-push checklist. This reduces cognitive overhead.
- **OpenAPI linting in CI** ensures the spec stays valid and consistent. Breaking changes to the API surface are caught before they reach code review.
- **No E2E tests that require Barbacane in the loop.** Burst's CI tests the application in isolation. Testing the full Barbacane + Burst topology is a deployment validation concern, not a CI concern.
