use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

#[derive(Debug, FromRow)]
pub struct WebhookRow {
    pub id: Uuid,
    pub channel_id: Uuid,
    pub kind: String,
    pub name: String,
    pub url: Option<String>,
    pub secret: Option<String>,
    pub token: String,
    pub created_by: Uuid,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
}

pub struct CreateWebhook<'a> {
    pub id: Uuid,
    pub channel_id: Uuid,
    pub kind: &'a str,
    pub name: &'a str,
    pub url: Option<&'a str>,
    pub secret: Option<&'a str>,
    pub token: &'a str,
    pub created_by: Uuid,
}

pub async fn create(pool: &PgPool, wh: &CreateWebhook<'_>) -> Result<WebhookRow, sqlx::Error> {
    sqlx::query_as::<_, WebhookRow>(
        "INSERT INTO webhooks (id, channel_id, kind, name, url, secret, token, created_by) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8) \
         RETURNING id, channel_id, kind, name, url, secret, token, created_by, is_active, created_at",
    )
    .bind(wh.id)
    .bind(wh.channel_id)
    .bind(wh.kind)
    .bind(wh.name)
    .bind(wh.url)
    .bind(wh.secret)
    .bind(wh.token)
    .bind(wh.created_by)
    .fetch_one(pool)
    .await
}

pub async fn find_by_id(pool: &PgPool, id: Uuid) -> Result<Option<WebhookRow>, sqlx::Error> {
    sqlx::query_as::<_, WebhookRow>(
        "SELECT id, channel_id, kind, name, url, secret, token, created_by, is_active, created_at \
         FROM webhooks WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

pub async fn find_by_token(pool: &PgPool, token: &str) -> Result<Option<WebhookRow>, sqlx::Error> {
    sqlx::query_as::<_, WebhookRow>(
        "SELECT id, channel_id, kind, name, url, secret, token, created_by, is_active, created_at \
         FROM webhooks WHERE token = $1",
    )
    .bind(token)
    .fetch_optional(pool)
    .await
}

pub async fn list_by_channel(
    pool: &PgPool,
    channel_id: Uuid,
    cursor: Option<Uuid>,
    limit: i64,
) -> Result<Vec<WebhookRow>, sqlx::Error> {
    if let Some(cursor_id) = cursor {
        sqlx::query_as::<_, WebhookRow>(
            "SELECT id, channel_id, kind, name, url, secret, token, created_by, is_active, created_at \
             FROM webhooks WHERE channel_id = $1 AND id < $2 \
             ORDER BY id DESC LIMIT $3",
        )
        .bind(channel_id)
        .bind(cursor_id)
        .bind(limit)
        .fetch_all(pool)
        .await
    } else {
        sqlx::query_as::<_, WebhookRow>(
            "SELECT id, channel_id, kind, name, url, secret, token, created_by, is_active, created_at \
             FROM webhooks WHERE channel_id = $1 \
             ORDER BY id DESC LIMIT $2",
        )
        .bind(channel_id)
        .bind(limit)
        .fetch_all(pool)
        .await
    }
}

pub async fn list_all(
    pool: &PgPool,
    cursor: Option<Uuid>,
    limit: i64,
) -> Result<Vec<WebhookRow>, sqlx::Error> {
    if let Some(cursor_id) = cursor {
        sqlx::query_as::<_, WebhookRow>(
            "SELECT id, channel_id, kind, name, url, secret, token, created_by, is_active, created_at \
             FROM webhooks WHERE id < $1 \
             ORDER BY id DESC LIMIT $2",
        )
        .bind(cursor_id)
        .bind(limit)
        .fetch_all(pool)
        .await
    } else {
        sqlx::query_as::<_, WebhookRow>(
            "SELECT id, channel_id, kind, name, url, secret, token, created_by, is_active, created_at \
             FROM webhooks \
             ORDER BY id DESC LIMIT $1",
        )
        .bind(limit)
        .fetch_all(pool)
        .await
    }
}

pub async fn list_active_outgoing_by_channel(
    pool: &PgPool,
    channel_id: Uuid,
) -> Result<Vec<WebhookRow>, sqlx::Error> {
    sqlx::query_as::<_, WebhookRow>(
        "SELECT id, channel_id, kind, name, url, secret, token, created_by, is_active, created_at \
         FROM webhooks WHERE channel_id = $1 AND kind = 'outgoing' AND is_active = TRUE",
    )
    .bind(channel_id)
    .fetch_all(pool)
    .await
}

pub struct UpdateWebhook<'a> {
    pub name: Option<&'a str>,
    pub url: Option<&'a str>,
    pub is_active: Option<bool>,
}

pub async fn update(
    pool: &PgPool,
    id: Uuid,
    upd: &UpdateWebhook<'_>,
) -> Result<Option<WebhookRow>, sqlx::Error> {
    sqlx::query_as::<_, WebhookRow>(
        "UPDATE webhooks SET \
         name = COALESCE($2, name), \
         url = COALESCE($3, url), \
         is_active = COALESCE($4, is_active) \
         WHERE id = $1 \
         RETURNING id, channel_id, kind, name, url, secret, token, created_by, is_active, created_at",
    )
    .bind(id)
    .bind(upd.name)
    .bind(upd.url)
    .bind(upd.is_active)
    .fetch_optional(pool)
    .await
}

pub async fn delete(pool: &PgPool, id: Uuid) -> Result<bool, sqlx::Error> {
    let result = sqlx::query("DELETE FROM webhooks WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}

pub async fn regenerate_token(
    pool: &PgPool,
    id: Uuid,
    new_token: &str,
) -> Result<Option<WebhookRow>, sqlx::Error> {
    sqlx::query_as::<_, WebhookRow>(
        "UPDATE webhooks SET token = $2 WHERE id = $1 \
         RETURNING id, channel_id, kind, name, url, secret, token, created_by, is_active, created_at",
    )
    .bind(id)
    .bind(new_token)
    .fetch_optional(pool)
    .await
}
