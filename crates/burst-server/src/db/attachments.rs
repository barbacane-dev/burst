use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

#[derive(Debug, FromRow)]
pub struct AttachmentRow {
    pub id: Uuid,
    pub message_id: Uuid,
    pub file_name: String,
    pub file_size: i64,
    pub content_type: String,
    pub storage_key: String,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

pub struct CreateAttachment {
    pub id: Uuid,
    pub message_id: Uuid,
    pub file_name: String,
    pub file_size: i64,
    pub content_type: String,
    pub storage_key: String,
    pub metadata: serde_json::Value,
}

pub async fn create(pool: &PgPool, att: &CreateAttachment) -> Result<AttachmentRow, sqlx::Error> {
    sqlx::query_as::<_, AttachmentRow>(
        "INSERT INTO attachments (id, message_id, file_name, file_size, content_type, storage_key, metadata) \
         VALUES ($1, $2, $3, $4, $5, $6, $7) \
         RETURNING id, message_id, file_name, file_size, content_type, storage_key, metadata, created_at",
    )
    .bind(att.id)
    .bind(att.message_id)
    .bind(&att.file_name)
    .bind(att.file_size)
    .bind(&att.content_type)
    .bind(&att.storage_key)
    .bind(&att.metadata)
    .fetch_one(pool)
    .await
}

pub async fn find_by_id(pool: &PgPool, id: Uuid) -> Result<Option<AttachmentRow>, sqlx::Error> {
    sqlx::query_as::<_, AttachmentRow>(
        "SELECT id, message_id, file_name, file_size, content_type, storage_key, metadata, created_at \
         FROM attachments WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

/// Batch-fetch attachments for a set of message IDs.
pub async fn list_for_messages(
    pool: &PgPool,
    message_ids: &[Uuid],
) -> Result<Vec<AttachmentRow>, sqlx::Error> {
    if message_ids.is_empty() {
        return Ok(vec![]);
    }
    sqlx::query_as::<_, AttachmentRow>(
        "SELECT id, message_id, file_name, file_size, content_type, storage_key, metadata, created_at \
         FROM attachments \
         WHERE message_id = ANY($1) \
         ORDER BY created_at ASC",
    )
    .bind(message_ids)
    .fetch_all(pool)
    .await
}
