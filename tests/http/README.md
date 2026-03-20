# HTTP Smoke Tests

End-to-end tests from a user's perspective, exercising the full stack:
**mock OIDC → Barbacane gateway → Burst API → PostgreSQL**

Powered by [Grafana k6](https://grafana.com/docs/k6/).

## Prerequisites

```bash
brew install k6              # Install k6 (once)
make services                # PostgreSQL + mock OIDC + RustFS (Docker)
make seed                    # Seed alice + bob users
make gateway-compile         # Compile the BCA artifact
make server                  # Burst API on :3000
make gateway                 # Barbacane gateway on :8080
```

## Running

```bash
make smoke                   # Core API smoke test (106 checks)
make smoke-s3                # S3 storage smoke test (7 checks, requires RustFS)
```

## Core Smoke Test (`smoke.js`)

Single-iteration test (1 VU). All 106 checks must pass.

| # | Section | What it covers |
|---|---------|----------------|
| 0 | Health checks | OIDC discovery, Burst server reachable |
| 1 | Authentication | Obtain JWT tokens for Alice and Bob |
| 2 | Gateway auth | Reject no-auth, bad-auth; pass valid tokens |
| 3 | Channels | Create, get, update, list, duplicate slug conflict |
| 4 | Membership | Join, list members |
| 5 | Messages | Send, reply (thread), list, edit, get single |
| 6 | Reactions | Add, verify, remove emoji reaction |
| 7 | Attachments | Upload text + PNG files, download, SHA-256 integrity, auth guard |
| 8 | Search | Full-text search, channel-scoped, empty query error |
| 9 | Pagination | Limit + cursor-based paging |
| 10 | Mark read | Update last-read timestamp |
| 11 | Error cases | 404s, 400s, RFC 9457 error format |
| 12 | Cleanup | Soft delete, leave channel |

## S3 Storage Smoke Test (`smoke-s3.js`)

Tests the full file storage path through Barbacane's S3 dispatcher to RustFS:
**Burst (GatewayStorage) → Barbacane S3 dispatcher → RustFS**

### Additional Prerequisites

1. RustFS running (`docker compose -f docker-compose.dev.yml up rustfs`)
2. Create the `burst-files` bucket: `mc alias set rustfs http://localhost:9000 burst burstpass && mc mb rustfs/burst-files`
3. S3 env vars in `.env` (see `.env` file)
4. Recompile gateway: `make gateway-compile`

### What it tests

| # | Check | What it covers |
|---|-------|----------------|
| 1 | Create channel | Setup for file upload |
| 2 | Upload PNG via multipart | File sent through Burst → Barbacane S3 → RustFS |
| 3 | Download via attachment endpoint | File retrieved back through the full stack |
| 4 | SHA-256 integrity | Binary content matches original after roundtrip |
| 5 | Soft delete | Message deleted, file still accessible until cleanup |
