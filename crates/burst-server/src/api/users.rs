use axum::extract::{Path, Query, State};
use axum::routing::{get, put};
use axum::{Json, Router};
use burst_core::dnd::{DoNotDisturb, Schedule};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::AppState;
use crate::api::PaginatedResponse;
use crate::api::extractors::{AuthUser, PaginationParams};
use crate::db;
use crate::error::ApiError;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/users", get(list_users))
        .route("/users/{user_id}", get(get_user))
        .route("/users/me", get(get_me).patch(update_me))
        .route("/users/me/status", put(set_status).delete(clear_status))
        .route(
            "/users/me/do-not-disturb",
            get(get_do_not_disturb).put(set_do_not_disturb),
        )
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserResponse {
    pub id: String,
    pub username: String,
    pub display_name: String,
    pub email: Option<String>,
    pub avatar_url: Option<String>,
    pub role: String,
    pub status: String,
    pub status_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_emoji: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_expires_at: Option<String>,
    /// When the user's current quiet period ends; absent when not quiet.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub do_not_disturb_until: Option<String>,
    pub is_bot: bool,
    pub created_at: String,
}

/// A status past its expiry is reported as no status.
fn user_to_response(row: &db::users::UserRow) -> UserResponse {
    let status = burst_core::status::is_active(row.status_expires_at, chrono::Utc::now());
    UserResponse {
        id: burst_core::id::format_user_id(row.id),
        username: row.username.clone(),
        display_name: row.display_name.clone(),
        email: row.email.clone(),
        avatar_url: row.avatar_url.clone(),
        role: row.role.clone(),
        status: row.status.clone(),
        status_text: row.status_text.clone().filter(|_| status),
        status_emoji: row.status_emoji.clone().filter(|_| status),
        status_expires_at: row
            .status_expires_at
            .filter(|_| status)
            .map(|at| at.to_rfc3339()),
        do_not_disturb_until: row
            .do_not_disturb()
            .quiet_until(chrono::Utc::now())
            .map(|at| at.to_rfc3339()),
        is_bot: row.is_bot,
        created_at: row.created_at.to_rfc3339(),
    }
}

async fn list_users(
    _auth: AuthUser,
    State(state): State<AppState>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<PaginatedResponse<UserResponse>>, ApiError> {
    let limit = params.clamped_limit();
    let users = db::users::list(&state.db, params.cursor_uuid(), limit + 1).await?;

    let has_more = users.len() as i64 > limit;
    let items: Vec<_> = users
        .iter()
        .take(limit as usize)
        .map(user_to_response)
        .collect();
    let cursor = if has_more {
        items.last().map(|u| u.id.clone())
    } else {
        None
    };

    Ok(Json(PaginatedResponse { items, cursor }))
}

async fn get_user(
    _auth: AuthUser,
    State(state): State<AppState>,
    Path(user_id): Path<String>,
) -> Result<Json<UserResponse>, ApiError> {
    let id = burst_core::id::parse_prefixed_id(&user_id, "usr_")
        .or_else(|| Uuid::parse_str(&user_id).ok())
        .ok_or_else(|| ApiError::BadRequest("invalid user ID".into()))?;

    let user = db::users::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| ApiError::NotFound("User".into()))?;

    Ok(Json(user_to_response(&user)))
}

