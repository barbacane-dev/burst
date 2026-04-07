-- Webhooks (ADR-007, Milestone 8)
CREATE TABLE webhooks (
    id          UUID PRIMARY KEY,
    channel_id  UUID NOT NULL REFERENCES channels(id) ON DELETE CASCADE,
    kind        TEXT NOT NULL CHECK (kind IN ('incoming', 'outgoing')),
    name        TEXT NOT NULL,
    url         TEXT,
    secret      TEXT,
    token       TEXT NOT NULL UNIQUE,
    created_by  UUID NOT NULL REFERENCES users(id),
    is_active   BOOLEAN NOT NULL DEFAULT TRUE,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_webhooks_channel ON webhooks(channel_id);
CREATE INDEX idx_webhooks_token ON webhooks(token);
