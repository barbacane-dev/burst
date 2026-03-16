use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

#[derive(Debug, FromRow)]
pub struct SearchResultRow {
    pub id: Uuid,
    pub channel_id: Uuid,
    pub user_id: Uuid,
    pub thread_id: Option<Uuid>,
    pub content: String,
    pub headline: String,
    pub created_at: DateTime<Utc>,
}

/// Full-text search across messages the user has access to.
///
/// Uses PostgreSQL `websearch_to_tsquery` for user-friendly query syntax
/// (quoted phrases, `-exclude`) and `ts_headline` for highlighted snippets.
/// Access control is enforced by joining `channel_members`.
pub async fn search_messages(
    pool: &PgPool,
    user_id: Uuid,
    query: &str,
    channel_id: Option<Uuid>,
    cursor: Option<Uuid>,
    limit: i64,
) -> Result<Vec<SearchResultRow>, sqlx::Error> {
    sqlx::query_as::<_, SearchResultRow>(
        "SELECT m.id, m.channel_id, m.user_id, m.thread_id, m.content, \
         ts_headline('english', m.content, websearch_to_tsquery('english', $1), \
           'StartSel=<mark>, StopSel=</mark>, MaxFragments=2, MaxWords=30') AS headline, \
         m.created_at \
         FROM messages m \
         JOIN channel_members cm ON cm.channel_id = m.channel_id AND cm.user_id = $2 \
         WHERE m.search_vec @@ websearch_to_tsquery('english', $1) \
           AND m.deleted_at IS NULL \
           AND ($3::uuid IS NULL OR m.channel_id = $3) \
           AND ($4::uuid IS NULL OR m.id < $4) \
         ORDER BY ts_rank(m.search_vec, websearch_to_tsquery('english', $1)) DESC, m.id DESC \
         LIMIT $5",
    )
    .bind(query)
    .bind(user_id)
    .bind(channel_id)
    .bind(cursor)
    .bind(limit)
    .fetch_all(pool)
    .await
}
