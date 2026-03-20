use std::collections::HashSet;
use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket};
use axum::extract::{Query, State, WebSocketUpgrade};
use axum::http::HeaderMap;
use axum::response::IntoResponse;
use serde::Deserialize;
use uuid::Uuid;

use crate::AppState;
use crate::db;
use crate::error::ApiError;
use crate::ws::{ClientEvent, EventBuffer, ServerEvent};
use burst_core::id::{format_channel_id, format_user_id, parse_prefixed_id};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WsQuery {
    pub last_event_id: Option<String>,
}

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    headers: HeaderMap,
    Query(query): Query<WsQuery>,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    let external_id = headers
        .get("x-auth-consumer")
        .and_then(|v| v.to_str().ok())
        .ok_or(ApiError::Unauthorized)?;

    let user = match db::users::find_by_external_id(&state.db, external_id).await? {
        Some(user) => user,
        None => {
            // JIT provisioning for WebSocket connections.
            let display_name = {
                let mut chars = external_id.chars();
                match chars.next() {
                    None => String::new(),
                    Some(c) => c.to_uppercase().to_string() + chars.as_str(),
                }
            };
            crate::api::users::jit_provision(
                &state.db,
                external_id,
                external_id,
                &display_name,
                None,
            )
            .await?;
            db::users::find_by_external_id(&state.db, external_id)
                .await?
                .ok_or(ApiError::Unauthorized)?
        }
    };

    let last_event_id = query
        .last_event_id
        .as_deref()
        .and_then(|s| Uuid::parse_str(s).ok());

    Ok(ws.on_upgrade(move |socket| handle_socket(socket, state, user.id, last_event_id)))
}

