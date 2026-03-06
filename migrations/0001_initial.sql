CREATE TABLE users (
    id              UUID PRIMARY KEY,
    external_id     TEXT UNIQUE,
    username        TEXT NOT NULL UNIQUE,
    display_name    TEXT NOT NULL,
    email           TEXT UNIQUE,
    avatar_url      TEXT,
    role            TEXT NOT NULL DEFAULT 'member',
    status          TEXT NOT NULL DEFAULT 'offline',
    status_text     TEXT,
    password_hash   TEXT,
    is_bot          BOOLEAN NOT NULL DEFAULT FALSE,
    deactivated_at  TIMESTAMPTZ,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE refresh_tokens (
    id          UUID PRIMARY KEY,
    user_id     UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token_hash  TEXT NOT NULL UNIQUE,
    expires_at  TIMESTAMPTZ NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_refresh_tokens_user ON refresh_tokens(user_id);
