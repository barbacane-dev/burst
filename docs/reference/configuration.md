## Configuration Reference

Burst reads its configuration from a TOML file. By default it looks for
`burst.toml` in the working directory. You can override the path with:

```bash
burst --config /path/to/burst.toml
```

Every TOML key can also be set via an environment variable. Environment variables
take precedence over the config file. See
[Environment Variables](./environment-variables.md) for the full mapping.

## [server]

Controls the HTTP server that handles API requests and WebSocket connections.

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `listen` | string | `"127.0.0.1:3000"` | Address and port for the main server |
| `admin_listen` | string | `"127.0.0.1:3001"` | Address and port for the admin server (health, metrics) |
| `shutdown_timeout_secs` | integer | `30` | Seconds to wait for in-flight requests during graceful shutdown |

Example:

```toml
[server]
listen = "0.0.0.0:3000"
admin_listen = "0.0.0.0:3001"
shutdown_timeout_secs = 15
```

The admin port exposes `/health` and `/metrics` (Prometheus format). It should
not be exposed to end users.

## [database]

Configures the PostgreSQL connection pool.

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `url` | string | *required* | PostgreSQL connection string |
| `max_connections` | integer | `10` | Maximum number of connections in the pool |

Example:

```toml
[database]
url = "postgres://burst:password@localhost:5432/burst"
max_connections = 20
```

Burst runs migrations automatically on startup. You do not need to run a
separate migration step.

## [storage]

Controls file storage for attachments.

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `backend` | string | `"local"` | Storage backend: `"local"` or `"gateway"` |
| `local_path` | string | `"./uploads"` | Directory for local file storage (only when `backend = "local"`) |
| `gateway_url` | string | *none* | URL of the Barbacane S3 sidecar (only when `backend = "gateway"`) |
| `gateway_api_key` | string | *none* | API key to authenticate with the S3 sidecar |
| `max_file_size` | string | `"50MB"` | Maximum size per uploaded file |
| `max_files_per_message` | integer | `10` | Maximum number of files attached to a single message |
| `blocked_extensions` | array | `["exe", "bat", "cmd", "scr", "ps1"]` | File extensions rejected on upload |
| `cleanup_interval_secs` | integer | `3600` | How often the background cleanup task runs (seconds) |
| `cleanup_retention_days` | integer | `30` | Days to keep soft-deleted files before permanent removal |

### Local storage

```toml
[storage]
backend = "local"
local_path = "/var/lib/burst/uploads"
max_file_size = "100MB"
```

### S3 via Barbacane sidecar

```toml
[storage]
backend = "gateway"
gateway_url = "http://127.0.0.1:8081"
gateway_api_key = "a-long-random-secret"
max_file_size = "100MB"
```

When using the `gateway` backend, `local_path` is ignored. The `gateway_url`
must point to the S3 sidecar, not the public gateway. See
[S3 Storage](../guide/s3-storage.md) for the full setup.

## [websocket]

Tunes WebSocket event broadcasting.

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `broadcast_capacity` | integer | `1024` | Channel capacity for broadcasting events to connected clients |
| `event_buffer_capacity` | integer | `256` | Per-connection buffer for outgoing events |

Example:

```toml
[websocket]
broadcast_capacity = 2048
event_buffer_capacity = 512
```

Increase `broadcast_capacity` if you see dropped events under high load.
The `event_buffer_capacity` controls how many events queue per WebSocket
connection before back-pressure kicks in.

## [telemetry]

Configures OpenTelemetry tracing.

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `otlp_endpoint` | string | *none* | OTLP gRPC endpoint for trace export (e.g., `http://localhost:4317`) |
| `trace_sample_rate` | float | `1.0` | Sampling rate from `0.0` (no traces) to `1.0` (all traces) |

Example:

```toml
[telemetry]
otlp_endpoint = "http://jaeger:4317"
trace_sample_rate = 0.1
```

When `otlp_endpoint` is not set, tracing is disabled and no spans are exported.
Burst creates child spans under Barbacane's root span when requests arrive
through the gateway.

## [broker]

Configures the event broker for multi-node deployments.

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `backend` | string | `"local"` | Broker backend: `"local"` or `"pg_notify"` |

Example:

```toml
[broker]
backend = "pg_notify"
```

- `"local"` — in-process broadcasting only. Suitable for single-node deployments.
- `"pg_notify"` — uses PostgreSQL `LISTEN/NOTIFY` to propagate events across
  nodes. Requires all nodes to share the same database. See
  [Multi-Node Deployment](../guide/multi-node.md) for details.

## Full Example

A production-ready configuration with S3 storage and PG NOTIFY:

```toml
[server]
listen = "0.0.0.0:3000"
admin_listen = "0.0.0.0:3001"
shutdown_timeout_secs = 30

[database]
url = "postgres://burst:password@db.internal:5432/burst"
max_connections = 15

[storage]
backend = "gateway"
gateway_url = "http://127.0.0.1:8081"
gateway_api_key = "a-long-random-secret"
max_file_size = "100MB"
max_files_per_message = 10
blocked_extensions = ["exe", "bat", "cmd", "scr", "ps1", "msi"]
cleanup_interval_secs = 3600
cleanup_retention_days = 30

[websocket]
broadcast_capacity = 2048
event_buffer_capacity = 512

[telemetry]
otlp_endpoint = "http://jaeger:4317"
trace_sample_rate = 0.1

[broker]
backend = "pg_notify"
```

## Minimal Example

The only required setting is the database URL. Everything else has sensible
defaults for single-node local development:

```toml
[database]
url = "postgres://burst:password@localhost:5432/burst"
```
