pub mod api;
pub mod config;
pub mod db;
pub mod error;
pub mod services;
pub mod storage;
pub mod ws;

use std::sync::Arc;

use axum::Router;
use sqlx::PgPool;

use ws::{Broker, EventBuffer, new_broker_with_capacity, presence::PresenceState};

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub config: config::AppConfig,
    pub broker: Broker,
    pub event_buffer: Arc<EventBuffer>,
    pub presence: Arc<PresenceState>,
    pub storage: storage::Storage,
}

impl AppState {
    pub fn new(db: PgPool, config: config::AppConfig, storage: storage::Storage) -> Self {
        let ws_config = &config.websocket;
        Self {
            db,
            broker: new_broker_with_capacity(ws_config.broadcast_capacity),
            event_buffer: EventBuffer::new(ws_config.event_buffer_capacity),
            presence: PresenceState::new(),
            storage,
            config,
        }
    }
}

pub fn app_router(state: AppState) -> Router {
    Router::new()
        .merge(api::users::router())
        .merge(api::channels::router())
        .merge(api::search::router())
        .merge(api::attachments::router())
        .merge(api::admin::router())
        .route("/ws", axum::routing::get(ws::handler::ws_handler))
        .with_state(state)
}

pub fn admin_router(state: AppState) -> Router {
    Router::new().merge(api::health::router()).with_state(state)
}
