//! Delivering the recipients `burst_core::notify` picks for a new message.

use std::collections::HashSet;

use uuid::Uuid;

use crate::AppState;
use crate::db;
use crate::ws::ServerEvent;
use burst_core::notify::{self, Preference};

/// The most of a message a notification carries, in characters.
const PREVIEW_CHARS: usize = 200;

/// Notifies the members of `channel_id` who should hear about a new message.
///
/// Runs after the message is persisted and broadcast, so a failure here is
/// logged rather than returned: the message exists either way, and failing the
/// request would invite a retry that posts it twice.
pub async fn notify_new_message(
    state: &AppState,
    channel_id: Uuid,
    message: &db::messages::MessageRow,
    mentioned: &HashSet<Uuid>,
) {
    if let Err(e) = deliver(state, channel_id, message, mentioned).await {
        tracing::error!(
            channel_id = %channel_id,
            message_id = %message.id,
            error = %e,
            "failed to deliver notifications"
        );
    }
}

async fn deliver(
    state: &AppState,
    channel_id: Uuid,
    message: &db::messages::MessageRow,
    mentioned: &HashSet<Uuid>,
) -> Result<(), sqlx::Error> {
    let members: Vec<(Uuid, Preference)> = db::channels::list_members(&state.db, channel_id)
        .await?
        .into_iter()
        .map(|m| (m.user_id, Preference::parse(&m.notify)))
        .collect();

    let mut recipients = notify::recipients(&members, message.user_id, mentioned);
    if recipients.is_empty() {
        return Ok(());
    }

    // Do not disturb holds a notification back entirely; the message still
    // counts as unread and a mention is still recorded.
    let ids: Vec<Uuid> = recipients.iter().map(|(user, _)| *user).collect();
    let now = chrono::Utc::now();
    let quiet: HashSet<Uuid> = db::users::do_not_disturb_of(&state.db, &ids)
        .await?
        .into_iter()
        .filter(|(_, dnd)| dnd.quiet_until(now).is_some())
        .map(|(user, _)| user)
        .collect();
    recipients.retain(|(user, _)| !quiet.contains(user));
    if recipients.is_empty() {
        return Ok(());
    }

    let author_name = db::users::find_by_id(&state.db, message.user_id)
        .await?
        .map(|u| u.display_name)
        .unwrap_or_default();
    let channel_name = db::channels::find_by_id(&state.db, channel_id)
        .await?
        .and_then(|c| c.name);
    let preview = preview(&message.content);

    for (recipient, reason) in recipients {
        let event = ServerEvent::NotificationCreated {
            notification_id: burst_core::id::new_id().to_string(),
            recipient_id: burst_core::id::format_user_id(recipient),
            channel_id: burst_core::id::format_channel_id(channel_id),
            message_id: burst_core::id::format_message_id(message.id),
            reason: reason.as_str().to_string(),
            author_name: author_name.clone(),
            channel_name: channel_name.clone(),
            preview: preview.clone(),
        };
        // Published, not buffered: see `ServerEvent::NotificationCreated`.
        state.broker.publish(event);
    }
    Ok(())
}

/// The start of a message, cut on a character boundary.
fn preview(content: &str) -> String {
    let mut chars = content.chars();
    let head: String = chars.by_ref().take(PREVIEW_CHARS).collect();
    if chars.next().is_some() {
        format!("{head}…")
    } else {
        head
    }
}

#[cfg(test)]
mod tests {
    use super::preview;

    #[test]
    fn a_short_message_is_whole() {
        assert_eq!(preview("hello"), "hello");
    }

    #[test]
    fn a_long_message_is_cut_and_marked() {
        let long = "a".repeat(250);
        let out = preview(&long);
        assert_eq!(out.chars().count(), 201);
        assert!(out.ends_with('…'));
    }

    #[test]
    fn cutting_respects_multibyte_characters() {
        let long = "é".repeat(300);
        let out = preview(&long);
        assert_eq!(out.chars().count(), 201);
    }

    #[test]
    fn exactly_the_limit_is_not_marked() {
        let exact = "b".repeat(200);
        assert_eq!(preview(&exact), exact);
    }
}
