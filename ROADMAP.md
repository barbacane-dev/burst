# Roadmap

Prioritised development roadmap for Burst.

## What ships as v1.0?

v1.0 is the public release. It covers Milestones 1–9 (the messaging core, integration surface and SSO) plus Milestones 10–13: a standalone deployment tier that runs as one binary with PostgreSQL, local accounts, and the public release itself. Milestones 1–9 are complete.

**v1.0 is not a feature-complete product.** It is the smallest version that proves the core hypothesis: a focused, fully open-source messaging tool with no feature gating and no user cap beats the alternatives for teams that need reliability and operational simplicity over breadth.

**v1.0 success looks like:** at least three external teams self-hosting Burst as their primary internal messaging tool within 6 months of public release, with no blocking issues caused by missing core features.

**Deployment tiers at v1.0 ([ADR-014](adr/014-gateway-optional-deployment.md)):**
- **Standalone:** `burst` + PostgreSQL. Local accounts and/or an external OIDC provider. Single-node rate limiting. Bot accounts unavailable.
- **Gateway (recommended for production):** Barbacane in front of Burst. Zero-trust boundary, multi-node rate limiting, native WAF, gateway observability, MCP exposure, bot accounts via `apikey-auth`.

**Known gaps at v1.0:**
- No LDAP auth. Teams with LDAP-only directories use an OIDC bridge (e.g., Keycloak in front of LDAP). LDAP is planned for M14 (see Future Considerations).
- No mobile push notifications. Browser notifications cover the primary use case. Native push is deferred.
- No link unfurling. Links are rendered as plain clickable text in v1.
- Bot accounts require the gateway tier. Standalone bot credentials are planned for M14.
- Local accounts and an external IdP cannot both be active behind the gateway until the gateway supports two issuers on one route ([ADR-015](adr/015-local-accounts.md)).

**Pre-release gates (must complete before tagging v1.0):**
- Licensing ADR completed and committed. Done.
- Milestones 10–13 complete.
- Repo made public.

---

## Milestone 1 — Foundation

Scaffold the project, prove the architecture, get a message on screen.

### Backend

- [x] Workspace setup — Cargo workspace with `burst`, `burst-core`, `burst-server` crates (ADR-008)
- [x] Configuration loading — TOML config + env var overrides (ADR-009)
- [x] Database setup — sqlx connection pool, migration runner on startup (ADR-009)
- [x] User model — `users` table, CRUD endpoints, JIT provisioning from `X-Auth-Consumer` (ADR-006, ADR-007)
- [x] Local auth — `POST /auth/login`, JWT issuing, refresh tokens (ADR-006)
- [x] Health endpoints — `/health/live`, `/health/ready` on admin port 3001 (ADR-009)

### Frontend

- [x] Vite/React/Tailwind scaffold — project structure per ADR-013
- [x] Login page — local auth flow
- [x] App shell — sidebar, channel header, main content area

### Infrastructure

- [x] Initial database migrations (ADR-007)
- [x] Docker Compose for local development (Burst + PostgreSQL + Barbacane)
- [x] CI pipeline — fmt, clippy, unit tests, OpenAPI lint (ADR-012)
- [x] OpenAPI spec — `specs/burst-api.yaml` with auth and user endpoints + Barbacane gateway config (ADR-005, ADR-006)

---

## Milestone 2 — Channels & Messages

Core messaging: create channels, send and receive messages via REST.

### Backend

- [x] Channel model — `channels`, `channel_members` tables, CRUD endpoints (ADR-007)
- [x] Message model — `messages` table, send/list/edit/delete endpoints (ADR-007)
- [x] Cursor-based pagination — UUIDv7 cursors on message listing (ADR-005, ADR-007)
- [x] Channel membership — join, leave, invite, member list
- [x] Soft deletes — message deletion preserves timeline (ADR-007)
- [x] ProblemDetails errors — `urn:burst:error:*` error types (ADR-005)

### Frontend

- [x] Channel sidebar — channel list, create channel dialog
- [x] Message list — display messages with author, timestamp
- [x] Message composer — text input, send on Enter
- [x] Channel switching — load messages on channel select

---

## Milestone 3 — Real-time

WebSocket connection, live message delivery, presence, typing indicators.

### Backend

- [x] WebSocket upgrade — Axum route, auth via first frame (ADR-004)
- [x] In-process broker — tokio broadcast channels for event fan-out (ADR-004)
- [x] Persist-first flow — insert message → broadcast to subscribers (ADR-004)
- [x] Event envelope — typed events with UUIDv7 IDs (ADR-004)
- [x] Presence tracking — online/away/offline with 30s grace period (ADR-004)
- [x] Typing indicators — `typing.start`/`typing.stop` events (ADR-004)
- [x] Gap-fill on reconnect — send missed events since last event ID (ADR-004)

