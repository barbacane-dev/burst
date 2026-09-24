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
    /// Only messages this user wrote.
    from: Option<String>,
    /// RFC 3339. Only messages sent at or after this instant.
    after: Option<String>,
    /// RFC 3339. Only messages sent before this instant.
    before: Option<String>,
    /// Only messages carrying at least one file.
    has_file: Option<bool>,
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
    /// The attachment whose name matched the query, when one did.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub matched_file: Option<String>,
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

    let filters = db::search::SearchFilters {
        channel_id,
        from: params
            .from
            .as_deref()
            .map(|s| {
                burst_core::id::parse_prefixed_id(s, "usr_")
                    .or_else(|| Uuid::parse_str(s).ok())
                    .ok_or_else(|| ApiError::BadRequest("invalid user ID in 'from'".into()))
            })
            .transpose()?,
        after: params
            .after
            .as_deref()
            .map(|s| instant(s, "after"))
            .transpose()?,
        before: params
            .before
            .as_deref()
            .map(|s| instant(s, "before"))
            .transpose()?,
        has_file: params.has_file.unwrap_or(false),
    };
    if let (Some(after), Some(before)) = (filters.after, filters.before)
        && after >= before
    {
        return Err(ApiError::BadRequest(
            "'after' must be earlier than 'before'".into(),
        ));
    }

    let limit = params.limit.clamp(1, 200);
    let cursor = params.cursor.as_deref().map(decode_cursor).transpose()?;
    let rows = db::search::search_messages(&state.db, auth.user_id, q, &filters, cursor, limit + 1)
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
            matched_file: r.matched_file.clone(),
        })
        .collect();

    let cursor = if has_more {
        rows.last().map(|r| encode_cursor(r.rank, r.id))
    } else {
        None
    };

    Ok(Json(PaginatedResponse { items, cursor }))
}

fn instant(value: &str, name: &str) -> Result<chrono::DateTime<chrono::Utc>, ApiError> {
    chrono::DateTime::parse_from_rfc3339(value)
        .map(|t| t.with_timezone(&chrono::Utc))
        .map_err(|_| ApiError::BadRequest(format!("'{name}' must be an RFC 3339 timestamp")))
}

/// The next page starts after this relevance and id. Opaque to clients; the
/// shape is `<rank>_<uuid>`, and `f32`'s `Display` round-trips exactly.
fn encode_cursor(rank: f32, id: Uuid) -> String {
    format!("{rank}_{id}")
}

fn decode_cursor(cursor: &str) -> Result<db::search::SearchCursor, ApiError> {
    let invalid = || ApiError::BadRequest("invalid search cursor".into());
    let (rank, id) = cursor.split_once('_').ok_or_else(invalid)?;
    Ok(db::search::SearchCursor {
        rank: rank.parse().map_err(|_| invalid())?,
        id: Uuid::parse_str(id).map_err(|_| invalid())?,
    })
}

fn parse_channel_id(channel_id: &str) -> Result<Uuid, ApiError> {
    burst_core::id::parse_prefixed_id(channel_id, "ch_")
        .or_else(|| Uuid::parse_str(channel_id).ok())
        .ok_or_else(|| ApiError::BadRequest("invalid channel ID".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cursor_round_trips_exactly() {
        let id = Uuid::from_u128(42);
        for rank in [0.0_f32, 0.05, 0.0607927, 1.0, 0.1 + 0.2] {
            let decoded = decode_cursor(&encode_cursor(rank, id)).unwrap();
            assert_eq!(decoded.rank.to_bits(), rank.to_bits(), "{rank}");
            assert_eq!(decoded.id, id);
        }
    }

    #[test]
    fn a_malformed_cursor_is_refused() {
        for bad in [
            "",
            "abc",
            "0.5_not-a-uuid",
            "x_00000000-0000-0000-0000-000000000001",
        ] {
            assert!(decode_cursor(bad).is_err(), "{bad}");
        }
    }
}