async fn handle_socket(
    mut socket: WebSocket,
    state: AppState,
    user_id: Uuid,
    last_event_id: Option<Uuid>,
) {
    crate::metrics::ws_connection_opened();

    // ── Load channel memberships for filtering ────────────────────────────────
    let mut channel_ids: HashSet<Uuid> =
        match db::channels::channel_ids_for_user(&state.db, user_id).await {
            Ok(ids) => ids.into_iter().collect(),
            Err(e) => {
                tracing::error!(user_id = %user_id, error = %e, "failed to load memberships");
                return;
            }
        };

    // ── Presence: mark online ─────────────────────────────────────────────────
    let just_came_online = state.presence.connect(user_id).await;
    if just_came_online {
        let ev = presence_event(user_id, "online");
        push_and_broadcast(&state.broker, &state.event_buffer, ev).await;
    }

    // ── Gap-fill on reconnect ─────────────────────────────────────────────────
    if let Some(last_id) = last_event_id {
        gap_fill(
            &mut socket,
            &state.event_buffer,
            last_id,
            user_id,
            &channel_ids,
        )
        .await;
    }

    // ── Subscribe to broadcast ────────────────────────────────────────────────
    let mut rx = state.broker.subscribe();

    // ── Main event loop ───────────────────────────────────────────────────────
    loop {
        tokio::select! {
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        handle_client_message(
                            &text, user_id, &state, &channel_ids,
                        ).await;
                    }
                    Some(Ok(Message::Ping(data))) => {
                        if socket.send(Message::Pong(data)).await.is_err() {
                            break;
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    _ => {}
                }
            }
            event = rx.recv() => {
                match event {
                    Ok(ev) => {
                        // Keep membership set current: when this user joins a new channel
                        // (e.g. a freshly created DM), add it so subsequent events are forwarded.
                        if let ServerEvent::ChannelJoined { user_id: uid_str, channel_id: ch_str, .. } = &ev
                            && let (Some(uid), Some(ch_id)) = (
                                parse_prefixed_id(uid_str, "usr_").or_else(|| Uuid::parse_str(uid_str).ok()),
                                parse_prefixed_id(ch_str, "ch_").or_else(|| Uuid::parse_str(ch_str).ok()),
                            )
                            && uid == user_id
                        {
                            channel_ids.insert(ch_id);
                        }
                        if should_forward(&ev, user_id, &channel_ids) {
                            let text = match serde_json::to_string(&ev) {
                                Ok(t) => t,
                                Err(_) => continue,
                            };
                            if socket.send(Message::Text(text.into())).await.is_err() {
                                break;
                            }
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                        // We dropped events — tell client to re-fetch via REST
                        let _ = socket
                            .send(Message::Text(r#"{"type":"gap_fill_needed"}"#.into()))
                            .await;
                    }
                    Err(_) => break,
                }
            }
            // Graceful shutdown: send Close(1001) and exit the loop.
            () = state.shutdown.cancelled() => {
                let close_frame = axum::extract::ws::CloseFrame {
                    code: 1001,
                    reason: "server shutting down".into(),
                };
                let _ = socket.send(Message::Close(Some(close_frame))).await;
                break;
            }
        }
    }

    // ── Cleanup: mark offline ─────────────────────────────────────────────────
    crate::metrics::ws_connection_closed();
    let just_went_offline = state.presence.disconnect(user_id).await;
    if just_went_offline {
        let ev = presence_event(user_id, "offline");
        push_and_broadcast(&state.broker, &state.event_buffer, ev).await;
    }
    crate::metrics::set_active_users(state.presence.online_count().await as f64);
}

// ── Helpers ───────────────────────────────────────────────────────────────────

async fn handle_client_message(
    text: &str,
    user_id: Uuid,
    state: &AppState,
    memberships: &HashSet<Uuid>,
) {
    let event: ClientEvent = match serde_json::from_str(text) {
        Ok(e) => e,
        Err(_) => return,
    };

    match event {
        ClientEvent::Heartbeat => {} // keep-alive, no action needed
        ClientEvent::TypingStart { channel_id } => {
            let Some(ch_id) =
                parse_prefixed_id(&channel_id, "ch_").or_else(|| Uuid::parse_str(&channel_id).ok())
            else {
                return;
            };
            if memberships.contains(&ch_id) {
                let ev = ServerEvent::TypingStart {
                    event_id: burst_core::id::new_id().to_string(),
                    channel_id: format_channel_id(ch_id),
                    user_id: format_user_id(user_id),
                };
                push_and_broadcast(&state.broker, &state.event_buffer, ev).await;
            }
        }
        ClientEvent::TypingStop { channel_id } => {
            let Some(ch_id) =
                parse_prefixed_id(&channel_id, "ch_").or_else(|| Uuid::parse_str(&channel_id).ok())
            else {
                return;
            };
            if memberships.contains(&ch_id) {
                let ev = ServerEvent::TypingStop {
                    event_id: burst_core::id::new_id().to_string(),
                    channel_id: format_channel_id(ch_id),
                    user_id: format_user_id(user_id),
                };
                push_and_broadcast(&state.broker, &state.event_buffer, ev).await;
            }
        }
    }
}

fn should_forward(event: &ServerEvent, self_user_id: Uuid, memberships: &HashSet<Uuid>) -> bool {
    // Typing events are not sent back to the sender
    match event {
        ServerEvent::TypingStart { user_id, .. } | ServerEvent::TypingStop { user_id, .. } => {
            if let Ok(uid) = Uuid::parse_str(user_id.trim_start_matches("usr_"))
                && uid == self_user_id
            {
                return false;
            }
        }
        _ => {}
    }

    // Channel-scoped events require membership
    if let Some(ch_id_str) = event.channel_id() {
        let ch_id = parse_prefixed_id(ch_id_str, "ch_").or_else(|| Uuid::parse_str(ch_id_str).ok());
        return ch_id.map(|id| memberships.contains(&id)).unwrap_or(false);
    }

    // Presence events go to everyone
    true
}

async fn gap_fill(
    socket: &mut WebSocket,
    buffer: &Arc<EventBuffer>,
    last_event_id: Uuid,
    user_id: Uuid,
    memberships: &HashSet<Uuid>,
) {
    match buffer.events_since(last_event_id).await {
        Some(events) => {
            for ev in events {
                if should_forward(&ev, user_id, memberships) {
                    let text = match serde_json::to_string(&ev) {
                        Ok(t) => t,
                        Err(_) => continue,
                    };
                    if socket.send(Message::Text(text.into())).await.is_err() {
                        return;
                    }
                }
            }
        }
        None => {
            // last_event_id not in buffer — tell client to re-fetch
            let _ = socket
                .send(Message::Text(r#"{"type":"gap_fill_needed"}"#.into()))
                .await;
        }
    }
}

pub async fn push_and_broadcast(
    broker: &crate::ws::Broker,
    buffer: &Arc<EventBuffer>,
    event: ServerEvent,
) {
    let key = Uuid::parse_str(event.event_id()).unwrap_or_else(|_| burst_core::id::new_id());
    buffer.push(key, event.clone()).await;
    let _ = broker.send(event);
}

fn presence_event(user_id: Uuid, status: &str) -> ServerEvent {
    ServerEvent::PresenceUpdate {
        event_id: burst_core::id::new_id().to_string(),
        user_id: format_user_id(user_id),
        status: status.to_string(),
    }
}
