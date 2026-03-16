use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

#[derive(Debug, FromRow)]
pub struct AuditLogRow {
    pub id: Uuid,
    pub user_id: Option<Uuid>,
    pub action: String,
    pub target_type: String,
    pub target_id: Uuid,
    pub metadata: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
}

/// Inserts a new audit log entry. The audit log is append-only.
pub async fn insert(
    pool: &PgPool,
    id: Uuid,
    user_id: Uuid,
    action: &str,
    target_type: &str,
    target_id: Uuid,
    metadata: Option<serde_json::Value>,
) -> Result<AuditLogRow, sqlx::Error> {
    sqlx::query_as::<_, AuditLogRow>(
        "INSERT INTO audit_log (id, user_id, action, target_type, target_id, metadata) \
         VALUES ($1, $2, $3, $4, $5, $6) \
         RETURNING id, user_id, action, target_type, target_id, metadata, created_at",
    )
    .bind(id)
    .bind(user_id)
    .bind(action)
    .bind(target_type)
    .bind(target_id)
    .bind(metadata)
    .fetch_one(pool)
    .await
}

/// Lists audit log entries with cursor-based pagination, optionally filtered by target type.
pub async fn list(
    pool: &PgPool,
    target_type: Option<&str>,
    cursor: Option<Uuid>,
    limit: i64,
) -> Result<Vec<AuditLogRow>, sqlx::Error> {
    match (target_type, cursor) {
        (Some(tt), Some(cursor_id)) => {
            sqlx::query_as::<_, AuditLogRow>(
                "SELECT id, user_id, action, target_type, target_id, metadata, created_at \
                 FROM audit_log \
                 WHERE target_type = $1 AND id < $2 \
                 ORDER BY id DESC LIMIT $3",
            )
            .bind(tt)
            .bind(cursor_id)
            .bind(limit)
            .fetch_all(pool)
            .await
        }
        (Some(tt), None) => {
            sqlx::query_as::<_, AuditLogRow>(
                "SELECT id, user_id, action, target_type, target_id, metadata, created_at \
                 FROM audit_log \
                 WHERE target_type = $1 \
                 ORDER BY id DESC LIMIT $2",
            )
            .bind(tt)
            .bind(limit)
            .fetch_all(pool)
            .await
        }
        (None, Some(cursor_id)) => {
            sqlx::query_as::<_, AuditLogRow>(
                "SELECT id, user_id, action, target_type, target_id, metadata, created_at \
                 FROM audit_log \
                 WHERE id < $1 \
                 ORDER BY id DESC LIMIT $2",
            )
            .bind(cursor_id)
            .bind(limit)
            .fetch_all(pool)
            .await
        }
        (None, None) => {
            sqlx::query_as::<_, AuditLogRow>(
                "SELECT id, user_id, action, target_type, target_id, metadata, created_at \
                 FROM audit_log \
                 ORDER BY id DESC LIMIT $1",
            )
            .bind(limit)
            .fetch_all(pool)
            .await
        }
    }
}
