-- Tighten foreign key constraints and add missing indexes.

-- messages.user_id: prevent user hard-deletion while messages exist.
-- Burst uses soft-delete (deactivation), so RESTRICT guards against accidents.
ALTER TABLE messages
    DROP CONSTRAINT messages_user_id_fkey,
    ADD CONSTRAINT messages_user_id_fkey
        FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE RESTRICT;

-- messages.thread_id: cascade-delete replies when root message is deleted.
ALTER TABLE messages
    DROP CONSTRAINT messages_thread_id_fkey,
    ADD CONSTRAINT messages_thread_id_fkey
        FOREIGN KEY (thread_id) REFERENCES messages(id) ON DELETE CASCADE;

-- pinned_messages.pinned_by: cascade on user deletion (pin stays, pinned_by gone).
-- pinned_by is NOT NULL, so we cascade to remove the pin record entirely.
ALTER TABLE pinned_messages
    DROP CONSTRAINT pinned_messages_pinned_by_fkey,
    ADD CONSTRAINT pinned_messages_pinned_by_fkey
        FOREIGN KEY (pinned_by) REFERENCES users(id) ON DELETE CASCADE;

-- Missing index: reactions by user (needed for "my reactions" queries).
CREATE INDEX idx_reactions_user ON reactions(user_id);
