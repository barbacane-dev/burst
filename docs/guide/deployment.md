# Deployment

This guide covers deploying Burst in production. The minimum setup is a single binary plus PostgreSQL.

## Requirements

- **PostgreSQL 16+** (17 recommended)
- **Barbacane gateway** (handles auth, rate limiting, S3 proxy)
- An **OIDC provider** (Keycloak, Auth0, Okta, etc.)

Optional:
- **S3-compatible storage** (RustFS, MinIO, AWS S3) for file attachments in multi-node deployments
- **OTLP collector** (Jaeger, Grafana Tempo) for distributed tracing

## Quick Start (Single Binary)

```bash
# 1. Download the binary (from GitHub Releases)
curl -fSL -o burst https://github.com/barbacane-dev/burst/releases/latest/download/burst-linux-amd64
chmod +x burst

# 2. Create config file
cat > burst.toml <<'TOML'
[server]
listen = "0.0.0.0:3000"
admin_listen = "0.0.0.0:3001"

[database]
url = "postgres://burst:secretpass@db.example.com:5432/burst"
max_connections = 20
TOML

# 3. Run (migrations applied automatically on startup)
./burst burst.toml
```

## Verify

```bash
# Health check (admin port)
curl http://localhost:3001/health/ready
# Prometheus metrics
curl http://localhost:3001/metrics
```

## Deployment Topology

### Sidecar (same host)

```
Barbacane (:8080) → Burst (:3000)  [same VM/pod]
                         ↓
                    PostgreSQL
```

Burst listens on `127.0.0.1:3000` (localhost only). Barbacane forwards authenticated requests.

### Separate services (recommended, zero trust)

```
Barbacane (host A) → Burst (host B) → PostgreSQL (host C)
```

All traffic between services uses mTLS or a network policy. Burst's API port (3000) is never exposed publicly. Only the admin port (3001) is accessible for health checks and metrics scraping.

## Startup Sequence

1. Load configuration (TOML + env var overrides)
2. Connect to PostgreSQL (fails fast if unreachable)
3. Run embedded migrations automatically
4. Initialize storage backend, broker, metrics
5. Start listening on API port (3000) and admin port (3001)

## Graceful Shutdown

On `SIGTERM` or `SIGINT`:

1. Stop accepting new connections
2. Send WebSocket Close(1001) frames to connected clients
3. Wait for in-flight requests (configurable timeout, default 30s)
4. Close database connection pool
5. Flush OpenTelemetry spans
6. Exit with code 0
