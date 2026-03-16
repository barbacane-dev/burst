# HTTP Smoke Tests

End-to-end tests from a user's perspective, exercising the full stack:
**mock OIDC → Barbacane gateway → Burst API → PostgreSQL**

Powered by [Grafana k6](https://grafana.com/docs/k6/).

## Prerequisites

```bash
brew install k6              # Install k6 (once)
make services                # PostgreSQL + mock OIDC (Docker)
make seed                    # Seed alice + bob users
make gateway-compile         # Compile the BCA artifact
make server                  # Burst API on :3000
make gateway                 # Barbacane gateway on :8080
```

## Running

```bash
k6 run tests/http/smoke.js
```

The script runs as a single-iteration smoke test (1 VU, 1 iteration).
All 65 checks must pass for the run to succeed.

## Scenario

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
