use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

const COLUMNS: &str = "id, requested_by, scope, channel_id, status, storage_key, size_bytes, \
                       error, created_at, updated_at, completed_at";

#[derive(Debug, Clone, FromRow)]
pub struct ExportRow {
    pub id: Uuid,
    pub requested_by: Uuid,
    pub scope: String,
    pub channel_id: Option<Uuid>,
    pub status: String,
    pub storage_key: Option<String>,
    pub size_bytes: Option<i64>,
    pub error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

/// Records a pending export. `None` when another export is already pending or
/// running.
pub async fn create(
    pool: &PgPool,
    id: Uuid,
    requested_by: Uuid,
    channel_id: Option<Uuid>,
) -> Result<Option<ExportRow>, sqlx::Error> {
    let scope = if channel_id.is_some() {
        "channel"
    } else {
        "instance"
    };
    let result = sqlx::query_as::<_, ExportRow>(&format!(
        "INSERT INTO exports (id, requested_by, scope, channel_id) VALUES ($1, $2, $3, $4) \
         RETURNING {COLUMNS}"
    ))
    .bind(id)
    .bind(requested_by)
    .bind(scope)
    .bind(channel_id)
    .fetch_one(pool)
    .await;
    match result {
        Ok(row) => Ok(Some(row)),
        Err(sqlx::Error::Database(e)) if e.constraint() == Some("exports_one_active") => Ok(None),
        Err(e) => Err(e),
    }
}

pub async fn find(pool: &PgPool, id: Uuid) -> Result<Option<ExportRow>, sqlx::Error> {
    sqlx::query_as::<_, ExportRow>(&format!("SELECT {COLUMNS} FROM exports WHERE id = $1"))
        .bind(id)
        .fetch_optional(pool)
        .await
}

/// Newest first.
pub async fn list(pool: &PgPool, limit: i64) -> Result<Vec<ExportRow>, sqlx::Error> {
    sqlx::query_as::<_, ExportRow>(&format!(
        "SELECT {COLUMNS} FROM exports ORDER BY created_at DESC, id DESC LIMIT $1"
    ))
    .bind(limit)
    .fetch_all(pool)
    .await
}

/// Marks the export running, or records progress on one that is.
pub async fn touch_running(pool: &PgPool, id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE exports SET status = 'running', updated_at = now() \
         WHERE id = $1 AND status IN ('pending', 'running')",
    )
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn complete(
    pool: &PgPool,
    id: Uuid,
    storage_key: &str,
    size_bytes: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE exports SET status = 'completed', storage_key = $2, size_bytes = $3, \
         updated_at = now(), completed_at = now() WHERE id = $1",
    )
    .bind(id)
    .bind(storage_key)
    .bind(size_bytes)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn fail(pool: &PgPool, id: Uuid, error: &str) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE exports SET status = 'failed', error = $2, updated_at = now(), \
         completed_at = now() WHERE id = $1",
    )
    .bind(id)
    .bind(error)
    .execute(pool)
    .await?;
    Ok(())
}

/// Marks exports that stopped making progress as failed, so a new one can
/// start. Returns how many.
pub async fn fail_stalled(pool: &PgPool, older_than: DateTime<Utc>) -> Result<u64, sqlx::Error> {
    let done = sqlx::query(
        "UPDATE exports SET status = 'failed', error = 'interrupted', completed_at = now() \
         WHERE status IN ('pending', 'running') AND updated_at < $1",
    )
    .bind(older_than)
    .execute(pool)
    .await?;
    Ok(done.rows_affected())
}

pub async fn delete(pool: &PgPool, id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM exports WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

// ── Content ──────────────────────────────────────────────────────────────────

/// The channels an export covers: one, or every channel including private
/// ones and direct messages.
pub async fn channels_in_scope(
    pool: &PgPool,
    channel_id: Option<Uuid>,
) -> Result<Vec<super::channels::ChannelRow>, sqlx::Error> {
    sqlx::query_as::<_, super::channels::ChannelRow>(
        "SELECT id, kind, name, slug, topic, description, created_by, is_archived, is_readonly, \
         created_at, updated_at FROM channels \
         WHERE $1::uuid IS NULL OR id = $1 ORDER BY id",
    )
    .bind(channel_id)
    .fetch_all(pool)
    .await
}

/// A page of a channel's messages, oldest first, replies included, deleted
/// messages left out.
pub async fn messages_page(
    pool: &PgPool,
    channel_id: Uuid,
    after: Option<Uuid>,
    limit: i64,
) -> Result<Vec<super::messages::MessageRow>, sqlx::Error> {
    sqlx::query_as::<_, super::messages::MessageRow>(
        "SELECT id, channel_id, user_id, thread_id, content, edited_at, deleted_at, created_at \
         FROM messages \
         WHERE channel_id = $1 AND deleted_at IS NULL AND ($2::uuid IS NULL OR id > $2) \
         ORDER BY id LIMIT $3",
    )
    .bind(channel_id)
    .bind(after)
    .bind(limit)
    .fetch_all(pool)
    .await
}

/// The users an export names: all of them, or the members and authors of one
/// channel.
pub async fn users_in_scope(
    pool: &PgPool,
    channel_id: Option<Uuid>,
) -> Result<Vec<ExportUserRow>, sqlx::Error> {
    sqlx::query_as::<_, ExportUserRow>(
        "SELECT id, username, display_name, email, role, is_bot, deactivated_at, created_at \
         FROM users u \
         WHERE $1::uuid IS NULL \
            OR EXISTS (SELECT 1 FROM channel_members cm WHERE cm.user_id = u.id AND cm.channel_id = $1) \
            OR EXISTS (SELECT 1 FROM messages m WHERE m.user_id = u.id AND m.channel_id = $1) \
         ORDER BY id",
    )
    .bind(channel_id)
    .fetch_all(pool)
    .await
}

#[derive(Debug, FromRow)]
pub struct ExportUserRow {
    pub id: Uuid,
    pub username: String,
    pub display_name: String,
    pub email: Option<String>,
    pub role: String,
    pub is_bot: bool,
    pub deactivated_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}
