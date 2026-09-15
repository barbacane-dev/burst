## Barbacane Gateway Integration

Burst uses [Barbacane](https://github.com/barbacane-dev/barbacane) as its API gateway.
Barbacane is a spec-driven gateway: your OpenAPI spec **is** the gateway configuration.
There is no separate routing file — dispatch targets, authentication, rate limiting,
and ACL rules all live inside `specs/burst-api.yaml`.

## Two Gateway Instances

A typical Burst deployment runs **two** Barbacane instances, each with a distinct role.

### Public gateway (port 8080)

Handles all client-facing traffic — REST API calls and WebSocket connections.

| Plugin | Purpose |
|--------|---------|
| `oidc-auth` | Validates JWT tokens from your identity provider |
| `acl` | Enforces role-based access (admin, moderator, member, guest) |
| `rate-limit` | Throttles requests per consumer |
| `http-upstream` | Proxies REST requests to the Burst server |
| `ws-upstream` | Proxies WebSocket connections to the Burst server |

### S3 sidecar (port 8081)

Handles file storage. Burst calls this sidecar internally to upload and download
files — it is **not** exposed to end users.

| Plugin | Purpose |
|--------|---------|
| `s3` | Dispatches requests directly to S3-compatible storage |
| `apikey-auth` | Authenticates requests from the Burst server |

## Spec-as-Config Model

Every operation in `specs/burst-api.yaml` carries two Barbacane extensions:

- **`x-barbacane-dispatch`** — tells the gateway where to route the request
  (e.g., `http-upstream` to the Burst server, or `ws-upstream` for WebSocket).
- **`x-barbacane-middlewares`** — lists the plugins that run before dispatch
  (e.g., `oidc-auth`, `acl`, `rate-limit`). An empty array `[]` disables
  all global middlewares for that operation.

Example from the spec:

```yaml
paths:
  /api/channels:
    get:
      x-barbacane-dispatch:
        name: http-upstream
        config:
          url: "env://BURST_UPSTREAM_URL"
          path: /api/channels
          timeout: 30.0
      x-barbacane-middlewares:
        - name: oidc-auth
          config:
            issuer_url: "env://BURST_OIDC_ISSUER_URL"
        - name: acl
          config:
            allow: [admin, member]
```

All URLs and secrets use `env://` references so you never hard-code values into the
spec. The actual values come from your `.env` file or environment at gateway startup.

## Environment Variables

Both gateway instances read their configuration from environment variables.
Here is a minimal `.env.example` covering all referenced `env://` values:

```bash
# --- Both gateway processes ---
# Burst, RustFS and an IdP on the same host or network are internal
# addresses; Barbacane's plugin SSRF guard blocks egress to them unless set.
BARBACANE_ALLOW_INTERNAL_EGRESS=true

# --- Public gateway (port 8080) ---
BURST_UPSTREAM_URL=http://127.0.0.1:3000
BURST_UPSTREAM_WS_URL=ws://127.0.0.1:3000

BURST_OIDC_ISSUER_URL=https://auth.example.com/realms/burst
# Optional: override the issuer URL used for token validation
# (useful when the gateway reaches the IdP via a different address)
BURST_OIDC_ISSUER_OVERRIDE=http://keycloak:8080/realms/burst

# --- S3 sidecar (port 8081) ---
BURST_S3_REGION=us-east-1
BURST_S3_BUCKET=burst-files
BURST_S3_ACCESS_KEY_ID=minioadmin
BURST_S3_SECRET_ACCESS_KEY=minioadmin
BURST_S3_ENDPOINT=http://127.0.0.1:9000
BURST_S3_API_KEY=a-long-random-secret
```

## Compiling Gateway Artifacts

Barbacane compiles OpenAPI specs into binary artifacts (`.bca` files) for fast
startup. You compile both artifacts with a single command:

```bash
make gateway-compile
```

This produces two files:

| Artifact | Source | Instance |
|----------|--------|----------|
| `burst-api.bca` | `specs/burst-api.yaml` | Public gateway |
| `burst-s3.bca` | `specs/burst-s3.yaml` | S3 sidecar |

You must recompile whenever you change the spec. The compiled artifacts are
checked into version control so that `barbacane run` can start without the
compiler installed.

## Running the Gateways

Start each instance pointing to its compiled artifact:

```bash
# Public gateway
barbacane run burst-api.bca --listen 0.0.0.0:8080

# S3 sidecar
barbacane run burst-s3.bca --listen 0.0.0.0:8081
```

Both commands read `env://` values from the process environment.
In development, use a `.env` file or a tool like `direnv`.

## Gateway Manifest

The file `barbacane.yaml` at the repository root declares which Barbacane plugins
Burst requires and where to find them:

```yaml
plugins:
  - oidc-auth
  - acl
  - rate-limit
  - http-upstream
  - ws-upstream
  - s3
  - apikey-auth
```

This manifest is used during compilation to resolve plugin paths.

## Roles and ACL

Barbacane maps JWT claims to consumer groups using the `groups_claim` option in
the `oidc-auth` plugin. Burst configures this via the `BURST_OIDC_GROUPS_CLAIM`
environment variable (defaults to `roles`), so the named claim in your JWT
becomes the list of groups for ACL evaluation.

Admin routes are protected at **two** layers:

1. **Gateway ACL** — `allow: [admin]` in `x-barbacane-middlewares`.
2. **Backend extractor** — the `AdminUser` extractor in Burst rejects requests
   that somehow bypass the gateway.

## Troubleshooting

If the gateway rejects requests unexpectedly, check:

- **OIDC discovery** — the gateway must reach `BURST_OIDC_ISSUER_URL/.well-known/openid-configuration`.
  Use `BURST_OIDC_ISSUER_OVERRIDE` when the internal URL differs from the public one.
- **Internal egress blocked** — every valid token is rejected with 401 while unauthenticated
  requests also get 401: the plugin SSRF guard is blocking the discovery/JWKS fetch (or the
  upstream) because it resolves to a loopback or private address. Set
  `BARBACANE_ALLOW_INTERNAL_EGRESS=true` on the gateway process.
- **Upstream connectivity** — verify `BURST_UPSTREAM_URL` and `BURST_UPSTREAM_WS_URL`
  point to a running Burst server.
- **Rate limits** — rate-limit errors return HTTP 429. Adjust thresholds in the spec
  if you see false positives during development.
- **Spec linting** — run `vacuum lint` before compiling to catch structural problems.
  See the [contributing guide](../CONTRIBUTING.md) for the exact command.
