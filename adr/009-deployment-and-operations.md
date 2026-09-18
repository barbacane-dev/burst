# ADR-009: Deployment & Operations

**Status:** Accepted, amended by [ADR-014](014-gateway-optional-deployment.md) (2026-09-15): the minimum deployment is the Burst binary plus PostgreSQL (standalone tier); the Barbacane topologies below describe the gateway tier.
**Date:** 2026-03-06

## Context

Per [ADR-001](001-project-vision-and-scope.md), Burst must be simple to operate — a small team should be able to install, configure, and maintain it without dedicated infrastructure expertise. Per [ADR-003](003-technology-stack.md), Burst compiles to a single API binary with embedded database migrations; the frontend is served separately by nginx. Per [ADR-006](006-authentication-and-authorization.md), Burst sits behind a Barbacane API gateway.

We need to define how Burst is distributed, configured, started, and kept running in production.

## Decision

### Distribution

#### Single binary

The primary distribution is a statically linked binary for Linux (x86_64, aarch64) and macOS (aarch64). The binary includes:

- The compiled Rust API server.
- Database migration files, embedded and applied on startup.

The React frontend is served separately by nginx (see `docker/Dockerfile.nginx`). Burst is a pure API server — it does not serve static files. No runtime dependencies beyond PostgreSQL and a filesystem (or S3-compatible store for attachments).

#### Docker image

An official Docker image is published for containerised deployments:

Two Docker images:

- **Burst API** (`Dockerfile`) — Rust binary only, no frontend:
  ```dockerfile
  FROM debian:bookworm-slim
  COPY burst /usr/local/bin/burst
  EXPOSE 3000 3001
  ENTRYPOINT ["burst"]
  ```
- **nginx SPA** (`docker/Dockerfile.nginx`) — builds the React frontend and serves it via nginx, proxying `/api/*` and `/ws` to Barbacane.

Minimal base images. No build tools at runtime. Both are multi-arch (amd64, arm64).

#### GitHub Releases

Pre-built binaries and Docker images are published on every tagged release. CI builds cover:

- Linux x86_64 (primary)
- Linux aarch64 (ARM servers, Raspberry Pi)
- macOS aarch64 (Apple Silicon, development)
- Docker multi-arch image (ghcr.io)

### Configuration

Burst is configured via a single TOML file, with environment variable overrides for secrets and containerised deployments.

```toml
[server]
listen = "0.0.0.0:3000"

[database]
url = "postgres://burst:password@localhost:5432/burst"
max_connections = 20

[storage]
backend = "local"                      # "local" or "s3"
local_path = "/var/lib/burst/uploads"

[storage.s3]                           # only when backend = "s3"
bucket = "burst-uploads"
region = "us-east-1"
endpoint = ""                          # custom endpoint for MinIO, etc.

[search]
backend = "postgres"                   # "postgres" or "typesense"

[search.typesense]                     # only when backend = "typesense"
url = "http://localhost:8108"
api_key = ""

[auth]
jwt_secret = ""                        # required for local auth
jwt_expiry = "15m"
refresh_expiry = "7d"
```

Environment variables override any config value using the pattern `BURST_<SECTION>_<KEY>` (e.g., `BURST_DATABASE_URL`, `BURST_AUTH_JWT_SECRET`). This follows the common convention for twelve-factor apps and works naturally with Docker/Kubernetes secret injection.

CLI flags are minimal — just `--config <path>` to specify the config file location (defaults to `./burst.toml`).

### Startup sequence

On `burst serve` (or just `burst` with no subcommand):

1. **Load configuration** — read TOML file, apply environment variable overrides, validate.
2. **Connect to PostgreSQL** — establish the connection pool, fail fast if unreachable.
3. **Run migrations** — apply any pending sqlx migrations. All migrations are embedded in the binary. This ensures the schema is always up to date without a separate migration step.
4. **Initialise services** — storage backend, search backend (sync index if Typesense), in-process broker.
5. **Assemble Axum router** — mount API routes from `burst-server`.
6. **Start listening** — bind to the configured address, log the startup banner.

If any step fails, Burst exits with a clear error message and non-zero exit code. No partial startup, no degraded mode.

### Barbacane gateway topology

Per [ADR-006](006-authentication-and-authorization.md), Barbacane sits in front of Burst and handles authentication, rate limiting, and route-level authorization. Two deployment patterns:

#### Sidecar (simple deployments)

```
                  ┌─────────────────────────────────┐
                  │  Pod / VM                        │
                  │                                  │
Client ──────► Barbacane (port 8080) ──► Burst (port 3000)
                  │   auth, rate-limit,              │
                  │   ACL, observability             │
                  └─────────────────────────────────┘
```

Barbacane and Burst run as separate processes in the same pod or VM. Barbacane listens on the public port, Burst listens on localhost only. The Barbacane spec points to `http://localhost:3000` as the upstream. Simple to set up, no network configuration needed.

