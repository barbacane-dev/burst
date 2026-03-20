use std::sync::Arc;
use std::time::Instant;

use axum::extract::State;
use axum::http::{Request, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use sqlx::PgPool;

// ── Metric names (ADR-010) ──────────────────────────────────────────────────

pub const HTTP_REQUESTS_TOTAL: &str = "burst_http_requests_total";
pub const HTTP_REQUEST_DURATION: &str = "burst_http_request_duration_seconds";
pub const DB_POOL_ACTIVE: &str = "burst_db_pool_connections_active";
pub const DB_POOL_IDLE: &str = "burst_db_pool_connections_idle";
pub const WS_CONNECTIONS_ACTIVE: &str = "burst_ws_connections_active";
pub const WS_CONNECTIONS_TOTAL: &str = "burst_ws_connections_total";
pub const WS_MESSAGES_SENT: &str = "burst_ws_messages_sent_total";
pub const WS_MESSAGES_RECEIVED: &str = "burst_ws_messages_received_total";
pub const MESSAGES_CREATED: &str = "burst_messages_created_total";
pub const USERS_ACTIVE: &str = "burst_users_active";

// ── Prometheus handle wrapper ───────────────────────────────────────────────

/// Opaque wrapper around the Prometheus exporter handle.
/// Stored in `AppState` so the `/metrics` endpoint can render output.
#[derive(Clone)]
pub struct MetricsHandle(Arc<dyn MetricsRenderer>);

/// Abstraction so tests can provide a no-op handle.
pub trait MetricsRenderer: Send + Sync {
    fn render(&self) -> String;
}

impl MetricsHandle {
    pub fn render(&self) -> String {
        self.0.render()
    }
}

struct PrometheusRenderer(metrics_exporter_prometheus::PrometheusHandle);

impl MetricsRenderer for PrometheusRenderer {
    fn render(&self) -> String {
        self.0.render()
    }
}

struct NoopRenderer;

impl MetricsRenderer for NoopRenderer {
    fn render(&self) -> String {
        String::new()
    }
}

/// Install the Prometheus metrics recorder and return a handle for the
/// `/metrics` endpoint. Call once at startup in main.rs.
pub fn install() -> MetricsHandle {
    let builder = metrics_exporter_prometheus::PrometheusBuilder::new();
    match builder.install_recorder() {
        Ok(handle) => MetricsHandle(Arc::new(PrometheusRenderer(handle))),
        Err(e) => {
            tracing::warn!("failed to install metrics recorder: {e}");
            MetricsHandle(Arc::new(NoopRenderer))
        }
    }
}

/// Create a no-op handle (for tests).
pub fn noop() -> MetricsHandle {
    MetricsHandle(Arc::new(NoopRenderer))
}

// ── HTTP metrics middleware ─────────────────────────────────────────────────

pub async fn http_metrics(request: Request<axum::body::Body>, next: Next) -> Response {
    let method = request.method().to_string();
    let path = request.uri().path().to_string();
    let start = Instant::now();

    let response = next.run(request).await;

    let status = response.status().as_u16().to_string();
    let duration = start.elapsed().as_secs_f64();

    metrics::counter!(HTTP_REQUESTS_TOTAL, "method" => method.clone(), "path" => path.clone(), "status" => status.clone())
        .increment(1);
    metrics::histogram!(HTTP_REQUEST_DURATION, "method" => method, "path" => path, "status" => status)
        .record(duration);

    response
}

// ── /metrics endpoint ───────────────────────────────────────────────────────

pub async fn metrics_handler(State(handle): State<MetricsHandle>) -> impl IntoResponse {
    (StatusCode::OK, handle.render())
}

// ── DB pool metrics (spawned as a periodic task) ────────────────────────────

pub fn spawn_db_pool_metrics(pool: PgPool) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(15));
        loop {
            interval.tick().await;
            let size = pool.size() as f64;
            let idle = pool.num_idle() as f64;
            metrics::gauge!(DB_POOL_ACTIVE).set(size - idle);
            metrics::gauge!(DB_POOL_IDLE).set(idle);
        }
    });
}

// ── WS metrics helpers ──────────────────────────────────────────────────────

pub fn ws_connection_opened() {
    metrics::counter!(WS_CONNECTIONS_TOTAL).increment(1);
    metrics::gauge!(WS_CONNECTIONS_ACTIVE).increment(1.0);
}

pub fn ws_connection_closed() {
    metrics::gauge!(WS_CONNECTIONS_ACTIVE).decrement(1.0);
}

pub fn ws_message_sent(event_type: &str) {
    metrics::counter!(WS_MESSAGES_SENT, "event_type" => event_type.to_string()).increment(1);
}

pub fn ws_message_received(event_type: &str) {
    metrics::counter!(WS_MESSAGES_RECEIVED, "event_type" => event_type.to_string()).increment(1);
}

// ── Business metrics helpers ────────────────────────────────────────────────

pub fn message_created() {
    metrics::counter!(MESSAGES_CREATED).increment(1);
}

pub fn set_active_users(count: f64) {
    metrics::gauge!(USERS_ACTIVE).set(count);
}
