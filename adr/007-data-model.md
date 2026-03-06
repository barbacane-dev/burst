# ADR-007: Data Model

**Status:** Accepted
**Date:** 2026-03-06

## Context

Per [ADR-003](003-technology-stack.md), Burst uses PostgreSQL as its sole data store for users, channels, messages, and metadata. Per [ADR-004](004-real-time-architecture.md), events use UUIDv7 for time-sortable IDs and gap-fill on reconnect. Per [ADR-006](006-authentication-and-authorization.md), user identity comes from Barbacane headers with JIT provisioning from external IdPs.

We need a schema that supports:

- Channels (public, private) and direct messages (1-to-1, group).
- Threaded replies without fragmenting the channel timeline.
- Full-text search via tsvector (default) or async sync to Typesense.
- Cursor-based pagination on messages (per ADR-005).
- Audit trail for edits and deletions.
- Efficient fan-out queries (which users need to receive this event?).

## Decision

### ID strategy

All primary keys use **UUIDv7** — time-sortable, globally unique, no sequence contention. Generated application-side, not by PostgreSQL. This enables:

- Natural chronological ordering without a separate `created_at` index for pagination.
- Cursor-based pagination using the ID itself as the cursor.
- Distributed ID generation (no coordination needed across Burst nodes).

IDs are prefixed in the API layer for readability (`usr_`, `ch_`, `msg_`, `thr_`, `att_`) but stored as raw UUID in the database.

### Core tables

#### users

```sql
CREATE TABLE users (
    id          UUID PRIMARY KEY,           -- UUIDv7
    external_id TEXT UNIQUE,                -- IdP subject (sub claim), nullable for local users
    username    TEXT NOT NULL UNIQUE,
    display_name TEXT NOT NULL,
    email       TEXT UNIQUE,
    avatar_url  TEXT,
    role        TEXT NOT NULL DEFAULT 'member',  -- admin, moderator, member, guest
    status      TEXT NOT NULL DEFAULT 'offline', -- online, away, offline, dnd
    status_text TEXT,                        -- custom status message
    password_hash TEXT,                      -- nullable, only for local auth users
    is_bot      BOOLEAN NOT NULL DEFAULT FALSE,
    deactivated_at TIMESTAMPTZ,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
```

- `external_id` links to the IdP subject for JIT-provisioned users.
- `password_hash` is only set for local auth users (ADR-006). NULL for SSO/OIDC users.
- `deactivated_at` is a soft delete — deactivated users remain for message attribution but cannot log in.

#### channels

```sql
CREATE TABLE channels (
    id          UUID PRIMARY KEY,           -- UUIDv7
    kind        TEXT NOT NULL,              -- 'public', 'private', 'dm', 'group_dm'
    name        TEXT,                       -- nullable for DMs
    slug        TEXT UNIQUE,                -- URL-friendly name, nullable for DMs
    topic       TEXT,
    description TEXT,
    created_by  UUID NOT NULL REFERENCES users(id),
    is_archived BOOLEAN NOT NULL DEFAULT FALSE,
    is_readonly BOOLEAN NOT NULL DEFAULT FALSE,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_channels_kind ON channels(kind);
```

- `kind` distinguishes channel types. DMs are channels with `kind = 'dm'` or `'group_dm'` — no special table, same fan-out logic.
- `slug` is used for URL-friendly channel references (`/channels/engineering`). NULL for DMs.

#### channel_members

```sql
CREATE TABLE channel_members (
    channel_id  UUID NOT NULL REFERENCES channels(id) ON DELETE CASCADE,
    user_id     UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role        TEXT NOT NULL DEFAULT 'member',  -- 'owner', 'moderator', 'member'
    notify      TEXT NOT NULL DEFAULT 'all',     -- 'all', 'mentions', 'nothing'
    joined_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (channel_id, user_id)
);

CREATE INDEX idx_channel_members_user ON channel_members(user_id);
```

- Composite primary key — a user is either in a channel or not.
- `role` is channel-scoped (separate from the instance-wide role in `users`).
- `notify` stores per-channel notification preference.
- `idx_channel_members_user` enables fast "which channels is this user in?" queries (needed for WebSocket subscription on connect).

