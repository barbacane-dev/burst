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
use crate::services;
use burst_core::models::channel::ChannelMemberRole;
use burst_core::permissions::{Action, Actor, InstanceRole};

/// The caller as the permission matrix sees them in `channel_id`.
async fn actor_in(state: &AppState, auth: &AuthUser, channel_id: Uuid) -> Result<Actor, ApiError> {
    let member = db::channels::find_member(&state.db, channel_id, auth.user_id).await?;
    Ok(Actor {
        instance: InstanceRole::parse(&auth.user_role),
        // A stored role the enum does not know is still a membership.
        channel: member.map(|m| m.role.parse().unwrap_or(ChannelMemberRole::Member)),
    })
}

/// The caller for decisions that involve no particular channel.
fn instance_actor(auth: &AuthUser) -> Actor {
    Actor {
        instance: InstanceRole::parse(&auth.user_role),
        channel: None,
    }
}

fn require(actor: &Actor, action: Action) -> Result<(), ApiError> {
    if actor.can(action) {
        Ok(())
    } else {
        Err(ApiError::Forbidden)
    }
}

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
            "/channels/{channel_id}/members/{user_id}",
            delete(remove_member).patch(update_member_role),
        )
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

fn member_to_response(m: &db::channels::ChannelMemberRow) -> ChannelMemberResponse {
    ChannelMemberResponse {
        user_id: burst_core::id::format_user_id(m.user_id),
        role: m.role.clone(),
        joined_at: m.joined_at.to_rfc3339(),
    }
}

