use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

#[derive(Debug, FromRow)]
pub struct UserRow {
    pub id: Uuid,
    pub external_id: Option<String>,
    pub username: String,
    pub display_name: String,
    pub email: Option<String>,
    pub avatar_url: Option<String>,
    pub role: String,
    pub status: String,
    pub status_text: Option<String>,
    pub password_hash: Option<String>,
    pub is_bot: bool,
    pub deactivated_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub async fn find_by_id(pool: &PgPool, id: Uuid) -> Result<Option<UserRow>, sqlx::Error> {
    sqlx::query_as::<_, UserRow>(
        "SELECT id, external_id, username, display_name, email, avatar_url, \
         role, status, status_text, password_hash, is_bot, deactivated_at, \
         created_at, updated_at \
         FROM users WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

pub async fn find_by_email(pool: &PgPool, email: &str) -> Result<Option<UserRow>, sqlx::Error> {
    sqlx::query_as::<_, UserRow>(
        "SELECT id, external_id, username, display_name, email, avatar_url, \
         role, status, status_text, password_hash, is_bot, deactivated_at, \
         created_at, updated_at \
         FROM users WHERE email = $1",
    )
    .bind(email)
    .fetch_optional(pool)
    .await
}

pub async fn find_by_username(
    pool: &PgPool,
    username: &str,
) -> Result<Option<UserRow>, sqlx::Error> {
    sqlx::query_as::<_, UserRow>(
        "SELECT id, external_id, username, display_name, email, avatar_url, \
         role, status, status_text, password_hash, is_bot, deactivated_at, \
         created_at, updated_at \
         FROM users WHERE username = $1",
    )
    .bind(username)
    .fetch_optional(pool)
    .await
}

pub async fn find_by_external_id(
    pool: &PgPool,
    external_id: &str,
) -> Result<Option<UserRow>, sqlx::Error> {
    sqlx::query_as::<_, UserRow>(
        "SELECT id, external_id, username, display_name, email, avatar_url, \
         role, status, status_text, password_hash, is_bot, deactivated_at, \
         created_at, updated_at \
         FROM users WHERE external_id = $1",
    )
    .bind(external_id)
    .fetch_optional(pool)
    .await
}

pub struct CreateUser {
    pub id: Uuid,
    pub username: String,
    pub display_name: String,
    pub email: Option<String>,
    pub password_hash: Option<String>,
    pub external_id: Option<String>,
    pub role: String,
}

pub async fn create(pool: &PgPool, user: &CreateUser) -> Result<UserRow, sqlx::Error> {
    sqlx::query_as::<_, UserRow>(
        "INSERT INTO users (id, username, display_name, email, password_hash, external_id, role) \
         VALUES ($1, $2, $3, $4, $5, $6, $7) \
         RETURNING id, external_id, username, display_name, email, avatar_url, \
         role, status, status_text, password_hash, is_bot, deactivated_at, \
         created_at, updated_at",
    )
    .bind(user.id)
    .bind(&user.username)
    .bind(&user.display_name)
    .bind(&user.email)
    .bind(&user.password_hash)
    .bind(&user.external_id)
    .bind(&user.role)
    .fetch_one(pool)
    .await
}

pub async fn list(
    pool: &PgPool,
    cursor: Option<Uuid>,
    limit: i64,
) -> Result<Vec<UserRow>, sqlx::Error> {
    match cursor {
        Some(cursor_id) => {
            sqlx::query_as::<_, UserRow>(
                "SELECT id, external_id, username, display_name, email, avatar_url, \
                 role, status, status_text, password_hash, is_bot, deactivated_at, \
                 created_at, updated_at \
                 FROM users WHERE id > $1 ORDER BY id LIMIT $2",
            )
            .bind(cursor_id)
            .bind(limit)
            .fetch_all(pool)
            .await
        }
        None => {
            sqlx::query_as::<_, UserRow>(
                "SELECT id, external_id, username, display_name, email, avatar_url, \
                 role, status, status_text, password_hash, is_bot, deactivated_at, \
                 created_at, updated_at \
                 FROM users ORDER BY id LIMIT $1",
            )
            .bind(limit)
            .fetch_all(pool)
            .await
        }
    }
}

pub async fn update_role(
    pool: &PgPool,
    id: Uuid,
    role: &str,
) -> Result<Option<UserRow>, sqlx::Error> {
    sqlx::query_as::<_, UserRow>(
        "UPDATE users SET role = $2, updated_at = now() \
         WHERE id = $1 \
         RETURNING id, external_id, username, display_name, email, avatar_url, \
         role, status, status_text, password_hash, is_bot, deactivated_at, \
         created_at, updated_at",
    )
    .bind(id)
    .bind(role)
    .fetch_optional(pool)
    .await
}

pub async fn deactivate(pool: &PgPool, id: Uuid) -> Result<Option<UserRow>, sqlx::Error> {
    sqlx::query_as::<_, UserRow>(
        "UPDATE users SET deactivated_at = now(), updated_at = now() \
         WHERE id = $1 AND deactivated_at IS NULL \
         RETURNING id, external_id, username, display_name, email, avatar_url, \
         role, status, status_text, password_hash, is_bot, deactivated_at, \
         created_at, updated_at",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

pub async fn reactivate(pool: &PgPool, id: Uuid) -> Result<Option<UserRow>, sqlx::Error> {
    sqlx::query_as::<_, UserRow>(
        "UPDATE users SET deactivated_at = NULL, updated_at = now() \
         WHERE id = $1 AND deactivated_at IS NOT NULL \
         RETURNING id, external_id, username, display_name, email, avatar_url, \
         role, status, status_text, password_hash, is_bot, deactivated_at, \
         created_at, updated_at",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

pub async fn list_all(
    pool: &PgPool,
    cursor: Option<Uuid>,
    limit: i64,
) -> Result<Vec<UserRow>, sqlx::Error> {
    match cursor {
        Some(cursor_id) => {
            sqlx::query_as::<_, UserRow>(
                "SELECT id, external_id, username, display_name, email, avatar_url, \
                 role, status, status_text, password_hash, is_bot, deactivated_at, \
                 created_at, updated_at \
                 FROM users WHERE id > $1 ORDER BY id LIMIT $2",
            )
            .bind(cursor_id)
            .bind(limit)
            .fetch_all(pool)
            .await
        }
        None => {
            sqlx::query_as::<_, UserRow>(
                "SELECT id, external_id, username, display_name, email, avatar_url, \
                 role, status, status_text, password_hash, is_bot, deactivated_at, \
                 created_at, updated_at \
                 FROM users ORDER BY id LIMIT $1",
            )
            .bind(limit)
            .fetch_all(pool)
            .await
        }
    }
}

pub async fn list_all_channels(
    pool: &PgPool,
    cursor: Option<Uuid>,
    limit: i64,
) -> Result<Vec<super::channels::ChannelRow>, sqlx::Error> {
    match cursor {
        Some(cursor_id) => {
            sqlx::query_as::<_, super::channels::ChannelRow>(
                "SELECT id, kind, name, slug, topic, description, created_by, \
                 is_archived, is_readonly, created_at, updated_at \
                 FROM channels WHERE id > $1 ORDER BY id LIMIT $2",
            )
            .bind(cursor_id)
            .bind(limit)
            .fetch_all(pool)
            .await
        }
        None => {
            sqlx::query_as::<_, super::channels::ChannelRow>(
                "SELECT id, kind, name, slug, topic, description, created_by, \
                 is_archived, is_readonly, created_at, updated_at \
                 FROM channels ORDER BY id LIMIT $1",
            )
            .bind(limit)
            .fetch_all(pool)
            .await
        }
    }
}

pub struct UpdateUser {
    pub display_name: Option<String>,
    pub email: Option<String>,
    pub avatar_url: Option<String>,
    pub status_text: Option<String>,
}

pub async fn update(
    pool: &PgPool,
    id: Uuid,
    update: &UpdateUser,
) -> Result<Option<UserRow>, sqlx::Error> {
    sqlx::query_as::<_, UserRow>(
        "UPDATE users SET \
         display_name = COALESCE($2, display_name), \
         email = COALESCE($3, email), \
         avatar_url = COALESCE($4, avatar_url), \
         status_text = COALESCE($5, status_text), \
         updated_at = now() \
         WHERE id = $1 \
         RETURNING id, external_id, username, display_name, email, avatar_url, \
         role, status, status_text, password_hash, is_bot, deactivated_at, \
         created_at, updated_at",
    )
    .bind(id)
    .bind(&update.display_name)
    .bind(&update.email)
    .bind(&update.avatar_url)
    .bind(&update.status_text)
    .fetch_optional(pool)
    .await
}
