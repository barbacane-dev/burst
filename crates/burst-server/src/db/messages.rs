use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

#[derive(Debug, FromRow)]
pub struct MessageRow {
    pub id: Uuid,
    pub channel_id: Uuid,
    pub user_id: Uuid,
    pub thread_id: Option<Uuid>,
    pub content: String,
    pub edited_at: Option<DateTime<Utc>>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

pub async fn create(
    pool: &PgPool,
    id: Uuid,
    channel_id: Uuid,
    user_id: Uuid,
    thread_id: Option<Uuid>,
    content: &str,
) -> Result<MessageRow, sqlx::Error> {
    sqlx::query_as::<_, MessageRow>(
        "INSERT INTO messages (id, channel_id, user_id, thread_id, content) \
         VALUES ($1, $2, $3, $4, $5) \
         RETURNING id, channel_id, user_id, thread_id, content, \
         edited_at, deleted_at, created_at",
    )
    .bind(id)
    .bind(channel_id)
    .bind(user_id)
    .bind(thread_id)
    .bind(content)
    .fetch_one(pool)
    .await
}

pub async fn find_by_id(pool: &PgPool, id: Uuid) -> Result<Option<MessageRow>, sqlx::Error> {
    sqlx::query_as::<_, MessageRow>(
        "SELECT id, channel_id, user_id, thread_id, content, \
         edited_at, deleted_at, created_at \
         FROM messages WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

/// Fetches multiple messages by ID in a single query.
pub async fn find_by_ids(pool: &PgPool, ids: &[Uuid]) -> Result<Vec<MessageRow>, sqlx::Error> {
    if ids.is_empty() {
        return Ok(vec![]);
    }
    sqlx::query_as::<_, MessageRow>(
        "SELECT id, channel_id, user_id, thread_id, content, \
         edited_at, deleted_at, created_at \
         FROM messages WHERE id = ANY($1)",
    )
    .bind(ids)
    .fetch_all(pool)
    .await
}

/// Lists top-level messages in a channel (thread replies excluded).
pub async fn list_in_channel(
    pool: &PgPool,
    channel_id: Uuid,
    cursor: Option<Uuid>,
    limit: i64,
) -> Result<Vec<MessageRow>, sqlx::Error> {
    match cursor {
        Some(cursor_id) => {
            sqlx::query_as::<_, MessageRow>(
                "SELECT id, channel_id, user_id, thread_id, content, \
                 edited_at, deleted_at, created_at \
                 FROM messages \
                 WHERE channel_id = $1 AND thread_id IS NULL AND id < $2 \
                 ORDER BY id DESC LIMIT $3",
            )
            .bind(channel_id)
            .bind(cursor_id)
            .bind(limit)
            .fetch_all(pool)
            .await
        }
        None => {
            sqlx::query_as::<_, MessageRow>(
                "SELECT id, channel_id, user_id, thread_id, content, \
                 edited_at, deleted_at, created_at \
                 FROM messages \
                 WHERE channel_id = $1 AND thread_id IS NULL \
                 ORDER BY id DESC LIMIT $2",
            )
            .bind(channel_id)
            .bind(limit)
            .fetch_all(pool)
            .await
        }
    }
}

/// Lists replies in a thread (ascending, oldest first).
pub async fn list_in_thread(
    pool: &PgPool,
    thread_id: Uuid,
    cursor: Option<Uuid>,
    limit: i64,
) -> Result<Vec<MessageRow>, sqlx::Error> {
    match cursor {
        Some(cursor_id) => {
            sqlx::query_as::<_, MessageRow>(
                "SELECT id, channel_id, user_id, thread_id, content, \
                 edited_at, deleted_at, created_at \
                 FROM messages \
                 WHERE thread_id = $1 AND id > $2 \
                 ORDER BY id ASC LIMIT $3",
            )
            .bind(thread_id)
            .bind(cursor_id)
            .bind(limit)
            .fetch_all(pool)
            .await
        }
        None => {
            sqlx::query_as::<_, MessageRow>(
                "SELECT id, channel_id, user_id, thread_id, content, \
                 edited_at, deleted_at, created_at \
                 FROM messages \
                 WHERE thread_id = $1 \
                 ORDER BY id ASC LIMIT $2",
            )
            .bind(thread_id)
            .bind(limit)
            .fetch_all(pool)
            .await
        }
    }
}

/// Returns non-deleted reply counts for a batch of message IDs.
/// Only IDs with at least one reply are present in the result.
pub async fn reply_counts(
    pool: &PgPool,
    message_ids: &[Uuid],
) -> Result<Vec<(Uuid, i64)>, sqlx::Error> {
    if message_ids.is_empty() {
        return Ok(vec![]);
    }
    sqlx::query_as::<_, (Uuid, i64)>(
        "SELECT thread_id, COUNT(*) \
         FROM messages \
         WHERE thread_id = ANY($1) AND deleted_at IS NULL \
         GROUP BY thread_id",
    )
    .bind(message_ids)
    .fetch_all(pool)
    .await
}

pub async fn update_content(
    pool: &PgPool,
    id: Uuid,
    user_id: Uuid,
    content: &str,
) -> Result<Option<MessageRow>, sqlx::Error> {
    sqlx::query_as::<_, MessageRow>(
        "UPDATE messages SET content = $3, edited_at = now() \
         WHERE id = $1 AND user_id = $2 AND deleted_at IS NULL \
         RETURNING id, channel_id, user_id, thread_id, content, \
         edited_at, deleted_at, created_at",
    )
    .bind(id)
    .bind(user_id)
    .bind(content)
    .fetch_optional(pool)
    .await
}

pub async fn soft_delete(
    pool: &PgPool,
    id: Uuid,
    user_id: Uuid,
) -> Result<Option<MessageRow>, sqlx::Error> {
    sqlx::query_as::<_, MessageRow>(
        "UPDATE messages SET content = '', deleted_at = now() \
         WHERE id = $1 AND user_id = $2 AND deleted_at IS NULL \
         RETURNING id, channel_id, user_id, thread_id, content, \
         edited_at, deleted_at, created_at",
    )
    .bind(id)
    .bind(user_id)
    .fetch_optional(pool)
    .await
}

/// Soft-deletes any message in a channel, whoever wrote it. For moderators;
/// `soft_delete` is the author's own path.
pub async fn soft_delete_in_channel(
    pool: &PgPool,
    id: Uuid,
    channel_id: Uuid,
) -> Result<Option<MessageRow>, sqlx::Error> {
    sqlx::query_as::<_, MessageRow>(
        "UPDATE messages SET content = '', deleted_at = now() \
         WHERE id = $1 AND channel_id = $2 AND deleted_at IS NULL \
         RETURNING id, channel_id, user_id, thread_id, content, \
         edited_at, deleted_at, created_at",
    )
    .bind(id)
    .bind(channel_id)
    .fetch_optional(pool)
    .await
}
