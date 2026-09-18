use axum::extract::{Path, Query, State};
use axum::routing::get;
use axum::{Json, Router};
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
    pub is_bot: bool,
    pub created_at: String,
}

fn user_to_response(row: &db::users::UserRow) -> UserResponse {
    UserResponse {
        id: burst_core::id::format_user_id(row.id),
        username: row.username.clone(),
        display_name: row.display_name.clone(),
        email: row.email.clone(),
        avatar_url: row.avatar_url.clone(),
        role: row.role.clone(),
        status: row.status.clone(),
        status_text: row.status_text.clone(),
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

    let user = db::users::update(&state.db, auth.user_id, &update)
        .await?
        .ok_or_else(|| ApiError::NotFound("User".into()))?;

    Ok(Json(user_to_response(&user)))
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
