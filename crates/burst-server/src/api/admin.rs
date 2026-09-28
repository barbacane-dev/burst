use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::routing::{delete, get, patch};
use axum::{Json, Router};
use bytes::Bytes;
use serde::{Deserialize, Serialize};

use crate::AppState;
use crate::api::PaginatedResponse;
use crate::api::channels::{ChannelResponse, channel_to_response};
use crate::api::extractors::{AdminUser, AuthUser, IntegrationUser, PaginationParams};
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
        .route("/admin/emojis", get(list_emojis_admin).post(create_emoji))
        .route("/admin/emojis/{emoji_id}", delete(delete_emoji))
        .route("/emojis", get(list_emojis))
        .route("/emojis/{emoji_id}/image", get(emoji_image))
        .route("/admin/bots", get(list_bots).post(create_bot))
        .route("/admin/bots/{bot_id}", patch(update_bot).delete(delete_bot))
        .route("/admin/exports", get(list_exports).post(create_export))
        .route(
            "/admin/exports/{export_id}",
            get(get_export).delete(delete_export),
        )
        .route("/admin/exports/{export_id}/download", get(download_export))
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
        if !matches!(
            role.as_str(),
            "admin" | "integrator" | "moderator" | "member" | "guest"
        ) {
            return Err(ApiError::BadRequest(
                "role must be admin, integrator, moderator, member, or guest".into(),
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

// ── Custom emoji management ──

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomEmojiResponse {
    pub id: String,
    pub shortcode: String,
    pub image_url: String,
    pub created_by: String,
    pub created_at: String,
}

fn emoji_to_response(row: &db::custom_emojis::CustomEmojiRow) -> CustomEmojiResponse {
    CustomEmojiResponse {
        id: row.id.to_string(),
        shortcode: row.shortcode.clone(),
        image_url: format!("/api/emojis/{}/image", row.id),
        created_by: burst_core::id::format_user_id(row.created_by),
        created_at: row.created_at.to_rfc3339(),
    }
}

/// Public endpoint: list all custom emojis (for the emoji picker).
async fn list_emojis(
    _auth: AuthUser,
    State(state): State<AppState>,
) -> Result<Json<PaginatedResponse<CustomEmojiResponse>>, ApiError> {
    let emojis = db::custom_emojis::list(&state.db).await?;
    let items = emojis.iter().map(emoji_to_response).collect();
    Ok(Json(PaginatedResponse {
        items,
        cursor: None,
    }))
}

/// Admin endpoint: list all custom emojis.
async fn list_emojis_admin(
    _admin: AdminUser,
    State(state): State<AppState>,
) -> Result<Json<PaginatedResponse<CustomEmojiResponse>>, ApiError> {
    let emojis = db::custom_emojis::list(&state.db).await?;
    let items = emojis.iter().map(emoji_to_response).collect();
    Ok(Json(PaginatedResponse {
        items,
        cursor: None,
    }))
}

async fn create_emoji(
    admin: AdminUser,
    State(state): State<AppState>,
    headers: HeaderMap,
    body: axum::body::Body,
) -> Result<(axum::http::StatusCode, Json<CustomEmojiResponse>), ApiError> {
    let content_type = headers
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    if !content_type.starts_with("multipart/form-data") {
        return Err(ApiError::BadRequest("expected multipart/form-data".into()));
    }

    let boundary = multer::parse_boundary(content_type)
        .map_err(|_| ApiError::BadRequest("missing multipart boundary".into()))?;
    let stream = body.into_data_stream();
    let mut multipart = multer::Multipart::new(stream, boundary);

    let mut shortcode = String::new();
    let mut image_data: Option<Bytes> = None;
    let mut image_ct = String::new();

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| ApiError::BadRequest(format!("multipart error: {e}")))?
    {
        let name = field.name().unwrap_or("").to_string();
        match name.as_str() {
            "shortcode" => {
                shortcode = field
                    .text()
                    .await
                    .map_err(|e| ApiError::BadRequest(format!("invalid shortcode: {e}")))?;
            }
            "image" => {
                image_ct = field
                    .content_type()
                    .map(|ct| ct.to_string())
                    .unwrap_or_else(|| "image/png".into());
                image_data = Some(
                    field
                        .bytes()
                        .await
                        .map_err(|e| ApiError::BadRequest(format!("failed to read image: {e}")))?,
                );
            }
            _ => {}
        }
    }

    // Validate shortcode
    let shortcode = shortcode.trim().to_lowercase();
    if shortcode.len() < 2 || shortcode.len() > 32 {
        return Err(ApiError::BadRequest(
            "shortcode must be 2-32 characters".into(),
        ));
    }
    if !shortcode
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        return Err(ApiError::BadRequest(
            "shortcode must contain only alphanumeric characters and underscores".into(),
        ));
    }

    let data = image_data.ok_or_else(|| ApiError::BadRequest("image field is required".into()))?;

    if !image_ct.starts_with("image/") {
        return Err(ApiError::BadRequest(
            "image must have an image/* content type".into(),
        ));
    }
    if data.len() > 256 * 1024 {
        return Err(ApiError::PayloadTooLarge(
            "emoji image must be under 256 KB".into(),
        ));
    }

    // Store image
    let emoji_id = burst_core::id::new_id();
    let ext = match image_ct.as_str() {
        "image/gif" => "gif",
        "image/webp" => "webp",
        "image/svg+xml" => "svg",
        _ => "png",
    };
    let storage_key = format!("emojis/{emoji_id}.{ext}");
    state
        .storage
        .put(&storage_key, data, &image_ct)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let emoji =
        db::custom_emojis::create(&state.db, emoji_id, &shortcode, &storage_key, admin.user_id)
            .await?;

    db::audit_log::insert(
        &state.db,
        burst_core::id::new_id(),
        admin.user_id,
        "emoji.created",
        "emoji",
        emoji_id,
        Some(serde_json::json!({ "shortcode": shortcode })),
    )
    .await?;

    Ok((
        axum::http::StatusCode::CREATED,
        Json(emoji_to_response(&emoji)),
    ))
}

fn parse_emoji_id(s: &str) -> Result<uuid::Uuid, ApiError> {
    uuid::Uuid::parse_str(s).map_err(|_| ApiError::BadRequest("invalid emoji ID".into()))
}

/// The image of a custom emoji, for any signed-in user. Emojis are workspace
/// wide, so there is no membership check. The CSP and `nosniff` keep an SVG
/// from running script when opened directly.
async fn emoji_image(
    _auth: AuthUser,
    State(state): State<AppState>,
    Path(emoji_id): Path<String>,
) -> Result<axum::response::Response, ApiError> {
    use axum::http::header;
    use axum::response::IntoResponse;

    let id = parse_emoji_id(&emoji_id)?;
    let emoji = db::custom_emojis::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| ApiError::NotFound("Emoji".into()))?;
    let (data, content_type) = state
        .storage
        .get(&emoji.image_url)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok((
        [
            (header::CONTENT_TYPE, content_type),
            (header::CACHE_CONTROL, "private, max-age=86400".to_string()),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff".to_string()),
            (
                header::CONTENT_SECURITY_POLICY,
                "default-src 'none'; style-src 'unsafe-inline'; sandbox".to_string(),
            ),
        ],
        data,
    )
        .into_response())
}

