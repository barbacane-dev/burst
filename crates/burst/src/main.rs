use std::path::PathBuf;

use burst_server::config::Config;
use burst_server::{AppState, admin_router, app_router};
use sqlx::postgres::PgPoolOptions;
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Logging
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_env("BURST_LOG")
                .unwrap_or_else(|_| EnvFilter::new("info,burst=debug,burst_server=debug")),
        )
        .json()
        .init();

    // Config
    let config_path = std::env::args()
        .nth(1)
        .filter(|a| !a.starts_with('-'))
        .or_else(|| {
            std::env::args()
                .position(|a| a == "--config")
                .and_then(|i| std::env::args().nth(i + 1))
        })
        .map(PathBuf::from);

    let config = Config::load(config_path.as_deref())?;

    tracing::info!(listen = %config.server.listen, admin = %config.server.admin_listen, "starting burst");

    // Database
    let pool = PgPoolOptions::new()
        .max_connections(config.database.max_connections)
        .connect(&config.database.url)
        .await?;

    tracing::info!("connected to database");

    // Run migrations
    sqlx::migrate!("../../migrations").run(&pool).await?;
    tracing::info!("migrations applied");

    let state = AppState {
        db: pool,
        config: config.app_config(),
    };

    // Main server
    let app = app_router(state.clone());
    let main_listener = TcpListener::bind(&config.server.listen).await?;
    tracing::info!(addr = %config.server.listen, "listening");

    // Admin server
    let admin = admin_router(state);
    let admin_listener = TcpListener::bind(&config.server.admin_listen).await?;
    tracing::info!(addr = %config.server.admin_listen, "admin listening");

    // Graceful shutdown
    let shutdown = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to listen for ctrl+c");
        tracing::info!("shutting down");
    };

    tokio::select! {
        result = axum::serve(main_listener, app) => {
            if let Err(e) = result {
                tracing::error!("main server error: {e}");
            }
        }
        result = axum::serve(admin_listener, admin) => {
            if let Err(e) = result {
                tracing::error!("admin server error: {e}");
            }
        }
        () = shutdown => {}
    }

    Ok(())
}
