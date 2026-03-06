CREATE TABLE channels (
    id          UUID PRIMARY KEY,
    kind        TEXT NOT NULL,
    name        TEXT,
    slug        TEXT UNIQUE,
    topic       TEXT,
    description TEXT,
    created_by  UUID NOT NULL REFERENCES users(id),
    is_archived BOOLEAN NOT NULL DEFAULT FALSE,
    is_readonly BOOLEAN NOT NULL DEFAULT FALSE,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_channels_kind ON channels(kind);

CREATE TABLE channel_members (
    channel_id  UUID NOT NULL REFERENCES channels(id) ON DELETE CASCADE,
    user_id     UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role        TEXT NOT NULL DEFAULT 'member',
    notify      TEXT NOT NULL DEFAULT 'all',
    joined_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (channel_id, user_id)
);

CREATE INDEX idx_channel_members_user ON channel_members(user_id);

CREATE TABLE messages (
    id          UUID PRIMARY KEY,
    channel_id  UUID NOT NULL REFERENCES channels(id) ON DELETE CASCADE,
    user_id     UUID NOT NULL REFERENCES users(id),
    thread_id   UUID REFERENCES messages(id),
    content     TEXT NOT NULL,
    edited_at   TIMESTAMPTZ,
    deleted_at  TIMESTAMPTZ,
    search_vec  TSVECTOR,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_messages_channel_id ON messages(channel_id, id);
CREATE INDEX idx_messages_thread_id ON messages(thread_id, id) WHERE thread_id IS NOT NULL;
CREATE INDEX idx_messages_search ON messages USING GIN(search_vec);

CREATE FUNCTION update_search_vec() RETURNS TRIGGER AS $$
BEGIN
    NEW.search_vec := to_tsvector('english', COALESCE(NEW.content, ''));
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_messages_search
    BEFORE INSERT OR UPDATE OF content ON messages
    FOR EACH ROW EXECUTE FUNCTION update_search_vec();
