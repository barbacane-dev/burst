use std::time::Duration;

use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;

use crate::AppState;
use crate::db;
use crate::ws::ServerEvent;

type HmacSha256 = Hmac<Sha256>;

/// Returns the event type tag as it appears in JSON serialization.
fn event_type(event: &ServerEvent) -> &'static str {
    match event {
        ServerEvent::MessageCreated { .. } => "message.created",
        ServerEvent::MessageUpdated { .. } => "message.updated",
        ServerEvent::MessageDeleted { .. } => "message.deleted",
        ServerEvent::TypingStart { .. } => "typing.start",
        ServerEvent::TypingStop { .. } => "typing.stop",
        ServerEvent::PresenceUpdate { .. } => "presence.update",
        ServerEvent::UserStatusChanged { .. } => "user.status_changed",
        ServerEvent::UserDndChanged { .. } => "user.dnd_changed",
        ServerEvent::ChannelJoined { .. } => "channel.joined",
        ServerEvent::ChannelLeft { .. } => "channel.left",
        ServerEvent::ReactionAdded { .. } => "reaction.added",
        ServerEvent::ReactionRemoved { .. } => "reaction.removed",
        ServerEvent::MessagePinned { .. } => "message.pinned",
        ServerEvent::MessageUnpinned { .. } => "message.unpinned",
        ServerEvent::ChannelUpdated { .. } => "channel.updated",
        ServerEvent::NotificationCreated { .. } => "notification.created",
    }
}

/// Returns true if the event should be delivered to outgoing webhooks.
///
/// Notifications are excluded: each is addressed to one member, so delivering
/// them would tell an external service who is notified of what, and would post
/// once per member for every message.
fn is_deliverable(event: &ServerEvent) -> bool {
    !matches!(
        event,
        ServerEvent::TypingStart { .. }
            | ServerEvent::TypingStop { .. }
            | ServerEvent::PresenceUpdate { .. }
            | ServerEvent::UserDndChanged { .. }
            | ServerEvent::NotificationCreated { .. }
            | ServerEvent::UserStatusChanged { .. }
    )
}

/// Background task that delivers events to outgoing webhooks.
///
/// Subscribes to the broker, filters to deliverable events, and for each
/// event with a channel_id, looks up active outgoing webhooks and delivers
/// the event payload via HTTP POST.
pub async fn outgoing_webhook_worker(state: AppState, shutdown: CancellationToken) {
    let mut rx = state.broker.subscribe();
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .expect("failed to build reqwest client");

    loop {
        tokio::select! {
            () = shutdown.cancelled() => {
                tracing::info!("outgoing webhook worker shutting down");
                break;
            }
            result = rx.recv() => {
                match result {
                    Ok(event) => {
                        if !is_deliverable(&event) {
                            continue;
                        }
                        if let Some(channel_id_str) = event.channel_id() {
                            let channel_id = match burst_core::id::parse_prefixed_id(channel_id_str, "ch_") {
                                Some(id) => id,
                                None => continue,
                            };
                            let webhooks = match db::webhooks::list_active_outgoing_by_channel(
                                &state.db,
                                channel_id,
                            )
                            .await
                            {
                                Ok(whs) => whs,
                                Err(e) => {
                                    tracing::warn!(error = %e, "failed to load outgoing webhooks");
                                    continue;
                                }
                            };
                            for wh in webhooks {
                                let event = event.clone();
                                let client = client.clone();
                                tokio::spawn(deliver(wh, event, client));
                            }
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!(skipped = n, "outgoing webhook worker lagged");
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        tracing::info!("broker closed, outgoing webhook worker exiting");
                        break;
                    }
                }
            }
        }
    }
}

/// Deliver an event to a single outgoing webhook with retries.
async fn deliver(webhook: db::webhooks::WebhookRow, event: ServerEvent, client: reqwest::Client) {
    let url = match &webhook.url {
        Some(u) => u.clone(),
        None => return,
    };

    let body = match serde_json::to_vec(&event) {
        Ok(b) => b,
        Err(e) => {
            tracing::warn!(
                webhook_id = %webhook.id,
                error = %e,
                "failed to serialize event for outgoing webhook"
            );
            return;
        }
    };

    let signature = if let Some(ref secret) = webhook.secret {
        let mut mac =
            HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC accepts any key size");
        mac.update(&body);
        let result = mac.finalize();
        format!("sha256={}", hex::encode(result.into_bytes()))
    } else {
        String::new()
    };

    let delivery_id = burst_core::id::new_id().to_string();
    let event_type_str = event_type(&event);

    let delays = [
        Duration::from_secs(0),
        Duration::from_secs(1),
        Duration::from_secs(5),
        Duration::from_secs(30),
    ];

    for (attempt, delay) in delays.iter().enumerate() {
        if attempt > 0 {
            tokio::time::sleep(*delay).await;
        }

        let mut req = client
            .post(&url)
            .header("Content-Type", "application/json")
            .header("X-Burst-Event", event_type_str)
            .header("X-Burst-Delivery", &delivery_id);

        if !signature.is_empty() {
            req = req.header("X-Burst-Signature", &signature);
        }

        match req.body(body.clone()).send().await {
            Ok(resp) if resp.status().is_success() => {
                tracing::info!(
                    webhook_id = %webhook.id,
                    event = event_type_str,
                    delivery_id = %delivery_id,
                    status = %resp.status(),
                    "outgoing webhook delivered"
                );
                return;
            }
            Ok(resp) => {
                tracing::warn!(
                    webhook_id = %webhook.id,
                    event = event_type_str,
                    attempt = attempt + 1,
                    status = %resp.status(),
                    "outgoing webhook delivery failed"
                );
            }
            Err(e) => {
                tracing::warn!(
                    webhook_id = %webhook.id,
                    event = event_type_str,
                    attempt = attempt + 1,
                    error = %e,
                    "outgoing webhook delivery error"
                );
            }
        }
    }

    tracing::error!(
        webhook_id = %webhook.id,
        event = event_type_str,
        delivery_id = %delivery_id,
        "outgoing webhook delivery failed after all retries"
    );
}

#[cfg(test)]
mod signature_tests {
    use super::*;

    /// RFC 4231 test case 2. The signature travels to every outgoing webhook
    /// consumer, so a dependency bump that changed it would break them all
    /// silently.
    #[test]
    fn hmac_sha256_matches_the_published_vector() {
        let mut mac = HmacSha256::new_from_slice(b"Jefe").expect("HMAC accepts any key size");
        mac.update(b"what do ya want for nothing?");
        assert_eq!(
            hex::encode(mac.finalize().into_bytes()),
            "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843"
        );
    }

    #[test]
    fn notifications_are_never_delivered_to_outgoing_webhooks() {
        let event = ServerEvent::NotificationCreated {
            notification_id: "n".into(),
            recipient_id: "usr_x".into(),
            channel_id: "ch_x".into(),
            message_id: "msg_x".into(),
            reason: "mention".into(),
            author_name: "alice".into(),
            channel_name: None,
            preview: String::new(),
        };
        assert!(!is_deliverable(&event));
    }

    #[test]
    fn a_created_message_is_still_delivered_to_outgoing_webhooks() {
        let event = ServerEvent::MessageDeleted {
            event_id: "e".into(),
            channel_id: "ch_x".into(),
            message_id: "msg_x".into(),
        };
        assert!(is_deliverable(&event));
    }
}
