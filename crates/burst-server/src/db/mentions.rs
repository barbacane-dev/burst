use sqlx::PgPool;
use uuid::Uuid;

/// Inserts mention records for a message. Skips duplicates.
pub async fn insert_mentions(
    pool: &PgPool,
    message_id: Uuid,
    user_ids: &[Uuid],
) -> Result<(), sqlx::Error> {
    for &user_id in user_ids {
        sqlx::query(
            "INSERT INTO message_mentions (message_id, user_id) \
             VALUES ($1, $2) ON CONFLICT DO NOTHING",
        )
        .bind(message_id)
        .bind(user_id)
        .execute(pool)
        .await?;
    }
    Ok(())
}

/// Returns all user IDs mentioned in the given messages.
pub async fn list_for_messages(
    pool: &PgPool,
    message_ids: &[Uuid],
) -> Result<Vec<(Uuid, Uuid)>, sqlx::Error> {
    if message_ids.is_empty() {
        return Ok(vec![]);
    }
    let rows = sqlx::query_as::<_, (Uuid, Uuid)>(
        "SELECT message_id, user_id FROM message_mentions \
         WHERE message_id = ANY($1)",
    )
    .bind(message_ids)
    .fetch_all(pool)
    .await?;

    Ok(rows)
}
