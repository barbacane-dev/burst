-- Data exports: an archive of messages and files, built by a background job.
CREATE TABLE exports (
    id           UUID PRIMARY KEY,
    requested_by UUID NOT NULL REFERENCES users(id),
    scope        TEXT NOT NULL CHECK (scope IN ('instance', 'channel')),
    channel_id   UUID REFERENCES channels(id) ON DELETE SET NULL,
    status       TEXT NOT NULL DEFAULT 'pending'
                 CHECK (status IN ('pending', 'running', 'completed', 'failed')),
    storage_key  TEXT,
    size_bytes   BIGINT,
    error        TEXT,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    -- Touched as the job progresses; a running export left untouched is dead.
    updated_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    completed_at TIMESTAMPTZ
);

CREATE INDEX exports_created_at ON exports (created_at DESC);
-- One export in progress at a time.
CREATE UNIQUE INDEX exports_one_active ON exports ((true)) WHERE status IN ('pending', 'running');