async fn get_me(
    auth: AuthUser,
    State(state): State<AppState>,
) -> Result<Json<UserResponse>, ApiError> {
    let user = db::users::find_by_id(&state.db, auth.user_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("User".into()))?;

    Ok(Json(user_to_response(&user)))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateMeRequest {
    display_name: Option<String>,
    email: Option<String>,
    status_text: Option<String>,
}

async fn update_me(
    auth: AuthUser,
    State(state): State<AppState>,
    Json(body): Json<UpdateMeRequest>,
) -> Result<Json<UserResponse>, ApiError> {
    let update = db::users::UpdateUser {
        display_name: body.display_name,
        email: body.email,
        avatar_url: None,
        status_text: body.status_text,
    };

    let status_changed = update.status_text.is_some();
    let user = db::users::update(&state.db, auth.user_id, &update)
        .await?
        .ok_or_else(|| ApiError::NotFound("User".into()))?;

    let response = user_to_response(&user);
    if status_changed {
        announce_status(&state, &response).await;
    }
    Ok(Json(response))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SetStatusRequest {
    text: Option<String>,
    emoji: Option<String>,
    expires_at: Option<String>,
}

async fn set_status(
    auth: AuthUser,
    State(state): State<AppState>,
    Json(body): Json<SetStatusRequest>,
) -> Result<Json<UserResponse>, ApiError> {
    let expires_at = body
        .expires_at
        .as_deref()
        .map(|at| {
            chrono::DateTime::parse_from_rfc3339(at)
                .map(|at| at.with_timezone(&chrono::Utc))
                .map_err(|_| ApiError::BadRequest("expiresAt must be an RFC 3339 date-time".into()))
        })
        .transpose()?;
    let status = burst_core::status::CustomStatus::new(
        body.text.as_deref(),
        body.emoji.as_deref(),
        expires_at,
        chrono::Utc::now(),
    )
    .map_err(|e| ApiError::BadRequest(e.to_string()))?;

    save_status(&state, auth.user_id, Some(&status)).await
}

async fn clear_status(
    auth: AuthUser,
    State(state): State<AppState>,
) -> Result<Json<UserResponse>, ApiError> {
    save_status(&state, auth.user_id, None).await
}

async fn save_status(
    state: &AppState,
    user_id: Uuid,
    status: Option<&burst_core::status::CustomStatus>,
) -> Result<Json<UserResponse>, ApiError> {
    let user = db::users::set_status(&state.db, user_id, status)
        .await?
        .ok_or_else(|| ApiError::NotFound("User".into()))?;
    let response = user_to_response(&user);
    announce_status(state, &response).await;
    Ok(Json(response))
}

/// Tells every connected client the user's status as it now reads.
async fn announce_status(state: &AppState, user: &UserResponse) {
    let event = crate::ws::ServerEvent::UserStatusChanged {
        event_id: burst_core::id::new_id().to_string(),
        user_id: user.id.clone(),
        text: user.status_text.clone(),
        emoji: user.status_emoji.clone(),
        expires_at: user.status_expires_at.clone(),
    };
    crate::services::broadcast(state, event).await;
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScheduleBody {
    /// `HH:MM`, local to `time_zone`.
    pub start: String,
    pub end: String,
    pub days: Vec<String>,
    /// IANA name, e.g. `Europe/Paris`.
    pub time_zone: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DoNotDisturbResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snooze_until: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schedule: Option<ScheduleBody>,
    /// When the current quiet period ends; absent when not quiet.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quiet_until: Option<String>,
}

fn dnd_response(dnd: &DoNotDisturb) -> DoNotDisturbResponse {
    let now = chrono::Utc::now();
    DoNotDisturbResponse {
        snooze_until: dnd
            .snooze_until
            .filter(|at| *at > now)
            .map(|at| at.to_rfc3339()),
        schedule: dnd.schedule.map(|s| ScheduleBody {
            start: s.start.format("%H:%M").to_string(),
            end: s.end.format("%H:%M").to_string(),
            days: s.day_names().into_iter().map(String::from).collect(),
            time_zone: s.time_zone.name().to_string(),
        }),
        quiet_until: dnd.quiet_until(now).map(|at| at.to_rfc3339()),
    }
}

async fn get_do_not_disturb(
    auth: AuthUser,
    State(state): State<AppState>,
) -> Result<Json<DoNotDisturbResponse>, ApiError> {
    let user = db::users::find_by_id(&state.db, auth.user_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("User".into()))?;
    Ok(Json(dnd_response(&user.do_not_disturb())))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SetDoNotDisturbRequest {
    snooze_until: Option<String>,
    schedule: Option<ScheduleBody>,
}

/// Replaces the whole setting: an omitted snooze or schedule is cleared.
async fn set_do_not_disturb(
    auth: AuthUser,
    State(state): State<AppState>,
    Json(body): Json<SetDoNotDisturbRequest>,
) -> Result<Json<DoNotDisturbResponse>, ApiError> {
    let snooze_until = body
        .snooze_until
        .as_deref()
        .map(|at| {
            chrono::DateTime::parse_from_rfc3339(at)
                .map(|at| at.with_timezone(&chrono::Utc))
                .map_err(|_| {
                    ApiError::BadRequest("snoozeUntil must be an RFC 3339 date-time".into())
                })
        })
        .transpose()?;
    if snooze_until.is_some_and(|at| at <= chrono::Utc::now()) {
        return Err(ApiError::BadRequest(
            burst_core::dnd::DndError::SnoozeInPast.to_string(),
        ));
    }
    let schedule = body
        .schedule
        .map(|s| Schedule::parse(&s.start, &s.end, &s.days, &s.time_zone))
        .transpose()
        .map_err(|e| ApiError::BadRequest(e.to_string()))?;
    let dnd = DoNotDisturb {
        snooze_until,
        schedule,
    };

    let user = db::users::set_do_not_disturb(&state.db, auth.user_id, &dnd)
        .await?
        .ok_or_else(|| ApiError::NotFound("User".into()))?;
    let response = dnd_response(&user.do_not_disturb());
    crate::services::broadcast(
        &state,
        crate::ws::ServerEvent::UserDndChanged {
            event_id: burst_core::id::new_id().to_string(),
            user_id: burst_core::id::format_user_id(auth.user_id),
            until: response.quiet_until.clone(),
        },
    )
    .await;
    Ok(Json(response))
}

/// JIT provisioning: create or return an existing user from an external identity.
pub async fn jit_provision(
    pool: &sqlx::PgPool,
    external_id: &str,
    username: &str,
    display_name: &str,
    email: Option<&str>,
    role: &str,
) -> Result<Uuid, ApiError> {
    if let Some(user) = db::users::find_by_external_id(pool, external_id).await? {
        return Ok(user.id);
    }

    let id = burst_core::id::new_id();
    let user = db::users::create_or_get_by_external_id(
        pool,
        &db::users::CreateUser {
            id,
            username: username.to_string(),
            display_name: display_name.to_string(),
            email: email.map(String::from),
            password_hash: None,
            external_id: Some(external_id.to_string()),
            role: role.to_string(),
        },
        external_id,
    )
    .await?;

    // No row for this identity after a refused insert means the username or
    // email belongs to somebody else, which is a genuine conflict.
    match user {
        Some(user) => Ok(user.id),
        None => Err(ApiError::Conflict(format!(
            "cannot provision '{external_id}': its username or email is already taken by another user"
        ))),
    }
}
