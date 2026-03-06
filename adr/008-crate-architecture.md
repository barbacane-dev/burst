# ADR-008: Crate Architecture

**Status:** Accepted
**Date:** 2026-03-06

## Context

Per [ADR-003](003-technology-stack.md), Burst is a Rust workspace that compiles to a single binary embedding frontend assets and database migrations. Per [ADR-001](001-project-vision-and-scope.md), the project must be simple — both to operate and to contribute to.

Barbacane uses a multi-crate workspace (`crates/` + `plugins/`), which makes sense for a gateway with a plugin SDK, a compiler, a control plane, and a data plane. Burst is a simpler product: one binary, one purpose, no plugin system.

We want clear code boundaries, but not at the cost of premature abstraction. A messaging app does not need 7+ crates on day one. We can split later if compile times or cognitive load demand it.

## Decision

### 3 crates, start lean

```
burst/
├── Cargo.toml                  # Workspace root
├── crates/
│   ├── burst/                  # Binary — entry point, CLI, startup, asset embedding
│   ├── burst-core/             # Domain types, business rules, zero infra dependencies
│   └── burst-server/           # Everything else: API, DB, WebSocket, auth, storage
├── migrations/                 # sqlx migrations (SQL files)
├── specs/                      # OpenAPI spec (source of truth)
├── ui/                         # React frontend (Vite project)
├── adr/                        # Architecture decisions
└── docker/                     # Dockerfile, docker-compose
```

### Crate responsibilities

#### `burst` (binary)

The entry point. Wires everything together, owns nothing else:

- CLI parsing (clap) — config file path, listen address, database URL.
- Configuration loading and validation.
- Database pool setup and migration runner.
- Axum router assembly (mounts routes from `burst-server`).
- Graceful shutdown handling.
- Embeds frontend assets (`ui/dist/`) for single-binary distribution.

#### `burst-core` (library, zero infra deps)

Pure domain logic. Depends only on fundamental crates (`serde`, `uuid`, `chrono`):

- Domain types: `User`, `Channel`, `Message`, `Reaction`, `Attachment`, etc.
- Business rules: permission checks, channel membership logic, role validation.
- ID generation (UUIDv7).
- Event types and envelope (per ADR-004).
- Markdown processing and sanitisation.
- RFC 9457 ProblemDetails types with `urn:burst:error:*` URNs.

This is the crate everything depends on and that depends on nothing. Domain logic is testable without a database, HTTP server, or mocks.

#### `burst-server` (library, all infrastructure)

Everything that touches I/O:

- **API layer:** Axum routes and handlers, request/response types, input validation, pagination helpers, Barbacane header extractors (`X-Auth-Consumer`).
- **Database:** sqlx queries (compile-time checked), repository implementations, cursor-based pagination, full-text search, Typesense sync.
- **Real-time:** WebSocket upgrade, connection lifecycle, in-process broker (tokio broadcast), PG LISTEN/NOTIFY broker, presence tracker, gap-fill, typing indicators.
- **Auth:** Password hashing (argon2), JWT issuing (local auth), refresh token management.
- **Storage:** `StorageBackend` trait, local filesystem backend, S3-compatible backend.
- **Webhooks:** Outgoing webhook dispatcher, incoming webhook handler.

Organised internally via modules:

```
burst-server/src/
├── api/
│   ├── channels.rs
│   ├── messages.rs
│   ├── users.rs
│   ├── auth.rs
│   ├── search.rs
│   ├── webhooks.rs
│   ├── admin.rs
│   └── extractors.rs       # Barbacane header extraction, pagination
├── db/
│   ├── channels.rs
│   ├── messages.rs
│   ├── users.rs
│   └── search.rs
├── realtime/
│   ├── ws.rs               # WebSocket handler
│   ├── broker.rs           # Pub/sub trait + PG LISTEN/NOTIFY impl
│   ├── presence.rs
│   └── events.rs
├── auth.rs                  # Password hashing, JWT issuing
├── storage.rs               # Storage trait + backends
├── error.rs                 # ProblemDetails impl (IntoResponse)
└── lib.rs
```

Modules provide logical separation. If any module grows unwieldy or compile times suffer, it can be extracted to its own crate — but only when the pain is real.

### Dependency graph

```
burst (binary)
├── burst-server
│   └── burst-core
└── (embeds ui/ assets)
```

Clean, acyclic, shallow. `burst-core` is the leaf.

### What we deliberately avoided

- **No `burst-common` or `burst-utils`.** Shared code belongs in `burst-core` (domain) or `burst-server` (infra). No grab-bag crates.
- **No trait crate.** Storage and search traits live in `burst-server` next to their implementations.
- **No crate-per-concern.** Auth, storage, realtime, DB — all in `burst-server` as modules. Split when needed, not before.
- **No publishing.** These crates are workspace-internal only. They will never be on crates.io.

### Frontend build

The `ui/` directory is a standard Vite/React project. Built separately (`npm run build`) as a prerequisite to `cargo build`. The `burst` binary embeds `ui/dist/` at compile time. The frontend build is orchestrated by the `Makefile` or CI, not by Cargo.

### When to split further

Extract a module to its own crate if:

- **Compile time:** a change in one module triggers recompilation of unrelated code and it hurts iteration speed.
- **Dependency weight:** a module pulls in heavy dependencies (e.g., S3 SDK) that slow compilation for everyone.
- **Team boundaries:** multiple people working on clearly separate concerns and stepping on each other.

Until then, modules within `burst-server` are sufficient.

## Consequences

- **3 crates is minimal for a Rust workspace.** This is intentional. We follow the same "start simple" principle as the rest of the project. Over-splitting creates navigation overhead and slow `cargo check` across many crates.
- **`burst-core` with zero infra deps** ensures domain logic stays pure and testable. This is the one split we do upfront because it prevents the most common architectural mistake: database or HTTP concerns leaking into business rules.
- **`burst-server` as a single library crate** means one place to look for all infrastructure code. The module structure provides logical organisation without the overhead of crate boundaries.
- **The split path is clear.** If `burst-server` grows too large, the module boundaries (`api/`, `db/`, `realtime/`, `storage.rs`, `auth.rs`) are natural extraction points. The refactoring is mechanical: move the module to `crates/burst-<concern>/`, add it to the workspace, update imports.
