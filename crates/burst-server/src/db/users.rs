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
    pub status_emoji: Option<String>,
    pub status_expires_at: Option<DateTime<Utc>>,
    pub dnd_until: Option<DateTime<Utc>>,
    pub dnd_start: Option<chrono::NaiveTime>,
    pub dnd_end: Option<chrono::NaiveTime>,
    pub dnd_days: Option<i16>,
    pub dnd_time_zone: Option<String>,
    pub password_hash: Option<String>,
    pub is_bot: bool,
    pub deactivated_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub async fn find_by_id(pool: &PgPool, id: Uuid) -> Result<Option<UserRow>, sqlx::Error> {
    sqlx::query_as::<_, UserRow>(
        "SELECT id, external_id, username, display_name, email, avatar_url, \
         role, status, status_text, status_emoji, status_expires_at, dnd_until, dnd_start, dnd_end, \
         dnd_days, dnd_time_zone, password_hash, is_bot, deactivated_at, \
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
         role, status, status_text, status_emoji, status_expires_at, dnd_until, dnd_start, dnd_end, \
         dnd_days, dnd_time_zone, password_hash, is_bot, deactivated_at, \
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
         role, status, status_text, status_emoji, status_expires_at, dnd_until, dnd_start, dnd_end, \
         dnd_days, dnd_time_zone, password_hash, is_bot, deactivated_at, \
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
         role, status, status_text, status_emoji, status_expires_at, dnd_until, dnd_start, dnd_end, \
         dnd_days, dnd_time_zone, password_hash, is_bot, deactivated_at, \
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
         role, status, status_text, status_emoji, status_expires_at, dnd_until, dnd_start, dnd_end, \
         dnd_days, dnd_time_zone, password_hash, is_bot, deactivated_at, \
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

/// Insert the user for `external_id`, or return the one a concurrent request
/// already inserted.
///
/// Every request carrying an identity Burst has not seen provisions it, so
/// several arrive at once on a first login and race. `ON CONFLICT DO NOTHING`
/// without a target absorbs whichever unique constraint fires, since an
/// identical row collides on `external_id`, `username` and `email` alike, and
/// the follow-up select returns the winner's row.
///
/// `Ok(None)` means the insert was refused and no row carries this
/// `external_id`, so the collision was a different user holding the username or
/// email, which is a real conflict rather than a race.
pub async fn create_or_get_by_external_id(
    pool: &PgPool,
    user: &CreateUser,
    external_id: &str,
) -> Result<Option<UserRow>, sqlx::Error> {
    let inserted = sqlx::query_as::<_, UserRow>(
        "INSERT INTO users (id, username, display_name, email, password_hash, external_id, role) \
         VALUES ($1, $2, $3, $4, $5, $6, $7) \
         ON CONFLICT DO NOTHING \
         RETURNING id, external_id, username, display_name, email, avatar_url, \
         role, status, status_text, status_emoji, status_expires_at, dnd_until, dnd_start, dnd_end, \
         dnd_days, dnd_time_zone, password_hash, is_bot, deactivated_at, \
         created_at, updated_at",
    )
    .bind(user.id)
    .bind(&user.username)
    .bind(&user.display_name)
    .bind(&user.email)
    .bind(&user.password_hash)
    .bind(&user.external_id)
    .bind(&user.role)
    .fetch_optional(pool)
    .await?;

    match inserted {
        Some(row) => Ok(Some(row)),
        None => find_by_external_id(pool, external_id).await,
    }
}

pub async fn create_bot(
    pool: &PgPool,
    id: Uuid,
    username: &str,
    display_name: &str,
) -> Result<UserRow, sqlx::Error> {
    sqlx::query_as::<_, UserRow>(
        "INSERT INTO users (id, username, display_name, role, is_bot) \
         VALUES ($1, $2, $3, 'member', TRUE) \
         RETURNING id, external_id, username, display_name, email, avatar_url, \
         role, status, status_text, status_emoji, status_expires_at, dnd_until, dnd_start, dnd_end, \
         dnd_days, dnd_time_zone, password_hash, is_bot, deactivated_at, \
         created_at, updated_at",
    )
    .bind(id)
    .bind(username)
    .bind(display_name)
    .fetch_one(pool)
    .await
}

pub async fn list_bots(pool: &PgPool) -> Result<Vec<UserRow>, sqlx::Error> {
    sqlx::query_as::<_, UserRow>(
        "SELECT id, external_id, username, display_name, email, avatar_url, \
         role, status, status_text, status_emoji, status_expires_at, dnd_until, dnd_start, dnd_end, \
         dnd_days, dnd_time_zone, password_hash, is_bot, deactivated_at, \
         created_at, updated_at \
         FROM users WHERE is_bot = TRUE ORDER BY created_at DESC",
    )
    .fetch_all(pool)
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
                 role, status, status_text, status_emoji, status_expires_at, dnd_until, dnd_start, dnd_end, \
         dnd_days, dnd_time_zone, password_hash, is_bot, deactivated_at, \
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
                 role, status, status_text, status_emoji, status_expires_at, dnd_until, dnd_start, dnd_end, \
         dnd_days, dnd_time_zone, password_hash, is_bot, deactivated_at, \
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
         role, status, status_text, status_emoji, status_expires_at, dnd_until, dnd_start, dnd_end, \
         dnd_days, dnd_time_zone, password_hash, is_bot, deactivated_at, \
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
         role, status, status_text, status_emoji, status_expires_at, dnd_until, dnd_start, dnd_end, \
         dnd_days, dnd_time_zone, password_hash, is_bot, deactivated_at, \
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
         role, status, status_text, status_emoji, status_expires_at, dnd_until, dnd_start, dnd_end, \
         dnd_days, dnd_time_zone, password_hash, is_bot, deactivated_at, \
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
                 role, status, status_text, status_emoji, status_expires_at, dnd_until, dnd_start, dnd_end, \
         dnd_days, dnd_time_zone, password_hash, is_bot, deactivated_at, \
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
                 role, status, status_text, status_emoji, status_expires_at, dnd_until, dnd_start, dnd_end, \
         dnd_days, dnd_time_zone, password_hash, is_bot, deactivated_at, \
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

pub async fn find_ids_by_usernames(
    pool: &PgPool,
    usernames: &[String],
) -> Result<Vec<Uuid>, sqlx::Error> {
    if usernames.is_empty() {
        return Ok(vec![]);
    }
    let rows = sqlx::query_scalar::<_, Uuid>("SELECT id FROM users WHERE username = ANY($1)")
        .bind(usernames)
        .fetch_all(pool)
        .await?;
    Ok(rows)
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
         role, status, status_text, status_emoji, status_expires_at, dnd_until, dnd_start, dnd_end, \
         dnd_days, dnd_time_zone, password_hash, is_bot, deactivated_at, \
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

/// Sets a user's custom status, or clears it with `None`.
pub async fn set_status(
    pool: &PgPool,
    id: Uuid,
    status: Option<&burst_core::status::CustomStatus>,
) -> Result<Option<UserRow>, sqlx::Error> {
    sqlx::query_as::<_, UserRow>(
        "UPDATE users SET status_text = $2, status_emoji = $3, status_expires_at = $4, \
         updated_at = now() \
         WHERE id = $1 \
         RETURNING id, external_id, username, display_name, email, avatar_url, \
         role, status, status_text, status_emoji, status_expires_at, dnd_until, dnd_start, dnd_end, \
         dnd_days, dnd_time_zone, password_hash, is_bot, \
         deactivated_at, created_at, updated_at",
    )
    .bind(id)
    .bind(status.and_then(|s| s.text.as_deref()))
    .bind(status.and_then(|s| s.emoji.as_deref()))
    .bind(status.and_then(|s| s.expires_at))
    .fetch_optional(pool)
    .await
}

impl UserRow {
    /// The user's do-not-disturb settings. A stored schedule that no longer
    /// parses (an unknown time zone) is treated as none.
    pub fn do_not_disturb(&self) -> burst_core::dnd::DoNotDisturb {
        burst_core::dnd::DoNotDisturb {
            snooze_until: self.dnd_until,
            schedule: schedule_of(
                self.dnd_start,
                self.dnd_end,
                self.dnd_days,
                self.dnd_time_zone.as_deref(),
            ),
        }
    }
}

fn schedule_of(
    start: Option<chrono::NaiveTime>,
    end: Option<chrono::NaiveTime>,
    days: Option<i16>,
    time_zone: Option<&str>,
) -> Option<burst_core::dnd::Schedule> {
    Some(burst_core::dnd::Schedule {
        start: start?,
        end: end?,
        days: u8::try_from(days?).ok()?,
        time_zone: time_zone?.parse().ok()?,
    })
}

/// Replaces a user's do-not-disturb settings.
pub async fn set_do_not_disturb(
    pool: &PgPool,
    id: Uuid,
    dnd: &burst_core::dnd::DoNotDisturb,
) -> Result<Option<UserRow>, sqlx::Error> {
    let schedule = dnd.schedule.as_ref();
    sqlx::query_as::<_, UserRow>(
        "UPDATE users SET dnd_until = $2, dnd_start = $3, dnd_end = $4, dnd_days = $5, \
         dnd_time_zone = $6, updated_at = now() \
         WHERE id = $1 \
         RETURNING id, external_id, username, display_name, email, avatar_url, \
         role, status, status_text, status_emoji, status_expires_at, dnd_until, dnd_start, \
         dnd_end, dnd_days, dnd_time_zone, password_hash, is_bot, deactivated_at, created_at, \
         updated_at",
    )
    .bind(id)
    .bind(dnd.snooze_until)
    .bind(schedule.map(|s| s.start))
    .bind(schedule.map(|s| s.end))
    .bind(schedule.map(|s| i16::from(s.days)))
    .bind(schedule.map(|s| s.time_zone.name()))
    .fetch_optional(pool)
    .await
}

/// The do-not-disturb settings of each of `ids`.
pub async fn do_not_disturb_of(
    pool: &PgPool,
    ids: &[Uuid],
) -> Result<Vec<(Uuid, burst_core::dnd::DoNotDisturb)>, sqlx::Error> {
    let rows = sqlx::query_as::<_, DndRow>(
        "SELECT id, dnd_until, dnd_start, dnd_end, dnd_days, dnd_time_zone \
         FROM users WHERE id = ANY($1)",
    )
    .bind(ids)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| {
            let dnd = burst_core::dnd::DoNotDisturb {
                snooze_until: r.dnd_until,
                schedule: schedule_of(
                    r.dnd_start,
                    r.dnd_end,
                    r.dnd_days,
                    r.dnd_time_zone.as_deref(),
                ),
            };
            (r.id, dnd)
        })
        .collect())
}

#[derive(FromRow)]
struct DndRow {
    id: Uuid,
    dnd_until: Option<DateTime<Utc>>,
    dnd_start: Option<chrono::NaiveTime>,
    dnd_end: Option<chrono::NaiveTime>,
    dnd_days: Option<i16>,
    dnd_time_zone: Option<String>,
}

/// Update avatar URL (used during JIT provisioning from OIDC picture claim).
pub async fn update_avatar(pool: &PgPool, id: Uuid, avatar_url: &str) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE users SET avatar_url = $2, updated_at = now() WHERE id = $1")
        .bind(id)
        .bind(avatar_url)
        .execute(pool)
        .await?;
    Ok(())
}

/// Sync profile fields from OIDC claims. Only updates non-null values.
pub async fn sync_profile(
    pool: &PgPool,
    id: Uuid,
    display_name: Option<&str>,
    email: Option<&str>,
    avatar_url: Option<&str>,
    username: Option<&str>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE users SET \
         display_name = COALESCE($2, display_name), \
         email = COALESCE($3, email), \
         avatar_url = COALESCE($4, avatar_url), \
         username = COALESCE($5, username), \
         updated_at = now() \
         WHERE id = $1",
    )
    .bind(id)
    .bind(display_name)
    .bind(email)
    .bind(avatar_url)
    .bind(username)
    .execute(pool)
    .await?;
    Ok(())
}
