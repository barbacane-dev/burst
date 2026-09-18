# ADR-002: Core Feature Set

**Status:** Accepted, amended by [ADR-014](014-gateway-optional-deployment.md) (2026-09-15): the Barbacane gateway is the recommended production front door, not a requirement.
**Date:** 2026-03-06

## Context

Per [ADR-001](001-project-vision-and-scope.md), Burst is a focused messaging tool. We need to define precisely what "messaging" means for us — what is in scope, what is explicitly out, and what sits at the boundary as a future consideration.

The goal is to ship a product that covers the daily communication needs of a small-to-medium technical team without creeping into adjacent domains.

## Decision

### In scope (core)

**Conversations**
- **Channels** — public and private, with topic/description and pinned messages.
- **Direct messages** — 1-to-1 and group DMs.
- **Threads** — reply threads on any message to keep conversations organised without fragmenting channel flow.
- **Mentions** — @user, @channel, @here with proper notification routing.

**Messages**
- Rich text with Markdown support.
- Message editing and deletion (with audit trail).
- Reactions (emoji).
- Link previews (unfurling).

**File sharing**
- Upload and share files within conversations (images, documents, archives).
- Inline image/media preview.
- Configurable storage backend (local filesystem, S3-compatible).

**Search**
- Full-text search across messages and file names.
- Scoped search (per channel, per conversation).
- Search filters (from user, date range, has file).

**Notifications**
- Per-channel notification preferences (all, mentions only, nothing).
- Desktop and mobile push notifications.
- Do-not-disturb / schedule-based muting.
- Unread indicators and badge counts.

**User presence**
- Online / away / offline status.
- Custom status messages.

**Administration**
- User management (invite, deactivate, roles).
- Channel management (archive, read-only).
- Role-based permissions (admin, moderator, member, guest).
- LDAP / SSO integration for authentication (not gated behind enterprise).
- Audit log for administrative actions.
- Data export (messages, files) for compliance and portability.

**Integration surface**
- Incoming and outgoing webhooks.
- Bot user accounts with API access.
- REST API for all core features, designed spec-first with OpenAPI.
- **Barbacane API gateway as the front door** — Burst's REST API is served through Barbacane, dogfooding the gateway for authentication, rate limiting, request validation, and observability. This makes Burst a real-world proving ground for Barbacane itself.

### Explicitly out of scope

These are capabilities we will **not** build into Burst, per the principles in ADR-001:

- Video / audio calling — use dedicated tools (Jitsi, Meet, etc.).
- App marketplace / plugin store — we provide webhooks and APIs, not an app platform.
- Omnichannel / customer support features — Burst is for internal team communication.
- Built-in AI assistants or LLM integrations — can be built externally via the bot/API surface.
- Email integration (send/receive email as messages).
- Task management / kanban boards.
- CRM or contact management.

### Boundary (future consideration)

These are features we may consider later but are **not in the initial scope**:

- End-to-end encryption for DMs.
- Voice messages (audio clips in chat).
- Message scheduling (send later).
- Federated messaging between Burst instances.
- Bridge to other messaging protocols (Matrix, IRC, XMPP).

## Consequences

- The core feature set is deliberately conventional. We are not trying to innovate on what messaging is — we are trying to execute it well and keep it open.
- Saying no to video, AI, and marketplace features will disappoint some evaluators. We accept this trade-off and point them to the integration surface instead.
- LDAP/SSO and audit logs being in the core (not enterprise-gated) is a differentiator. It raises the bar for the initial release but is essential to the project's identity.
- The integration surface (webhooks, bots, REST API) must be well-designed from the start, since it is our answer to "but I need feature X" — teams can build it themselves or connect external tools.
- Using Barbacane as the API gateway creates a dogfooding loop: Burst stress-tests Barbacane in production, and Barbacane provides Burst with battle-tested auth, rate limiting, and validation out of the box. This also means the Burst API is spec-first by design (OpenAPI as the source of truth), which aligns naturally with Barbacane's spec-driven philosophy.