### Frontend

- [x] WebSocket client — connection, reconnection with exponential backoff (ADR-013)
- [x] Live message delivery — new messages appear without refresh
- [x] Typing indicator — "Nicolas is typing..." below message list
- [x] Presence indicators — online dot on user avatars
- [x] Optimistic message sending — WS delivery used instead (avoids race duplicates)

---

## Milestone 4 — Threads, Reactions, DMs

Complete the core messaging experience.

### Backend

- [x] Threaded replies — `thread_id` on messages, thread listing endpoint (ADR-007)
- [x] Reactions — add/remove reactions per message (ADR-007)
- [x] Custom emoji — `custom_emojis` table, upload endpoint, admin-only (ADR-007)
- [x] Direct messages — DM channel creation via `POST /dms`, find-or-create (ADR-007)
- [x] Group DMs — multi-participant DM channels (ADR-007)
- [x] Mentions — `@username` parsing, mention persistence — completed in M6

### Frontend

- [x] Thread panel — side panel with replies, reusing message components
- [x] Reaction picker — emoji hover menu, reaction pills on messages
- [x] DM list — separate section in sidebar with New DM dialog
- [x] Mention autocomplete — `@` trigger in composer — completed in M6
- [x] Unread counts — per-channel badge, cleared on visit

---

## Milestone 5 — Search & File Sharing

Find messages and share files.

### Backend

- [x] PostgreSQL full-text search — `search_vec` trigger, search endpoint (ADR-003, ADR-007)
- [ ] Search trait — unified interface for PG FTS and Typesense (ADR-003) — deferred to M8 (ships with Typesense)
- [x] File upload — multipart form, storage trait, local FS backend (ADR-011)
- [x] File download — access control, Content-Disposition headers (ADR-011)
- [x] Image metadata — dimension extraction on upload (ADR-011)
- [x] File constraints — size limits, blocked extensions (ADR-011)

### Frontend

- [x] Search — input with debounce, results with highlighted matches, channel context
- [x] File upload — drag-and-drop or button in composer
- [x] File preview — inline images, download link for other types
- [x] Virtualised message list — react-virtuoso for large channel histories (ADR-013) — completed in M6

---

## Milestone 6 — Polish, Admin & UX Completeness

Notifications, pins, administration, and the UI features needed before v1.0 is usable end-to-end.

### Backend

- [x] Notification preferences — per-channel settings in `channel_members.notify` (ADR-007)
- [x] Pinned messages — pin/unpin endpoints, `pinned_messages` table (ADR-007)
- [x] Channel archival — archive/unarchive, read-only mode (ADR-007)
- [x] Admin endpoints — user management, channel management, instance settings
- [x] Audit log — append-only log of admin actions (ADR-007)

### Frontend

- [x] Markdown rendering — react-markdown + remark-gfm (ADR-013)
- [x] Browser notifications — Notification API for new messages when tab unfocused
- [x] Notification preferences — per-channel settings in UI (settings page)
- [x] Pinned messages — pin/unpin action, pinned messages panel
- [x] Admin panel — user list, channel management, audit log
- [x] Settings page — profile editing, notification preferences, theme toggle
- [x] Accessibility — skip-to-content, ARIA roles/labels, keyboard navigation (ADR-013)
- [x] Dark mode — class-based toggle with localStorage persistence (ADR-013)

> **Note:** Markdown rendering is placed here, not M7, because messages without formatting look visually incomplete. M5 delivers the ability to send and search messages; M6 makes them look right.

---

## Milestone 7 — Production Readiness

Observability, S3 storage, multi-node hardening. **Completing this milestone = v1.0 candidate.**

### Backend

- [x] Prometheus metrics — HTTP, DB, WebSocket, search metrics on admin port (ADR-010)
- [x] Distributed tracing — OpenTelemetry spans, OTLP export (ADR-010)
- [x] Structured logging — JSON to stdout with trace correlation (ADR-010)
- [x] S3 storage via Barbacane — gateway storage backend using S3 dispatcher (ADR-011)
- [x] PG LISTEN/NOTIFY broker — multi-node event synchronisation (ADR-004)
- [x] Graceful shutdown — SIGTERM handling, WebSocket close frames (ADR-009)
- [x] Deferred file cleanup — background job for soft-deleted attachment removal (ADR-011)

### Infrastructure

- [x] Docker image — multi-arch, minimal base (ADR-009)
- [x] GitHub Actions release workflow — build binaries, publish Docker image on tag
- [x] Barbacane spec for production — S3 dispatcher routes, OIDC auth, ACL rules (ADR-006, ADR-011)
- [x] Licensing — AGPL-3.0 dual-license (follows Barbacane pattern, LICENSE + LICENSING.md)

