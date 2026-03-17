use std::path::Path as StdPath;

use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::routing::{delete, get, put};
use axum::{Json, Router};
use bytes::Bytes;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::AppState;
use crate::api::PaginatedResponse;
use crate::api::attachments::{AttachmentResponse, attachment_to_response};
use crate::api::extractors::{AuthUser, PaginationParams};
use crate::db;
use crate::error::ApiError;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/channels", get(list_channels).post(create_channel))
        .route("/dms", axum::routing::post(create_or_get_dm))
        .route(
            "/channels/{channel_id}",
            get(get_channel).patch(update_channel),
        )
        .route(
            "/channels/{channel_id}/members",
            get(list_members).post(join_channel),
        )
        .route("/channels/{channel_id}/members/me", delete(leave_channel))
        .route(
            "/channels/{channel_id}/members/me/last-read",
            axum::routing::patch(mark_read),
        )
        .route(
            "/channels/{channel_id}/members/me/notify",
            axum::routing::patch(update_notify),
        )
        .route(
            "/channels/{channel_id}/messages",
            get(list_messages).post(send_message),
        )
        .route(
            "/channels/{channel_id}/messages/{message_id}",
            get(get_message).patch(edit_message).delete(delete_message),
        )
        .route(
            "/channels/{channel_id}/messages/{message_id}/replies",
            get(list_thread_replies),
        )
        .route(
            "/channels/{channel_id}/messages/{message_id}/reactions/{emoji}",
            put(add_reaction).delete(remove_reaction),
        )
        .route(
            "/channels/{channel_id}/messages/{message_id}/pin",
            put(pin_message).delete(unpin_message),
        )
        .route("/channels/{channel_id}/pins", get(list_pins))
        .route(
            "/channels/{channel_id}/archive",
            axum::routing::post(archive_channel),
        )
        .route(
            "/channels/{channel_id}/unarchive",
            axum::routing::post(unarchive_channel),
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
    pub unread_count: i64,
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

fn channel_with_unread_to_response(row: &db::channels::ChannelWithUnreadRow) -> ChannelResponse {
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
        unread_count: row.unread_count,
        created_at: row.created_at.to_rfc3339(),
        updated_at: row.updated_at.to_rfc3339(),
    }
}

pub fn channel_to_response(row: &db::channels::ChannelRow) -> ChannelResponse {
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
        unread_count: 0,
        created_at: row.created_at.to_rfc3339(),
        updated_at: row.updated_at.to_rfc3339(),
    }
}

// ── Message types ──

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReactionResponse {
    pub emoji: String,
    pub count: i64,
    pub user_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
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
    pub reply_count: i64,
    pub reactions: Vec<ReactionResponse>,
    pub attachments: Vec<AttachmentResponse>,
    pub created_at: String,
}

fn build_message_response(
    row: &db::messages::MessageRow,
    reply_count: i64,
    reactions: Vec<ReactionResponse>,
    attachments: Vec<AttachmentResponse>,
) -> MessageResponse {
    MessageResponse {
        id: burst_core::id::format_message_id(row.id),
        channel_id: burst_core::id::format_channel_id(row.channel_id),
        user_id: burst_core::id::format_user_id(row.user_id),
        thread_id: row.thread_id.map(burst_core::id::format_message_id),
        content: row.content.clone(),
        edited_at: row.edited_at.map(|t| t.to_rfc3339()),
        deleted_at: row.deleted_at.map(|t| t.to_rfc3339()),
        reply_count,
        reactions,
        attachments,
        created_at: row.created_at.to_rfc3339(),
    }
}

/// Groups attachment rows by message ID for batch response building.
fn group_attachments(
    rows: &[db::attachments::AttachmentRow],
    message_id: Uuid,
) -> Vec<AttachmentResponse> {
    rows.iter()
        .filter(|a| a.message_id == message_id)
        .map(attachment_to_response)
        .collect()
}

