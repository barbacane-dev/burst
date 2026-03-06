# ADR-010: Observability

**Status:** Accepted
**Date:** 2026-03-06

## Context

Per [ADR-003](003-technology-stack.md), Burst uses the `tracing` crate for structured logging and aligns with Barbacane's telemetry approach. Per [ADR-009](009-deployment-and-operations.md), Burst runs behind a Barbacane gateway and exposes an admin port (`3001`) for operational endpoints.

Barbacane already provides comprehensive gateway-level observability (see Barbacane ADR-0010):

- **Metrics:** `barbacane_requests_total`, `barbacane_request_duration_seconds`, middleware/dispatch durations, active connections — all exposed via Prometheus on the admin port.
- **Traces:** W3C Trace Context propagation, per-middleware spans, OTLP export.
- **Logs:** Structured JSON to stdout with trace correlation.

Burst does not need to duplicate gateway observability. What Burst needs is application-level observability: database query performance, WebSocket connection lifecycle, message throughput, search latency, and business metrics (active users, messages per day). The goal is a complete picture when Barbacane and Burst observability are combined.

## Decision

### Three pillars, same as Barbacane

Burst follows the same observability structure as Barbacane: metrics, traces, logs — all based on OpenTelemetry standards, all emitted from the application, never blocking request processing.

### 1. Metrics

Prometheus metrics exposed on the admin port (`GET /metrics` on port `3001`):

#### HTTP metrics (application-level)

| Metric | Type | Labels |
|--------|------|--------|
| `burst_http_requests_total` | Counter | `method`, `path`, `status` |
| `burst_http_request_duration_seconds` | Histogram | `method`, `path`, `status` |

These complement Barbacane's gateway metrics. Barbacane measures total request latency (including auth, rate limiting, validation). Burst measures application processing time only — the difference reveals gateway overhead.

#### Database metrics

| Metric | Type | Labels |
|--------|------|--------|
| `burst_db_query_duration_seconds` | Histogram | `query` |
| `burst_db_pool_connections_active` | Gauge | — |
| `burst_db_pool_connections_idle` | Gauge | — |

`query` is a short label (e.g., `get_messages`, `create_message`), not the SQL itself. Pool metrics come from sqlx's built-in instrumentation.

#### WebSocket metrics

| Metric | Type | Labels |
|--------|------|--------|
| `burst_ws_connections_active` | Gauge | — |
| `burst_ws_connections_total` | Counter | — |
| `burst_ws_messages_sent_total` | Counter | `event_type` |
| `burst_ws_messages_received_total` | Counter | `event_type` |

#### Search metrics

| Metric | Type | Labels |
|--------|------|--------|
| `burst_search_query_duration_seconds` | Histogram | `backend` |
| `burst_search_index_lag_seconds` | Gauge | — |

`backend` is `postgres` or `typesense`. Index lag tracks how far behind the Typesense async sync is.

#### Business metrics

| Metric | Type | Labels |
|--------|------|--------|
| `burst_messages_created_total` | Counter | — |
| `burst_users_active` | Gauge | — |

Lightweight counters for operational dashboards. `users_active` tracks users with at least one WebSocket connection.

### 2. Traces

Burst participates in distributed traces initiated by Barbacane:

```
barbacane.request (root span — created by Barbacane)
├── barbacane.middleware.oidc-auth
├── barbacane.middleware.acl
├── barbacane.dispatch.http-upstream
│     └── burst.request (child span — created by Burst)
│           ├── burst.db.get_channel_members
│           ├── burst.db.insert_message
│           ├── burst.broker.publish
│           └── burst.response
```

Burst extracts the W3C `traceparent` header from incoming requests (forwarded by Barbacane) and creates child spans. This gives a single trace view from client → gateway → application → database.

#### Span naming convention

- `burst.request` — root application span per HTTP request.
- `burst.db.<query_name>` — database query spans.
- `burst.broker.publish` — event publication to the broker.
- `burst.ws.send` — WebSocket message dispatch.
- `burst.search.<backend>` — search query spans.

#### Export

Traces are exported via OTLP (gRPC or HTTP) to an OpenTelemetry Collector, same as Barbacane:

```toml
[telemetry]
otlp_endpoint = ""                     # e.g., "http://otel-collector:4317"
trace_sample_rate = 1.0                # 1.0 = all, 0.1 = 10%, 0.0 = disabled
```

When `otlp_endpoint` is empty, trace export is disabled (spans are still created for log correlation). Trace sampling is configurable — in production, 10-20% sampling is typical to manage volume.

