# ADR-003: Technology Stack

**Status:** Accepted, amended by [ADR-014](014-gateway-optional-deployment.md) (2026-09-15): authentication is delegated to Barbacane in the gateway tier only; the standalone tier validates tokens and serves the SPA from the Burst binary.
**Date:** 2026-03-06

## Context

Per [ADR-001](001-project-vision-and-scope.md), Burst must be lightweight, fast, and simple to operate. Per [ADR-002](002-core-feature-set.md), Burst's API sits behind the Barbacane API gateway, and the product needs real-time messaging (WebSocket), full-text search, and file storage.

We also want to maximise knowledge sharing with the Barbacane project. Barbacane is built with Rust (Tokio, Axum, sqlx, PostgreSQL) on the backend and React (Vite, Tailwind CSS) on the frontend. Sharing the same stack across both projects means:

- A single talent pool for contributors and maintainers.
- Shared libraries, patterns, and tooling (CI, linting, testing).
- Credibility — we use the same technologies we ask others to trust for their API infrastructure.

Alternatives considered:

| Option | Pros | Cons |
|--------|------|------|
| **Go** | Simpler language, faster onboarding | GC pauses under load, separate ecosystem from Barbacane |
| **Elixir/Phoenix** | Excellent real-time primitives (channels, presence) | Entirely different ecosystem, small hiring pool |
| **TypeScript (Node)** | Huge ecosystem, shared language with frontend | Runtime overhead, less predictable performance under load |
| **Rust (Axum)** | Shared stack with Barbacane, low resource usage, safe concurrency | Steeper learning curve, slower iteration on CRUD code |

## Decision

### Backend

- **Language:** Rust
- **Async runtime:** Tokio
- **Web framework:** Axum — for the REST API and WebSocket upgrade handling.
- **Real-time transport:** WebSocket via tokio-tungstenite, upgraded from Axum routes. Messages fan out through an in-process pub/sub layer. For multi-node deployments, PostgreSQL LISTEN/NOTIFY synchronises events across instances (see [ADR-004](004-real-time-architecture.md)).
- **Database:** PostgreSQL — primary data store for users, channels, messages, and metadata. Chosen for its reliability, full-text search (tsvector/tsquery), JSON support, and LISTEN/NOTIFY for lightweight event propagation.
- **Database access:** sqlx — compile-time checked queries, no ORM overhead, already proven in Barbacane.
- **Search:** Two-tier approach behind a unified search trait:
  - **Default (minimal setup):** PostgreSQL full-text search (tsvector/tsquery) — works out of the box, no extra dependency.
  - **Recommended (better UX):** Typesense — a fully open-source (GPL-3.0) search engine that provides typo tolerance, fuzzy matching, and instant results. Runs as a separate service. Burst syncs messages to Typesense asynchronously. Meilisearch was considered but rejected due to its move to BUSL 1.1 for enterprise features, which conflicts with our fully open-source stance (ADR-001).
- **File storage:** Local filesystem by default, with an S3-compatible backend as an option. Abstracted behind a simple trait so other backends can be added.
- **Authentication:** Delegated to Barbacane gateway for API-level auth (JWT validation, rate limiting). Burst itself handles session management, LDAP/SSO integration, and user identity.
- **Observability:** tracing + OpenTelemetry, consistent with Barbacane's telemetry approach.

### Frontend

- **Framework:** React with TypeScript
- **Build tool:** Vite
- **Styling:** Tailwind CSS
- **Real-time:** Native WebSocket client, reconnecting with exponential backoff.
- **State management:** Lightweight — React context + a normalised message store. No heavy state framework unless complexity demands it.
- **Desktop/Mobile:** Web-first. Desktop via Tauri (Rust-based, lightweight alternative to Electron) as a future step. Mobile via responsive web initially, native apps as a boundary consideration.

### Infrastructure & Deployment

- **Single binary:** The Burst API server compiles to a single binary with embedded database migrations. The React frontend is served separately by nginx.
- **Container images:** Burst API (`Dockerfile`) and nginx SPA (`docker/Dockerfile.nginx`) for containerised deployments.
- **Barbacane gateway:** Deployed as a sidecar or reverse proxy in front of the Burst API, configured via the Burst OpenAPI spec.
- **Minimum deployment:** Burst API + nginx + Barbacane + PostgreSQL. No Redis, no Elasticsearch, no message broker required for single-node setups. `docker compose -f docker/docker-compose.yaml up` runs the full stack.

## Consequences

- Sharing the Rust + React stack with Barbacane creates a unified developer experience across the organisation. Contributors move between projects without context-switching on language or tooling.
- Rust's learning curve is the main risk for community contributions. We mitigate this with clear documentation, well-structured crates, and the fact that much of Burst's code is standard CRUD — not the low-level systems code that makes Rust challenging.
- PostgreSQL as the sole required dependency keeps the minimum deployment simple. Adding Typesense is recommended for production use — it delivers the search UX users expect from a modern messaging tool (typo tolerance, instant results) — but is not mandatory. The search trait abstraction ensures both backends are first-class citizens.
- The single-binary distribution model means a small team can go from download to running instance in minutes, which directly supports the "simple to operate" principle from ADR-001.
- Choosing Tauri over Electron for future desktop support is consistent with the Rust-first approach and keeps resource usage low, but the Tauri ecosystem is younger and less battle-tested.
