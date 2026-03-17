# Changelog

All notable changes to Burst are documented in this file.
Format follows [Keep a Changelog](https://keepachangelog.com/).

## [Unreleased]

### Added
- Architecture decision records (ADR-001 through ADR-013)
- Core data model: users, channels, messages, reactions, attachments
- Channels and messages API
- Real-time WebSocket with PG LISTEN/NOTIFY
- Threads, reactions, and direct messages
- Spec-first gate: OpenAPI spec kept in sync, Vacuum linting, drift blocking
- Spec-sync tests to catch drift between Rust types and OpenAPI spec
- ADR-011b: attachment schema with JSONB metadata
- Full authentication delegation to Barbacane gateway (ADR-006 amended)
- JIT user provisioning from external identity (`X-Auth-Consumer` header)
- Docker Compose dev environment (PostgreSQL + mock OAuth2 server)
- Barbacane gateway manifest (`barbacane.yaml`) and compilation targets in Makefile
- WebSocket routing through Barbacane `ws-upstream` dispatcher
- `README.md` with project overview and setup instructions
- Optimistic cache updates for sent messages in UI
- Makefile, Procfile, seed script
- React/Vite/Tailwind frontend with TanStack Query
- File attachments: multipart upload, auth-guarded download, local FS storage with storage trait
- Full-text search: PG tsvector-based message search with channel scoping
- Attachment and search endpoints in OpenAPI spec
- Barbacane Vacuum ruleset integration for OpenAPI linting
- k6 smoke tests (65 checks: auth, channels, messages, reactions, attachments, search, pagination, errors)
- UI: attachment preview component, search dialog, sidebar search
- M6 data model: `pinned_messages`, `audit_log`, `custom_emojis`, `message_mentions` tables
- Pin/unpin message endpoints with WebSocket events (`message.pinned`, `message.unpinned`)
- Channel archive/unarchive endpoints with read-only enforcement
- Per-channel notification preferences (`PATCH /channels/{id}/members/me/notify`)
- Admin endpoints: user management, channel management, audit log
- `AdminUser` extractor for defense-in-depth admin route protection
- `@mention` parsing on message send with `message_mentions` persistence
- Dark mode: class-based toggle with localStorage persistence and system theme detection
- Markdown rendering in messages (react-markdown + remark-gfm)
- Settings page: profile editing, theme selection, browser notification toggle
- Browser notifications for new messages when tab is unfocused
- Admin panel UI: user list with role/deactivation controls, channel management, audit log
- Pinned messages panel with pin/unpin from message hover actions
- Virtualised message list using react-virtuoso for large channel histories
- `@mention` autocomplete in message composer
- Accessibility: skip-to-content link, ARIA landmarks/roles/labels on messages and controls
- OpenAPI spec: pin, archive, notify, and admin endpoints with schemas
- Gateway-level ACL on admin routes via `groups_claim` in oidc-auth plugin
- JIT user provisioning wired into `AuthUser` extractor and WebSocket handler
- Backend integration tests: admin, pins, mentions, notifications, WS event buffer
- Playwright E2E tests: authentication, messaging, admin panel (13 tests)
- Mock OAuth configured with per-user role claims for dev/test

### Changed
- Unified WS and REST message types
- API spec: replaced `jwt-auth` middleware with `oidc-auth`
- API spec: `/ws` endpoint now uses `ws-upstream` dispatch
- UI login flow: OIDC password grant instead of local `/auth/login`
- UI token storage: `sessionStorage` instead of in-memory + refresh cookie
- `AuthUser` extractor reads `X-Auth-Consumer` header (external_id lookup)
- Tests authenticate via `X-Auth-Consumer` header instead of JWT tokens
- Makefile reorganised with `gateway-compile`, `services`, `stop`, `restart` targets
- Vite WS proxy uses `http://` target with `ws: true` (not `ws://`)
- Pagination cursors accept prefixed IDs (`msg_`, `ch_`) instead of raw UUIDs
- Login form uses username instead of email (matches OIDC sub claims)

### Fixed
- Real-time message delivery through Barbacane gateway (ws-upstream runtime affinity bug)
- Stale cache on logout, typing self-echo, and composer focus
- Duplicate `ChannelJoined` variant in `ServerEvent` enum
- Auth refresh on page reload
- Unread count bugs
- Channel membership UX
- Null guard on reactions in channel page
- Pagination cursor round-trip through Barbacane (prefixed ID stripping)
- Seed `external_id` values to match mock OIDC server
- OpenAPI spec: added `maxLength` on limit params, added RFC 8725 reference

### Removed
- Local password authentication (`/auth/login`, `/auth/logout`, `/auth/refresh`)
- JWT issuance and validation (`auth` module, `jsonwebtoken` crate)
- Password hashing (`argon2` crate)
- Refresh token storage (`refresh_tokens` table, DB module)
- `jwt_secret`, `cookie_secure`, `trust_auth_headers` config options
- `ClientEvent::Auth` WebSocket variant (auth on HTTP upgrade instead)
