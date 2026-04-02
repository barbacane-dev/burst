# Changelog

All notable changes to Burst are documented in this file.
Format follows [Keep a Changelog](https://keepachangelog.com/).

## [Unreleased]

## [0.0.4] - 2026-04-02

### Added
- All-in-one Docker image (`burst-aio`) bundling nginx, Barbacane gateway, S3 sidecar, and Burst API in a single container via s6-overlay
- Runtime OIDC configuration for Docker images via `/env.js` injection — no rebuild needed to change OIDC provider
- OIDC frontend tests (7 tests covering runtime injection, precedence, and defaults)

### Changed
- All API routes now use explicit `/api` prefix end-to-end (spec, gateway, backend, proxy)
- Dispatch `path` overrides removed from OpenAPI spec (paths match end-to-end)
- nginx no longer strips `/api` prefix (direct pass-through to Barbacane)
- Vite dev proxy forwards `/api` requests as-is (no rewrite)

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
