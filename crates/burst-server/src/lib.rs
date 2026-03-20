pub mod api;
pub mod config;
pub mod db;
pub mod error;
pub mod metrics;
pub mod services;
pub mod storage;
pub mod telemetry;
pub mod ws;

use std::sync::Arc;

use axum::Router;
use sqlx::PgPool;

use tokio_util::sync::CancellationToken;
use ws::{Broker, EventBuffer, InProcessBroker, presence::PresenceState};

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub config: config::AppConfig,
    pub broker: Broker,
    pub event_buffer: Arc<EventBuffer>,
    pub presence: Arc<PresenceState>,
    pub storage: storage::Storage,
    pub metrics_handle: metrics::MetricsHandle,
    /// Triggered during graceful shutdown so WS handlers can send Close frames.
    pub shutdown: CancellationToken,
}

impl AppState {
    pub fn new(
        db: PgPool,
        config: config::AppConfig,
        storage: storage::Storage,
        metrics_handle: metrics::MetricsHandle,
        shutdown: CancellationToken,
    ) -> Self {
        let ws_config = &config.websocket;
        Self {
            db,
            broker: InProcessBroker::new(ws_config.broadcast_capacity),
            event_buffer: EventBuffer::new(ws_config.event_buffer_capacity),
            presence: PresenceState::new(),
            storage,
            metrics_handle,
            shutdown,
            config,
        }
    }

    /// Create an AppState with a custom broker (e.g. PgNotifyBroker).
    pub fn with_broker(
        db: PgPool,
        config: config::AppConfig,
        storage: storage::Storage,
        metrics_handle: metrics::MetricsHandle,
        shutdown: CancellationToken,
        broker: Broker,
    ) -> Self {
        let ws_config = &config.websocket;
        Self {
            db,
            broker,
            event_buffer: EventBuffer::new(ws_config.event_buffer_capacity),
            presence: PresenceState::new(),
            storage,
            metrics_handle,
            shutdown,
            config,
        }
    }
}

pub fn app_router(state: AppState) -> Router {
    use axum::middleware;

    let api = Router::new()
        .merge(api::users::router())
        .merge(api::channels::router())
        .merge(api::search::router())
        .merge(api::attachments::router())
        .merge(api::admin::router())
        .route("/ws", axum::routing::get(ws::handler::ws_handler))
        .layer(middleware::from_fn(metrics::http_metrics))
        .with_state(state.clone());

    // If a static directory is configured, serve the built frontend at /static
    // with SPA fallback (unmatched paths serve index.html for client-side routing).
    if let Some(ref dir) = state.config.server_static_dir {
        let serve = tower_http::services::ServeDir::new(dir).not_found_service(
            tower_http::services::ServeFile::new(format!("{dir}/index.html")),
        );
        api.nest_service("/static", serve)
    } else {
        api
    }
}

pub fn admin_router(state: AppState) -> Router {
    Router::new()
        .merge(api::health::router())
        .route(
            "/metrics",
            axum::routing::get(metrics::metrics_handler).with_state(state.metrics_handle.clone()),
        )
        .with_state(state)
}