/// Aggregates reaction rows into per-emoji summaries.
fn aggregate_reactions(
    rows: &[db::reactions::ReactionRow],
    message_id: Uuid,
) -> Vec<ReactionResponse> {
    use std::collections::HashMap;
    let mut map: HashMap<&str, (i64, Vec<String>)> = HashMap::new();
    for r in rows.iter().filter(|r| r.message_id == message_id) {
        let entry = map.entry(r.emoji.as_str()).or_default();
        entry.0 += 1;
        entry.1.push(burst_core::id::format_user_id(r.user_id));
    }
    let mut out: Vec<ReactionResponse> = map
        .into_iter()
        .map(|(emoji, (count, user_ids))| ReactionResponse {
            emoji: emoji.to_string(),
            count,
            user_ids,
        })
        .collect();
    out.sort_by(|a, b| a.emoji.cmp(&b.emoji));
    out
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

    db::channels::add_member(&state.db, id, auth.user_id, "owner").await?;

    Ok((
        axum::http::StatusCode::CREATED,
        Json(channel_to_response(&channel)),
    ))
}

// ?joined=true  → channels the caller is a member of (sidebar)
// ?joined=false  → all public channels (discovery/browse); default
#[derive(Deserialize)]
struct ListChannelsParams {
    #[serde(flatten)]
    pagination: PaginationParams,
    joined: Option<bool>,
}

