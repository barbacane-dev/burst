# Changelog

All notable changes to Burst are documented in this file.
Format follows [Keep a Changelog](https://keepachangelog.com/).

## [Unreleased]

### Added
- Full authentication delegation to Barbacane gateway (ADR-006 amended)
- JIT user provisioning from external identity (`X-Auth-Consumer` header)
- Docker Compose dev environment (PostgreSQL + mock OAuth2 server)
- Barbacane gateway manifest (`barbacane.yaml`) and compilation targets in Makefile
- WebSocket routing through Barbacane `ws-upstream` dispatcher
- `README.md` with project overview and setup instructions
- Optimistic cache updates for sent messages in UI

### Changed
- API spec: replaced `jwt-auth` middleware with `oidc-auth`
- API spec: `/ws` endpoint now uses `ws-upstream` dispatch
- UI login flow: OIDC password grant instead of local `/auth/login`
- UI token storage: `sessionStorage` instead of in-memory + refresh cookie
- `AuthUser` extractor reads `X-Auth-Consumer` header (external_id lookup)
- Tests authenticate via `X-Auth-Consumer` header instead of JWT tokens
- Makefile reorganised with `gateway-compile`, `services`, `stop`, `restart` targets
- Vite WS proxy uses `http://` target with `ws: true` (not `ws://`)

### Removed
- Local password authentication (`/auth/login`, `/auth/logout`, `/auth/refresh`)
- JWT issuance and validation (`auth` module, `jsonwebtoken` crate)
- Password hashing (`argon2` crate)
- Refresh token storage (`refresh_tokens` table, DB module)
- `jwt_secret`, `cookie_secure`, `trust_auth_headers` config options
- `ClientEvent::Auth` WebSocket variant (auth on HTTP upgrade instead)

## [0.4.0] — 2026-03-12

### Fixed
- Real-time message delivery through Barbacane gateway (ws-upstream runtime affinity bug)
- Stale cache on logout, typing self-echo, and composer focus
- Duplicate `ChannelJoined` variant in `ServerEvent` enum

## [0.3.0] — 2026-03-10

### Added
- Spec-sync tests to catch drift between Rust types and OpenAPI spec
- ADR-011b: attachment schema with JSONB metadata

### Changed
- Unified WS and REST message types

### Fixed
- Auth refresh on page reload
- Unread count bugs
- Channel membership UX
- Null guard on reactions in channel page

## [0.2.0] — 2026-03-09

### Added
- Threads, reactions, and direct messages (Milestone 4)
- Spec-first gate: OpenAPI spec kept in sync, Vacuum linting, drift blocking

## [0.1.0] — 2026-03-07

### Added
- Architecture decision records (ADR-001 through ADR-013)
- Core data model: users, channels, messages, reactions, attachments
- Channels and messages API (Milestone 2)
- Real-time WebSocket with PG LISTEN/NOTIFY (Milestone 3)
- Local password authentication with JWT
- Makefile, Procfile, seed script
- React/Vite/Tailwind frontend with TanStack Query
