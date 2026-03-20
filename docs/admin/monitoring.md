## Monitoring & Observability

Burst exposes Prometheus metrics, OpenTelemetry traces, and structured JSON logs to give you full visibility into the running system.

## Prometheus Metrics

Burst serves metrics in Prometheus exposition format on the **admin port** (default: `3001`):

```
GET http://localhost:3001/metrics
```

The admin port is separate from the main application port and should not be exposed publicly.

### Available Metrics

| Metric | Type | Description |
|--------|------|-------------|
| `burst_http_requests_total` | counter | Total HTTP requests, labeled by method, path, and status code. |
| `burst_http_request_duration_seconds` | histogram | Request latency distribution in seconds, labeled by method and path. |
| `burst_db_pool_connections_active` | gauge | Number of active database connections in the pool. |
| `burst_db_pool_connections_idle` | gauge | Number of idle database connections in the pool. |
| `burst_ws_connections_active` | gauge | Number of currently open WebSocket connections. |
| `burst_ws_connections_total` | counter | Total WebSocket connections opened since startup. |
| `burst_ws_messages_sent_total` | counter | Total WebSocket messages sent to clients. |
| `burst_ws_messages_received_total` | counter | Total WebSocket messages received from clients. |
| `burst_messages_created_total` | counter | Total chat messages created. |
| `burst_users_active` | gauge | Number of users with an active WebSocket connection. |

### Grafana Scrape Configuration

Add the following to your Prometheus configuration to scrape Burst metrics:

```yaml
scrape_configs:
  - job_name: "burst"
    scrape_interval: 15s
    static_configs:
      - targets: ["burst-server:3001"]
    metrics_path: /metrics
```

If you run multiple Burst instances, add each one as a target or use service discovery.

### Useful Grafana Panels

Here are some queries to get started with dashboards:

**Request rate:**
```promql
rate(burst_http_requests_total[5m])
```

**P95 request latency:**
```promql
histogram_quantile(0.95, rate(burst_http_request_duration_seconds_bucket[5m]))
```

**Active WebSocket connections:**
```promql
burst_ws_connections_active
```

**Database pool saturation:**
```promql
burst_db_pool_connections_active / (burst_db_pool_connections_active + burst_db_pool_connections_idle)
```

## OpenTelemetry Tracing

Burst supports exporting traces via the OpenTelemetry Protocol (OTLP). When a request passes through the Barbacane gateway, Burst creates child spans under the gateway's root span, giving you end-to-end visibility.

Enable OTLP export by setting the endpoint in your configuration:

```toml
[telemetry]
otlp_endpoint = "http://otel-collector:4317"
```

Traces include spans for HTTP handlers, database queries, and WebSocket event processing. Each span carries relevant attributes such as the user ID, channel ID, and operation type.

## Structured Logging

Burst emits structured JSON logs to stdout. Each log entry includes:

| Field | Description |
|-------|-------------|
| `timestamp` | ISO 8601 timestamp. |
| `level` | Log level (`trace`, `debug`, `info`, `warn`, `error`). |
| `message` | Human-readable log message. |
| `target` | Rust module path that produced the log. |
| `trace_id` | OpenTelemetry trace ID (when tracing is enabled). |
| `span_id` | OpenTelemetry span ID (when tracing is enabled). |

The `trace_id` and `span_id` fields let you correlate log entries with distributed traces in your observability backend (Jaeger, Grafana Tempo, etc.).

### Log Level

Control the log level with the `BURST_LOG` environment variable:

```bash
# Show info and above (default)
BURST_LOG=info

# Enable debug logging for Burst only
BURST_LOG=info,burst=debug

# Enable trace logging for the WebSocket module
BURST_LOG=info,burst_server::ws=trace
```

The syntax follows the [`tracing-subscriber` EnvFilter format](https://docs.rs/tracing-subscriber/latest/tracing_subscriber/filter/struct.EnvFilter.html).

## Health Check

The admin port also exposes a health endpoint:

```
GET http://localhost:3001/health
```

Returns `200 OK` when the server is ready to accept requests. Use this for container liveness and readiness probes.

## See Also

- [Configuration Reference](../reference/configuration.md) for all telemetry settings.
- [Docker Compose](../guide/docker.md) for setting up collectors alongside Burst.
