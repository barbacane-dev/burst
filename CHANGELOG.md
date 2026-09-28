# Changelog

All notable changes to Burst are documented in this file.
Format follows [Keep a Changelog](https://keepachangelog.com/).

## [Unreleased]

### Changed
- Bumped Barbacane to v0.11.0. The gateway now forwards only the request headers an operation admits, and refuses an operation that runs an authentication middleware without naming the security scheme carrying the credential. Two spec changes were needed:
  - `specs/burst-s3.yaml` declares a `StorageKey` `apiKey` scheme naming `X-Storage-Key`, the header its global `apikey-auth` middleware reads. Without it the spec no longer compiles.
  - `specs/burst-api.yaml` declares a `WebhookToken` bearer scheme on `POST /api/webhooks/{webhookId}/trigger`. That operation opts out of the OIDC chain and is authenticated by the upstream against the webhook's own token, so its `security: []` would have dropped the `Authorization` header and broken every incoming webhook.
- Bumped Barbacane from v0.6.3 to v0.10.0 (plugin manifests, `barbacane-standalone` image, CI and Makefile). Barbacane 0.10 verifies plugin capability manifests at load and rejects artifacts built from pre-0.8.1 plugin binaries, so artifacts must be recompiled against the new manifests.
- Gateway processes run with `BARBACANE_ALLOW_INTERNAL_EGRESS=true` (CI, Makefile, Procfile, compose files, all-in-one entrypoint). Barbacane 0.8+ blocks plugin egress to loopback and private addresses by default, which covers every Burst upstream (Burst API, S3 sidecar, RustFS, and an IdP on the same network); without it `oidc-auth` cannot fetch discovery/JWKS and every authenticated request is rejected.
- CI prints the gateway and server logs when the smoke job fails.

### Fixed
- File uploads larger than about 750 KB are stored instead of failing with 500. Barbacane 0.12.2's `s3` dispatcher hashes the body on the host instead of running out of fuel in WASM, so uploads up to the storage gateway's `--max-body-size` go through.
- CI `openapi-lint` no longer hardcodes the Barbacane vacuum function list; it is read from the downloaded ruleset. The hardcoded list referenced a function Barbacane has since removed, which made the job fail on every run.

## [0.0.7] - 2026-04-08

### Changed
- `groups_claim` in OIDC config is now configurable via `BURST_OIDC_GROUPS_CLAIM` env var (was hardcoded to `roles`)
- Defaults to `roles` for backwards compatibility; set to `groups` for Authelia or any provider-specific claim name
- OIDC docs updated with Authelia setup guide and provider comparison table

### Fixed
- Release workflow: reverted `action-gh-release` from v3 to v2 (v3 tag does not exist)

### Dependencies
- vite 8.0.2 → 8.0.7
- react-router-dom 7.13.2 → 7.14.0
- tokio 1.50.0 → 1.51.0
- @playwright/test 1.58.2 → 1.59.1
- jsdom 29.0.1 → 29.0.2
- vitest 4.1.1 → 4.1.3
- react-virtuoso 4.18.3 → 4.18.4
- @tanstack/react-query 5.95.2 → 5.96.2
- toml 1.1.0 → 1.1.2

## [0.0.6] - 2026-04-08

### Added
- Incoming webhooks — external systems can post messages to channels via `POST /api/webhooks/{id}/trigger` with bearer token auth (bypasses OIDC)
- Outgoing webhooks — channel events delivered to external URLs with HMAC-SHA256 signing (`X-Burst-Signature`), retry with exponential backoff
- Webhook CRUD endpoints — create, list, get, update, delete, regenerate token (channel-scoped)
- Bot user accounts — `POST /api/admin/bots` creates users with `is_bot=true`, credentials managed by Barbacane `apikey-auth` plugin
- `integrator` role — can manage webhooks and bots without full admin access
- `IntegrationUser` extractor — accepts `admin` or `integrator` roles for integration management endpoints
- Admin panel: Webhooks tab — list all webhooks, create form with channel selector, kind/status display, trigger URL for incoming
- Admin panel: Bots tab — list bots, create form, deactivate
- Integrator mock user (ivy) in dev OIDC server for local testing
- Webhook integration tests (13 tests), bot integration tests (7 tests)
- Webhook/bot E2E tests (6 Playwright tests): tab visibility, webhook creation, trigger delivery, bot creation
- Webhook/bot smoke tests (20 k6 checks): full CRUD, trigger auth, permission checks, bot lifecycle
- Frontend unit tests (6 Vitest tests): tab filtering, webhook list, create form

### Changed
- Admin panel tabs filtered by role: integrators see only Webhooks and Bots; admins see all tabs
- Sidebar admin button visible to both `admin` and `integrator` roles
- Role enum extended: `admin`, `integrator`, `moderator`, `member`, `guest` (was 4 roles, now 5)
- `groups_claim` mapping recognizes `integrator` group from OIDC JWT
- Integration management endpoints use `acl: { allow: [admin, integrator] }` in OpenAPI spec
- `BURST_SPA_BUCKET` env var added to `.env` for local dev

### Fixed
- All-in-one Docker image crash on amd64: switched runtime base from `debian:bookworm-slim` (glibc 2.36) to `debian:trixie-slim` (glibc 2.40) to match CI-built binary (#62)

## [0.0.5] - 2026-04-07

### Added
- OIDC discovery — resolve authorize/token endpoints from `.well-known/openid-configuration` (works with any provider)
- PKCE (S256) for secure public client authentication
- JIT user provisioning maps `preferred_username`, `name`, `email` from OIDC claims
- Role mapping from `x-auth-consumer-groups` header (`admin` group → admin role)
- Profile and role re-synced from OIDC claims on every authenticated request
- `LOGIN_LOCAL` env var to hide credentials form when SSO-only
- SPA catch-all route via Barbacane S3 dispatcher with `fallback_key: index.html`
- JIT provisioning integration tests (3 tests) and extractor unit tests (11 tests)

### Changed
- All-in-one image: removed nginx and s6-overlay, replaced with shell entrypoint (4 → 3 processes)
- All-in-one image: SPA served from RustFS via Barbacane S3 dispatcher (was nginx static files)
- All-in-one image: SPA uploaded to RustFS at container startup via `curl --aws-sigv4`
- Bumped Barbacane from v0.6.1 to v0.6.3
- Login button: generic "Sign in with SSO" (was Google-specific)
- Default OIDC scopes: `openid email profile groups` (added `groups`)
- Username set from `preferred_username` claim (was UUID `sub`)

### Removed
- nginx from all-in-one Docker image
- s6-overlay process supervisor from all-in-one Docker image
- Google-specific OIDC endpoint hardcoding

## [0.0.4] - 2026-04-03

### Added
- All-in-one Docker image (`burst-aio`) bundling nginx, Barbacane gateway, S3 sidecar, and Burst API in a single container via s6-overlay
- Runtime OIDC configuration for Docker images via `/env.js` injection — no rebuild needed to change OIDC provider
- OIDC frontend tests (7 tests covering runtime injection, precedence, and defaults)

### Changed
- All API routes now use explicit `/api` prefix end-to-end (spec, gateway, backend, proxy)
- Dispatch `path` overrides removed from OpenAPI spec (paths match end-to-end)
- nginx no longer strips `/api` prefix (direct pass-through to Barbacane)
- Vite dev proxy forwards `/api` requests as-is (no rewrite)
- Bumped Barbacane from v0.5.1 to v0.6.1
- CI: single build job shares artifacts (eliminates 3 redundant Rust compilations)
- CI: switched from `cargo-audit` to `cargo-deny` (instant startup vs 2 min compile)

## [0.0.3] - 2026-04-02

### Fixed
- Docker image: switched to distroless/cc-debian13 base (GLIBC 2.38 compat)
- Release workflow: continue on custom registry login failure
- Smoke test rate-limit scenario for 300 req/min quota

### Added
- nginx frontend Docker image (`burst-nginx`) in release workflow
- Docker image published to GHCR alongside custom registry
- E2E test isolation and rate limit bump to 300 req/min

## [0.0.2] - 2026-03-24

### Changed
- Docker image switched to distroless to fix CVEs
- Bumped OpenTelemetry crates from 0.27 to 0.31

### Fixed
- Broken frontend dependencies and lint errors

## [0.0.1] - 2026-03-24

### Changed
- Reuse pre-built binaries in Docker image build (faster CI)

## [0.0.0] - 2026-03-24

Initial release — full-featured team messaging app.

### Added
- Architecture decision records (ADR-001 through ADR-013)
- Core data model: users, channels, messages, reactions, attachments
- Channels and messages API
- Real-time WebSocket with PG LISTEN/NOTIFY
- Threads, reactions, and direct messages
- Spec-first gate: OpenAPI spec kept in sync, Vacuum linting, drift blocking
- Spec-sync tests to catch drift between Rust types and OpenAPI spec
- Full authentication delegation to Barbacane gateway (ADR-006)
- JIT user provisioning from external identity (`X-Auth-Consumer` header)
- Docker Compose dev environment (PostgreSQL + mock OAuth2 server)
- Barbacane gateway manifest and compilation targets in Makefile
- WebSocket routing through Barbacane `ws-upstream` dispatcher
- React/Vite/Tailwind frontend with TanStack Query
- File attachments: multipart upload, auth-guarded download, local FS storage with storage trait
- Full-text search: PG tsvector-based message search with channel scoping
- Barbacane Vacuum ruleset integration for OpenAPI linting
- k6 smoke tests (65 checks: auth, channels, messages, reactions, attachments, search, pagination, errors)
- Pin/unpin message endpoints with WebSocket events
- Channel archive/unarchive endpoints with read-only enforcement
- Per-channel notification preferences
- Admin endpoints: user management, channel management, audit log
- `@mention` parsing on message send with `message_mentions` persistence
- Dark mode with system theme detection
- Markdown rendering in messages (react-markdown + remark-gfm)
- Settings page: profile editing, theme selection, browser notification toggle
- Browser notifications for new messages when tab is unfocused
- Admin panel UI: user list with role/deactivation controls, channel management, audit log
- Pinned messages panel with pin/unpin from message hover actions
- Virtualised message list using react-virtuoso
- `@mention` autocomplete in message composer
- Accessibility: skip-to-content link, ARIA landmarks/roles/labels
- Gateway-level ACL on admin routes via `groups_claim` in oidc-auth plugin
- Backend integration tests: admin, pins, mentions, notifications, WS event buffer
- Playwright E2E tests: authentication, messaging, admin panel (13 tests)
- Mock OAuth configured with per-user role claims for dev/test