#### messages

```sql
CREATE TABLE messages (
    id          UUID PRIMARY KEY,           -- UUIDv7
    channel_id  UUID NOT NULL REFERENCES channels(id) ON DELETE CASCADE,
    user_id     UUID NOT NULL REFERENCES users(id),
    thread_id   UUID REFERENCES messages(id),  -- NULL = top-level, set = reply in thread
    content     TEXT NOT NULL,
    edited_at   TIMESTAMPTZ,
    deleted_at  TIMESTAMPTZ,                -- soft delete, content cleared
    search_vec  TSVECTOR,                   -- full-text search index
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_messages_channel_id ON messages(channel_id, id);
CREATE INDEX idx_messages_thread_id ON messages(thread_id, id) WHERE thread_id IS NOT NULL;
CREATE INDEX idx_messages_search ON messages USING GIN(search_vec);
```

- `thread_id` is a self-referencing FK to the thread's root message. Top-level messages have `thread_id = NULL`. This keeps threads flat (no nested replies) and allows querying a thread as `WHERE thread_id = :root_msg_id ORDER BY id`.
- `deleted_at` enables soft delete — content is cleared but the row remains for timeline continuity ("This message was deleted").
- `search_vec` is auto-updated via a trigger on INSERT/UPDATE. Used by PG full-text search. When Typesense is enabled, an async job syncs new/updated messages.
- `idx_messages_channel_id` is the primary query path: "messages in this channel, ordered by time" = `WHERE channel_id = :id ORDER BY id` (UUIDv7 is time-ordered).

#### custom_emojis

```sql
CREATE TABLE custom_emojis (
    id          UUID PRIMARY KEY,           -- UUIDv7
    shortcode   TEXT NOT NULL UNIQUE,        -- e.g., 'partyparrot', 'shipit'
    image_url   TEXT NOT NULL,               -- storage path (local FS or S3)
    created_by  UUID NOT NULL REFERENCES users(id),
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
```

- Instance-wide custom emoji, uploaded by admins.
- `shortcode` is the text identifier used in messages (`:partyparrot:`) and reactions.

#### reactions

```sql
CREATE TABLE reactions (
    message_id  UUID NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
    user_id     UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    emoji       TEXT NOT NULL,               -- unicode emoji (e.g., '👍') or custom shortcode (e.g., 'partyparrot')
    is_custom   BOOLEAN NOT NULL DEFAULT FALSE,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (message_id, user_id, emoji)
);
```

- Composite PK prevents duplicate reactions (same user, same emoji, same message).
- `is_custom` distinguishes unicode emoji from custom emoji. When `is_custom = true`, `emoji` contains the shortcode that references `custom_emojis.shortcode`.
- Queried as aggregates: `SELECT emoji, is_custom, COUNT(*), array_agg(user_id) FROM reactions WHERE message_id = :id GROUP BY emoji, is_custom`.

#### attachments

```sql
CREATE TABLE attachments (
    id          UUID PRIMARY KEY,           -- UUIDv7
    message_id  UUID NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
    file_name   TEXT NOT NULL,
    file_size   BIGINT NOT NULL,
    content_type TEXT NOT NULL,
    storage_key TEXT NOT NULL,               -- path in local FS or S3 key
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_attachments_message ON attachments(message_id);
```

- Files are stored externally (local FS or S3 per ADR-003). The `storage_key` references the location.
- `content_type` enables inline preview decisions on the frontend (image, video, PDF vs. generic download).

#### pinned_messages

```sql
CREATE TABLE pinned_messages (
    channel_id  UUID NOT NULL REFERENCES channels(id) ON DELETE CASCADE,
    message_id  UUID NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
    pinned_by   UUID NOT NULL REFERENCES users(id),
    pinned_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (channel_id, message_id)
);
```

#### webhooks

```sql
CREATE TABLE webhooks (
    id          UUID PRIMARY KEY,           -- UUIDv7
    channel_id  UUID NOT NULL REFERENCES channels(id) ON DELETE CASCADE,
    kind        TEXT NOT NULL,               -- 'incoming', 'outgoing'
    name        TEXT NOT NULL,
    url         TEXT,                        -- target URL for outgoing
    secret      TEXT,                        -- HMAC signing secret for outgoing
    token       TEXT NOT NULL UNIQUE,        -- auth token for incoming
    created_by  UUID NOT NULL REFERENCES users(id),
    is_active   BOOLEAN NOT NULL DEFAULT TRUE,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
```