### 3. Logs

Structured JSON to stdout via the `tracing` crate, consistent with [ADR-009](009-deployment-and-operations.md) and Barbacane's log format:

```json
{
  "timestamp": "2026-03-06T14:22:10.123Z",
  "level": "info",
  "target": "burst_server::api::messages",
  "message": "message created",
  "trace_id": "abc123def456",
  "span_id": "789012",
  "request_id": "req_abc",
  "user_id": "usr_123",
  "channel_id": "ch_456",
  "duration_ms": 8
}
```

#### Key conventions

- **`trace_id` on every log line** — correlates logs with distributed traces. Same trace ID that Barbacane uses, enabling cross-component log correlation.
- **`request_id`** — a per-request identifier, propagated from Barbacane's `X-Request-ID` or `X-Correlation-ID` header (set by Barbacane's `correlation-id` plugin).
- **Domain context** — `user_id`, `channel_id`, `message_id` are included as structured fields where relevant, not embedded in the message string.
- **No PII in logs** — usernames, email addresses, and message content are never logged. Only IDs.

#### Log levels

| Level | Usage |
|-------|-------|
| `error` | Unrecoverable failures: database connection lost, migration failure, panic recovery |
| `warn` | Recoverable issues: WebSocket auth failure, search sync lag, storage write retry |
| `info` | Significant events: server started, migration applied, user provisioned, WebSocket connected |
| `debug` | Request/response details, query execution, broker publish |
| `trace` | Wire-level detail: WebSocket frame contents, full SQL with params |

Default level is `info`. Configurable via `BURST_LOG` or `RUST_LOG` environment variable. `RUST_LOG` supports per-module filtering (e.g., `RUST_LOG=burst_server::realtime=debug,info`).

### What Barbacane already provides (no duplication)

Burst does not re-implement these — they are handled at the gateway layer:

| Concern | Barbacane provides |
|---------|-------------------|
| Request rate by route | `barbacane_requests_total` with path/method labels |
| Auth failure metrics | `barbacane_requests_total{status="401"}` |
| Rate limiting metrics | `barbacane_middleware_duration_seconds{middleware="rate-limit"}` |
| TLS metrics | `barbacane_active_connections` |
| Request/response size | `barbacane_request_size_bytes`, `barbacane_response_size_bytes` |
| Gateway latency | `barbacane_request_duration_seconds` |
| Upstream call latency | `barbacane_dispatch_duration_seconds` |

Combined, Barbacane metrics + Burst metrics give a complete picture without overlap.

### Admin port endpoints

All observability endpoints are served on the admin port (per [ADR-009](009-deployment-and-operations.md)):

| Endpoint | Description |
|----------|-------------|
| `GET /health/live` | Liveness probe |
| `GET /health/ready` | Readiness probe (DB check) |
| `GET /metrics` | Prometheus metrics (text format) |

### Implementation

Burst uses the standard Rust observability stack:

- **`tracing`** — structured logging and span instrumentation. Already used by Axum, sqlx, and tokio, so library-level spans are included automatically.
- **`tracing-subscriber`** — JSON formatter for stdout output.
- **`tracing-opentelemetry`** — bridges `tracing` spans to OpenTelemetry for OTLP export.
- **`metrics` + `metrics-exporter-prometheus`** — Prometheus metric collection and exposition. Lightweight, no external dependency.

This is the same crate ecosystem Barbacane uses, ensuring consistent behaviour and familiar patterns for contributors working across both projects.

## Consequences

- **No observability infrastructure is required to run Burst.** Logs go to stdout, metrics are available on the admin port if scraped, traces export only when an OTLP endpoint is configured. A minimal deployment works with just `docker logs` and nothing else.
- **Barbacane + Burst observability is complementary, not redundant.** Gateway metrics cover auth, rate limiting, and routing. Burst metrics cover DB, WebSocket, search, and business logic. A single Grafana dashboard combining both namespaces (`barbacane_*` + `burst_*`) provides full visibility.
- **Distributed tracing crosses the gateway boundary.** A single trace shows the full request lifecycle: client → Barbacane (auth, middleware) → Burst (handler, DB, broker). This is the primary debugging tool for latency investigations.
- **No PII in logs or traces** is a hard rule, not a guideline. Message content, usernames, and emails are never emitted. This simplifies compliance and makes it safe to ship logs to external aggregators.
- **The `tracing` crate provides instrumentation for free** from Axum (request spans), sqlx (query spans), and tokio (task spans). Burst adds domain-specific spans on top of what the ecosystem already provides.