async fn delete_emoji(
    admin: AdminUser,
    State(state): State<AppState>,
    Path(emoji_id): Path<String>,
) -> Result<axum::http::StatusCode, ApiError> {
    let id = parse_emoji_id(&emoji_id)?;
    let storage_key = db::custom_emojis::delete(&state.db, id)
        .await?
        .ok_or_else(|| ApiError::NotFound("Emoji".into()))?;
    // The emoji is gone either way; a file left behind only costs space.
    if let Err(e) = state.storage.delete(&storage_key).await {
        tracing::warn!(error = %e, key = %storage_key, "could not delete an emoji image");
    }

    db::audit_log::insert(
        &state.db,
        burst_core::id::new_id(),
        admin.user_id,
        "emoji.deleted",
        "emoji",
        id,
        None,
    )
    .await?;

    Ok(axum::http::StatusCode::NO_CONTENT)
}

// ── Bot management ──

async fn list_bots(
    _integration: IntegrationUser,
    State(state): State<AppState>,
) -> Result<Json<PaginatedResponse<AdminUserResponse>>, ApiError> {
    let bots = db::users::list_bots(&state.db).await?;
    let items = bots.iter().map(user_to_response).collect();
    Ok(Json(PaginatedResponse {
        items,
        cursor: None,
    }))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateBotRequest {
    username: String,
    display_name: String,
}

async fn create_bot(
    integration: IntegrationUser,
    State(state): State<AppState>,
    Json(body): Json<CreateBotRequest>,
) -> Result<(axum::http::StatusCode, Json<AdminUserResponse>), ApiError> {
    let username = body.username.trim().to_lowercase();
    if username.is_empty() || username.len() > 64 {
        return Err(ApiError::BadRequest(
            "username must be 1-64 characters".into(),
        ));
    }
    if !username
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Err(ApiError::BadRequest(
            "username must contain only alphanumeric characters, underscores, or hyphens".into(),
        ));
    }

    let display_name = body.display_name.trim().to_string();
    if display_name.is_empty() {
        return Err(ApiError::BadRequest("displayName cannot be empty".into()));
    }

    // Check uniqueness.
    if db::users::find_by_username(&state.db, &username)
        .await?
        .is_some()
    {
        return Err(ApiError::Conflict("username already taken".into()));
    }

    let id = burst_core::id::new_id();
    let user = db::users::create_bot(&state.db, id, &username, &display_name).await?;

    db::audit_log::insert(
        &state.db,
        burst_core::id::new_id(),
        integration.user_id,
        "bot.created",
        "user",
        id,
        Some(serde_json::json!({ "username": username })),
    )
    .await?;

    Ok((
        axum::http::StatusCode::CREATED,
        Json(user_to_response(&user)),
    ))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateBotRequest {
    display_name: Option<String>,
}

async fn update_bot(
    integration: IntegrationUser,
    State(state): State<AppState>,
    Path(bot_id): Path<String>,
    Json(body): Json<UpdateBotRequest>,
) -> Result<Json<AdminUserResponse>, ApiError> {
    let uid = parse_user_id(&bot_id)?;
    let user = db::users::find_by_id(&state.db, uid)
        .await?
        .ok_or_else(|| ApiError::NotFound("Bot".into()))?;

    if !user.is_bot {
        return Err(ApiError::BadRequest("user is not a bot".into()));
    }

    let display_name = body.display_name.as_deref().map(str::trim);
    if display_name == Some("") {
        return Err(ApiError::BadRequest("displayName cannot be empty".into()));
    }

    let updated = db::users::update(
        &state.db,
        uid,
        &db::users::UpdateUser {
            display_name: display_name.map(String::from),
            email: None,
            avatar_url: None,
            status_text: None,
        },
    )
    .await?
    .ok_or_else(|| ApiError::NotFound("Bot".into()))?;

    db::audit_log::insert(
        &state.db,
        burst_core::id::new_id(),
        integration.user_id,
        "bot.updated",
        "user",
        uid,
        None,
    )
    .await?;

    Ok(Json(user_to_response(&updated)))
}

async fn delete_bot(
    integration: IntegrationUser,
    State(state): State<AppState>,
    Path(bot_id): Path<String>,
) -> Result<axum::http::StatusCode, ApiError> {
    let uid = parse_user_id(&bot_id)?;
    let user = db::users::find_by_id(&state.db, uid)
        .await?
        .ok_or_else(|| ApiError::NotFound("Bot".into()))?;

    if !user.is_bot {
        return Err(ApiError::BadRequest("user is not a bot".into()));
    }

    db::users::deactivate(&state.db, uid).await?;

    db::audit_log::insert(
        &state.db,
        burst_core::id::new_id(),
        integration.user_id,
        "bot.deactivated",
        "user",
        uid,
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

// ── Data exports ──

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportResponse {
    pub id: String,
    pub scope: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channel_id: Option<String>,
    pub status: String,
    pub requested_by: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub created_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<String>,
}

fn export_to_response(row: &db::exports::ExportRow) -> ExportResponse {
    ExportResponse {
        id: burst_core::id::format_export_id(row.id),
        scope: row.scope.clone(),
        channel_id: row.channel_id.map(burst_core::id::format_channel_id),
        status: row.status.clone(),
        requested_by: burst_core::id::format_user_id(row.requested_by),
        size_bytes: row.size_bytes,
        error: row.error.clone(),
        created_at: row.created_at.to_rfc3339(),
        completed_at: row.completed_at.map(|at| at.to_rfc3339()),
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateExportRequest {
    /// One channel; the whole instance when absent.
    channel_id: Option<String>,
}

/// Exports that stopped making progress are failed first, so they neither
/// show as running nor block a new export.
async fn fail_stalled_exports(state: &AppState) -> Result<(), ApiError> {
    let before = chrono::Utc::now() - crate::services::export::STALL_AFTER;
    db::exports::fail_stalled(&state.db, before).await?;
    Ok(())
}

fn parse_export_id(raw: &str) -> Result<uuid::Uuid, ApiError> {
    burst_core::id::parse_prefixed_id(raw, "exp_")
        .ok_or_else(|| ApiError::BadRequest("invalid export ID".into()))
}

async fn create_export(
    admin: AdminUser,
    State(state): State<AppState>,
    body: Option<Json<CreateExportRequest>>,
) -> Result<(axum::http::StatusCode, Json<ExportResponse>), ApiError> {
    let channel_id = match body.and_then(|Json(b)| b.channel_id) {
        Some(raw) => {
            let id = crate::services::parse_id(&raw, "ch_", "channel")?;
            db::channels::find_by_id(&state.db, id)
                .await?
                .ok_or_else(|| ApiError::NotFound("Channel".into()))?;
            Some(id)
        }
        None => None,
    };

    fail_stalled_exports(&state).await?;
    let export = db::exports::create(
        &state.db,
        burst_core::id::new_id(),
        admin.user_id,
        channel_id,
    )
    .await?
    .ok_or_else(|| ApiError::Conflict("an export is already in progress".into()))?;

    db::audit_log::insert(
        &state.db,
        burst_core::id::new_id(),
        admin.user_id,
        "export.requested",
        "export",
        export.id,
        Some(serde_json::json!({
            "scope": export.scope,
            "channelId": channel_id.map(burst_core::id::format_channel_id),
        })),
    )
    .await?;

    let response = export_to_response(&export);
    crate::services::export::spawn(state, export);
    Ok((axum::http::StatusCode::ACCEPTED, Json(response)))
}

async fn list_exports(
    _admin: AdminUser,
    State(state): State<AppState>,
) -> Result<Json<PaginatedResponse<ExportResponse>>, ApiError> {
    fail_stalled_exports(&state).await?;
    let rows = db::exports::list(&state.db, 50).await?;
    Ok(Json(PaginatedResponse {
        items: rows.iter().map(export_to_response).collect(),
        cursor: None,
    }))
}

async fn get_export(
    _admin: AdminUser,
    State(state): State<AppState>,
    Path(export_id): Path<String>,
) -> Result<Json<ExportResponse>, ApiError> {
    fail_stalled_exports(&state).await?;
    let export = db::exports::find(&state.db, parse_export_id(&export_id)?)
        .await?
        .ok_or_else(|| ApiError::NotFound("Export".into()))?;
    Ok(Json(export_to_response(&export)))
}

async fn download_export(
    admin: AdminUser,
    State(state): State<AppState>,
    Path(export_id): Path<String>,
) -> Result<axum::response::Response, ApiError> {
    let export = db::exports::find(&state.db, parse_export_id(&export_id)?)
        .await?
        .ok_or_else(|| ApiError::NotFound("Export".into()))?;
    let key = match (export.status.as_str(), &export.storage_key) {
        ("completed", Some(key)) => key.clone(),
        _ => return Err(ApiError::Conflict("the export is not complete".into())),
    };
    let (stream, len) = state.storage.get_stream(&key).await.map_err(|e| match e {
        crate::storage::StorageError::NotFound(_) => ApiError::NotFound("Export file".into()),
        e => ApiError::Internal(e.to_string()),
    })?;

    db::audit_log::insert(
        &state.db,
        burst_core::id::new_id(),
        admin.user_id,
        "export.downloaded",
        "export",
        export.id,
        None,
    )
    .await?;

    let file_name = format!(
        "burst-export-{}-{}.zip",
        export.created_at.format("%Y%m%d"),
        &export.id.simple().to_string()[..8]
    );
    let mut response = axum::response::Response::builder()
        .header(axum::http::header::CONTENT_TYPE, "application/zip")
        .header(
            axum::http::header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{file_name}\""),
        );
    if let Some(len) = len {
        response = response.header(axum::http::header::CONTENT_LENGTH, len);
    }
    response
        .body(axum::body::Body::from_stream(stream))
        .map_err(|e| ApiError::Internal(e.to_string()))
}

async fn delete_export(
    admin: AdminUser,
    State(state): State<AppState>,
    Path(export_id): Path<String>,
) -> Result<axum::http::StatusCode, ApiError> {
    fail_stalled_exports(&state).await?;
    let export = db::exports::find(&state.db, parse_export_id(&export_id)?)
        .await?
        .ok_or_else(|| ApiError::NotFound("Export".into()))?;
    if matches!(export.status.as_str(), "pending" | "running") {
        return Err(ApiError::Conflict("the export is still in progress".into()));
    }
    if let Some(key) = &export.storage_key {
        state
            .storage
            .delete(key)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?;
    }
    db::exports::delete(&state.db, export.id).await?;
    db::audit_log::insert(
        &state.db,
        burst_core::id::new_id(),
        admin.user_id,
        "export.deleted",
        "export",
        export.id,
        None,
    )
    .await?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}
