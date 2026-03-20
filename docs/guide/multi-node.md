## Multi-Node Deployment

You can run multiple Burst instances behind a load balancer for high availability
and horizontal scaling. This guide covers the requirements and configuration.

## Prerequisites

Horizontal scaling requires two shared resources:

1. **Shared PostgreSQL database** — all nodes connect to the same database.
2. **S3-compatible storage** — files must be accessible from every node.
   Local filesystem storage does not work in multi-node setups.

See [S3 Storage](./s3-storage.md) for setting up S3 via the Barbacane sidecar.

## Architecture Overview

```
                    Load Balancer
                    /     |     \
                   /      |      \
            Node A     Node B     Node C
           --------   --------   --------
           Burst      Burst      Burst
           S3 sidecar S3 sidecar S3 sidecar
                   \      |      /
                    \     |     /
                   PostgreSQL + S3
```

Each node runs:

- A **Burst server** instance.
- An **S3 sidecar** (Barbacane instance) for file storage.

Shared across all nodes:

- A **PostgreSQL** database.
- An **S3 provider** (RustFS, MinIO, or AWS S3).
- A **public Barbacane gateway** (or one per node — see below).

## PG LISTEN/NOTIFY Broker

In a single-node deployment, events (new messages, typing indicators, reactions)
are broadcast in-process. In multi-node mode, you enable the PostgreSQL-based
broker so events propagate across all nodes.

```toml
[broker]
backend = "pg_notify"
```

Or with an environment variable:

```bash
BURST_BROKER_BACKEND=pg_notify
```

The broker uses PostgreSQL's built-in `LISTEN/NOTIFY` mechanism. Each Burst
instance subscribes to notification channels on the shared database. When a node
produces an event, it issues a `NOTIFY` that all other nodes receive.

This approach requires **no additional infrastructure** — if you already have
PostgreSQL, you have the broker.

## Event Deduplication

Each Burst node generates a unique `node_id` at startup. Events carry this ID
so that a node can skip events it originated. This prevents duplicate
processing — the originating node already handled the event locally before
broadcasting it.

You do not need to configure `node_id`. It is generated automatically.

## Sticky Sessions

Sticky sessions (session affinity) are **not required**. Any node can serve
any request because:

- All state lives in PostgreSQL.
- File storage is shared via S3.
- WebSocket connections are stateless from the load balancer's perspective —
  events reach every node through PG NOTIFY.

You can use round-robin, least-connections, or any other load balancing strategy.

## Gateway Topology

You have two options for the public Barbacane gateway:

### Shared gateway (recommended)

Run a single public gateway that load-balances across all Burst nodes.
Configure `BURST_UPSTREAM_URL` to point to the load balancer:

```bash
BURST_UPSTREAM_URL=http://load-balancer:3000
BURST_UPSTREAM_WS_URL=ws://load-balancer:3000
```

This is the simplest setup and works well when the gateway and load balancer
run on the same machine or network.

### Gateway per node

Run a public gateway on each node, each pointing to its local Burst instance.
An external load balancer then distributes traffic across the gateways:

```bash
# On each node
BURST_UPSTREAM_URL=http://127.0.0.1:3000
BURST_UPSTREAM_WS_URL=ws://127.0.0.1:3000
```

This avoids an extra network hop but requires more Barbacane processes.

## S3 Sidecar

Each node runs its own S3 sidecar. The sidecar is lightweight and only proxies
file requests to the shared S3 backend, so running one per node adds negligible
overhead.

All sidecars share the same S3 credentials and bucket:

```bash
# Same on every node
BURST_S3_REGION=us-east-1
BURST_S3_BUCKET=burst-files
BURST_S3_ACCESS_KEY_ID=minioadmin
BURST_S3_SECRET_ACCESS_KEY=minioadmin
BURST_S3_ENDPOINT=http://s3.internal:9000
BURST_S3_API_KEY=a-long-random-secret
```

Each node's Burst instance points to its local sidecar:

```toml
[storage]
backend = "gateway"
gateway_url = "http://127.0.0.1:8081"
gateway_api_key = "a-long-random-secret"
```

## Database Connections

With multiple nodes sharing one database, pay attention to connection pool sizing.
Each node opens up to `database.max_connections` connections. If you run 3 nodes
with the default of 10 connections each, your database must support at least 30
concurrent connections (plus connections for PG NOTIFY listeners).

```toml
[database]
max_connections = 10
```

Reduce this value if you run many nodes or if your PostgreSQL instance has a low
`max_connections` setting.

## Minimal Multi-Node Configuration

Here is a complete Burst configuration for a node in a multi-node deployment:

```toml
[server]
listen = "0.0.0.0:3000"
admin_listen = "0.0.0.0:3001"

[database]
url = "postgres://burst:password@db.internal:5432/burst"
max_connections = 10

[storage]
backend = "gateway"
gateway_url = "http://127.0.0.1:8081"
gateway_api_key = "a-long-random-secret"

[broker]
backend = "pg_notify"
```

The only difference from a single-node setup is `broker.backend = "pg_notify"`
and `storage.backend = "gateway"`.

## Health Checks

Each node exposes a health endpoint on the admin port (default 3001). Configure
your load balancer to probe this endpoint:

```
GET http://<node>:3001/health
```

A healthy node returns HTTP 200. Remove unhealthy nodes from the pool
automatically.

## Scaling Considerations

- **CPU** — Burst is lightweight. A single node can handle thousands of
  concurrent WebSocket connections. Scale horizontally for redundancy more
  than raw capacity.
- **Database** — PostgreSQL is usually the bottleneck. Use connection pooling
  (PgBouncer) if you run many nodes.
- **PG NOTIFY** — suitable for small to medium deployments (tens of nodes).
  For larger clusters, the broker trait is designed to support alternative
  backends in the future.
