# Burst

**Open-source team messaging built on [Barbacane](https://barbacane.dev).**

Burst is a focused, lightweight messaging tool for teams. No platform creep, no feature gating — just messaging that works.

## Why Burst?

The team messaging space has a gap. Rocket.Chat and Mattermost started as focused tools, then became platforms: project management, CRM, AI copilots, video calls. Features get gated behind enterprise licenses. Simple messaging tools become complex to operate.

Burst takes a different path:

- **Messaging only** — channels, DMs, threads, search, file sharing. Nothing else.
- **Fully open-source** — Apache-2.0, no enterprise edition, no feature gating.
- **Simple ops** — single binary, PostgreSQL, optional S3. Runs on a $5 VPS.
- **Barbacane-native** — uses the [Barbacane API gateway](https://barbacane.dev) for auth, rate limiting, and S3 proxying.

## Quick Start

```bash
# Clone and start
git clone https://github.com/barbacane-dev/burst
cd burst

# Start PostgreSQL + mock OIDC
make services

# Compile gateway + seed database
make gateway-compile
make seed

# Start everything (3 terminals)
make gateway    # Barbacane on :8080
make server     # Burst on :3000
make ui         # Vite on :5173
```

Open [http://localhost:5173](http://localhost:5173) and sign in with `alice` (any password).

Or with Docker:

```bash
make gateway-compile
docker compose -f docker/docker-compose.yaml up --build
```

## Architecture

```
Browser → Barbacane (:8080) → Burst (:3000) → PostgreSQL
               ↑                    ↓
          OIDC Provider        Barbacane S3 → RustFS [optional]
```

Barbacane handles authentication (OIDC/JWT), rate limiting, and S3 storage proxying. Burst handles messaging, WebSocket connections, and business logic. PostgreSQL stores everything.

## What's in the box

| Feature | Status |
|---------|--------|
| Channels (public, private) | Available |
| Direct messages | Available |
| Threaded replies | Available |
| Emoji reactions | Available |
| File sharing (images, documents) | Available |
| Full-text search | Available |
| Pinned messages | Available |
| Channel archival | Available |
| Admin panel | Available |
| Browser notifications | Available |
| Dark mode | Available |
| Prometheus metrics | Available |
| OpenTelemetry tracing | Available |
| S3 storage (via Barbacane) | Available |
| Multi-node (PG LISTEN/NOTIFY) | Available |
| Docker deployment | Available |

## Footprint

Burst is designed for lightweight infrastructure:

| Component | Memory (idle) | Image size |
|-----------|--------------|------------|
| Burst | ~2 MB | 173 MB |
| PostgreSQL | ~21 MB | 661 MB |
| RustFS (optional) | ~53 MB | 245 MB |
| **Total (minimal)** | **~23 MB** | **834 MB** |

The minimum deployment (Burst + PostgreSQL with local file storage) runs comfortably on a 512 MB VPS.
