use std::path::PathBuf;
use std::time::Duration;

use burst_server::config::Config;
use burst_server::storage::{Storage, local::LocalStorage};
use burst_server::{AppState, admin_router, app_router, metrics, telemetry};
use sqlx::postgres::PgPoolOptions;
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Config (loaded first so telemetry config is available for subscriber init)
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

    // Telemetry: OpenTelemetry tracer (optional, based on config)
    let tracer_provider = telemetry::init_tracer(&config.telemetry);

    // Logging: layered subscriber — JSON formatter + optional OTel layer
    let env_filter = EnvFilter::try_from_env("BURST_LOG")
        .unwrap_or_else(|_| EnvFilter::new("info,burst=debug,burst_server=debug"));
    let fmt_layer = tracing_subscriber::fmt::layer().json();

    let registry = tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt_layer);

    if let Some(ref provider) = tracer_provider {
        use opentelemetry::trace::TracerProvider;
        let otel_layer = tracing_opentelemetry::layer().with_tracer(provider.tracer("burst"));
        registry.with(otel_layer).init();
    } else {
        registry.init();
    }

    tracing::info!(listen = %config.server.listen, admin = %config.server.admin_listen, "starting burst");

    // Metrics: Prometheus recorder
    let metrics_handle = metrics::install();

    // Database
    let pool = PgPoolOptions::new()
        .max_connections(config.database.max_connections)
        .connect(&config.database.url)
        .await?;

    tracing::info!("connected to database");

    // Run migrations
    sqlx::migrate!("../../migrations").run(&pool).await?;
    tracing::info!("migrations applied");

    // DB pool metrics (periodic gauge update)
    metrics::spawn_db_pool_metrics(pool.clone());

    let shutdown_token = CancellationToken::new();
    let shutdown_timeout = Duration::from_secs(config.server.shutdown_timeout_secs);

    let app_config = config.app_config();
    let storage = Storage::Local(
        LocalStorage::new(std::path::PathBuf::from(&app_config.storage.local_path))
            .expect("failed to initialise local storage"),
    );
    let state = AppState::new(
        pool.clone(),
        app_config,
        storage,
        metrics_handle,
        shutdown_token.clone(),
    );

    // Main server
    let app = app_router(state.clone());
    let main_listener = TcpListener::bind(&config.server.listen).await?;
    tracing::info!(addr = %config.server.listen, "listening");

    // Admin server
    let admin = admin_router(state);
    let admin_listener = TcpListener::bind(&config.server.admin_listen).await?;
    tracing::info!(addr = %config.server.admin_listen, "admin listening");

    // Graceful shutdown signal: SIGINT (Ctrl+C) or SIGTERM
    let shutdown_signal = {
        let token = shutdown_token.clone();
        async move {
            let ctrl_c = tokio::signal::ctrl_c();

            #[cfg(unix)]
            let terminate = async {
                tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                    .expect("failed to listen for SIGTERM")
                    .recv()
                    .await;
            };
            #[cfg(not(unix))]
            let terminate = std::future::pending::<()>();

            tokio::select! {
                _ = ctrl_c => {},
                () = terminate => {},
            }
            tracing::info!("shutdown signal received");
            // Notify WS handlers to send Close(1001) frames.
            token.cancel();
        }
    };

    // Run both servers with graceful shutdown
    let main_handle = tokio::spawn(
        axum::serve(main_listener, app)
            .with_graceful_shutdown({
                let token = shutdown_token.clone();
                async move { token.cancelled().await }
            })
            .into_future(),
    );
    let admin_handle = tokio::spawn(
        axum::serve(admin_listener, admin)
            .with_graceful_shutdown({
                let token = shutdown_token.clone();
                async move { token.cancelled().await }
            })
            .into_future(),
    );

    // Wait for shutdown signal
    shutdown_signal.await;

    // Wait for servers to drain in-flight requests (with timeout)
    tracing::info!(
        timeout_secs = shutdown_timeout.as_secs(),
        "draining connections"
    );
    let _ = tokio::time::timeout(shutdown_timeout, async {
        let _ = main_handle.await;
        let _ = admin_handle.await;
    })
    .await;

    // Close DB pool
    pool.close().await;
    tracing::info!("database pool closed");

    // Flush OTel spans before exit
    telemetry::shutdown(tracer_provider);
    tracing::info!("shutdown complete");

    Ok(())
}
