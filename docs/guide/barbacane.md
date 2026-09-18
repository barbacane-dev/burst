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
# Token claim holding the user's groups, mapped to x-auth-consumer-groups
BURST_OIDC_GROUPS_CLAIM=roles

# --- S3 sidecar (port 8081) ---
BURST_S3_REGION=us-east-1
BURST_S3_BUCKET=burst-files
BURST_S3_ACCESS_KEY_ID=minioadmin
BURST_S3_SECRET_ACCESS_KEY=minioadmin
BURST_S3_ENDPOINT=http://127.0.0.1:9000
BURST_S3_API_KEY=a-long-random-secret
```

`BURST_S3_API_KEY` is one half of a pair. The sidecar's `apikey-auth` accepts
that key; Burst presents it when calling the sidecar, as
`BURST_STORAGE_GATEWAY_API_KEY` alongside `BURST_STORAGE_GATEWAY_URL`. The two
must hold the same value, which is why the compose files feed both from one
`S3_API_KEY`.

Burst itself needs `BURST_AUTH_TRUSTED_PROXIES`, listing the peers whose
`X-Auth-*` headers it believes. It refuses to start without one, since those
headers name the caller and carry the role. List the gateway, and nothing
else.

## Compiling Gateway Artifacts

Barbacane compiles the OpenAPI specs into binary artifacts (`.bca` files) so the
gateway starts without a compiler. Both are produced by one command:

```bash
make gateway-compile
```

| Artifact | Source | Instance |
|----------|--------|----------|
| `burst-api.bca` | `specs/burst-api.yaml` | Public gateway |
| `burst-s3.bca` | `specs/burst-s3.yaml` | S3 sidecar |

Artifacts are build output, not source: `*.bca` is gitignored. The compose
topology compiles them with `make gateway-compile` before starting, and the
all-in-one image compiles them at build time.

Recompile whenever a spec changes. An artifact built by an older Barbacane is
refused at load by version, naming the recompile.

## Running the Gateways

For local development, one command compiles the spec, serves it, and recompiles
on save:

```bash
make gateway     # barbacane dev, watching specs/burst-api.yaml, on :8080
```

Deployments serve a prebuilt artifact instead, which is what the compose files
and the all-in-one entrypoint do:

```bash
barbacane serve --artifact burst-api.bca --listen 0.0.0.0:8080 \
  --allow-plaintext-upstream --max-body-size 10485760

barbacane serve --artifact burst-s3.bca --listen 0.0.0.0:8081 \
  --allow-plaintext-upstream --max-body-size 104857600
```

`--max-body-size` differs by instance: the S3 sidecar carries file uploads.
Both read `env://` values from the process environment; in development use a
`.env` file or `direnv`.

## Gateway Manifest

`barbacane.yaml` declares the plugins Burst needs, each pinned to a release
asset by URL and checksum, so a compile fetches exactly the reviewed binary:

```yaml
plugins:
  oidc-auth:
    url: https://github.com/barbacane-dev/barbacane/releases/download/v0.11.0/oidc-auth.wasm
    sha256: ded95c05c1eaeb4b307871a6f18c6289b9c31f73b6bd1a40fecc2817fdc2fb43
  rate-limit:
    url: https://github.com/barbacane-dev/barbacane/releases/download/v0.11.0/rate-limit.wasm
    sha256: ...
```

`barbacane-s3.yaml` does the same for the sidecar, which needs only `s3` and
`apikey-auth`. The checksums for a release are published alongside it as
`plugin-checksums.txt`.

## Declaring Authentication

From Barbacane 0.11 an operation that runs an authentication middleware must
name the security scheme carrying its credential, and only the headers an
operation admits reach the upstream. Two consequences for these specs:

- `specs/burst-s3.yaml` declares a `StorageKey` `apiKey` scheme naming
  `X-Storage-Key`, the header its `apikey-auth` middleware reads.
- `POST /api/webhooks/{webhookId}/trigger` opts out of the OIDC chain and is
  authenticated by Burst against the webhook's own token, so it declares a
  `WebhookToken` bearer scheme. Without it the gateway would strip the
  `Authorization` header and the webhook would fail with nothing logged.

Headers a plugin's own configuration names, such as the `rate-limit` partition
key, are admitted automatically and need no declaration.

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
- **A header never arrives** — from 0.11 the gateway forwards only the headers an
  operation admits. Declare it as an `in: header` parameter on the operation, or as
  the security scheme carrying it. `barbacane dev` logs each dropped header by name,
  which is the quickest way to see what is missing.
- **Burst refuses to start** — `auth.trusted_proxies` is empty. It is required, since
  the `X-Auth-*` headers name the caller and carry the role.
- **Every request is 401 with a valid token** — Burst is reachable from a peer outside
  `trusted_proxies`, so it treats the gateway's identity headers as untrusted. Check the
  address the gateway connects from, remembering that a container network assigns it
  dynamically.
- **Upstream connectivity** — verify `BURST_UPSTREAM_URL` and `BURST_UPSTREAM_WS_URL`
  point to a running Burst server.
- **Rate limits** — rate-limit errors return HTTP 429. Adjust thresholds in the spec
  if you see false positives during development.
- **Spec linting** — run `vacuum lint` before compiling to catch structural problems.
  See the [contributing guide](../CONTRIBUTING.md) for the exact command.
