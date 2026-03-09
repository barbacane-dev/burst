use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

#[derive(Debug, FromRow)]
pub struct ChannelRow {
    pub id: Uuid,
    pub kind: String,
    pub name: Option<String>,
    pub slug: Option<String>,
    pub topic: Option<String>,
    pub description: Option<String>,
    pub created_by: Uuid,
    pub is_archived: bool,
    pub is_readonly: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Like `ChannelRow` but also carries the unread message count for the requesting user.
#[derive(Debug, FromRow)]
pub struct ChannelWithUnreadRow {
    pub id: Uuid,
    pub kind: String,
    pub name: Option<String>,
    pub slug: Option<String>,
    pub topic: Option<String>,
    pub description: Option<String>,
    pub created_by: Uuid,
    pub is_archived: bool,
    pub is_readonly: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub unread_count: i64,
}

#[derive(Debug, FromRow)]
pub struct ChannelMemberRow {
    pub channel_id: Uuid,
    pub user_id: Uuid,
    pub role: String,
    pub notify: String,
    pub joined_at: DateTime<Utc>,
}

pub struct CreateChannel {
    pub id: Uuid,
    pub kind: String,
    pub name: Option<String>,
    pub slug: Option<String>,
    pub topic: Option<String>,
    pub description: Option<String>,
    pub created_by: Uuid,
}

pub async fn create(pool: &PgPool, ch: &CreateChannel) -> Result<ChannelRow, sqlx::Error> {
    sqlx::query_as::<_, ChannelRow>(
        "INSERT INTO channels (id, kind, name, slug, topic, description, created_by) \
         VALUES ($1, $2, $3, $4, $5, $6, $7) \
         RETURNING id, kind, name, slug, topic, description, created_by, \
         is_archived, is_readonly, created_at, updated_at",
    )
    .bind(ch.id)
    .bind(&ch.kind)
    .bind(&ch.name)
    .bind(&ch.slug)
    .bind(&ch.topic)
    .bind(&ch.description)
    .bind(ch.created_by)
    .fetch_one(pool)
    .await
}

pub async fn find_by_id(pool: &PgPool, id: Uuid) -> Result<Option<ChannelRow>, sqlx::Error> {
    sqlx::query_as::<_, ChannelRow>(
        "SELECT id, kind, name, slug, topic, description, created_by, \
         is_archived, is_readonly, created_at, updated_at \
         FROM channels WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

pub async fn find_by_slug(pool: &PgPool, slug: &str) -> Result<Option<ChannelRow>, sqlx::Error> {
    sqlx::query_as::<_, ChannelRow>(
        "SELECT id, kind, name, slug, topic, description, created_by, \
         is_archived, is_readonly, created_at, updated_at \
         FROM channels WHERE slug = $1",
    )
    .bind(slug)
    .fetch_optional(pool)
    .await
}

pub async fn list_for_user(
    pool: &PgPool,
    user_id: Uuid,
    cursor: Option<Uuid>,
    limit: i64,
) -> Result<Vec<ChannelWithUnreadRow>, sqlx::Error> {
    match cursor {
        Some(cursor_id) => {
            sqlx::query_as::<_, ChannelWithUnreadRow>(
                "SELECT c.id, c.kind, c.name, c.slug, c.topic, c.description, c.created_by, \
                 c.is_archived, c.is_readonly, c.created_at, c.updated_at, \
                 COALESCE((\
                   SELECT COUNT(*) FROM messages m \
                   WHERE m.channel_id = c.id \
                   AND m.thread_id IS NULL \
                   AND m.deleted_at IS NULL \
                   AND m.user_id != $1 \
                   AND m.created_at > COALESCE(cm.last_read_at, '1970-01-01'::timestamptz) \
                 ), 0) AS unread_count \
                 FROM channels c \
                 INNER JOIN channel_members cm ON cm.channel_id = c.id \
                 WHERE cm.user_id = $1 AND c.id > $2 \
                 ORDER BY c.id LIMIT $3",
            )
            .bind(user_id)
            .bind(cursor_id)
            .bind(limit)
            .fetch_all(pool)
            .await
        }
        None => {
            sqlx::query_as::<_, ChannelWithUnreadRow>(
                "SELECT c.id, c.kind, c.name, c.slug, c.topic, c.description, c.created_by, \
                 c.is_archived, c.is_readonly, c.created_at, c.updated_at, \
                 COALESCE((\
                   SELECT COUNT(*) FROM messages m \
                   WHERE m.channel_id = c.id \
                   AND m.thread_id IS NULL \
                   AND m.deleted_at IS NULL \
                   AND m.user_id != $1 \
                   AND m.created_at > COALESCE(cm.last_read_at, '1970-01-01'::timestamptz) \
                 ), 0) AS unread_count \
                 FROM channels c \
                 INNER JOIN channel_members cm ON cm.channel_id = c.id \
                 WHERE cm.user_id = $1 \
                 ORDER BY c.id LIMIT $2",
            )
            .bind(user_id)
            .bind(limit)
            .fetch_all(pool)
            .await
        }
    }
}

pub async fn list_public(
    pool: &PgPool,
    cursor: Option<Uuid>,
    limit: i64,
) -> Result<Vec<ChannelRow>, sqlx::Error> {
    match cursor {
        Some(cursor_id) => {
            sqlx::query_as::<_, ChannelRow>(
                "SELECT id, kind, name, slug, topic, description, created_by, \
                 is_archived, is_readonly, created_at, updated_at \
                 FROM channels WHERE kind = 'public' AND id > $1 \
                 ORDER BY id LIMIT $2",
            )
            .bind(cursor_id)
            .bind(limit)
            .fetch_all(pool)
            .await
        }
        None => {
            sqlx::query_as::<_, ChannelRow>(
                "SELECT id, kind, name, slug, topic, description, created_by, \
                 is_archived, is_readonly, created_at, updated_at \
                 FROM channels WHERE kind = 'public' \
                 ORDER BY id LIMIT $1",
            )
            .bind(limit)
            .fetch_all(pool)
            .await
        }
    }
}

pub struct UpdateChannel {
    pub name: Option<String>,
    pub topic: Option<String>,
    pub description: Option<String>,
}

pub async fn update(
    pool: &PgPool,
    id: Uuid,
    upd: &UpdateChannel,
) -> Result<Option<ChannelRow>, sqlx::Error> {
    sqlx::query_as::<_, ChannelRow>(
        "UPDATE channels SET \
         name = COALESCE($2, name), \
         topic = COALESCE($3, topic), \
         description = COALESCE($4, description), \
         updated_at = now() \
         WHERE id = $1 \
         RETURNING id, kind, name, slug, topic, description, created_by, \
         is_archived, is_readonly, created_at, updated_at",
    )
    .bind(id)
    .bind(&upd.name)
    .bind(&upd.topic)
    .bind(&upd.description)
    .fetch_optional(pool)
    .await
}

pub async fn add_member(
    pool: &PgPool,
    channel_id: Uuid,
    user_id: Uuid,
    role: &str,
) -> Result<ChannelMemberRow, sqlx::Error> {
    // ON CONFLICT DO NOTHING returns no rows if the member already exists,
    // so use fetch_optional and fall back to a SELECT for the existing row.
    let row = sqlx::query_as::<_, ChannelMemberRow>(
        "INSERT INTO channel_members (channel_id, user_id, role) \
         VALUES ($1, $2, $3) \
         ON CONFLICT (channel_id, user_id) DO NOTHING \
         RETURNING channel_id, user_id, role, notify, joined_at",
    )
    .bind(channel_id)
    .bind(user_id)
    .bind(role)
    .fetch_optional(pool)
    .await?;

    if let Some(row) = row {
        return Ok(row);
    }

    sqlx::query_as::<_, ChannelMemberRow>(
        "SELECT channel_id, user_id, role, notify, joined_at \
         FROM channel_members WHERE channel_id = $1 AND user_id = $2",
    )
    .bind(channel_id)
    .bind(user_id)
    .fetch_one(pool)
    .await
}

pub async fn remove_member(
    pool: &PgPool,
    channel_id: Uuid,
    user_id: Uuid,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query("DELETE FROM channel_members WHERE channel_id = $1 AND user_id = $2")
        .bind(channel_id)
        .bind(user_id)
        .execute(pool)
        .await?;

    Ok(result.rows_affected() > 0)
}

pub async fn is_member(
    pool: &PgPool,
    channel_id: Uuid,
    user_id: Uuid,
) -> Result<bool, sqlx::Error> {
    let row = sqlx::query_as::<_, (i64,)>(
        "SELECT COUNT(*) FROM channel_members WHERE channel_id = $1 AND user_id = $2",
    )
    .bind(channel_id)
    .bind(user_id)
    .fetch_one(pool)
    .await?;

    Ok(row.0 > 0)
}

pub async fn list_members(
    pool: &PgPool,
    channel_id: Uuid,
) -> Result<Vec<ChannelMemberRow>, sqlx::Error> {
    sqlx::query_as::<_, ChannelMemberRow>(
        "SELECT channel_id, user_id, role, notify, joined_at \
         FROM channel_members WHERE channel_id = $1 \
         ORDER BY joined_at",
    )
    .bind(channel_id)
    .fetch_all(pool)
    .await
}

pub async fn channel_ids_for_user(pool: &PgPool, user_id: Uuid) -> Result<Vec<Uuid>, sqlx::Error> {
    let rows =
        sqlx::query_as::<_, (Uuid,)>("SELECT channel_id FROM channel_members WHERE user_id = $1")
            .bind(user_id)
            .fetch_all(pool)
            .await?;
    Ok(rows.into_iter().map(|r| r.0).collect())
}

/// Finds an existing DM channel between two users, or creates one.
pub async fn find_or_create_dm(
    pool: &PgPool,
    user_a: Uuid,
    user_b: Uuid,
    new_id: Uuid,
) -> Result<ChannelRow, sqlx::Error> {
    // Look for an existing dm channel where both users are members
    let existing = sqlx::query_as::<_, ChannelRow>(
        "SELECT c.id, c.kind, c.name, c.slug, c.topic, c.description, c.created_by, \
         c.is_archived, c.is_readonly, c.created_at, c.updated_at \
         FROM channels c \
         INNER JOIN channel_members cm1 ON cm1.channel_id = c.id AND cm1.user_id = $1 \
         INNER JOIN channel_members cm2 ON cm2.channel_id = c.id AND cm2.user_id = $2 \
         WHERE c.kind = 'dm' \
         LIMIT 1",
    )
    .bind(user_a)
    .bind(user_b)
    .fetch_optional(pool)
    .await?;

    if let Some(ch) = existing {
        return Ok(ch);
    }

    let ch = sqlx::query_as::<_, ChannelRow>(
        "INSERT INTO channels (id, kind, created_by) \
         VALUES ($1, 'dm', $2) \
         RETURNING id, kind, name, slug, topic, description, created_by, \
         is_archived, is_readonly, created_at, updated_at",
    )
    .bind(new_id)
    .bind(user_a)
    .fetch_one(pool)
    .await?;

    for user_id in [user_a, user_b] {
        sqlx::query(
            "INSERT INTO channel_members (channel_id, user_id, role) \
             VALUES ($1, $2, 'member') ON CONFLICT DO NOTHING",
        )
        .bind(ch.id)
        .bind(user_id)
        .execute(pool)
        .await?;
    }

    Ok(ch)
}

/// Updates the last-read timestamp for a user in a channel.
pub async fn update_last_read(
    pool: &PgPool,
    channel_id: Uuid,
    user_id: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE channel_members SET last_read_at = NOW() \
         WHERE channel_id = $1 AND user_id = $2",
    )
    .bind(channel_id)
    .bind(user_id)
    .execute(pool)
    .await?;
    Ok(())
}
