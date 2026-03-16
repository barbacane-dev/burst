pub mod api;
pub mod config;
pub mod db;
pub mod error;
pub mod storage;
pub mod ws;

use std::sync::Arc;

use axum::Router;
use sqlx::PgPool;

use ws::{BUFFER_CAPACITY, Broker, EventBuffer, new_broker, presence::PresenceState};

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
        Self {
            db,
            config,
            broker: new_broker(),
            event_buffer: EventBuffer::new(BUFFER_CAPACITY),
            presence: PresenceState::new(),
            storage,
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
