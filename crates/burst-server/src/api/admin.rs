use axum::extract::{Path, Query, State};
use axum::routing::{get, patch};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use crate::AppState;
use crate::api::PaginatedResponse;
use crate::api::channels::{ChannelResponse, channel_to_response};
use crate::api::extractors::{AdminUser, PaginationParams};
use crate::db;
use crate::error::ApiError;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/admin/users", get(list_users))
        .route("/admin/users/{user_id}", patch(update_user))
        .route("/admin/channels", get(list_channels))
        .route(
            "/admin/channels/{channel_id}",
            patch(update_channel).delete(delete_channel),
        )
        .route("/admin/audit-log", get(list_audit_log))
}

// ── Response types ──

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AdminUserResponse {
    id: String,
    username: String,
    display_name: String,
    email: Option<String>,
    role: String,
    is_bot: bool,
    deactivated_at: Option<String>,
    created_at: String,
}

fn user_to_response(row: &db::users::UserRow) -> AdminUserResponse {
    AdminUserResponse {
        id: burst_core::id::format_user_id(row.id),
        username: row.username.clone(),
        display_name: row.display_name.clone(),
        email: row.email.clone(),
        role: row.role.clone(),
        is_bot: row.is_bot,
        deactivated_at: row.deactivated_at.map(|t| t.to_rfc3339()),
        created_at: row.created_at.to_rfc3339(),
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AuditLogResponse {
    id: String,
    user_id: Option<String>,
    action: String,
    target_type: String,
    target_id: String,
    metadata: Option<serde_json::Value>,
    created_at: String,
}

// ── User management ──

async fn list_users(
    _admin: AdminUser,
    State(state): State<AppState>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<PaginatedResponse<AdminUserResponse>>, ApiError> {
    let limit = params.clamped_limit();
    let users = db::users::list_all(&state.db, params.cursor_uuid(), limit + 1).await?;
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AdminUpdateUserRequest {
    role: Option<String>,
    deactivated: Option<bool>,
}

fn parse_user_id(user_id: &str) -> Result<uuid::Uuid, ApiError> {
    burst_core::id::parse_prefixed_id(user_id, "usr_")
        .or_else(|| uuid::Uuid::parse_str(user_id).ok())
        .ok_or_else(|| ApiError::BadRequest("invalid user ID".into()))
}

fn parse_channel_id(channel_id: &str) -> Result<uuid::Uuid, ApiError> {
    burst_core::id::parse_prefixed_id(channel_id, "ch_")
        .or_else(|| uuid::Uuid::parse_str(channel_id).ok())
        .ok_or_else(|| ApiError::BadRequest("invalid channel ID".into()))
}

async fn update_user(
    admin: AdminUser,
    State(state): State<AppState>,
    Path(user_id): Path<String>,
    Json(body): Json<AdminUpdateUserRequest>,
) -> Result<Json<AdminUserResponse>, ApiError> {
    let uid = parse_user_id(&user_id)?;

    let mut user = db::users::find_by_id(&state.db, uid)
        .await?
        .ok_or_else(|| ApiError::NotFound("User".into()))?;

    if let Some(ref role) = body.role {
        if !matches!(role.as_str(), "admin" | "moderator" | "member" | "guest") {
            return Err(ApiError::BadRequest(
                "role must be admin, moderator, member, or guest".into(),
            ));
        }
        user = db::users::update_role(&state.db, uid, role)
            .await?
            .ok_or_else(|| ApiError::NotFound("User".into()))?;

        db::audit_log::insert(
            &state.db,
            burst_core::id::new_id(),
            admin.user_id,
            "user.role_changed",
            "user",
            uid,
            Some(serde_json::json!({ "newRole": role })),
        )
        .await?;
    }

    if let Some(deactivated) = body.deactivated {
        if deactivated {
            if let Some(u) = db::users::deactivate(&state.db, uid).await? {
                user = u;
                db::audit_log::insert(
                    &state.db,
                    burst_core::id::new_id(),
                    admin.user_id,
                    "user.deactivated",
                    "user",
                    uid,
                    None,
                )
                .await?;
            }
        } else if let Some(u) = db::users::reactivate(&state.db, uid).await? {
            user = u;
            db::audit_log::insert(
                &state.db,
                burst_core::id::new_id(),
                admin.user_id,
                "user.reactivated",
                "user",
                uid,
                None,
            )
            .await?;
        }
    }

    Ok(Json(user_to_response(&user)))
}

// ── Channel management ──

async fn list_channels(
    _admin: AdminUser,
    State(state): State<AppState>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<PaginatedResponse<ChannelResponse>>, ApiError> {
    let limit = params.clamped_limit();
    let channels = db::users::list_all_channels(&state.db, params.cursor_uuid(), limit + 1).await?;
    let has_more = channels.len() as i64 > limit;
    let items: Vec<_> = channels
        .iter()
        .take(limit as usize)
        .map(channel_to_response)
        .collect();
    let cursor = if has_more {
        items.last().map(|c| c.id.clone())
    } else {
        None
    };
    Ok(Json(PaginatedResponse { items, cursor }))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AdminUpdateChannelRequest {
    is_archived: Option<bool>,
}

async fn update_channel(
    admin: AdminUser,
    State(state): State<AppState>,
    Path(channel_id): Path<String>,
    Json(body): Json<AdminUpdateChannelRequest>,
) -> Result<Json<ChannelResponse>, ApiError> {
    let ch_id = parse_channel_id(&channel_id)?;

    let channel = if let Some(archived) = body.is_archived {
        let ch = if archived {
            db::channels::archive(&state.db, ch_id).await?
        } else {
            db::channels::unarchive(&state.db, ch_id).await?
        };
        let ch = ch.ok_or_else(|| ApiError::NotFound("Channel".into()))?;

        let action = if archived {
            "channel.archived"
        } else {
            "channel.unarchived"
        };
        db::audit_log::insert(
            &state.db,
            burst_core::id::new_id(),
            admin.user_id,
            action,
            "channel",
            ch_id,
            None,
        )
        .await?;

        ch
    } else {
        db::channels::find_by_id(&state.db, ch_id)
            .await?
            .ok_or_else(|| ApiError::NotFound("Channel".into()))?
    };

    Ok(Json(channel_to_response(&channel)))
}

async fn delete_channel(
    admin: AdminUser,
    State(state): State<AppState>,
    Path(channel_id): Path<String>,
) -> Result<axum::http::StatusCode, ApiError> {
    let ch_id = parse_channel_id(&channel_id)?;

    db::channels::find_by_id(&state.db, ch_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("Channel".into()))?;

    sqlx::query("DELETE FROM channels WHERE id = $1")
        .bind(ch_id)
        .execute(&state.db)
        .await?;

    db::audit_log::insert(
        &state.db,
        burst_core::id::new_id(),
        admin.user_id,
        "channel.deleted",
        "channel",
        ch_id,
        None,
    )
    .await?;

    Ok(axum::http::StatusCode::NO_CONTENT)
}

// ── Audit log ──

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AuditLogParams {
    #[serde(flatten)]
    pagination: PaginationParams,
    target_type: Option<String>,
}

async fn list_audit_log(
    _admin: AdminUser,
    State(state): State<AppState>,
    Query(params): Query<AuditLogParams>,
) -> Result<Json<PaginatedResponse<AuditLogResponse>>, ApiError> {
    let limit = params.pagination.clamped_limit();
    let entries = db::audit_log::list(
        &state.db,
        params.target_type.as_deref(),
        params.pagination.cursor_uuid(),
        limit + 1,
    )
    .await?;

    let has_more = entries.len() as i64 > limit;
    let items: Vec<_> = entries
        .iter()
        .take(limit as usize)
        .map(|e| AuditLogResponse {
            id: e.id.to_string(),
            user_id: e.user_id.map(burst_core::id::format_user_id),
            action: e.action.clone(),
            target_type: e.target_type.clone(),
            target_id: e.target_id.to_string(),
            metadata: e.metadata.clone(),
            created_at: e.created_at.to_rfc3339(),
        })
        .collect();

    let cursor = if has_more {
        items.last().map(|e| e.id.clone())
    } else {
        None
    };

    Ok(Json(PaginatedResponse { items, cursor }))
}
