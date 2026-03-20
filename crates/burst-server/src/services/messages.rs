use std::collections::HashMap;

use sqlx::PgPool;
use uuid::Uuid;

use crate::api::attachments::{AttachmentResponse, attachment_to_response};
use crate::api::channels::{MessageResponse, ReactionResponse};
use crate::db;
use crate::error::ApiError;

/// Enriches a batch of messages with reactions, attachments, and reply counts.
///
/// This pattern was duplicated across `list_messages`, `get_message`,
/// `list_thread_replies`, `list_pins`, and `edit_message`. Centralising it
/// here ensures consistent loading and avoids N+1 queries.
pub async fn enrich(
    pool: &PgPool,
    messages: &[db::messages::MessageRow],
) -> Result<Vec<MessageResponse>, ApiError> {
    if messages.is_empty() {
        return Ok(vec![]);
    }

    let ids: Vec<Uuid> = messages.iter().map(|m| m.id).collect();

    let reply_count_map: HashMap<Uuid, i64> = db::messages::reply_counts(pool, &ids)
        .await?
        .into_iter()
        .collect();
    let reaction_rows = db::reactions::list_for_messages(pool, &ids).await?;
    let attachment_rows = db::attachments::list_for_messages(pool, &ids).await?;

    let items = messages
        .iter()
        .map(|m| {
            let reply_count = *reply_count_map.get(&m.id).unwrap_or(&0);
            let reactions = aggregate_reactions(&reaction_rows, m.id);
            let attachments = group_attachments(&attachment_rows, m.id);
            build_response(m, reply_count, reactions, attachments)
        })
        .collect();

    Ok(items)
}

/// Enriches a single message. Convenience wrapper around [`enrich`].
pub async fn enrich_one(
    pool: &PgPool,
    message: &db::messages::MessageRow,
) -> Result<MessageResponse, ApiError> {
    let mut items = enrich(pool, std::slice::from_ref(message)).await?;
    Ok(items.remove(0))
}

fn build_response(
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

fn group_attachments(
    rows: &[db::attachments::AttachmentRow],
    message_id: Uuid,
) -> Vec<AttachmentResponse> {
    rows.iter()
        .filter(|a| a.message_id == message_id)
        .map(attachment_to_response)
        .collect()
}

fn aggregate_reactions(
    rows: &[db::reactions::ReactionRow],
    message_id: Uuid,
) -> Vec<ReactionResponse> {
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
