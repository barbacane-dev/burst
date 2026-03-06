# Roadmap

Prioritized roadmap for Burst development.

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

- [ ] Channel model — `channels`, `channel_members` tables, CRUD endpoints (ADR-007)
- [ ] Message model — `messages` table, send/list/edit/delete endpoints (ADR-007)
- [ ] Cursor-based pagination — UUIDv7 cursors on message listing (ADR-005, ADR-007)
- [ ] Channel membership — join, leave, invite, member list
- [ ] Soft deletes — message deletion preserves timeline (ADR-007)
- [ ] ProblemDetails errors — `urn:burst:error:*` error types (ADR-005)

### Frontend

- [ ] Channel sidebar — channel list, create channel dialog
- [ ] Message list — display messages with author, timestamp
- [ ] Message composer — text input, send on Enter
- [ ] Channel switching — load messages on channel select

---

## Milestone 3 — Real-time

WebSocket connection, live message delivery, presence, typing indicators.

### Backend

- [ ] WebSocket upgrade — Axum route, auth via first frame (ADR-004)
- [ ] In-process broker — tokio broadcast channels for event fan-out (ADR-004)
- [ ] Persist-first flow — insert message → broadcast to subscribers (ADR-004)
- [ ] Event envelope — typed events with UUIDv7 IDs (ADR-004)
- [ ] Presence tracking — online/away/offline with 30s grace period (ADR-004)
- [ ] Typing indicators — `typing.start`/`typing.stop` events (ADR-004)
- [ ] Gap-fill on reconnect — send missed events since last event ID (ADR-004)

### Frontend

- [ ] WebSocket client — connection, reconnection with exponential backoff (ADR-013)
- [ ] Live message delivery — new messages appear without refresh
- [ ] Typing indicator — "Nicolas is typing..." below message list
- [ ] Presence indicators — online dot on user avatars
- [ ] Optimistic message sending — instant display with pending state

---

## Milestone 4 — Threads, Reactions, DMs

Complete the core messaging experience.

### Backend

- [ ] Threaded replies — `thread_id` on messages, thread listing endpoint (ADR-007)
- [ ] Reactions — add/remove reactions, emoji + custom emoji support (ADR-007)
- [ ] Custom emoji — `custom_emojis` table, upload endpoint, admin-only (ADR-007)
- [ ] Direct messages — DM channel creation, participant lookup (ADR-007)
- [ ] Group DMs — multi-participant DM channels (ADR-007)
- [ ] Mentions — `@username` parsing, mention notification events

### Frontend

- [ ] Thread panel — side panel with replies, reusing message components
- [ ] Reaction picker — emoji selector, reaction display on messages
- [ ] DM list — separate section in sidebar
- [ ] Mention autocomplete — `@` trigger in composer
- [ ] Unread counts — per-channel unread badge in sidebar

---

## Milestone 5 — Search & File Sharing

Find messages and share files.

### Backend

- [ ] PostgreSQL full-text search — `search_vec` trigger, search endpoint (ADR-003, ADR-007)
- [ ] Search trait — unified interface for PG FTS and Typesense (ADR-003)
- [ ] File upload — multipart form, storage trait, local FS backend (ADR-011)
- [ ] File download — access control, Content-Disposition headers (ADR-011)
- [ ] Image metadata — dimension extraction on upload (ADR-011)
- [ ] File constraints — size limits, blocked extensions (ADR-011)

### Frontend

- [ ] Search — input with debounce, results with highlighted matches, channel context
- [ ] File upload — drag-and-drop or button in composer
- [ ] File preview — inline images, download link for other types
- [ ] Virtualised message list — handle large channel histories (ADR-013)

---

## Milestone 6 — Notifications, Pins, Admin

Polish the experience, add administrative controls.

### Backend

- [ ] Notification preferences — per-channel settings in `channel_members.notify` (ADR-007)
- [ ] Pinned messages — pin/unpin endpoints, `pinned_messages` table (ADR-007)
- [ ] Channel archival — archive/unarchive, read-only mode (ADR-007)
- [ ] Admin endpoints — user management, channel management, instance settings
- [ ] Audit log — append-only log of admin actions (ADR-007)
- [ ] Webhooks — incoming/outgoing webhook CRUD, message posting (ADR-007)

### Frontend

- [ ] Browser notifications — Notification API for mentions and DMs when tab unfocused
- [ ] Notification preferences — per-channel settings in UI
- [ ] Pinned messages — pin/unpin action, pinned messages panel
- [ ] Admin panel — user list, channel management, webhook configuration
- [ ] Settings page — profile, notification preferences, theme toggle

---

## Milestone 7 — Production Readiness

Observability, S3 storage, multi-node, hardening.

### Backend

- [ ] Prometheus metrics — HTTP, DB, WebSocket, search metrics on admin port (ADR-010)
- [ ] Distributed tracing — OpenTelemetry spans, OTLP export (ADR-010)
- [ ] Structured logging — JSON to stdout with trace correlation (ADR-010)
- [ ] S3 storage via Barbacane — gateway storage backend using S3 dispatcher (ADR-011)
- [ ] PG LISTEN/NOTIFY broker — multi-node event synchronisation (ADR-004)
- [ ] Graceful shutdown — SIGTERM handling, WebSocket close frames (ADR-009)
- [ ] Deferred file cleanup — background job for soft-deleted attachment removal (ADR-011)

### Infrastructure

- [ ] Docker image — multi-arch, minimal base (ADR-009)
- [ ] GitHub Actions release workflow — build binaries, publish Docker image
- [ ] Barbacane spec for production — S3 dispatcher routes, OIDC auth, ACL rules (ADR-006, ADR-011)

### Frontend

- [ ] Markdown rendering — bold, italic, code blocks with shiki, emoji shortcodes (ADR-013)
- [ ] Accessibility — keyboard navigation, ARIA roles, screen reader support (ADR-013)
- [ ] Dark mode (ADR-013)

---

## Milestone 8 — Typesense & OIDC

Enhanced search and enterprise SSO.

- [ ] Typesense integration — async message sync, search via Typesense backend (ADR-003)
- [ ] Barbacane OIDC configuration — document full OIDC setup with oidc-auth plugin (ADR-006)
- [ ] JIT provisioning refinement — claim mapping, group sync, profile re-sync on login (ADR-006)

---

## Future Considerations

Not committed — revisit when demand or opportunity arises.

| Item | Context | ADR |
|------|---------|-----|
| Tauri desktop app | Lightweight alternative to Electron | ADR-003 |
| RobustMQ broker | Rust-native alternative to PG LISTEN/NOTIFY when production-ready | ADR-004 |
| Barbacane ldap-auth plugin | Needed for LDAP-only deployments, already in Barbacane roadmap P2 | ADR-006 |
| Barbacane websocket dispatcher | Could simplify WS proxying topology | NOTES |
| E2E encryption | Boundary consideration from ADR-002 | ADR-002 |
| Voice messages | Boundary consideration from ADR-002 | ADR-002 |
| Message scheduling | Boundary consideration from ADR-002 | ADR-002 |
| Federation / protocol bridges | Boundary consideration from ADR-002 | ADR-002 |
| i18n | English only in v1, add when community demand exists | ADR-013 |
| Shared UI component library | Extract `@barbacane/ui` when duplication justifies it | ADR-013 |
| Licensing ADR | Write before repo goes public | NOTES |
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