---

## Milestone 8 — Integration Surface

Webhooks, bot accounts, and enhanced search. **Post-v1.0.**

Webhooks and bots multiply the value of a stable core — they are not the thing being validated in v1. Shipping the REST API first (M1–M2) gives integrators a path forward while the webhooks layer is built properly.

### Backend

- [x] Incoming webhooks — receive messages via URL, `webhooks` table, bearer token auth bypasses OIDC (ADR-007)
- [x] Outgoing webhooks — post events to external URLs, HMAC-SHA256 signing, retry with exponential backoff (ADR-007)
- [x] Bot user accounts — `is_bot` flag, credentials managed by Barbacane `apikey-auth` plugin (ADR-006)
- [x] `integrator` role — manages webhooks and bots without full admin access, `IntegrationUser` extractor
- [ ] Typesense integration — async message sync, search via Typesense backend (ADR-003)

### Frontend

- [x] Webhook management — Webhooks tab in admin panel (create, list, delete, trigger URL display)
- [x] Bot management — Bots tab in admin panel (create, list, deactivate)
- [x] Role-filtered admin tabs — integrators see only Webhooks and Bots

---

## Milestone 9 — OIDC & SSO

Production-grade SSO documentation and JIT provisioning refinement.

- [x] Barbacane OIDC configuration — Google OIDC with oidc-auth plugin (ADR-006). GitHub deferred (requires OIDC bridge).
- [x] JIT provisioning refinement — claim mapping (name, email, picture), profile re-sync on login (ADR-006)

---

## Milestone 10 — Hygiene & Barbacane 0.11

Bring the repository current and close the identity-header trust gap. No product changes.

