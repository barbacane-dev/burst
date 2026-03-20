use std::path::PathBuf;

use burst_server::config::Config;
use burst_server::storage::{Storage, local::LocalStorage};
use burst_server::{AppState, admin_router, app_router, metrics, telemetry};
use sqlx::postgres::PgPoolOptions;
use tokio::net::TcpListener;
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

    let app_config = config.app_config();
    let storage = Storage::Local(
        LocalStorage::new(std::path::PathBuf::from(&app_config.storage.local_path))
            .expect("failed to initialise local storage"),
    );
    let state = AppState::new(pool, app_config, storage, metrics_handle);

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

    // Flush OTel spans before exit
    telemetry::shutdown(tracer_provider);

    Ok(())
}