async fn list_channels(
    auth: AuthUser,
    State(state): State<AppState>,
    Query(params): Query<ListChannelsParams>,
) -> Result<Json<PaginatedResponse<ChannelResponse>>, ApiError> {
    let limit = params.pagination.clamped_limit();
    let cursor = params.pagination.cursor_uuid();

    if params.joined.unwrap_or(false) {
        let channels =
            db::channels::list_for_user(&state.db, auth.user_id, cursor, limit + 1).await?;
        let has_more = channels.len() as i64 > limit;
        let items: Vec<_> = channels
            .iter()
            .take(limit as usize)
            .map(channel_with_unread_to_response)
            .collect();
        let cursor = if has_more {
            items.last().map(|c| c.id.clone())
        } else {
            None
        };
        Ok(Json(PaginatedResponse { items, cursor }))
    } else {
        let channels = db::channels::list_public(&state.db, cursor, limit + 1).await?;
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

fn parse_user_id(user_id: &str) -> Result<Uuid, ApiError> {
    burst_core::id::parse_prefixed_id(user_id, "usr_")
        .or_else(|| Uuid::parse_str(user_id).ok())
        .ok_or_else(|| ApiError::BadRequest("invalid user ID".into()))
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

async fn mark_read(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(channel_id): Path<String>,
) -> Result<axum::http::StatusCode, ApiError> {
    let id = parse_channel_id(&channel_id)?;
    db::channels::update_last_read(&state.db, id, auth.user_id).await?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

// ── DM handler ──

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateDmRequest {
    pub user_id: String,
}

async fn create_or_get_dm(
    auth: AuthUser,
    State(state): State<AppState>,
    Json(body): Json<CreateDmRequest>,
) -> Result<(axum::http::StatusCode, Json<ChannelResponse>), ApiError> {
    let target_id = parse_user_id(&body.user_id)?;

    if target_id == auth.user_id {
        return Err(ApiError::BadRequest("cannot DM yourself".into()));
    }

    // Verify target user exists
    db::users::find_by_id(&state.db, target_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("User".into()))?;

    let new_id = burst_core::id::new_id();
    let channel =
        db::channels::find_or_create_dm(&state.db, auth.user_id, target_id, new_id).await?;

    // Notify both users' open WS connections so they update their channel membership
    // set and can receive real-time events on this channel immediately.
    let ch_id_str = burst_core::id::format_channel_id(channel.id);
    for uid in [auth.user_id, target_id] {
        let ev = crate::ws::ServerEvent::ChannelJoined {
            event_id: burst_core::id::new_id().to_string(),
            channel_id: ch_id_str.clone(),
            user_id: burst_core::id::format_user_id(uid),
        };
        crate::ws::handler::push_and_broadcast(&state.broker, &state.event_buffer, ev).await;
    }

    Ok((
        axum::http::StatusCode::OK,
        Json(channel_to_response(&channel)),
    ))
}

// ── Message handlers ──

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendMessageRequest {
    pub content: String,
    pub thread_id: Option<String>,
}

async fn send_message(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(channel_id): Path<String>,
    headers: HeaderMap,
    body: axum::body::Body,
) -> Result<(axum::http::StatusCode, Json<MessageResponse>), ApiError> {
    let ch_id = parse_channel_id(&channel_id)?;

    if !db::channels::is_member(&state.db, ch_id, auth.user_id).await? {
        return Err(ApiError::Forbidden);
    }

    // Reject writes to archived/readonly channels.
    let channel = db::channels::find_by_id(&state.db, ch_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("Channel".into()))?;
    if channel.is_readonly {
        return Err(ApiError::Forbidden);
    }

    let content_type = headers
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("application/json");

    let (content, thread_id_str, files) = if content_type.starts_with("multipart/form-data") {
        parse_multipart_message(body, headers, &state).await?
    } else {
        let bytes = axum::body::to_bytes(body, 1024 * 1024)
            .await
            .map_err(|_| ApiError::BadRequest("invalid request body".into()))?;
        let req: SendMessageRequest =
            serde_json::from_slice(&bytes).map_err(|e| ApiError::BadRequest(e.to_string()))?;
        (req.content, req.thread_id, vec![])
    };

    let content = content.trim().to_string();
    if content.is_empty() && files.is_empty() {
        return Err(ApiError::BadRequest(
            "message content cannot be empty".into(),
        ));
    }

    // Resolve optional thread_id and verify it belongs to this channel
    let thread_id = if let Some(ref tid_str) = thread_id_str {
        let tid = parse_message_id(tid_str)?;
        let parent = db::messages::find_by_id(&state.db, tid)
            .await?
            .ok_or_else(|| ApiError::NotFound("Thread message".into()))?;
        if parent.channel_id != ch_id {
            return Err(ApiError::BadRequest(
                "thread message belongs to a different channel".into(),
            ));
        }
        Some(parent.thread_id.unwrap_or(parent.id))
    } else {
        None
    };

    // Use a placeholder content for file-only messages.
    let msg_content = if content.is_empty() { " " } else { &content };

    let id = burst_core::id::new_id();
    let message =
        db::messages::create(&state.db, id, ch_id, auth.user_id, thread_id, msg_content).await?;

    // Process file uploads.
    let mut attachment_responses = Vec::new();
    for (file_name, file_data, file_ct) in &files {
        let att_id = burst_core::id::new_id();
        let now = chrono::Utc::now();
        let storage_key = format!("{}/{}/{}/{}", ch_id, now.format("%Y/%m"), att_id, file_name);

        state
            .storage
            .put(&storage_key, file_data.clone(), file_ct)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?;

        // Extract image dimensions if applicable.
        let metadata = if file_ct.starts_with("image/") {
            extract_image_metadata(file_data)
        } else {
            serde_json::json!({})
        };

        let att = db::attachments::create(
            &state.db,
            &db::attachments::CreateAttachment {
                id: att_id,
                message_id: id,
                file_name: file_name.clone(),
                file_size: file_data.len() as i64,
                content_type: file_ct.clone(),
                storage_key,
                metadata,
            },
        )
        .await?;

        attachment_responses.push(attachment_to_response(&att));
    }

    // Parse @mentions and persist them.
    let mentioned_usernames: Vec<String> = parse_mentions(&content);
    if !mentioned_usernames.is_empty() {
        let mention_ids = db::users::find_ids_by_usernames(&state.db, &mentioned_usernames).await?;
        db::mentions::insert_mentions(&state.db, id, &mention_ids).await?;
    }

    let response = build_message_response(&message, 0, vec![], attachment_responses);
    let ev = crate::ws::ServerEvent::MessageCreated {
        event_id: burst_core::id::new_id().to_string(),
        channel_id: burst_core::id::format_channel_id(ch_id),
        message: response.clone(),
    };
    crate::ws::handler::push_and_broadcast(&state.broker, &state.event_buffer, ev).await;

    Ok((axum::http::StatusCode::CREATED, Json(response)))
}

/// Parse a multipart/form-data request into (content, thread_id, files).
async fn parse_multipart_message(
    body: axum::body::Body,
    headers: HeaderMap,
    state: &AppState,
) -> Result<(String, Option<String>, Vec<(String, Bytes, String)>), ApiError> {
    let boundary = headers
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .and_then(|ct| multer::parse_boundary(ct).ok())
        .ok_or_else(|| ApiError::BadRequest("missing multipart boundary".into()))?;

    let stream = body.into_data_stream();
    let mut multipart = multer::Multipart::new(stream, boundary);

    let storage_config = &state.config.storage;
    let mut content = String::new();
    let mut thread_id = None;
    let mut files: Vec<(String, Bytes, String)> = Vec::new();

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| ApiError::BadRequest(format!("multipart error: {e}")))?
    {
        let field_name = field.name().unwrap_or("").to_string();
        match field_name.as_str() {
            "content" => {
                content = field
                    .text()
                    .await
                    .map_err(|e| ApiError::BadRequest(format!("invalid content field: {e}")))?;
            }
            "threadId" => {
                thread_id =
                    Some(field.text().await.map_err(|e| {
                        ApiError::BadRequest(format!("invalid threadId field: {e}"))
                    })?);
            }
            "files" => {
                if files.len() >= storage_config.max_files_per_message {
                    return Err(ApiError::BadRequest(format!(
                        "maximum {} files per message",
                        storage_config.max_files_per_message
                    )));
                }

                let file_name = field.file_name().unwrap_or("unnamed").to_string();

                // Validate extension.
                if let Some(ext) = StdPath::new(&file_name)
                    .extension()
                    .and_then(|e| e.to_str())
                    && storage_config
                        .blocked_extensions
                        .iter()
                        .any(|b| b.eq_ignore_ascii_case(ext))
                {
                    return Err(ApiError::BadRequest(format!(
                        "file extension .{ext} is not allowed"
                    )));
                }

                let file_ct = field
                    .content_type()
                    .map(|ct| ct.to_string())
                    .unwrap_or_else(|| {
                        mime_guess::from_path(&file_name)
                            .first_or_octet_stream()
                            .to_string()
                    });

                let data = field
                    .bytes()
                    .await
                    .map_err(|e| ApiError::BadRequest(format!("failed to read file: {e}")))?;

                if data.len() as u64 > storage_config.max_file_size {
                    return Err(ApiError::PayloadTooLarge(format!(
                        "file exceeds maximum size of {} bytes",
                        storage_config.max_file_size
                    )));
                }

                files.push((file_name, data, file_ct));
            }
            _ => {} // ignore unknown fields
        }
    }

    Ok((content, thread_id, files))
}

fn extract_image_metadata(data: &Bytes) -> serde_json::Value {
    let cursor = std::io::Cursor::new(data.as_ref());
    match image::ImageReader::new(cursor)
        .with_guessed_format()
        .ok()
        .and_then(|r| r.into_dimensions().ok())
    {
        Some((width, height)) => serde_json::json!({ "width": width, "height": height }),
        None => serde_json::json!({}),
    }
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
        db::messages::list_in_channel(&state.db, ch_id, params.cursor_uuid(), limit + 1).await?;

    let has_more = messages.len() as i64 > limit;
    let messages: Vec<_> = messages.into_iter().take(limit as usize).collect();

    let message_ids: Vec<Uuid> = messages.iter().map(|m| m.id).collect();
    let reply_count_map: std::collections::HashMap<Uuid, i64> =
        db::messages::reply_counts(&state.db, &message_ids)
            .await?
            .into_iter()
            .collect();
    let reaction_rows = db::reactions::list_for_messages(&state.db, &message_ids).await?;
    let attachment_rows = db::attachments::list_for_messages(&state.db, &message_ids).await?;

    let items: Vec<_> = messages
        .iter()
        .map(|m| {
            let rc = *reply_count_map.get(&m.id).unwrap_or(&0);
            let reactions = aggregate_reactions(&reaction_rows, m.id);
            let attachments = group_attachments(&attachment_rows, m.id);
            build_message_response(m, rc, reactions, attachments)
        })
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

    let message_ids = [message.id];
    let reply_count_map: std::collections::HashMap<Uuid, i64> =
        db::messages::reply_counts(&state.db, &message_ids)
            .await?
            .into_iter()
            .collect();
    let reaction_rows = db::reactions::list_for_messages(&state.db, &message_ids).await?;
    let attachment_rows = db::attachments::list_for_messages(&state.db, &message_ids).await?;
    let rc = *reply_count_map.get(&message.id).unwrap_or(&0);
    let reactions = aggregate_reactions(&reaction_rows, message.id);
    let attachments = group_attachments(&attachment_rows, message.id);

    Ok(Json(build_message_response(
        &message,
        rc,
        reactions,
        attachments,
    )))
}

async fn list_thread_replies(
    auth: AuthUser,
    State(state): State<AppState>,
    Path((channel_id, message_id)): Path<(String, String)>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<PaginatedResponse<MessageResponse>>, ApiError> {
    let ch_id = parse_channel_id(&channel_id)?;

    if !db::channels::is_member(&state.db, ch_id, auth.user_id).await? {
        return Err(ApiError::Forbidden);
    }

    let thread_id = parse_message_id(&message_id)?;
    let limit = params.clamped_limit();
    let messages =
        db::messages::list_in_thread(&state.db, thread_id, params.cursor_uuid(), limit + 1).await?;

    let has_more = messages.len() as i64 > limit;
    let messages: Vec<_> = messages.into_iter().take(limit as usize).collect();

    let message_ids: Vec<Uuid> = messages.iter().map(|m| m.id).collect();
    let reaction_rows = db::reactions::list_for_messages(&state.db, &message_ids).await?;
    let attachment_rows = db::attachments::list_for_messages(&state.db, &message_ids).await?;

    let items: Vec<_> = messages
        .iter()
        .map(|m| {
            let reactions = aggregate_reactions(&reaction_rows, m.id);
            let attachments = group_attachments(&attachment_rows, m.id);
            build_message_response(m, 0, reactions, attachments)
        })
        .collect();

    let cursor = if has_more {
        items.last().map(|m| m.id.clone())
    } else {
        None
    };

    Ok(Json(PaginatedResponse { items, cursor }))
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

    let channel = db::channels::find_by_id(&state.db, ch_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("Channel".into()))?;
    if channel.is_readonly {
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

    let attachment_rows = db::attachments::list_for_messages(&state.db, &[message.id]).await?;
    let attachments = group_attachments(&attachment_rows, message.id);
    let response = build_message_response(&message, 0, vec![], attachments);
    let ev = crate::ws::ServerEvent::MessageUpdated {
        event_id: burst_core::id::new_id().to_string(),
        channel_id: burst_core::id::format_channel_id(ch_id),
        message: response.clone(),
    };
    crate::ws::handler::push_and_broadcast(&state.broker, &state.event_buffer, ev).await;

    Ok(Json(response))
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
    let deleted_msg = db::messages::soft_delete(&state.db, msg_id, auth.user_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("Message".into()))?;

    let ev = crate::ws::ServerEvent::MessageDeleted {
        event_id: burst_core::id::new_id().to_string(),
        channel_id: burst_core::id::format_channel_id(ch_id),
        message_id: burst_core::id::format_message_id(deleted_msg.id),
    };
    crate::ws::handler::push_and_broadcast(&state.broker, &state.event_buffer, ev).await;

    Ok(axum::http::StatusCode::NO_CONTENT)
}

// ── Reaction handlers ──

async fn add_reaction(
    auth: AuthUser,
    State(state): State<AppState>,
    Path((channel_id, message_id, emoji)): Path<(String, String, String)>,
) -> Result<axum::http::StatusCode, ApiError> {
    let ch_id = parse_channel_id(&channel_id)?;

    if !db::channels::is_member(&state.db, ch_id, auth.user_id).await? {
        return Err(ApiError::Forbidden);
    }

    let msg_id = parse_message_id(&message_id)?;

    // Verify message exists in this channel
    let message = db::messages::find_by_id(&state.db, msg_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("Message".into()))?;
    if message.channel_id != ch_id {
        return Err(ApiError::NotFound("Message".into()));
    }

    let is_new = db::reactions::add(&state.db, msg_id, auth.user_id, &emoji).await?;
    if is_new {
        let ev = crate::ws::ServerEvent::ReactionAdded {
            event_id: burst_core::id::new_id().to_string(),
            channel_id: burst_core::id::format_channel_id(ch_id),
            message_id: burst_core::id::format_message_id(msg_id),
            emoji,
            user_id: burst_core::id::format_user_id(auth.user_id),
        };
        crate::ws::handler::push_and_broadcast(&state.broker, &state.event_buffer, ev).await;
    }

    Ok(axum::http::StatusCode::NO_CONTENT)
}

async fn remove_reaction(
    auth: AuthUser,
    State(state): State<AppState>,
    Path((channel_id, message_id, emoji)): Path<(String, String, String)>,
) -> Result<axum::http::StatusCode, ApiError> {
    let ch_id = parse_channel_id(&channel_id)?;

    if !db::channels::is_member(&state.db, ch_id, auth.user_id).await? {
        return Err(ApiError::Forbidden);
    }

    let msg_id = parse_message_id(&message_id)?;
    let removed = db::reactions::remove(&state.db, msg_id, auth.user_id, &emoji).await?;
    if removed {
        let ev = crate::ws::ServerEvent::ReactionRemoved {
            event_id: burst_core::id::new_id().to_string(),
            channel_id: burst_core::id::format_channel_id(ch_id),
            message_id: burst_core::id::format_message_id(msg_id),
            emoji,
            user_id: burst_core::id::format_user_id(auth.user_id),
        };
        crate::ws::handler::push_and_broadcast(&state.broker, &state.event_buffer, ev).await;
    }

    Ok(axum::http::StatusCode::NO_CONTENT)
}

// ── Pin handlers ──

async fn pin_message(
    auth: AuthUser,
    State(state): State<AppState>,
    Path((channel_id, message_id)): Path<(String, String)>,
) -> Result<axum::http::StatusCode, ApiError> {
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

    let is_new = db::pinned_messages::pin(&state.db, ch_id, msg_id, auth.user_id).await?;
    if is_new {
        let ev = crate::ws::ServerEvent::MessagePinned {
            event_id: burst_core::id::new_id().to_string(),
            channel_id: burst_core::id::format_channel_id(ch_id),
            message_id: burst_core::id::format_message_id(msg_id),
            user_id: burst_core::id::format_user_id(auth.user_id),
        };
        crate::ws::handler::push_and_broadcast(&state.broker, &state.event_buffer, ev).await;
    }

    Ok(axum::http::StatusCode::NO_CONTENT)
}

async fn unpin_message(
    auth: AuthUser,
    State(state): State<AppState>,
    Path((channel_id, message_id)): Path<(String, String)>,
) -> Result<axum::http::StatusCode, ApiError> {
    let ch_id = parse_channel_id(&channel_id)?;

    if !db::channels::is_member(&state.db, ch_id, auth.user_id).await? {
        return Err(ApiError::Forbidden);
    }

    let msg_id = parse_message_id(&message_id)?;
    let removed = db::pinned_messages::unpin(&state.db, ch_id, msg_id).await?;
    if removed {
        let ev = crate::ws::ServerEvent::MessageUnpinned {
            event_id: burst_core::id::new_id().to_string(),
            channel_id: burst_core::id::format_channel_id(ch_id),
            message_id: burst_core::id::format_message_id(msg_id),
        };
        crate::ws::handler::push_and_broadcast(&state.broker, &state.event_buffer, ev).await;
    }

    Ok(axum::http::StatusCode::NO_CONTENT)
}

async fn list_pins(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(channel_id): Path<String>,
) -> Result<Json<Vec<MessageResponse>>, ApiError> {
    let ch_id = parse_channel_id(&channel_id)?;

    if !db::channels::is_member(&state.db, ch_id, auth.user_id).await? {
        return Err(ApiError::Forbidden);
    }

    let pins = db::pinned_messages::list_for_channel(&state.db, ch_id).await?;
    let message_ids: Vec<Uuid> = pins.iter().map(|p| p.message_id).collect();

    if message_ids.is_empty() {
        return Ok(Json(vec![]));
    }

    // Batch-fetch all pinned message data.
    let mut messages = Vec::new();
    for &mid in &message_ids {
        if let Some(msg) = db::messages::find_by_id(&state.db, mid).await? {
            messages.push(msg);
        }
    }

    let reaction_rows = db::reactions::list_for_messages(&state.db, &message_ids).await?;
    let attachment_rows = db::attachments::list_for_messages(&state.db, &message_ids).await?;
    let reply_counts: std::collections::HashMap<Uuid, i64> =
        db::messages::reply_counts(&state.db, &message_ids)
            .await?
            .into_iter()
            .collect();

    let items: Vec<_> = messages
        .iter()
        .map(|m| {
            let rc = *reply_counts.get(&m.id).unwrap_or(&0);
            let reactions = aggregate_reactions(&reaction_rows, m.id);
            let attachments = group_attachments(&attachment_rows, m.id);
            build_message_response(m, rc, reactions, attachments)
        })
        .collect();

    Ok(Json(items))
}

// ── Archive handlers ──

async fn archive_channel(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(channel_id): Path<String>,
) -> Result<Json<ChannelResponse>, ApiError> {
    let ch_id = parse_channel_id(&channel_id)?;

    // Only channel owners or instance admins can archive.
    let member = db::channels::list_members(&state.db, ch_id)
        .await?
        .into_iter()
        .find(|m| m.user_id == auth.user_id);

    let is_channel_owner = member.as_ref().is_some_and(|m| m.role == "owner");
    let is_admin = auth.user_role == "admin";
    if !is_channel_owner && !is_admin {
        return Err(ApiError::Forbidden);
    }

    let channel = db::channels::archive(&state.db, ch_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("Channel".into()))?;

    let ev = crate::ws::ServerEvent::ChannelUpdated {
        event_id: burst_core::id::new_id().to_string(),
        channel_id: burst_core::id::format_channel_id(ch_id),
    };
    crate::ws::handler::push_and_broadcast(&state.broker, &state.event_buffer, ev).await;

    Ok(Json(channel_to_response(&channel)))
}

async fn unarchive_channel(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(channel_id): Path<String>,
) -> Result<Json<ChannelResponse>, ApiError> {
    let ch_id = parse_channel_id(&channel_id)?;

    let member = db::channels::list_members(&state.db, ch_id)
        .await?
        .into_iter()
        .find(|m| m.user_id == auth.user_id);

    let is_channel_owner = member.as_ref().is_some_and(|m| m.role == "owner");
    let is_admin = auth.user_role == "admin";
    if !is_channel_owner && !is_admin {
        return Err(ApiError::Forbidden);
    }

    let channel = db::channels::unarchive(&state.db, ch_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("Channel".into()))?;

    let ev = crate::ws::ServerEvent::ChannelUpdated {
        event_id: burst_core::id::new_id().to_string(),
        channel_id: burst_core::id::format_channel_id(ch_id),
    };
    crate::ws::handler::push_and_broadcast(&state.broker, &state.event_buffer, ev).await;

    Ok(Json(channel_to_response(&channel)))
}

// ── Notification preference handler ──

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateNotifyRequest {
    pub notify: String,
}

async fn update_notify(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(channel_id): Path<String>,
    Json(body): Json<UpdateNotifyRequest>,
) -> Result<axum::http::StatusCode, ApiError> {
    let ch_id = parse_channel_id(&channel_id)?;

    if !matches!(body.notify.as_str(), "all" | "mentions" | "nothing") {
        return Err(ApiError::BadRequest(
            "notify must be 'all', 'mentions', or 'nothing'".into(),
        ));
    }

    db::channels::update_notify(&state.db, ch_id, auth.user_id, &body.notify)
        .await?
        .ok_or_else(|| ApiError::NotFound("Channel membership".into()))?;

    Ok(axum::http::StatusCode::NO_CONTENT)
}

fn slugify(name: &str) -> String {
    name.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

/// Extracts unique @username mentions from message content.
fn parse_mentions(content: &str) -> Vec<String> {
    let mut usernames = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let chars: Vec<char> = content.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '@' && (i == 0 || !chars[i - 1].is_alphanumeric()) {
            i += 1;
            let start = i;
            while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            if i > start {
                let username: String = chars[start..i].iter().collect();
                if seen.insert(username.clone()) {
                    usernames.push(username);
                }
            }
        } else {
            i += 1;
        }
    }
    usernames
}
