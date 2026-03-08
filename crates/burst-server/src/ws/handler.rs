use std::collections::HashSet;
use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket};
use axum::extract::{State, WebSocketUpgrade};
use axum::response::IntoResponse;
use tokio::time::{Duration, timeout};
use uuid::Uuid;

use crate::AppState;
use crate::auth::validate_access_token;
use crate::db;
use crate::ws::{ClientEvent, EventBuffer, ServerEvent};
use burst_core::id::{format_channel_id, format_user_id, parse_prefixed_id};

pub async fn ws_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

async fn handle_socket(mut socket: WebSocket, state: AppState) {
    // ── Auth handshake (10s timeout) ──────────────────────────────────────────
    let (user_id, last_event_id) = match auth_handshake(&mut socket, &state).await {
        Some(v) => v,
        None => return,
    };

    // ── Load channel memberships for filtering ────────────────────────────────
    let channel_ids: HashSet<Uuid> =
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
        }
    }

    // ── Cleanup: mark offline ─────────────────────────────────────────────────
    let just_went_offline = state.presence.disconnect(user_id).await;
    if just_went_offline {
        let ev = presence_event(user_id, "offline");
        push_and_broadcast(&state.broker, &state.event_buffer, ev).await;
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

async fn auth_handshake(socket: &mut WebSocket, state: &AppState) -> Option<(Uuid, Option<Uuid>)> {
    let msg = timeout(Duration::from_secs(10), socket.recv())
        .await
        .ok()??
        .ok()?;

    let text = match msg {
        Message::Text(t) => t,
        _ => return None,
    };

    let event: ClientEvent = serde_json::from_str(&text).ok()?;

    match event {
        ClientEvent::Auth {
            token,
            last_event_id,
        } => {
            let claims = validate_access_token(&state.config, &token).ok()?;
            let user_id = Uuid::parse_str(&claims.sub).ok()?;
            let last_id = last_event_id.and_then(|s| Uuid::parse_str(&s).ok());
            Some((user_id, last_id))
        }
        _ => None,
    }
}

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
        ClientEvent::Auth { .. } => {} // already authenticated
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
