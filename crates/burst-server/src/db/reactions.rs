use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

#[derive(Debug, FromRow)]
pub struct ReactionRow {
    pub message_id: Uuid,
    pub user_id: Uuid,
    pub emoji: String,
    pub created_at: DateTime<Utc>,
}

/// Adds a reaction. Returns `true` if newly inserted, `false` if already existed.
pub async fn add(
    pool: &PgPool,
    message_id: Uuid,
    user_id: Uuid,
    emoji: &str,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        "INSERT INTO reactions (message_id, user_id, emoji) \
         VALUES ($1, $2, $3) ON CONFLICT DO NOTHING",
    )
    .bind(message_id)
    .bind(user_id)
    .bind(emoji)
    .execute(pool)
    .await?;

    Ok(result.rows_affected() > 0)
}

/// Removes a reaction. Returns `true` if it existed and was removed.
pub async fn remove(
    pool: &PgPool,
    message_id: Uuid,
    user_id: Uuid,
    emoji: &str,
) -> Result<bool, sqlx::Error> {
    let result =
        sqlx::query("DELETE FROM reactions WHERE message_id = $1 AND user_id = $2 AND emoji = $3")
            .bind(message_id)
            .bind(user_id)
            .bind(emoji)
            .execute(pool)
            .await?;

    Ok(result.rows_affected() > 0)
}

/// Fetches all reactions for a set of messages in a single query.
pub async fn list_for_messages(
    pool: &PgPool,
    message_ids: &[Uuid],
) -> Result<Vec<ReactionRow>, sqlx::Error> {
    if message_ids.is_empty() {
        return Ok(vec![]);
    }
    sqlx::query_as::<_, ReactionRow>(
        "SELECT message_id, user_id, emoji, created_at \
         FROM reactions WHERE message_id = ANY($1) \
         ORDER BY created_at",
    )
    .bind(message_ids)
    .fetch_all(pool)
    .await
}
