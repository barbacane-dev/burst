# ADR-011b: Attachment Schema

**Status:** Draft
**Date:** 2026-03-09
**Extends:** [ADR-007](007-data-model.md) (`attachments` table), [ADR-011](011-file-storage.md) (upload flow)

## Context

[ADR-007](007-data-model.md) defines the base `attachments` table with `id`, `message_id`, `file_name`, `file_size`, `content_type`, `storage_key`, and `created_at`. [ADR-011](011-file-storage.md) states that image dimensions (width × height) are extracted during upload to allow the frontend to reserve layout space before images load — preventing cumulative layout shift (CLS).

The base schema has no columns for this metadata. As the application evolves, other media types will carry their own metadata: video files will have duration, PDFs may have page count. We need a schema that accommodates current and future media metadata without requiring a migration per new media type.

## Decision

Add a `metadata` JSONB column to the `attachments` table:

```sql
ALTER TABLE attachments
    ADD COLUMN metadata JSONB NOT NULL DEFAULT '{}';
```

Full revised schema:

```sql
CREATE TABLE attachments (
    id           UUID PRIMARY KEY,           -- UUIDv7
    message_id   UUID NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
    file_name    TEXT NOT NULL,
    file_size    BIGINT NOT NULL,
    content_type TEXT NOT NULL,
    storage_key  TEXT NOT NULL,              -- path in local FS or S3 key
    metadata     JSONB NOT NULL DEFAULT '{}',
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_attachments_message ON attachments(message_id);
```

### Metadata shape

The shape of `metadata` is determined by `content_type`. `content_type` is the canonical source of truth for media type — `metadata` is never queried at the database level and carries no type discriminator of its own.

| `content_type` | `metadata` shape |
|---|---|
| `image/*` | `{"width": 1920, "height": 1080}` |
| `video/*` (future) | `{"duration_s": 142}` |
| `application/pdf` (future) | `{"pages": 12}` |
| anything else | `{}` |

Non-media attachments carry an empty object. No sentinel values, no nulls on the column.

### Extraction failures

If metadata extraction fails (corrupt header, unsupported format), Burst stores `{}` and continues. The upload is not rejected. The frontend falls back to natural sizing for images.

## Alternatives considered

**Nullable `width`/`height` columns** — simple, typed, but requires a migration per new media type. Rejected because adding video duration or PDF page count post-v1 would each require a schema change.

**Separate `attachment_metadata` table** — requires a join on every message load with no benefit, since metadata is always fetched as part of a message and never queried at the database level. If typed, it still requires migrations per new media type. If generic key-value (EAV), it is worse than JSONB in every dimension.

**Type discriminator inside `metadata`** — redundant with `content_type`, which is already the authoritative media type field. Two sources of truth that can drift. Rejected.

## Consequences

- **No migrations for new media types.** Adding video duration or PDF page count is an application-layer change — no schema migration needed.
- **Frontend can avoid layout shift.** Image dimensions are returned in the message payload; the client knows the aspect ratio before the image loads.
- **No GIN index.** Metadata is never queried at the database level, only fetched. No index is warranted.
- **`content_type` drives interpretation.** The application reads `content_type` to decide which fields to expect in `metadata`. No duplication, no ambiguity.
- **Extraction failures are silent.** `{}` is indistinguishable from "no metadata available" vs "extraction failed". Acceptable for v1 — the upload succeeds either way.
