# Architecture Audit — Senior Tech Lead Review

**Date:** 2026-03-20
**Overall grade:** B+

The foundations are strong (clean layering, no circular deps, proper 12-factor config, excellent testing). The issues below are about what to fix *before* the codebase grows.

---

## 1. Missing Service Layer (SRP / Clean Architecture) — FIXED

~~Handlers currently do too much.~~

Introduced `services/` module (`services/mod.rs`, `services/messages.rs`):
- `services::messages::enrich()` — batch-loads reactions, attachments, reply counts (was duplicated 5x across handlers)
- `services::require_membership()` — replaces 8+ inline `is_member` + Forbidden checks
- `services::broadcast()` — wraps push_and_broadcast pattern
- `services::parse_id()` — generic prefixed-ID parser (replaces 4 near-identical functions)

`api/channels.rs` reduced by ~120 lines. Handlers now delegate to service functions for repeated patterns. `send_message` still owns multipart parsing (transport concern) but delegates message enrichment and broadcasting.

**Remaining opportunity:** Extract `send_message` file-upload logic into a dedicated `AttachmentService` if multipart handling grows more complex.

---

## 2. DRY Violations — Repeated Patterns — FIXED

### Backend — FIXED

- **Authorization boilerplate** — Extracted to `services::require_membership()`, called in all 8+ handlers.
- **ID parsing** — Single `services::parse_id(s, prefix, label)` replaces `parse_channel_id`, `parse_message_id`, `parse_user_id`, `parse_attachment_id`.
- **Message response assembly** — Centralised in `services::messages::enrich()` / `enrich_one()`. Duplicated `aggregate_reactions()`, `group_attachments()`, `build_message_response()` removed from `channels.rs`.

### Frontend — FIXED

- **`usersById` map** — Extracted to `useUsersById()` hook (`lib/hooks/use-users-by-id.ts`). Used by Sidebar, ChannelPage, SearchDialog.
- **DM partner name resolution** — Extracted to `resolveDmPartnerName()` (`lib/hooks/use-dm-label.ts`). Used by Sidebar and ChannelPage.
- **5 inline WS handlers** — Extracted to `useChannelWsSync()` hook (`lib/hooks/use-channel-ws-sync.ts`). ChannelPage reduced by ~115 lines.

### Remaining

- **Modal template HTML** — 3 dialogs still repeat backdrop/close structure. Extract `<Modal>` when a 4th dialog appears.

---

## 3. N+1 Query in `list_pins` — FIXED

Added `db::messages::find_by_ids(&[Uuid])` with `WHERE id = ANY($1)`. `list_pins` handler now does 1 query instead of N.

---

## 4. Error Handling Granularity — FIXED

Expanded `From<sqlx::Error>` to map 4 PostgreSQL error codes:
- `23505` (unique violation) → 409 Conflict
- `23503` (FK violation) → 400 Bad Request
- `23514` (check constraint) → 400 Bad Request
- `23502` (not-null violation) → 400 Bad Request

`Internal` variant now uses structured `tracing::error!(error = %err)` and returns generic "database error" to avoid leaking query params in responses.

---

## 5. Open/Closed Principle — Storage Enum — DEFERRED

```rust
pub enum Storage { Local(local::LocalStorage) }
```

Adding S3 requires modifying the enum and every match block. Switch to trait when S3 lands. With a single variant, enum dispatch is zero-cost and the project's own principle applies: "split only when pain is real."

---

## 6. Frontend — Business Logic in Presentational Components — PARTIALLY FIXED

- `ChannelPage` 5 inline WS handlers → extracted to `useChannelWsSync()` hook.
- `MessageBubble` still contains `useMutation` for reactions/pins. Acceptable trade-off: mutations are colocated with the UI that triggers them, and extracting would require prop-drilling callbacks. Revisit if MessageBubble grows beyond ~200 lines.

---

## 7. Missing Deferred Cleanup (Storage Leak) — OPEN

ADR-011 specifies "deferred cleanup after soft delete" but no implementation exists. Orphaned files accumulate indefinitely. Needs background task infrastructure (separate feature work).

---

## 8. Minor Issues

| Issue | Where | Impact | Status |
|-------|-------|--------|--------|
| WS buffer capacities hardcoded | `ws/mod.rs` | Ops pain at scale | **FIXED** — configurable via `[websocket]` config |
| Unread count correlated subquery | `db/channels.rs` | O(n) subqueries | Open |
| Raw sqlx error in logs | `error.rs` | Could leak query params | **FIXED** — structured tracing, generic response |
| No global error toast | `ui/src/` | Silent mutation errors | Open |
| WS handler calls `api::users::jit_provision` | `ws/handler.rs` | Layer violation | Open (minor) |

---

## Priority Roadmap

| Priority | Action | Pattern Fixed | Status |
|----------|--------|---------------|--------|
| **P0** | Fix N+1 in `list_pins` (add `find_by_ids`) | Performance | **Done** |
| **P1** | Extract `services/` module (messages, auth, broadcast, ID parsing) | SRP, DRY, testability | **Done** |
| **P2** | Add `useUsersById()` + `useChannelWsSync()` + `resolveDmPartnerName()` | DRY (frontend) | **Done** |
| **P2** | Expand `ApiError` variants + PG error code mapping | Error handling | **Done** |
| **P3** | Make WS buffer capacities configurable | 12-factor | **Done** |
| **P2** | Implement orphaned attachment cleanup job | Storage leak | Open |
| **P3** | Extract `<Modal>` component | DRY (frontend) | Deferred |
| **P3** | Storage trait for OCP when S3 lands | SOLID | Deferred |

---

## What's Already Good

- **Clean dependency graph**: core → server → binary, no cycles
- **12-factor config**: TOML + env overrides, secrets separated
- **Testing**: real DB integration tests, proper harness, E2E with Playwright
- **API governance**: OpenAPI source of truth, Vacuum CI gate, RFC 9457 errors
- **Domain isolation**: `burst-core` is pure, zero I/O
- **WebSocket maturity**: gap-fill, presence, event buffering, heartbeat
