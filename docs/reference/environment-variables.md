## Environment Variables

Every Burst configuration option can be set via an environment variable.
Environment variables take precedence over values in `burst.toml`.

Variables fall into two categories: **Burst server** variables (consumed by the
Burst binary) and **Barbacane gateway** variables (consumed by the Barbacane
processes via `env://` references in the OpenAPI spec).

## Burst Server Variables

These variables configure the Burst server itself. Each one maps directly to a
key in `burst.toml` (see [Configuration Reference](./configuration.md)).

| Variable | Config equivalent | Default | Description |
|----------|------------------|---------|-------------|
| `BURST_DATABASE_URL` | `database.url` | *required* | PostgreSQL connection string |
| `BURST_DATABASE_MAX_CONNECTIONS` | `database.max_connections` | `10` | Maximum pool connections |
| `BURST_SERVER_LISTEN` | `server.listen` | `127.0.0.1:3000` | Main server listen address |
| `BURST_SERVER_ADMIN_LISTEN` | `server.admin_listen` | `127.0.0.1:3001` | Admin server listen address |
| `BURST_SERVER_SHUTDOWN_TIMEOUT_SECS` | `server.shutdown_timeout_secs` | `30` | Graceful shutdown timeout |
| `BURST_STORAGE_BACKEND` | `storage.backend` | `local` | Storage backend: `local` or `gateway` |
| `BURST_STORAGE_LOCAL_PATH` | `storage.local_path` | `./uploads` | Local storage directory |
| `BURST_STORAGE_GATEWAY_URL` | `storage.gateway_url` | *none* | S3 sidecar URL |
| `BURST_STORAGE_GATEWAY_API_KEY` | `storage.gateway_api_key` | *none* | S3 sidecar API key |
| `BURST_STORAGE_MAX_FILE_SIZE` | `storage.max_file_size` | `20971520` | Max upload size, in bytes |
| `BURST_STORAGE_MAX_FILES_PER_MESSAGE` | `storage.max_files_per_message` | `10` | Max files per message |
| `BURST_STORAGE_CLEANUP_INTERVAL_SECS` | `storage.cleanup_interval_secs` | `3600` | Cleanup task interval |
| `BURST_STORAGE_CLEANUP_RETENTION_DAYS` | `storage.cleanup_retention_days` | `30` | Soft-delete retention |
| `BURST_WEBSOCKET_BROADCAST_CAPACITY` | `websocket.broadcast_capacity` | `1024` | Broadcast channel capacity |
| `BURST_WEBSOCKET_EVENT_BUFFER_CAPACITY` | `websocket.event_buffer_capacity` | `256` | Per-connection event buffer |
| `BURST_TELEMETRY_OTLP_ENDPOINT` | `telemetry.otlp_endpoint` | *none* | OTLP gRPC endpoint |
| `BURST_TELEMETRY_TRACE_SAMPLE_RATE` | `telemetry.trace_sample_rate` | `1.0` | Trace sampling rate |
| `BURST_BROKER_BACKEND` | `broker.backend` | `local` | Broker: `local` or `pg_notify` |

## Barbacane Gateway Variables

These variables are referenced by `env://` placeholders in the OpenAPI specs
(`specs/burst-api.yaml` and `specs/burst-s3.yaml`). They are read by the
Barbacane gateway processes, not by Burst itself.

### Gateway Process Variables

Read by the Barbacane binary itself, not through `env://` references. Apply to both gateway processes.

| Variable | Description |
|----------|-------------|
| `BARBACANE_ALLOW_INTERNAL_EGRESS` | Set to `true`. Disables Barbacane's plugin SSRF guard, which otherwise blocks egress to loopback, private-network and link-local addresses. Every Burst upstream (the Burst API, the S3 sidecar, RustFS, and an IdP on the same network) is such an address, so without it `oidc-auth` cannot fetch discovery/JWKS and every authenticated request is rejected with 401 |

### Public Gateway Variables

Used by the public Barbacane gateway (port 8080).

