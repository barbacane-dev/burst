use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

#[derive(Debug, FromRow)]
pub struct PinnedMessageRow {
    pub channel_id: Uuid,
    pub message_id: Uuid,
    pub pinned_by: Uuid,
    pub pinned_at: DateTime<Utc>,
}

/// Pins a message. Returns `true` if newly pinned, `false` if already pinned.
pub async fn pin(
    pool: &PgPool,
    channel_id: Uuid,
    message_id: Uuid,
    pinned_by: Uuid,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        "INSERT INTO pinned_messages (channel_id, message_id, pinned_by) \
         VALUES ($1, $2, $3) ON CONFLICT DO NOTHING",
    )
    .bind(channel_id)
    .bind(message_id)
    .bind(pinned_by)
    .execute(pool)
    .await?;

    Ok(result.rows_affected() > 0)
}

/// Unpins a message. Returns `true` if it was pinned and is now removed.
pub async fn unpin(pool: &PgPool, channel_id: Uuid, message_id: Uuid) -> Result<bool, sqlx::Error> {
    let result =
        sqlx::query("DELETE FROM pinned_messages WHERE channel_id = $1 AND message_id = $2")
            .bind(channel_id)
            .bind(message_id)
            .execute(pool)
            .await?;

    Ok(result.rows_affected() > 0)
}

/// Lists all pinned messages in a channel, newest pins first.
pub async fn list_for_channel(
    pool: &PgPool,
    channel_id: Uuid,
) -> Result<Vec<PinnedMessageRow>, sqlx::Error> {
    sqlx::query_as::<_, PinnedMessageRow>(
        "SELECT channel_id, message_id, pinned_by, pinned_at \
         FROM pinned_messages WHERE channel_id = $1 \
         ORDER BY pinned_at DESC",
    )
    .bind(channel_id)
    .fetch_all(pool)
    .await
}
