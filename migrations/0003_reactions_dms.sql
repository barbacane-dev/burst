-- M4: Reactions and unread tracking

-- Reactions: one row per (message, user, emoji) triple
CREATE TABLE reactions (
    message_id UUID        NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
    user_id    UUID        NOT NULL REFERENCES users(id)    ON DELETE CASCADE,
    emoji      TEXT        NOT NULL CHECK (char_length(emoji) <= 64),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (message_id, user_id, emoji)
);

CREATE INDEX reactions_message_idx ON reactions (message_id);

-- Track when each member last read a channel (drives unread badge in sidebar)
ALTER TABLE channel_members ADD COLUMN last_read_at TIMESTAMPTZ;
