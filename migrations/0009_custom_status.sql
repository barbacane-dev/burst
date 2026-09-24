-- Custom status: an emoji beside the status text, and when both stop showing.
ALTER TABLE users
    ADD COLUMN status_emoji      TEXT,
    ADD COLUMN status_expires_at TIMESTAMPTZ;
