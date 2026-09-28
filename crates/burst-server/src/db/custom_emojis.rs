use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

#[derive(Debug, FromRow)]
pub struct CustomEmojiRow {
    pub id: Uuid,
    pub shortcode: String,
    /// The image's storage key; clients load it from `GET /api/emojis/{id}/image`.
    pub image_url: String,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
}

pub async fn create(
    pool: &PgPool,
    id: Uuid,
    shortcode: &str,
    image_url: &str,
    created_by: Uuid,
) -> Result<CustomEmojiRow, sqlx::Error> {
    sqlx::query_as::<_, CustomEmojiRow>(
        "INSERT INTO custom_emojis (id, shortcode, image_url, created_by) \
         VALUES ($1, $2, $3, $4) \
         RETURNING id, shortcode, image_url, created_by, created_at",
    )
    .bind(id)
    .bind(shortcode)
    .bind(image_url)
    .bind(created_by)
    .fetch_one(pool)
    .await
}

/// Deletes the emoji and returns the storage key of its image, or `None` when
/// no emoji has that id.
pub async fn delete(pool: &PgPool, id: Uuid) -> Result<Option<String>, sqlx::Error> {
    sqlx::query_scalar("DELETE FROM custom_emojis WHERE id = $1 RETURNING image_url")
        .bind(id)
        .fetch_optional(pool)
        .await
}

pub async fn find_by_id(pool: &PgPool, id: Uuid) -> Result<Option<CustomEmojiRow>, sqlx::Error> {
    sqlx::query_as::<_, CustomEmojiRow>(
        "SELECT id, shortcode, image_url, created_by, created_at \
         FROM custom_emojis WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

pub async fn list(pool: &PgPool) -> Result<Vec<CustomEmojiRow>, sqlx::Error> {
    sqlx::query_as::<_, CustomEmojiRow>(
        "SELECT id, shortcode, image_url, created_by, created_at \
         FROM custom_emojis ORDER BY shortcode",
    )
    .fetch_all(pool)
    .await
}

pub async fn find_by_shortcode(
    pool: &PgPool,
    shortcode: &str,
) -> Result<Option<CustomEmojiRow>, sqlx::Error> {
    sqlx::query_as::<_, CustomEmojiRow>(
        "SELECT id, shortcode, image_url, created_by, created_at \
         FROM custom_emojis WHERE shortcode = $1",
    )
    .bind(shortcode)
    .fetch_optional(pool)
    .await
}
