# Docker Compose

> Commands below use `docker`. Podman is a drop-in alternative: `podman compose`
> and `podman build` take the same arguments, and podman reads a `Dockerfile`
> without renaming. The Makefile picks whichever is installed, so `make` targets
> need no change.

The full-stack Docker Compose runs Burst, PostgreSQL, RustFS, and Barbacane in containers with a zero-trust topology.

## Prerequisites

```bash
# Compile the Barbacane gateway artifact (required once, or after spec changes)
make gateway-compile
```

## Starting

```bash
docker compose -f docker/docker-compose.yaml up --build
```

This starts four services:

| Service | Role | Exposed |
|---------|------|---------|
| **barbacane** | API gateway (auth, rate limiting, S3 proxy) | `:8080` (public) |
| **burst** | Messaging API server | `:3001` admin only |
| **postgres** | Database | Internal only |
| **rustfs** | S3-compatible object storage | Internal only |

## Zero-Trust Topology

```
Browser → Barbacane (:8080) → Burst (internal :3000)
               ↑                    ↓
          OIDC Provider        Barbacane S3 → RustFS
                                    ↓
                              PostgreSQL
```

- Burst's API port (3000) is **not exposed** to the host. All API traffic goes through Barbacane.
- Burst's admin port (3001) is exposed for health checks and Prometheus metrics scraping.
- PostgreSQL and RustFS are only accessible on the Docker internal network.

## Configuration

The compose uses environment variables. Override defaults by setting them in your shell or a `.env` file:

```bash
# Required for the gateway (set in the compose file by default)
BURST_UPSTREAM_URL=http://burst:3000
BURST_UPSTREAM_WS_URL=ws://burst:3000
BURST_OIDC_ISSUER_URL=http://host.docker.internal:9099/burst
# Upstreams are on the private compose network; Barbacane's plugin SSRF
# guard blocks that egress unless set (set in the compose file by default)
BARBACANE_ALLOW_INTERNAL_EGRESS=true
```

See [Environment Variables](../reference/environment-variables.md) for the complete list.

## Memory Footprint

| Container | Memory (idle) |
|-----------|--------------|
| Burst | ~2 MB |
| PostgreSQL | ~21 MB |
| RustFS | ~53 MB |
| Barbacane | ~10 MB |
| **Total** | **~86 MB** |

The full stack runs comfortably on a 512 MB VPS.

## Building the Image Separately

```bash
docker build -t burst .
```

The Dockerfile uses a multi-stage build: Rust builder on `rust:1-bookworm`, runtime on `debian:bookworm-slim`. The final image is ~173 MB.

## Stopping

```bash
docker compose -f docker/docker-compose.yaml down
```

Add `-v` to also remove data volumes (PostgreSQL data, RustFS objects).
