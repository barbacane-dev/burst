use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::routing::{get, post};
use axum::{Json, Router};
use rand::RngExt;
use serde::{Deserialize, Serialize};

use crate::AppState;
use crate::api::PaginatedResponse;
use crate::api::channels::{MessageResponse, build_message_response_simple};
use crate::api::extractors::{IntegrationUser, PaginationParams};
use crate::db;
use crate::error::ApiError;
use crate::services;

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/channels/{channel_id}/webhooks",
            get(list_webhooks).post(create_webhook),
        )
        .route(
            "/channels/{channel_id}/webhooks/{webhook_id}",
            get(get_webhook)
                .patch(update_webhook)
                .delete(delete_webhook),
        )
        .route(
            "/channels/{channel_id}/webhooks/{webhook_id}/token",
            post(regenerate_token),
        )
        .route("/webhooks/{webhook_id}/trigger", post(trigger_incoming))
        .route("/admin/webhooks", get(list_all_webhooks))
}

// ── Response types ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebhookResponse {
    pub id: String,
    pub channel_id: String,
    pub kind: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    pub is_active: bool,
    pub created_by: String,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CreateWebhookResponse {
    #[serde(flatten)]
    webhook: WebhookResponse,
    token: String,
}

fn webhook_to_response(row: &db::webhooks::WebhookRow) -> WebhookResponse {
    WebhookResponse {
        id: burst_core::id::format_webhook_id(row.id),
        channel_id: burst_core::id::format_channel_id(row.channel_id),
        kind: row.kind.clone(),
        name: row.name.clone(),
        url: row.url.clone(),
        is_active: row.is_active,
        created_by: burst_core::id::format_user_id(row.created_by),
        created_at: row.created_at.to_rfc3339(),
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────────

fn generate_token() -> String {
    let bytes: [u8; 32] = rand::rng().random();
    hex::encode(bytes)
}

fn generate_secret() -> String {
    let bytes: [u8; 32] = rand::rng().random();
    hex::encode(bytes)
}

fn parse_channel_id(s: &str) -> Result<uuid::Uuid, ApiError> {
    services::parse_id(s, "ch_", "channel")
}

fn parse_webhook_id(s: &str) -> Result<uuid::Uuid, ApiError> {
    services::parse_id(s, "wh_", "webhook")
}

// ── Channel-scoped CRUD ─────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateWebhookRequest {
    kind: String,
    name: String,
    url: Option<String>,
}

async fn create_webhook(
    integration: IntegrationUser,
    State(state): State<AppState>,
    Path(channel_id): Path<String>,
    Json(body): Json<CreateWebhookRequest>,
) -> Result<(axum::http::StatusCode, Json<CreateWebhookResponse>), ApiError> {
    let ch_id = parse_channel_id(&channel_id)?;
    services::require_membership(&state.db, ch_id, integration.user_id).await?;

    if !matches!(body.kind.as_str(), "incoming" | "outgoing") {
        return Err(ApiError::BadRequest(
            "kind must be incoming or outgoing".into(),
        ));
    }

    if body.kind == "outgoing" && body.url.is_none() {
        return Err(ApiError::BadRequest(
            "url is required for outgoing webhooks".into(),
        ));
    }

    let token = generate_token();
    let secret = if body.kind == "outgoing" {
        Some(generate_secret())
    } else {
        None
    };

    let id = burst_core::id::new_id();
    let row = db::webhooks::create(
        &state.db,
        &db::webhooks::CreateWebhook {
            id,
            channel_id: ch_id,
            kind: &body.kind,
            name: &body.name,
            url: body.url.as_deref(),
            secret: secret.as_deref(),
            token: &token,
            created_by: integration.user_id,
        },
    )
    .await?;

    db::audit_log::insert(
        &state.db,
        burst_core::id::new_id(),
        integration.user_id,
        "webhook.created",
        "webhook",
        id,
        Some(serde_json::json!({
            "channelId": burst_core::id::format_channel_id(ch_id),
            "kind": body.kind,
            "name": body.name,
        })),
    )
    .await?;

    Ok((
        axum::http::StatusCode::CREATED,
        Json(CreateWebhookResponse {
            webhook: webhook_to_response(&row),
            token,
        }),
    ))
}

async fn list_webhooks(
    integration: IntegrationUser,
    State(state): State<AppState>,
    Path(channel_id): Path<String>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<PaginatedResponse<WebhookResponse>>, ApiError> {
    let ch_id = parse_channel_id(&channel_id)?;
    services::require_membership(&state.db, ch_id, integration.user_id).await?;

    let limit = params.clamped_limit();
    let rows =
        db::webhooks::list_by_channel(&state.db, ch_id, params.cursor_uuid(), limit + 1).await?;

    let has_more = rows.len() as i64 > limit;
    let items: Vec<WebhookResponse> = rows
        .iter()
        .take(limit as usize)
        .map(webhook_to_response)
        .collect();
    let cursor = if has_more {
        items.last().map(|w| w.id.clone())
    } else {
        None
    };

    Ok(Json(PaginatedResponse { items, cursor }))
}

async fn get_webhook(
    integration: IntegrationUser,
    State(state): State<AppState>,
    Path((channel_id, webhook_id)): Path<(String, String)>,
) -> Result<Json<WebhookResponse>, ApiError> {
    let ch_id = parse_channel_id(&channel_id)?;
    services::require_membership(&state.db, ch_id, integration.user_id).await?;

    let wh_id = parse_webhook_id(&webhook_id)?;
    let row = db::webhooks::find_by_id(&state.db, wh_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("Webhook".into()))?;

    if row.channel_id != ch_id {
        return Err(ApiError::NotFound("Webhook".into()));
    }

    Ok(Json(webhook_to_response(&row)))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateWebhookRequest {
    name: Option<String>,
    url: Option<String>,
    is_active: Option<bool>,
}

async fn update_webhook(
    integration: IntegrationUser,
    State(state): State<AppState>,
    Path((channel_id, webhook_id)): Path<(String, String)>,
    Json(body): Json<UpdateWebhookRequest>,
) -> Result<Json<WebhookResponse>, ApiError> {
    let ch_id = parse_channel_id(&channel_id)?;
    services::require_membership(&state.db, ch_id, integration.user_id).await?;

    let wh_id = parse_webhook_id(&webhook_id)?;

    // Verify webhook belongs to this channel.
    let existing = db::webhooks::find_by_id(&state.db, wh_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("Webhook".into()))?;
    if existing.channel_id != ch_id {
        return Err(ApiError::NotFound("Webhook".into()));
    }

    let row = db::webhooks::update(
        &state.db,
        wh_id,
        &db::webhooks::UpdateWebhook {
            name: body.name.as_deref(),
            url: body.url.as_deref(),
            is_active: body.is_active,
        },
    )
    .await?
    .ok_or_else(|| ApiError::NotFound("Webhook".into()))?;

    db::audit_log::insert(
        &state.db,
        burst_core::id::new_id(),
        integration.user_id,
        "webhook.updated",
        "webhook",
        wh_id,
        None,
    )
    .await?;

    Ok(Json(webhook_to_response(&row)))
}

async fn delete_webhook(
    integration: IntegrationUser,
    State(state): State<AppState>,
    Path((channel_id, webhook_id)): Path<(String, String)>,
) -> Result<axum::http::StatusCode, ApiError> {
    let ch_id = parse_channel_id(&channel_id)?;
    services::require_membership(&state.db, ch_id, integration.user_id).await?;

    let wh_id = parse_webhook_id(&webhook_id)?;

    let existing = db::webhooks::find_by_id(&state.db, wh_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("Webhook".into()))?;
    if existing.channel_id != ch_id {
        return Err(ApiError::NotFound("Webhook".into()));
    }

    db::webhooks::delete(&state.db, wh_id).await?;

    db::audit_log::insert(
        &state.db,
        burst_core::id::new_id(),
        integration.user_id,
        "webhook.deleted",
        "webhook",
        wh_id,
        Some(serde_json::json!({
            "channelId": burst_core::id::format_channel_id(ch_id),
        })),
    )
    .await?;

    Ok(axum::http::StatusCode::NO_CONTENT)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RegenerateTokenResponse {
    token: String,
}

async fn regenerate_token(
    integration: IntegrationUser,
    State(state): State<AppState>,
    Path((channel_id, webhook_id)): Path<(String, String)>,
) -> Result<Json<RegenerateTokenResponse>, ApiError> {
    let ch_id = parse_channel_id(&channel_id)?;
    services::require_membership(&state.db, ch_id, integration.user_id).await?;

    let wh_id = parse_webhook_id(&webhook_id)?;

    let existing = db::webhooks::find_by_id(&state.db, wh_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("Webhook".into()))?;
    if existing.channel_id != ch_id {
        return Err(ApiError::NotFound("Webhook".into()));
    }

    let new_token = generate_token();
    db::webhooks::regenerate_token(&state.db, wh_id, &new_token)
        .await?
        .ok_or_else(|| ApiError::NotFound("Webhook".into()))?;

    db::audit_log::insert(
        &state.db,
        burst_core::id::new_id(),
        integration.user_id,
        "webhook.token_regenerated",
        "webhook",
        wh_id,
        None,
    )
    .await?;

    Ok(Json(RegenerateTokenResponse { token: new_token }))
}

// ── Incoming webhook trigger ────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct IncomingWebhookPayload {
    content: String,
}

async fn trigger_incoming(
    State(state): State<AppState>,
    Path(webhook_id): Path<String>,
    headers: HeaderMap,
    Json(body): Json<IncomingWebhookPayload>,
) -> Result<(axum::http::StatusCode, Json<MessageResponse>), ApiError> {
    let wh_id = parse_webhook_id(&webhook_id)?;

    // Extract bearer token from Authorization header.
    let token = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or(ApiError::Unauthorized)?;

    let webhook = db::webhooks::find_by_id(&state.db, wh_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("Webhook".into()))?;

    if webhook.kind != "incoming" {
        return Err(ApiError::BadRequest(
            "only incoming webhooks can be triggered".into(),
        ));
    }

    if !webhook.is_active {
        return Err(ApiError::NotFound("Webhook".into()));
    }

    // Constant-time comparison to avoid timing attacks.
    if !constant_time_eq(token.as_bytes(), webhook.token.as_bytes()) {
        return Err(ApiError::Unauthorized);
    }

    let content = body.content.trim().to_string();
    if content.is_empty() {
        return Err(ApiError::BadRequest(
            "message content cannot be empty".into(),
        ));
    }

    let id = burst_core::id::new_id();
    let message = db::messages::create(
        &state.db,
        id,
        webhook.channel_id,
        webhook.created_by,
        None,
        &content,
    )
    .await?;

    let mentioned = crate::api::channels::persist_mentions(&state, id, &content).await?;

    let response = build_message_response_simple(&message, vec![]);
    let ev = crate::ws::ServerEvent::MessageCreated {
        event_id: burst_core::id::new_id().to_string(),
        channel_id: burst_core::id::format_channel_id(webhook.channel_id),
        message: response.clone(),
    };
    services::broadcast(&state, ev).await;
    crate::metrics::message_created();
    services::notifications::notify_new_message(&state, webhook.channel_id, &message, &mentioned)
        .await;

    Ok((axum::http::StatusCode::CREATED, Json(response)))
}

// ── Admin: list all webhooks ────────────────────────────────────────────────

/// Constant-time byte comparison to prevent timing attacks on token validation.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

async fn list_all_webhooks(
    _integration: IntegrationUser,
    State(state): State<AppState>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<PaginatedResponse<WebhookResponse>>, ApiError> {
    let limit = params.clamped_limit();
    let rows = db::webhooks::list_all(&state.db, params.cursor_uuid(), limit + 1).await?;

    let has_more = rows.len() as i64 > limit;
    let items: Vec<WebhookResponse> = rows
        .iter()
        .take(limit as usize)
        .map(webhook_to_response)
        .collect();
    let cursor = if has_more {
        items.last().map(|w| w.id.clone())
    } else {
        None
    };

    Ok(Json(PaginatedResponse { items, cursor }))
}
