pub mod handler;
pub mod presence;

use std::collections::VecDeque;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::{RwLock, broadcast};
use uuid::Uuid;

// ── Event types ──────────────────────────────────────────────────────────────

/// Events sent from server to clients over WebSocket.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum ServerEvent {
    #[serde(rename = "message.created", rename_all = "camelCase")]
    MessageCreated {
        event_id: String,
        channel_id: String,
        message: MessagePayload,
    },
    #[serde(rename = "message.updated", rename_all = "camelCase")]
    MessageUpdated {
        event_id: String,
        channel_id: String,
        message: MessagePayload,
    },
    #[serde(rename = "message.deleted", rename_all = "camelCase")]
    MessageDeleted {
        event_id: String,
        channel_id: String,
        message_id: String,
    },
    #[serde(rename = "typing.start", rename_all = "camelCase")]
    TypingStart {
        event_id: String,
        channel_id: String,
        user_id: String,
    },
    #[serde(rename = "typing.stop", rename_all = "camelCase")]
    TypingStop {
        event_id: String,
        channel_id: String,
        user_id: String,
    },
    #[serde(rename = "presence.update", rename_all = "camelCase")]
    PresenceUpdate {
        event_id: String,
        user_id: String,
        status: String,
    },
    #[serde(rename = "channel.joined", rename_all = "camelCase")]
    ChannelJoined {
        event_id: String,
        channel_id: String,
        user_id: String,
    },
    #[serde(rename = "reaction.added", rename_all = "camelCase")]
    ReactionAdded {
        event_id: String,
        channel_id: String,
        message_id: String,
        emoji: String,
        user_id: String,
    },
    #[serde(rename = "reaction.removed", rename_all = "camelCase")]
    ReactionRemoved {
        event_id: String,
        channel_id: String,
        message_id: String,
        emoji: String,
        user_id: String,
    },
    #[serde(rename = "message.pinned", rename_all = "camelCase")]
    MessagePinned {
        event_id: String,
        channel_id: String,
        message_id: String,
        user_id: String,
    },
    #[serde(rename = "message.unpinned", rename_all = "camelCase")]
    MessageUnpinned {
        event_id: String,
        channel_id: String,
        message_id: String,
    },
    #[serde(rename = "channel.updated", rename_all = "camelCase")]
    ChannelUpdated {
        event_id: String,
        channel_id: String,
    },
}

impl ServerEvent {
    pub fn event_id(&self) -> &str {
        match self {
            ServerEvent::MessageCreated { event_id, .. } => event_id,
            ServerEvent::MessageUpdated { event_id, .. } => event_id,
            ServerEvent::MessageDeleted { event_id, .. } => event_id,
            ServerEvent::TypingStart { event_id, .. } => event_id,
            ServerEvent::TypingStop { event_id, .. } => event_id,
            ServerEvent::PresenceUpdate { event_id, .. } => event_id,
            ServerEvent::ReactionAdded { event_id, .. } => event_id,
            ServerEvent::ReactionRemoved { event_id, .. } => event_id,
            ServerEvent::ChannelJoined { event_id, .. } => event_id,
            ServerEvent::MessagePinned { event_id, .. } => event_id,
            ServerEvent::MessageUnpinned { event_id, .. } => event_id,
            ServerEvent::ChannelUpdated { event_id, .. } => event_id,
        }
    }

    pub fn channel_id(&self) -> Option<&str> {
        match self {
            ServerEvent::MessageCreated { channel_id, .. } => Some(channel_id),
            ServerEvent::MessageUpdated { channel_id, .. } => Some(channel_id),
            ServerEvent::MessageDeleted { channel_id, .. } => Some(channel_id),
            ServerEvent::TypingStart { channel_id, .. } => Some(channel_id),
            ServerEvent::TypingStop { channel_id, .. } => Some(channel_id),
            ServerEvent::PresenceUpdate { .. } => None,
            ServerEvent::ReactionAdded { channel_id, .. } => Some(channel_id),
            ServerEvent::ReactionRemoved { channel_id, .. } => Some(channel_id),
            ServerEvent::ChannelJoined { channel_id, .. } => Some(channel_id),
            ServerEvent::MessagePinned { channel_id, .. } => Some(channel_id),
            ServerEvent::MessageUnpinned { channel_id, .. } => Some(channel_id),
            ServerEvent::ChannelUpdated { channel_id, .. } => Some(channel_id),
        }
    }
}

/// Message payload reused by both REST responses and WS events.
pub type MessagePayload = crate::api::channels::MessageResponse;
pub type ReactionPayload = crate::api::channels::ReactionResponse;

/// Events sent from client to server.
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ClientEvent {
    Heartbeat,
    #[serde(rename = "typing.start")]
    TypingStart {
        #[serde(rename = "channelId")]
        channel_id: String,
    },
    #[serde(rename = "typing.stop")]
    TypingStop {
        #[serde(rename = "channelId")]
        channel_id: String,
    },
}

// ── Broker ────────────────────────────────────────────────────────────────────

pub type Broker = Arc<broadcast::Sender<ServerEvent>>;

pub fn new_broker_with_capacity(capacity: usize) -> Broker {
    let (tx, _rx) = broadcast::channel(capacity);
    Arc::new(tx)
}

// ── Event ring buffer (gap-fill) ──────────────────────────────────────────────

pub struct EventBuffer {
    inner: RwLock<VecDeque<(Uuid, ServerEvent)>>,
    capacity: usize,
}

impl EventBuffer {
    pub fn new(capacity: usize) -> Arc<Self> {
        Arc::new(Self {
            inner: RwLock::new(VecDeque::with_capacity(capacity)),
            capacity,
        })
    }

    pub async fn push(&self, event_id: Uuid, event: ServerEvent) {
        let mut buf = self.inner.write().await;
        if buf.len() >= self.capacity {
            buf.pop_front();
        }
        buf.push_back((event_id, event));
    }

    /// Returns events that occurred after `last_event_id`.
    /// Returns `None` if `last_event_id` was not found (gap too large).
    pub async fn events_since(&self, last_event_id: Uuid) -> Option<Vec<ServerEvent>> {
        let buf = self.inner.read().await;
        let mut found = false;
        let mut result = Vec::new();
        for (id, event) in buf.iter() {
            if *id == last_event_id {
                found = true;
                continue;
            }
            if found {
                result.push(event.clone());
            }
        }
        if found { Some(result) } else { None }
    }
}
