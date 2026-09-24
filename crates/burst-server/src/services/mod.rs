pub mod cleanup;
pub mod messages;
pub mod notifications;
pub mod webhooks;

use uuid::Uuid;

use crate::db;
use crate::error::ApiError;

/// Generic prefixed-ID parser. Accepts `"ch_<uuid>"` or raw `"<uuid>"`.
pub fn parse_id(s: &str, prefix: &str, label: &str) -> Result<Uuid, ApiError> {
    burst_core::id::parse_prefixed_id(s, prefix)
        .or_else(|| Uuid::parse_str(s).ok())
        .ok_or_else(|| ApiError::BadRequest(format!("invalid {label} ID")))
}

/// Checks channel membership and returns `Err(Forbidden)` if not a member.
pub async fn require_membership(
    pool: &sqlx::PgPool,
    channel_id: Uuid,
    user_id: Uuid,
) -> Result<(), ApiError> {
    if !db::channels::is_member(pool, channel_id, user_id).await? {
        return Err(ApiError::Forbidden);
    }
    Ok(())
}

/// Broadcasts a server event via the broker and pushes it to the event buffer.
pub async fn broadcast(state: &crate::AppState, event: crate::ws::ServerEvent) {
    crate::ws::handler::push_and_broadcast(&state.broker, &state.event_buffer, event).await;
}