#### Separate services (recommended for production / zero trust)

```
Client ──────► Barbacane (pod/host A) ──────► Burst (pod/host B)
                                         (network policy / mTLS)
```

Barbacane and Burst run in separate pods or hosts. Communication between them is secured via Kubernetes network policies, service mesh mTLS, or private networking. This is the recommended topology for production deployments because:

- **Zero trust:** Burst is never directly reachable from the public network. Only Barbacane can reach it.
- **Independent scaling:** Barbacane and Burst scale independently based on their respective loads.
- **Blast radius:** A compromise of one component does not automatically grant access to the other's process space.

In both topologies, the Barbacane OpenAPI spec serves as the single source of truth for routing, security, and rate limiting. The spec lives in the Burst repository (`specs/`) and is deployed alongside the application.

### Health checks

Burst runs a separate admin listener on a dedicated port (default `3001`) for health and operational endpoints. This port is never exposed publicly and is not routed through Barbacane:

- `GET /health/live` — returns `200 OK` if the process is running. No dependency checks. Used for Kubernetes liveness probe.
- `GET /health/ready` — returns `200 OK` if PostgreSQL is reachable and migrations are applied. Used for readiness probe.

```toml
[server]
listen = "0.0.0.0:3000"        # application port (behind Barbacane)
admin_listen = "0.0.0.0:3001"  # admin port (health checks, future metrics)
```

Separating admin endpoints onto their own port ensures they are never accidentally exposed through the gateway and cannot be reached by external clients — even if Barbacane is misconfigured. In Kubernetes, the admin port is referenced by `livenessProbe` and `readinessProbe` but excluded from the Service/Ingress definition. This follows the same pattern as Barbacane's own admin introspection port (see Barbacane ADR-0022).

### Graceful shutdown

On SIGTERM or SIGINT:

1. Stop accepting new connections.
2. Close WebSocket connections with a `1001 Going Away` close frame (clients auto-reconnect with gap-fill per [ADR-004](004-real-time-architecture.md)).
3. Wait for in-flight HTTP requests to complete (with a timeout, e.g., 30 seconds).
4. Close the database pool.
5. Exit with code 0.

This enables zero-downtime rolling deployments in Kubernetes or behind a load balancer.

### Database management

- **Migrations are automatic.** Applied on startup, every time. No separate `burst migrate` command needed (though one may be added for dry-run / status checks).
- **Backup is the operator's responsibility.** Burst does not manage PostgreSQL backups. Documentation will recommend `pg_dump` / WAL archiving and point to standard PostgreSQL backup guides.
- **Connection pool sizing** defaults to 20 connections, configurable. Adequate for small-to-medium teams. Large deployments tune via config.

### Multi-node

For horizontal scaling (multiple Burst instances behind a load balancer):

- **Stateless API.** No server-side sessions — auth is token-based (validated by Barbacane). Any instance can handle any request.
- **PG LISTEN/NOTIFY** synchronises real-time events across instances (per [ADR-004](004-real-time-architecture.md)). No additional infrastructure needed.
- **Sticky sessions are not required** for HTTP. WebSocket connections are inherently sticky (long-lived), but reconnection can land on any node (gap-fill ensures continuity).
- **File storage must be shared** in multi-node setups. Local filesystem requires a shared mount (NFS, EFS). S3-compatible storage is recommended for multi-node.

### Logging

Structured JSON logging to stdout, consistent with twelve-factor app principles. Log level configurable via `BURST_LOG` or `RUST_LOG` environment variable. The `tracing` crate provides structured fields (request ID, user ID, channel ID) on every log line.

Burst does not write log files — log aggregation is delegated to the operator's infrastructure (Docker log driver, Kubernetes logging, systemd journal).

## Consequences

- **Automatic migrations on startup** means operators never run a separate migration step. The trade-off is that a bad migration can block startup — mitigated by testing migrations in CI and staging environments.
- **Burst API + nginx + Barbacane + PostgreSQL** is the minimum deployment. `docker compose -f docker/docker-compose.yaml up` runs the full stack. For teams that already run a reverse proxy, nginx can be replaced.
- **The Barbacane sidecar topology** adds one more process but zero auth code in Burst. For teams that already run Barbacane, the gateway is shared. For Burst-only deployments, the sidecar is a lightweight addition that brings production-grade auth and rate limiting out of the box.
- **No clustering coordination** beyond PostgreSQL. No Raft, no gossip protocol, no distributed consensus. PG LISTEN/NOTIFY handles cross-node events. This limits throughput to what PostgreSQL can handle, which is more than enough at team scale.
- **Health endpoints outside Barbacane** means the orchestrator can probe Burst directly. If Barbacane is down, the readiness probe still tells the orchestrator whether Burst itself is healthy — useful for debugging which component is failing.