#### audit_log

```sql
CREATE TABLE audit_log (
    id          UUID PRIMARY KEY,           -- UUIDv7
    user_id     UUID REFERENCES users(id),
    action      TEXT NOT NULL,               -- 'user.created', 'channel.archived', etc.
    target_type TEXT NOT NULL,               -- 'user', 'channel', 'message', etc.
    target_id   UUID NOT NULL,
    metadata    JSONB,                       -- action-specific details
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_audit_log_target ON audit_log(target_type, target_id);
CREATE INDEX idx_audit_log_created ON audit_log(created_at);
```

- Append-only. No updates, no deletes.
- `metadata` stores action-specific context (e.g., old/new values for settings changes, reason for user deactivation).

#### refresh_tokens

```sql
CREATE TABLE refresh_tokens (
    id          UUID PRIMARY KEY,           -- UUIDv7
    user_id     UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token_hash  TEXT NOT NULL UNIQUE,        -- bcrypt/argon2 hash of the token
    expires_at  TIMESTAMPTZ NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_refresh_tokens_user ON refresh_tokens(user_id);
```

- Only used for local auth (ADR-006). Tokens are stored hashed, never in plaintext.
- Expired tokens are cleaned up by a periodic background job.

### Full-text search trigger

```sql
CREATE FUNCTION update_search_vec() RETURNS TRIGGER AS $$
BEGIN
    NEW.search_vec := to_tsvector('english', COALESCE(NEW.content, ''));
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_messages_search
    BEFORE INSERT OR UPDATE OF content ON messages
    FOR EACH ROW EXECUTE FUNCTION update_search_vec();
```

### Conventions

- **Table names:** snake_case, plural (`users`, `channels`, `messages`).
- **Column names:** snake_case (`created_at`, `channel_id`).
- **Foreign keys:** `<referenced_table_singular>_id` (e.g., `user_id`, `channel_id`).
- **Indexes:** `idx_<table>_<columns>` (e.g., `idx_messages_channel_id`).
- **Timestamps:** All `TIMESTAMPTZ`, never `TIMESTAMP`. Always UTC.
- **Soft deletes:** via `<action>_at` columns (`deleted_at`, `deactivated_at`), not boolean flags.
- **No enums:** Use `TEXT` with application-level validation. PostgreSQL enums are hard to migrate.
- **Migrations:** Managed by sqlx-cli, embedded in the binary. Each migration is a numbered SQL file, applied on startup.

## Consequences

- **UUIDv7 as cursor** eliminates the need for a separate sequence or `created_at` index for pagination. The ID is the cursor. This simplifies the query pattern: `WHERE channel_id = :ch AND id < :cursor ORDER BY id DESC LIMIT :limit`.
- **DMs as channels** (no separate table) means all message queries, fan-out logic, and WebSocket subscription work identically for channels and DMs. The trade-off is that DM "creation" requires finding an existing DM channel between the same participants before creating a new one.
- **Flat threads** (one level of replies, no nesting) keeps the data model and UI simple. A reply is just a message with `thread_id` set. This matches Slack's model and avoids the complexity of deeply nested discussions.
- **Soft deletes everywhere** preserves timeline integrity and audit trail. The cost is slightly more complex queries (`WHERE deleted_at IS NULL`) — mitigated by partial indexes where needed.
- **TEXT instead of enums** trades type safety for migration flexibility. New roles, channel kinds, or statuses can be added without `ALTER TYPE` migrations that lock tables.
- **The schema is deliberately normalized.** We avoid denormalization until profiling shows a real bottleneck. PostgreSQL handles JOINs efficiently at team scale (hundreds of users, thousands of channels).
- **The search trigger adds write overhead** to every message INSERT/UPDATE. At team scale this is negligible. When Typesense is enabled, the trigger still runs (PG search remains the fallback) but the primary search path shifts to Typesense.
