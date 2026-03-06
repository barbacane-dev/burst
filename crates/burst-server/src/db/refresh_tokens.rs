use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

#[derive(Debug, FromRow)]
pub struct RefreshTokenRow {
    pub id: Uuid,
    pub user_id: Uuid,
    pub token_hash: String,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

pub async fn create(
    pool: &PgPool,
    user_id: Uuid,
    token_hash: &str,
    expiry_seconds: i64,
) -> Result<RefreshTokenRow, sqlx::Error> {
    let id = burst_core::id::new_id();
    sqlx::query_as::<_, RefreshTokenRow>(
        "INSERT INTO refresh_tokens (id, user_id, token_hash, expires_at) \
         VALUES ($1, $2, $3, now() + make_interval(secs => $4)) \
         RETURNING id, user_id, token_hash, expires_at, created_at",
    )
    .bind(id)
    .bind(user_id)
    .bind(token_hash)
    .bind(expiry_seconds as f64)
    .fetch_one(pool)
    .await
}

pub async fn find_valid(
    pool: &PgPool,
    token_hash: &str,
) -> Result<Option<RefreshTokenRow>, sqlx::Error> {
    sqlx::query_as::<_, RefreshTokenRow>(
        "SELECT id, user_id, token_hash, expires_at, created_at \
         FROM refresh_tokens \
         WHERE token_hash = $1 AND expires_at > now()",
    )
    .bind(token_hash)
    .fetch_optional(pool)
    .await
}

pub async fn delete(pool: &PgPool, id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM refresh_tokens WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn delete_all_for_user(pool: &PgPool, user_id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM refresh_tokens WHERE user_id = $1")
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(())
}
