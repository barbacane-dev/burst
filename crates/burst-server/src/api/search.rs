use axum::extract::{Query, State};
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::AppState;
use crate::api::PaginatedResponse;
use crate::api::extractors::AuthUser;
use crate::db;
use crate::error::ApiError;

pub fn router() -> Router<AppState> {
    Router::new().route("/search/messages", get(search_messages))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SearchParams {
    q: String,
    channel_id: Option<String>,
    cursor: Option<String>,
    #[serde(default = "default_limit")]
    limit: i64,
}

fn default_limit() -> i64 {
    50
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResultResponse {
    pub id: String,
    pub channel_id: String,
    pub user_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<String>,
    pub content: String,
    pub headline: String,
    pub created_at: String,
}

async fn search_messages(
    auth: AuthUser,
    State(state): State<AppState>,
    Query(params): Query<SearchParams>,
) -> Result<Json<PaginatedResponse<SearchResultResponse>>, ApiError> {
    let q = params.q.trim();
    if q.is_empty() {
        return Err(ApiError::BadRequest("search query cannot be empty".into()));
    }
    if q.len() > 200 {
        return Err(ApiError::BadRequest(
            "search query must be at most 200 characters".into(),
        ));
    }

    let channel_id = params
        .channel_id
        .as_deref()
        .map(parse_channel_id)
        .transpose()?;

    let limit = params.limit.clamp(1, 200);
    let cursor = params.cursor.as_deref().and_then(|s| {
        burst_core::id::parse_prefixed_id(s, "msg_").or_else(|| Uuid::parse_str(s).ok())
    });
    let rows =
        db::search::search_messages(&state.db, auth.user_id, q, channel_id, cursor, limit + 1)
            .await?;

    let has_more = rows.len() as i64 > limit;
    let rows: Vec<_> = rows.into_iter().take(limit as usize).collect();

    let items: Vec<_> = rows
        .iter()
        .map(|r| SearchResultResponse {
            id: burst_core::id::format_message_id(r.id),
            channel_id: burst_core::id::format_channel_id(r.channel_id),
            user_id: burst_core::id::format_user_id(r.user_id),
            thread_id: r.thread_id.map(burst_core::id::format_message_id),
            content: r.content.clone(),
            headline: r.headline.clone(),
            created_at: r.created_at.to_rfc3339(),
        })
        .collect();

    let cursor = if has_more {
        rows.last().map(|r| burst_core::id::format_message_id(r.id))
    } else {
        None
    };

    Ok(Json(PaginatedResponse { items, cursor }))
}

fn parse_channel_id(channel_id: &str) -> Result<Uuid, ApiError> {
    burst_core::id::parse_prefixed_id(channel_id, "ch_")
        .or_else(|| Uuid::parse_str(channel_id).ok())
        .ok_or_else(|| ApiError::BadRequest("invalid channel ID".into()))
}