- [x] Merge open dependency updates; fix the failing `actions/checkout@v7` CI run (#97, #99). The remaining twenty-two dependabot pull requests were resolved in #106 and #107: seventeen were no-ops the lockfiles already carried, `actions/checkout@v7` was already applied everywhere, and the four held majors (sha2, hmac, rand, action-gh-release) went in (#92)
- [x] Bump Barbacane 0.6.3 → 0.10.0 — plugin URLs and SHA256 in `barbacane.yaml` / `barbacane-s3.yaml`, `barbacane-standalone` image tag, `BARBACANE_VERSION` in CI and Makefile (#98). Gateway processes need `BARBACANE_ALLOW_INTERNAL_EGRESS=true`: 0.8+ blocks plugin egress to loopback/private upstreams by default
- [x] Bump Barbacane 0.10.0 → 0.11.0. The gateway forwards only the headers an operation admits and refuses an operation running an authentication middleware without a security scheme, so `burst-s3.yaml` declares a `StorageKey` apiKey scheme and the incoming-webhook operation declares a `WebhookToken` bearer scheme (its `security: []` would otherwise have dropped the credential the upstream validates). Clears the 0.10 blocker where the data plane rejected the artifact with `plugin 's3' violates its declared capabilities`: 0.11 plugin binaries carry their manifest and config schema inside the `.wasm`
- [x] Replace the manual compile-serve loop in `make gateway` with `barbacane dev`, which recompiles on spec changes (Barbacane 0.11 gave `dev` the request-limit and log-format flags, barbacane-dev/barbacane#177)
- [x] `[auth] mode` + `trusted_proxies` — refuse `X-Auth-*` headers from unlisted peers; refuse to start in `trusted-headers` mode without a list (ADR-014)
- [x] Fix role re-sync overwriting `PATCH /api/admin/users/{id}` on the next request (`sync_profile_from_claims`)
- [x] Remove the dead `[auth]` section from `burst.toml.example`; default `LOGIN_LOCAL` off in `docker/env.sh` until ADR-015 gives the form a backend
- [x] Remove the unused `tower-http` `cors` feature; M11 adds it back where the standalone tier terminates requests itself

---

## Milestone 11 — Standalone Tier

Burst runs as one binary with PostgreSQL. Barbacane becomes the recommended production topology instead of a requirement (ADR-014).

### Backend

- [ ] OIDC token validation — discovery, JWKS cache with rotation, issuer/audience/skew checks, producing the existing `AuthUser`
- [ ] `/ws` accepts `?access_token=` and validates it with the same path
- [ ] SPA served from the binary with `index.html` fallback; `/env.js` rendered from config
- [ ] CORS, request body limit and baseline security-header layers
- [ ] Single-node per-subject rate limiter; login and upload endpoints throttled
- [ ] Optional direct S3 storage backend (SigV4) as a third `Storage` variant

### Testing & CI

- [ ] OIDC-mode integration harness with a test issuer and JWKS; trusted-headers tests with and without a matching peer
- [ ] Smoke suite runs twice, through the gateway and direct against Burst; results diffed as a contract test

### Infrastructure & docs

- [ ] Standalone compose file (two services: `burst`, `postgres`) and bare-binary quick start
- [ ] All-in-one image stops uploading the SPA to S3; RustFS becomes optional
- [ ] Docs restructured around the two tiers; the gateway tier documents WAF, multi-node limits and MCP exposure

---

## Milestone 12 — Local Accounts

Sign in without an identity provider (ADR-015).

### Backend

- [ ] Argon2id password hashing; minimum length 12
- [ ] `POST /api/auth/login`, `/refresh`, `/logout`; `sessions` table with hashed, rotating refresh tokens
- [ ] Ed25519 signing key generated on first start; `/.well-known/jwks.json`
- [ ] First-run admin bootstrap (endpoint and `burst admin create`); disabled once a user exists
- [ ] Invitations; open self-registration as an instance setting, off by default
- [ ] Admin password reset; session revocation on deactivation
- [ ] Login throttling per account and per client address

### Frontend

- [ ] Login form targets `/api/auth/login`; shown only when local accounts are enabled (`/env.js` setting replaces `LOGIN_LOCAL`)
- [ ] First-run setup page; invitation acceptance page
- [ ] Admin panel: create user with password, reset password, invitation management

### Gateway tier

- [ ] `/api/auth/*` declared unauthenticated in the spec; `oidc-auth` configurable with Burst as issuer

---

## Milestone 13 — Public Release (v1.0)

- [ ] README rewritten around the standalone quick start and the two-tier model; positioning against user caps and SSO gating in the alternatives
- [ ] Release workflow publishes the standalone image alongside the gateway-tier images
- [ ] Repo made public; v1.0 tagged
- [ ] Announcement and a place for feedback (discussions or issues templates)

---

## Future Considerations

Not committed — revisit when demand or opportunity arises.

| Item | Context | ADR |
|------|---------|-----|
| LDAP auth (M14 candidate) | Gateway tier: the Barbacane `ldap-auth` plugin exists (native `ldap` host functions, Barbacane ADR-0032, merged 2026-09-16, ships in the next Barbacane release); groups arrive as `x-auth-consumer-groups` like every other auth plugin, so the spec change is a middleware swap. Standalone tier: direct LDAP bind via the `ldap3` crate (`default-features = false`, `tls-rustls-aws-lc-rs`). | ADR-006, ADR-014 |
| Standalone bot credentials (M14 candidate) | API keys issued and hashed in Burst, validated by the ADR-014 auth path, so bots work without the gateway. Needs a credential table, issuance/rotation/revocation endpoints and admin UI. | ADR-014 |
| Mobile push notifications | Browser notifications cover v1. Native push (APNs, FCM) requires per-platform cert management, service workers, and a notification relay service. Add when mobile usage data justifies the engineering cost. | ADR-002 |
| Link unfurling | In-scope in ADR-002 but deferred from v1. Requires an async fetch pipeline, timeout handling, and content sanitisation to do safely. Add in a post-v1 polish milestone. | ADR-002 |
| Tauri desktop app | Lightweight alternative to Electron | ADR-003 |
| RobustMQ broker | Rust-native alternative to PG LISTEN/NOTIFY when production-ready | ADR-004 |
| Barbacane websocket dispatcher | Could simplify WS proxying topology | NOTES |
| E2E encryption | Boundary consideration from ADR-002 | ADR-002 |
| Voice messages | Boundary consideration from ADR-002 | ADR-002 |
| Message scheduling | Boundary consideration from ADR-002 | ADR-002 |
| Federation / protocol bridges | Boundary consideration from ADR-002 | ADR-002 |
| i18n | English only in v1, add when community demand exists | ADR-013 |
| Shared UI component library | Extract `@barbacane/ui` when duplication justifies it | ADR-013 |
| OpenSpec evaluation | Evaluate for spec-driven implementation planning; deferred — revisit when workflow pain justifies it | — |

---

## Out of Scope

Per [ADR-001](adr/001-project-vision-and-scope.md) and [ADR-002](adr/002-core-feature-set.md):

| Item | Reason |
|------|--------|
| Video/audio calls | Not a messaging concern — use dedicated tools (Jitsi, Meet) |
| Plugin marketplace | Platform creep — the exact problem Burst was created to avoid |
| Omnichannel inbox | CRM territory, not team messaging |
| AI assistants / copilot | Adds complexity and cloud dependencies |
| Email integration | Different communication medium |
| Task management | Use dedicated tools (Linear, Jira) |
| CRM features | Out of scope entirely |