/// Adds someone to a channel. Without `userId`, or with the caller's own, the
/// caller joins a public channel themselves.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddMemberRequest {
    pub user_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateMemberRoleRequest {
    pub role: String,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReactionResponse {
    pub emoji: String,
    pub count: i64,
    pub user_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

/// Builds a single `MessageResponse` with no reactions/attachments/replies.
/// Used for freshly created or edited messages where metadata is already known.
pub fn build_message_response_simple(
    row: &db::messages::MessageRow,
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
        reply_count: 0,
        reactions: vec![],
        attachments,
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
    require(&instance_actor(&auth), Action::CreateChannel)?;
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
        // A guest finds channels only by being added to them.
        if !instance_actor(&auth).can(Action::BrowsePublicChannels) {
            return Ok(Json(PaginatedResponse {
                items: vec![],
                cursor: None,
            }));
        }
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

fn parse_channel_id(s: &str) -> Result<Uuid, ApiError> {
    services::parse_id(s, "ch_", "channel")
}

fn parse_message_id(s: &str) -> Result<Uuid, ApiError> {
    services::parse_id(s, "msg_", "message")
}

fn parse_user_id(s: &str) -> Result<Uuid, ApiError> {
    services::parse_id(s, "usr_", "user")
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

    // A channel the caller may not see is reported missing, not forbidden, so
    // its existence does not leak.
    let actor = actor_in(&state, &auth, id).await?;
    let visible = actor.channel.is_some()
        || (channel.kind == "public" && actor.can(Action::ViewPublicChannel));
    if !visible {
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
    services::require_membership(&state.db, id, auth.user_id).await?;
    require(&actor_in(&state, &auth, id).await?, Action::EditChannel)?;

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
    // Absent, `null` or without `userId`: the caller joins. Clients have sent
    // both of the first two, so both keep meaning that.
    body: Option<Json<Option<AddMemberRequest>>>,
) -> Result<axum::http::StatusCode, ApiError> {
    let id = parse_channel_id(&channel_id)?;

    let channel = db::channels::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| ApiError::NotFound("Channel".into()))?;

    let target = match body.and_then(|Json(b)| b).and_then(|b| b.user_id) {
        Some(raw) => Some(parse_user_id(&raw)?),
        None => None,
    }
    .filter(|user| *user != auth.user_id);

    let joining = match target {
        // Joining yourself: only a public channel, and not as a guest.
        None => {
            if channel.kind != "public" {
                return Err(ApiError::Forbidden);
            }
            require(&instance_actor(&auth), Action::JoinPublicChannel)?;
            auth.user_id
        }
        // Adding someone else: the way into a private channel, and the only
        // way a guest enters any channel. Direct messages keep their members.
        Some(user) => {
            if !matches!(channel.kind.as_str(), "public" | "private") {
                return Err(ApiError::BadRequest(
                    "members are added only to public and private channels".into(),
                ));
            }
            require(&actor_in(&state, &auth, id).await?, Action::AddMember)?;
            let added = db::users::find_by_id(&state.db, user)
                .await?
                .filter(|u| u.deactivated_at.is_none())
                .ok_or_else(|| ApiError::NotFound("User".into()))?;
            added.id
        }
    };

    db::channels::add_member(&state.db, id, joining, "member").await?;
    // The new member's open sockets start forwarding this channel on this.
    services::broadcast(
        &state,
        crate::ws::ServerEvent::ChannelJoined {
            event_id: burst_core::id::new_id().to_string(),
            channel_id: burst_core::id::format_channel_id(id),
            user_id: burst_core::id::format_user_id(joining),
        },
    )
    .await;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

/// Announces that `user` is no longer a member of `channel`.
async fn announce_left(state: &AppState, channel: Uuid, user: Uuid) {
    services::broadcast(
        state,
        crate::ws::ServerEvent::ChannelLeft {
            event_id: burst_core::id::new_id().to_string(),
            channel_id: burst_core::id::format_channel_id(channel),
            user_id: burst_core::id::format_user_id(user),
        },
    )
    .await;
}

/// Removes another member. Leaving is `leave_channel`.
async fn remove_member(
    auth: AuthUser,
    State(state): State<AppState>,
    Path((channel_id, user_id)): Path<(String, String)>,
) -> Result<axum::http::StatusCode, ApiError> {
    let id = parse_channel_id(&channel_id)?;
    let user = parse_user_id(&user_id)?;
    if user == auth.user_id {
        return Err(ApiError::BadRequest(
            "use DELETE /channels/{channelId}/members/me to leave".into(),
        ));
    }
    let actor = actor_in(&state, &auth, id).await?;
    let target = db::channels::find_member(&state.db, id, user)
        .await?
        .ok_or_else(|| ApiError::NotFound("Member".into()))?;
    let target_role = target.role.parse().unwrap_or(ChannelMemberRole::Member);
    if !actor.can_remove(target_role) {
        return Err(ApiError::Forbidden);
    }

    db::channels::remove_member(&state.db, id, user).await?;
    announce_left(&state, id, user).await;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

/// Makes a member a moderator of the channel, or a member again.
async fn update_member_role(
    auth: AuthUser,
    State(state): State<AppState>,
    Path((channel_id, user_id)): Path<(String, String)>,
    Json(body): Json<UpdateMemberRoleRequest>,
) -> Result<Json<ChannelMemberResponse>, ApiError> {
    let id = parse_channel_id(&channel_id)?;
    let user = parse_user_id(&user_id)?;
    let new_role: ChannelMemberRole = body
        .role
        .parse()
        .ok()
        .filter(|r| *r != ChannelMemberRole::Owner)
        .ok_or_else(|| ApiError::BadRequest("role must be 'moderator' or 'member'".into()))?;

    let actor = actor_in(&state, &auth, id).await?;
    let target = db::channels::find_member(&state.db, id, user)
        .await?
        .ok_or_else(|| ApiError::NotFound("Member".into()))?;
    let current = target.role.parse().unwrap_or(ChannelMemberRole::Member);
    if !actor.can_set_role(current, new_role) {
        return Err(ApiError::Forbidden);
    }
    // A guest reads and sends; the matrix would ignore the role anyway, and
    // storing it would suggest otherwise.
    if new_role == ChannelMemberRole::Moderator {
        let is_guest = db::users::find_by_id(&state.db, user)
            .await?
            .is_some_and(|u| InstanceRole::parse(&u.role) == InstanceRole::Guest);
        if is_guest {
            return Err(ApiError::BadRequest(
                "a guest cannot moderate a channel".into(),
            ));
        }
    }

    let updated = db::channels::update_member_role(&state.db, id, user, new_role.as_str())
        .await?
        .ok_or_else(|| ApiError::NotFound("Member".into()))?;
    services::broadcast(
        &state,
        crate::ws::ServerEvent::ChannelUpdated {
            event_id: burst_core::id::new_id().to_string(),
            channel_id: burst_core::id::format_channel_id(id),
        },
    )
    .await;
    Ok(Json(member_to_response(&updated)))
}

async fn leave_channel(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(channel_id): Path<String>,
) -> Result<axum::http::StatusCode, ApiError> {
    let id = parse_channel_id(&channel_id)?;
    if db::channels::remove_member(&state.db, id, auth.user_id).await? {
        announce_left(&state, id, auth.user_id).await;
    }
    Ok(axum::http::StatusCode::NO_CONTENT)
}

async fn list_members(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(channel_id): Path<String>,
) -> Result<Json<PaginatedResponse<ChannelMemberResponse>>, ApiError> {
    let id = parse_channel_id(&channel_id)?;
    services::require_membership(&state.db, id, auth.user_id).await?;

    let members = db::channels::list_members(&state.db, id).await?;
    let items = members.iter().map(member_to_response).collect();
    Ok(Json(PaginatedResponse {
        items,
        cursor: None,
    }))
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
    /// Single user ID (1-to-1 DM). Kept for backward compatibility.
    pub user_id: Option<String>,
    /// Multiple user IDs (group DM). If len == 1, treated as 1-to-1 DM.
    pub user_ids: Option<Vec<String>>,
}

async fn create_or_get_dm(
    auth: AuthUser,
    State(state): State<AppState>,
    Json(body): Json<CreateDmRequest>,
) -> Result<(axum::http::StatusCode, Json<ChannelResponse>), ApiError> {
    require(&instance_actor(&auth), Action::StartDirectMessage)?;
    // Resolve target user IDs from either field
    let raw_ids = match (body.user_ids, body.user_id) {
        (Some(ids), _) => ids,
        (None, Some(id)) => vec![id],
        (None, None) => {
            return Err(ApiError::BadRequest(
                "either userId or userIds is required".into(),
            ));
        }
    };

    let mut target_ids = Vec::with_capacity(raw_ids.len());
    for raw in &raw_ids {
        let id = parse_user_id(raw)?;
        if id == auth.user_id {
            return Err(ApiError::BadRequest("cannot DM yourself".into()));
        }
        // Verify user exists
        db::users::find_by_id(&state.db, id)
            .await?
            .ok_or_else(|| ApiError::NotFound("User".into()))?;
        target_ids.push(id);
    }

    if target_ids.is_empty() {
        return Err(ApiError::BadRequest("at least one user is required".into()));
    }

    let new_id = burst_core::id::new_id();
    let channel = if target_ids.len() == 1 {
        // 1-to-1 DM
        db::channels::find_or_create_dm(&state.db, auth.user_id, target_ids[0], new_id).await?
    } else {
        // Group DM
        let mut all_members = target_ids.clone();
        all_members.push(auth.user_id);
        db::channels::find_or_create_group_dm(&state.db, &all_members, auth.user_id, new_id).await?
    };

    // Notify all participants' open WS connections
    let ch_id_str = burst_core::id::format_channel_id(channel.id);
    let mut all_users = target_ids;
    all_users.push(auth.user_id);
    for uid in all_users {
        let ev = crate::ws::ServerEvent::ChannelJoined {
            event_id: burst_core::id::new_id().to_string(),
            channel_id: ch_id_str.clone(),
            user_id: burst_core::id::format_user_id(uid),
        };
        services::broadcast(&state, ev).await;
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
    services::require_membership(&state.db, ch_id, auth.user_id).await?;

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

    let mentioned = persist_mentions(&state, &message).await?;

    let response = build_message_response_simple(&message, attachment_responses);
    let ev = crate::ws::ServerEvent::MessageCreated {
        event_id: burst_core::id::new_id().to_string(),
        channel_id: burst_core::id::format_channel_id(ch_id),
        message: response.clone(),
    };
    services::broadcast(&state, ev).await;
    crate::metrics::message_created();
    services::notifications::notify_new_message(&state, ch_id, &message, &mentioned).await;

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
    services::require_membership(&state.db, ch_id, auth.user_id).await?;

    let limit = params.clamped_limit();
    let messages =
        db::messages::list_in_channel(&state.db, ch_id, params.cursor_uuid(), limit + 1).await?;

    let has_more = messages.len() as i64 > limit;
    let messages: Vec<_> = messages.into_iter().take(limit as usize).collect();
    let items = services::messages::enrich(&state.db, &messages).await?;

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
    services::require_membership(&state.db, ch_id, auth.user_id).await?;

    let msg_id = parse_message_id(&message_id)?;
    let message = db::messages::find_by_id(&state.db, msg_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("Message".into()))?;

    if message.channel_id != ch_id {
        return Err(ApiError::NotFound("Message".into()));
    }

    Ok(Json(
        services::messages::enrich_one(&state.db, &message).await?,
    ))
}

async fn list_thread_replies(
    auth: AuthUser,
    State(state): State<AppState>,
    Path((channel_id, message_id)): Path<(String, String)>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<PaginatedResponse<MessageResponse>>, ApiError> {
    let ch_id = parse_channel_id(&channel_id)?;
    services::require_membership(&state.db, ch_id, auth.user_id).await?;

    let thread_id = parse_message_id(&message_id)?;
    let limit = params.clamped_limit();
    let messages =
        db::messages::list_in_thread(&state.db, thread_id, params.cursor_uuid(), limit + 1).await?;

    let has_more = messages.len() as i64 > limit;
    let messages: Vec<_> = messages.into_iter().take(limit as usize).collect();
    let items = services::messages::enrich(&state.db, &messages).await?;

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
    services::require_membership(&state.db, ch_id, auth.user_id).await?;

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

    let response = services::messages::enrich_one(&state.db, &message).await?;
    let ev = crate::ws::ServerEvent::MessageUpdated {
        event_id: burst_core::id::new_id().to_string(),
        channel_id: burst_core::id::format_channel_id(ch_id),
        message: response.clone(),
    };
    services::broadcast(&state, ev).await;

    Ok(Json(response))
}

async fn delete_message(
    auth: AuthUser,
    State(state): State<AppState>,
    Path((channel_id, message_id)): Path<(String, String)>,
) -> Result<axum::http::StatusCode, ApiError> {
    let ch_id = parse_channel_id(&channel_id)?;
    services::require_membership(&state.db, ch_id, auth.user_id).await?;

    let msg_id = parse_message_id(&message_id)?;
    let deleted_msg = match db::messages::soft_delete(&state.db, msg_id, auth.user_id).await? {
        Some(msg) => msg,
        // Not the author's own. A moderator may still remove it, and that is
        // recorded, since it removes someone else's words.
        None if actor_in(&state, &auth, ch_id)
            .await?
            .can(Action::DeleteOthersMessage) =>
        {
            let msg = db::messages::soft_delete_in_channel(&state.db, msg_id, ch_id)
                .await?
                .ok_or_else(|| ApiError::NotFound("Message".into()))?;
            db::audit_log::insert(
                &state.db,
                burst_core::id::new_id(),
                auth.user_id,
                "message.deleted_by_moderator",
                "message",
                msg.id,
                Some(serde_json::json!({
                    "channelId": burst_core::id::format_channel_id(ch_id),
                    "authorId": burst_core::id::format_user_id(msg.user_id),
                })),
            )
            .await?;
            msg
        }
        None => return Err(ApiError::NotFound("Message".into())),
    };

    let ev = crate::ws::ServerEvent::MessageDeleted {
        event_id: burst_core::id::new_id().to_string(),
        channel_id: burst_core::id::format_channel_id(ch_id),
        message_id: burst_core::id::format_message_id(deleted_msg.id),
    };
    services::broadcast(&state, ev).await;

    Ok(axum::http::StatusCode::NO_CONTENT)
}

// ── Reaction handlers ──

async fn add_reaction(
    auth: AuthUser,
    State(state): State<AppState>,
    Path((channel_id, message_id, emoji)): Path<(String, String, String)>,
) -> Result<axum::http::StatusCode, ApiError> {
    let ch_id = parse_channel_id(&channel_id)?;
    services::require_membership(&state.db, ch_id, auth.user_id).await?;

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
        services::broadcast(&state, ev).await;
    }

    Ok(axum::http::StatusCode::NO_CONTENT)
}

async fn remove_reaction(
    auth: AuthUser,
    State(state): State<AppState>,
    Path((channel_id, message_id, emoji)): Path<(String, String, String)>,
) -> Result<axum::http::StatusCode, ApiError> {
    let ch_id = parse_channel_id(&channel_id)?;
    services::require_membership(&state.db, ch_id, auth.user_id).await?;

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
        services::broadcast(&state, ev).await;
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
    services::require_membership(&state.db, ch_id, auth.user_id).await?;
    require(&actor_in(&state, &auth, ch_id).await?, Action::PinMessage)?;

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
        services::broadcast(&state, ev).await;
    }

    Ok(axum::http::StatusCode::NO_CONTENT)
}

async fn unpin_message(
    auth: AuthUser,
    State(state): State<AppState>,
    Path((channel_id, message_id)): Path<(String, String)>,
) -> Result<axum::http::StatusCode, ApiError> {
    let ch_id = parse_channel_id(&channel_id)?;
    services::require_membership(&state.db, ch_id, auth.user_id).await?;
    require(&actor_in(&state, &auth, ch_id).await?, Action::PinMessage)?;

    let msg_id = parse_message_id(&message_id)?;
    let removed = db::pinned_messages::unpin(&state.db, ch_id, msg_id).await?;
    if removed {
        let ev = crate::ws::ServerEvent::MessageUnpinned {
            event_id: burst_core::id::new_id().to_string(),
            channel_id: burst_core::id::format_channel_id(ch_id),
            message_id: burst_core::id::format_message_id(msg_id),
        };
        services::broadcast(&state, ev).await;
    }

    Ok(axum::http::StatusCode::NO_CONTENT)
}

async fn list_pins(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(channel_id): Path<String>,
) -> Result<Json<PaginatedResponse<MessageResponse>>, ApiError> {
    let ch_id = parse_channel_id(&channel_id)?;
    services::require_membership(&state.db, ch_id, auth.user_id).await?;

    let pins = db::pinned_messages::list_for_channel(&state.db, ch_id).await?;
    let message_ids: Vec<Uuid> = pins.iter().map(|p| p.message_id).collect();

    if message_ids.is_empty() {
        return Ok(Json(PaginatedResponse {
            items: vec![],
            cursor: None,
        }));
    }

    let messages = db::messages::find_by_ids(&state.db, &message_ids).await?;
    let items = services::messages::enrich(&state.db, &messages).await?;

    Ok(Json(PaginatedResponse {
        items,
        cursor: None,
    }))
}

// ── Archive handlers ──

async fn archive_channel(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(channel_id): Path<String>,
) -> Result<Json<ChannelResponse>, ApiError> {
    let ch_id = parse_channel_id(&channel_id)?;

    require(
        &actor_in(&state, &auth, ch_id).await?,
        Action::ArchiveChannel,
    )?;

    let channel = db::channels::archive(&state.db, ch_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("Channel".into()))?;

    let ev = crate::ws::ServerEvent::ChannelUpdated {
        event_id: burst_core::id::new_id().to_string(),
        channel_id: burst_core::id::format_channel_id(ch_id),
    };
    services::broadcast(&state, ev).await;

    Ok(Json(channel_to_response(&channel)))
}

async fn unarchive_channel(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(channel_id): Path<String>,
) -> Result<Json<ChannelResponse>, ApiError> {
    let ch_id = parse_channel_id(&channel_id)?;

    require(
        &actor_in(&state, &auth, ch_id).await?,
        Action::ArchiveChannel,
    )?;

    let channel = db::channels::unarchive(&state.db, ch_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("Channel".into()))?;

    let ev = crate::ws::ServerEvent::ChannelUpdated {
        event_id: burst_core::id::new_id().to_string(),
        channel_id: burst_core::id::format_channel_id(ch_id),
    };
    services::broadcast(&state, ev).await;

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

/// Persists who a message mentions and returns their ids.
///
/// `@username` resolves to that user; a name that matches no user is dropped.
/// `@channel` adds every member of the channel and `@here` the members online
/// as the message is sent, so a member offline at the time is mentioned by
/// `@channel` and not by `@here`. The author is never added by a broadcast.
///
/// Online means connected to this node. With the in-process broker that is
/// every connection; behind `pg_notify` a member connected only to another
/// node is not counted, since presence is not shared between nodes.
pub(crate) async fn persist_mentions(
    state: &AppState,
    message: &db::messages::MessageRow,
) -> Result<std::collections::HashSet<Uuid>, ApiError> {
    let parsed = burst_core::mentions::parse(&message.content);
    let mut ids: std::collections::HashSet<Uuid> = if parsed.users.is_empty() {
        std::collections::HashSet::new()
    } else {
        db::users::find_ids_by_usernames(&state.db, &parsed.users)
            .await?
            .into_iter()
            .collect()
    };

    if parsed.is_broadcast() {
        for member in db::channels::list_members(&state.db, message.channel_id).await? {
            if member.user_id == message.user_id {
                continue;
            }
            if parsed.channel || state.presence.is_online(member.user_id).await {
                ids.insert(member.user_id);
            }
        }
    }

    if !ids.is_empty() {
        let list: Vec<Uuid> = ids.iter().copied().collect();
        db::mentions::insert_mentions(&state.db, message.id, &list).await?;
    }
    Ok(ids)
}