| Variable | Description |
|----------|-------------|
| `BURST_UPSTREAM_URL` | HTTP URL of the Burst server (e.g., `http://127.0.0.1:3000`) |
| `BURST_UPSTREAM_WS_URL` | WebSocket URL of the Burst server (e.g., `ws://127.0.0.1:3000`) |
| `BURST_OIDC_ISSUER_URL` | OIDC issuer URL for token validation (e.g., `https://auth.example.com/realms/burst`) |
| `BURST_OIDC_ISSUER_OVERRIDE` | Optional internal OIDC issuer URL when the gateway reaches the IdP via a different address |
| `BURST_OIDC_GROUPS_CLAIM` | JWT claim containing user groups/roles. Defaults to `roles`. Set to `groups` for Authelia, or whatever claim your IdP uses |

### S3 Sidecar Variables

Used by the S3 Barbacane sidecar (port 8081).

| Variable | Description |
|----------|-------------|
| `BURST_S3_REGION` | S3 region (e.g., `us-east-1`) |
| `BURST_S3_BUCKET` | S3 bucket name (e.g., `burst-files`) |
| `BURST_S3_ACCESS_KEY_ID` | S3 access key ID |
| `BURST_S3_SECRET_ACCESS_KEY` | S3 secret access key |
| `BURST_S3_ENDPOINT` | S3-compatible endpoint URL (e.g., `http://127.0.0.1:9000` for RustFS/MinIO) |
| `BURST_S3_API_KEY` | API key that authenticates Burst to the sidecar (must match `BURST_STORAGE_GATEWAY_API_KEY`) |

## Shared Secrets

Two variables must match between Burst and the S3 sidecar:

| Burst variable | Sidecar variable | Purpose |
|----------------|------------------|---------|
| `BURST_STORAGE_GATEWAY_API_KEY` | `BURST_S3_API_KEY` | Authenticates Burst to the S3 sidecar |

If these values do not match, file uploads and downloads will fail with
HTTP 403 errors.

## .env.example

Here is a complete `.env.example` you can copy and adapt:

```bash
# ── Burst Server ──────────────────────────────────────────────
BURST_DATABASE_URL=postgres://burst:password@localhost:5432/burst
BURST_SERVER_LISTEN=127.0.0.1:3000
BURST_SERVER_ADMIN_LISTEN=127.0.0.1:3001
BURST_STORAGE_BACKEND=local
BURST_STORAGE_LOCAL_PATH=./uploads
# BURST_STORAGE_BACKEND=gateway
# BURST_STORAGE_GATEWAY_URL=http://127.0.0.1:8081
# BURST_STORAGE_GATEWAY_API_KEY=change-me
# BURST_BROKER_BACKEND=pg_notify

# ── Both Barbacane processes ─────────────────────────────────
BARBACANE_ALLOW_INTERNAL_EGRESS=true

# ── Public Gateway (Barbacane :8080) ─────────────────────────
BURST_UPSTREAM_URL=http://127.0.0.1:3000
BURST_UPSTREAM_WS_URL=ws://127.0.0.1:3000
BURST_OIDC_ISSUER_URL=https://auth.example.com/realms/burst
# BURST_OIDC_ISSUER_OVERRIDE=http://keycloak:8080/realms/burst
BURST_OIDC_GROUPS_CLAIM=roles

# ── S3 Sidecar (Barbacane :8081) ─────────────────────────────
BURST_S3_REGION=us-east-1
BURST_S3_BUCKET=burst-files
BURST_S3_ACCESS_KEY_ID=minioadmin
BURST_S3_SECRET_ACCESS_KEY=minioadmin
BURST_S3_ENDPOINT=http://127.0.0.1:9000
BURST_S3_API_KEY=change-me
```

## Precedence

When the same setting is defined in multiple places, the following precedence
applies (highest to lowest):

1. **Environment variable** — always wins.
2. **Config file** (`burst.toml`) — used when no env var is set.
3. **Default value** — used when neither env var nor config file defines the key.
