use axum::extract::{Path, Query, State};
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::AppState;
use crate::api::extractors::{AuthUser, PaginationParams};
use crate::db;
use crate::error::ApiError;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/channels", get(list_channels).post(create_channel))
        .route(
            "/channels/{channel_id}",
            get(get_channel).patch(update_channel),
        )
        .route(
            "/channels/{channel_id}/members",
            get(list_members).post(join_channel),
        )
        .route(
            "/channels/{channel_id}/members/me",
            axum::routing::delete(leave_channel),
        )
        .route(
            "/channels/{channel_id}/messages",
            get(list_messages).post(send_message),
        )
        .route(
            "/channels/{channel_id}/messages/{message_id}",
            get(get_message).patch(edit_message).delete(delete_message),
        )
}

// ── Channel types ──

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelResponse {
    pub id: String,
    pub kind: String,
    pub name: Option<String>,
    pub slug: Option<String>,
    pub topic: Option<String>,
    pub description: Option<String>,
    pub created_by: String,
    pub is_archived: bool,
    pub is_readonly: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelMemberResponse {
    pub user_id: String,
    pub role: String,
    pub joined_at: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaginatedResponse<T> {
    pub items: Vec<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
}

fn channel_to_response(row: &db::channels::ChannelRow) -> ChannelResponse {
    ChannelResponse {
        id: burst_core::id::format_channel_id(row.id),
        kind: row.kind.clone(),
        name: row.name.clone(),
        slug: row.slug.clone(),
        topic: row.topic.clone(),
        description: row.description.clone(),
        created_by: burst_core::id::format_user_id(row.created_by),
        is_archived: row.is_archived,
        is_readonly: row.is_readonly,
        created_at: row.created_at.to_rfc3339(),
        updated_at: row.updated_at.to_rfc3339(),
    }
}

// ── Message types ──

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageResponse {
    pub id: String,
    pub channel_id: String,
    pub user_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<String>,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub edited_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deleted_at: Option<String>,
    pub created_at: String,
}

fn message_to_response(row: &db::messages::MessageRow) -> MessageResponse {
    MessageResponse {
        id: burst_core::id::format_message_id(row.id),
        channel_id: burst_core::id::format_channel_id(row.channel_id),
        user_id: burst_core::id::format_user_id(row.user_id),
        thread_id: row.thread_id.map(burst_core::id::format_message_id),
        content: row.content.clone(),
        edited_at: row.edited_at.map(|t| t.to_rfc3339()),
        deleted_at: row.deleted_at.map(|t| t.to_rfc3339()),
        created_at: row.created_at.to_rfc3339(),
    }
}

// ── Channel handlers ──

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateChannelRequest {
    pub name: String,
    pub slug: Option<String>,
    pub kind: Option<String>,
    pub topic: Option<String>,
    pub description: Option<String>,
}

async fn create_channel(
    auth: AuthUser,
    State(state): State<AppState>,
    Json(body): Json<CreateChannelRequest>,
) -> Result<(axum::http::StatusCode, Json<ChannelResponse>), ApiError> {
    let kind = body.kind.as_deref().unwrap_or("public");
    if !matches!(kind, "public" | "private") {
        return Err(ApiError::BadRequest(
            "kind must be 'public' or 'private'".into(),
        ));
    }

    let slug = body.slug.unwrap_or_else(|| slugify(&body.name));

    // Check slug uniqueness
    if db::channels::find_by_slug(&state.db, &slug)
        .await?
        .is_some()
    {
        return Err(ApiError::Conflict("Channel slug already exists".into()));
    }

    let id = burst_core::id::new_id();
    let channel = db::channels::create(
        &state.db,
        &db::channels::CreateChannel {
            id,
            kind: kind.to_string(),
            name: Some(body.name),
            slug: Some(slug),
            topic: body.topic,
            description: body.description,
            created_by: auth.user_id,
        },
    )
    .await?;

    // Add creator as owner
    db::channels::add_member(&state.db, id, auth.user_id, "owner").await?;

    Ok((
        axum::http::StatusCode::CREATED,
        Json(channel_to_response(&channel)),
    ))
}

async fn list_channels(
    auth: AuthUser,
    State(state): State<AppState>,
) -> Result<Json<Vec<ChannelResponse>>, ApiError> {
    let channels = db::channels::list_for_user(&state.db, auth.user_id).await?;
    let items = channels.iter().map(channel_to_response).collect();
    Ok(Json(items))
}

fn parse_channel_id(channel_id: &str) -> Result<Uuid, ApiError> {
    burst_core::id::parse_prefixed_id(channel_id, "ch_")
        .or_else(|| Uuid::parse_str(channel_id).ok())
        .ok_or_else(|| ApiError::BadRequest("invalid channel ID".into()))
}

fn parse_message_id(message_id: &str) -> Result<Uuid, ApiError> {
    burst_core::id::parse_prefixed_id(message_id, "msg_")
        .or_else(|| Uuid::parse_str(message_id).ok())
        .ok_or_else(|| ApiError::BadRequest("invalid message ID".into()))
}

async fn get_channel(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(channel_id): Path<String>,
) -> Result<Json<ChannelResponse>, ApiError> {
    let id = parse_channel_id(&channel_id)?;

    let channel = db::channels::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| ApiError::NotFound("Channel".into()))?;

    // Private channels require membership
    if channel.kind == "private" && !db::channels::is_member(&state.db, id, auth.user_id).await? {
        return Err(ApiError::NotFound("Channel".into()));
    }

    Ok(Json(channel_to_response(&channel)))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateChannelRequest {
    pub name: Option<String>,
    pub topic: Option<String>,
    pub description: Option<String>,
}

async fn update_channel(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(channel_id): Path<String>,
    Json(body): Json<UpdateChannelRequest>,
) -> Result<Json<ChannelResponse>, ApiError> {
    let id = parse_channel_id(&channel_id)?;

    if !db::channels::is_member(&state.db, id, auth.user_id).await? {
        return Err(ApiError::Forbidden);
    }

    let channel = db::channels::update(
        &state.db,
        id,
        &db::channels::UpdateChannel {
            name: body.name,
            topic: body.topic,
            description: body.description,
        },
    )
    .await?
    .ok_or_else(|| ApiError::NotFound("Channel".into()))?;

    Ok(Json(channel_to_response(&channel)))
}

// ── Membership handlers ──

async fn join_channel(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(channel_id): Path<String>,
) -> Result<axum::http::StatusCode, ApiError> {
    let id = parse_channel_id(&channel_id)?;

    let channel = db::channels::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| ApiError::NotFound("Channel".into()))?;

    if channel.kind != "public" {
        return Err(ApiError::Forbidden);
    }

    db::channels::add_member(&state.db, id, auth.user_id, "member").await?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

async fn leave_channel(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(channel_id): Path<String>,
) -> Result<axum::http::StatusCode, ApiError> {
    let id = parse_channel_id(&channel_id)?;
    db::channels::remove_member(&state.db, id, auth.user_id).await?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

async fn list_members(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(channel_id): Path<String>,
) -> Result<Json<Vec<ChannelMemberResponse>>, ApiError> {
    let id = parse_channel_id(&channel_id)?;

    if !db::channels::is_member(&state.db, id, auth.user_id).await? {
        return Err(ApiError::Forbidden);
    }

    let members = db::channels::list_members(&state.db, id).await?;
    let items = members
        .iter()
        .map(|m| ChannelMemberResponse {
            user_id: burst_core::id::format_user_id(m.user_id),
            role: m.role.clone(),
            joined_at: m.joined_at.to_rfc3339(),
        })
        .collect();
    Ok(Json(items))
}

// ── Message handlers ──

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendMessageRequest {
    pub content: String,
}

async fn send_message(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(channel_id): Path<String>,
    Json(body): Json<SendMessageRequest>,
) -> Result<(axum::http::StatusCode, Json<MessageResponse>), ApiError> {
    let ch_id = parse_channel_id(&channel_id)?;

    if !db::channels::is_member(&state.db, ch_id, auth.user_id).await? {
        return Err(ApiError::Forbidden);
    }

    let content = body.content.trim();
    if content.is_empty() {
        return Err(ApiError::BadRequest(
            "message content cannot be empty".into(),
        ));
    }

    let id = burst_core::id::new_id();
    let message = db::messages::create(&state.db, id, ch_id, auth.user_id, content).await?;

    Ok((
        axum::http::StatusCode::CREATED,
        Json(message_to_response(&message)),
    ))
}

async fn list_messages(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(channel_id): Path<String>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<PaginatedResponse<MessageResponse>>, ApiError> {
    let ch_id = parse_channel_id(&channel_id)?;

    if !db::channels::is_member(&state.db, ch_id, auth.user_id).await? {
        return Err(ApiError::Forbidden);
    }

    let limit = params.clamped_limit();
    let messages =
        db::messages::list_in_channel(&state.db, ch_id, params.cursor, limit + 1).await?;

    let has_more = messages.len() as i64 > limit;
    let items: Vec<_> = messages
        .iter()
        .take(limit as usize)
        .map(message_to_response)
        .collect();
    let cursor = if has_more {
        items.last().map(|m| m.id.clone())
    } else {
        None
    };

    Ok(Json(PaginatedResponse { items, cursor }))
}

async fn get_message(
    auth: AuthUser,
    State(state): State<AppState>,
    Path((channel_id, message_id)): Path<(String, String)>,
) -> Result<Json<MessageResponse>, ApiError> {
    let ch_id = parse_channel_id(&channel_id)?;

    if !db::channels::is_member(&state.db, ch_id, auth.user_id).await? {
        return Err(ApiError::Forbidden);
    }

    let msg_id = parse_message_id(&message_id)?;
    let message = db::messages::find_by_id(&state.db, msg_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("Message".into()))?;

    if message.channel_id != ch_id {
        return Err(ApiError::NotFound("Message".into()));
    }

    Ok(Json(message_to_response(&message)))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditMessageRequest {
    pub content: String,
}

async fn edit_message(
    auth: AuthUser,
    State(state): State<AppState>,
    Path((channel_id, message_id)): Path<(String, String)>,
    Json(body): Json<EditMessageRequest>,
) -> Result<Json<MessageResponse>, ApiError> {
    let ch_id = parse_channel_id(&channel_id)?;

    if !db::channels::is_member(&state.db, ch_id, auth.user_id).await? {
        return Err(ApiError::Forbidden);
    }

    let msg_id = parse_message_id(&message_id)?;
    let content = body.content.trim();
    if content.is_empty() {
        return Err(ApiError::BadRequest(
            "message content cannot be empty".into(),
        ));
    }

    let message = db::messages::update_content(&state.db, msg_id, auth.user_id, content)
        .await?
        .ok_or_else(|| ApiError::NotFound("Message".into()))?;

    if message.channel_id != ch_id {
        return Err(ApiError::NotFound("Message".into()));
    }

    Ok(Json(message_to_response(&message)))
}

async fn delete_message(
    auth: AuthUser,
    State(state): State<AppState>,
    Path((channel_id, message_id)): Path<(String, String)>,
) -> Result<axum::http::StatusCode, ApiError> {
    let ch_id = parse_channel_id(&channel_id)?;

    if !db::channels::is_member(&state.db, ch_id, auth.user_id).await? {
        return Err(ApiError::Forbidden);
    }

    let msg_id = parse_message_id(&message_id)?;
    db::messages::soft_delete(&state.db, msg_id, auth.user_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("Message".into()))?;

    Ok(axum::http::StatusCode::NO_CONTENT)
}

fn slugify(name: &str) -> String {
    name.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}
