pub mod api;
pub mod auth;
pub mod config;
pub mod db;
pub mod error;

use axum::Router;
use sqlx::PgPool;

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub config: config::AppConfig,
}

pub fn app_router(state: AppState) -> Router {
    Router::new()
        .merge(api::auth::router())
        .merge(api::users::router())
        .with_state(state)
}

pub fn admin_router(state: AppState) -> Router {
    Router::new().merge(api::health::router()).with_state(state)
}
