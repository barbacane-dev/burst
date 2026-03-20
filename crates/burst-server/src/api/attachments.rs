use axum::Router;
use axum::extract::{Path, State};
use axum::http::header;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use serde::Serialize;
use uuid::Uuid;

use crate::AppState;
use crate::api::extractors::AuthUser;
use crate::db;
use crate::error::ApiError;
use crate::services;

pub fn router() -> Router<AppState> {
    Router::new().route("/attachments/{attachment_id}", get(download_attachment))
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AttachmentResponse {
    pub id: String,
    pub file_name: String,
    pub file_size: i64,
    pub content_type: String,
    pub metadata: serde_json::Value,
    pub created_at: String,
}

pub fn attachment_to_response(row: &db::attachments::AttachmentRow) -> AttachmentResponse {
    AttachmentResponse {
        id: burst_core::id::format_attachment_id(row.id),
        file_name: row.file_name.clone(),
        file_size: row.file_size,
        content_type: row.content_type.clone(),
        metadata: row.metadata.clone(),
        created_at: row.created_at.to_rfc3339(),
    }
}

fn parse_attachment_id(s: &str) -> Result<Uuid, ApiError> {
    services::parse_id(s, "att_", "attachment")
}

async fn download_attachment(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(attachment_id): Path<String>,
) -> Result<Response, ApiError> {
    let att_id = parse_attachment_id(&attachment_id)?;
    let att = db::attachments::find_by_id(&state.db, att_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("Attachment".into()))?;

    // Look up message to verify channel membership.
    let msg = db::messages::find_by_id(&state.db, att.message_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("Attachment".into()))?;

    if !db::channels::is_member(&state.db, msg.channel_id, auth.user_id).await? {
        return Err(ApiError::NotFound("Attachment".into()));
    }

    let (data, content_type) = state
        .storage
        .get(&att.storage_key)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let disposition = if content_type.starts_with("image/") || content_type == "application/pdf" {
        format!("inline; filename=\"{}\"", att.file_name)
    } else {
        format!("attachment; filename=\"{}\"", att.file_name)
    };

    Ok((
        [
            (header::CONTENT_TYPE, content_type),
            (header::CONTENT_DISPOSITION, disposition),
        ],
        data,
    )
        .into_response())
}
