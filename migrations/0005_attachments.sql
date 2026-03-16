-- File attachments for messages (ADR-007, ADR-011, ADR-011b).

CREATE TABLE attachments (
    id           UUID PRIMARY KEY,
    message_id   UUID NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
    file_name    TEXT NOT NULL,
    file_size    BIGINT NOT NULL,
    content_type TEXT NOT NULL,
    storage_key  TEXT NOT NULL,
    metadata     JSONB NOT NULL DEFAULT '{}',
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_attachments_message ON attachments(message_id);
