# ADR-014: Gateway-Optional Deployment

**Status:** Accepted
**Date:** 2026-09-15

## Context

[ADR-002](002-core-feature-set.md), [ADR-003](003-technology-stack.md), [ADR-006](006-authentication-and-authorization.md) and [ADR-009](009-deployment-and-operations.md) make the Barbacane gateway the mandatory front door: every request reaches Burst through it, and Burst contains no authentication code of its own.

The consequences of that choice, as they stand today:

- **Minimum production footprint.** Running Burst requires PostgreSQL, an S3-compatible store (RustFS), two Barbacane processes (public gateway and S3 sidecar), the Burst binary, and an external OIDC identity provider. [ADR-001](001-project-vision-and-scope.md) principles 3 and 4 (simple to operate, lightweight) promise a binary plus a database. The competing products Burst positions against run as one binary plus PostgreSQL with built-in accounts.
- **No authentication fallback.** Identity is read from `X-Auth-Consumer`, `X-Auth-Consumer-Groups` and `X-Auth-Claims` in four places across two files (`api/extractors.rs`, `ws/handler.rs`). Burst performs no token validation and no trusted-peer check. Any client that can reach the Burst port directly can present arbitrary headers and be provisioned as an instance admin. Safety depends entirely on network placement.
- **Release coupling.** Burst pins Barbacane 0.6.3. Barbacane 0.10.0 compiles both Burst specs without error but rejects the resulting artifact at load time (`plugin 's3' violates its declared capabilities`) because pre-0.8.1 plugin binaries carry no embedded manifest. Every Barbacane release requires a coordinated Burst change before Burst can be deployed against it.
- **What the gateway actually carries.** Route-level ACLs are enforced identically by backend extractors (`AdminUser`, `IntegrationUser`); the gateway copy is redundant. File storage defaults to a working local backend. Incoming webhooks authenticate in Burst. The SPA performs its own OIDC discovery and PKCE flow; the gateway is not in the login path. The concerns that exist only at the gateway are: token validation, rate limiting, body size limits, security headers, static file serving, and bot credentials.
- **Dogfooding record.** Every Barbacane defect surfaced through Burst so far (glibc mismatch, standalone image capability enforcement, S3 `fallback_key`, 0.10 artifact rejection) was found by running Burst in CI or in a reference deployment. None came from an external deployment; the repository is private and has no external users.

The mandatory coupling optimises for Burst as a Barbacane showcase at the direct expense of Burst as a product. The two goals are separable: dogfooding requires that the maintainers run Burst behind Barbacane, not that every operator must.

## Decision

Burst supports two deployment tiers. Both are first-class, tested in CI, and documented.

### Standalone tier

`burst` binary + PostgreSQL. No gateway process.

- **Token validation in Burst.** Burst validates `Authorization: Bearer` tokens against a configured OIDC issuer: discovery via `.well-known/openid-configuration`, JWKS fetch with caching and rotation, issuer and audience checks, clock skew tolerance. A valid token produces the same `AuthUser` the extractors use today; roles derive from the configured groups claim on the verified token.
- **WebSocket auth in Burst.** The `/ws` upgrade accepts the token as `?access_token=` (RFC 6750 §2.3) and validates it with the same code path.
- **Static serving in Burst.** The compiled SPA is served by the binary with `index.html` fallback for client-side routes. `/env.js` is rendered from configuration at request time. The all-in-one image no longer uploads the SPA to S3.
- **Edge concerns in Burst.** CORS, request body limits, and baseline security headers are applied as Axum layers.
- **Rate limiting, single node.** An in-process per-subject limiter covers a single Burst instance. Multi-node rate limiting is a gateway-tier concern and is documented as such.
- **File storage** keeps the existing `Storage` seam. Local filesystem is the default; a direct S3 backend (SigV4) may be added as a third variant. The gateway-backed storage variant remains available for the gateway tier.
- **Not available standalone at this stage:** bot accounts. Bot credentials exist only in Barbacane `apikey-auth` configuration. Standalone bot credentials are a separate feature (schema, issuance, rotation, validation) and are tracked in the roadmap.

### Gateway tier

Barbacane in front of Burst, as today, and the recommended topology for production and for any multi-node deployment.

- Provides what a single binary cannot: a zero-trust network boundary, multi-node rate limiting, the native WAF, gateway-level observability and audit logging, and MCP exposure of the Burst API to agents from the same spec.
- Burst runs in trusted-headers auth mode (below) and does not re-validate tokens the gateway has already validated.
- The OpenAPI spec remains the single source of truth for both the API contract and the gateway configuration. Gateway ACL blocks are optional defence in depth; the backend extractors are authoritative and the gateway ACL is never the only check on a route.

### Authentication mode

Configuration gains an explicit mode:

```toml
[auth]
mode = "oidc"                       # "oidc" (standalone) or "trusted-headers" (gateway tier)
trusted_proxies = ["127.0.0.1/32"]  # required when mode = "trusted-headers"

[auth.oidc]                         # required when mode = "oidc"
issuer_url = "https://idp.example/realms/burst"
audience = "burst"
groups_claim = "groups"
```

- `oidc` is the default. In this mode `X-Auth-*` request headers are ignored.
- `trusted-headers` requires a non-empty `trusted_proxies` list. Burst refuses to start without it, and rejects `X-Auth-*` headers from any peer outside the list. This closes the header-spoofing exposure in both tiers.

### Dogfooding model

- The maintainers' reference deployment runs the gateway tier.
- CI runs the smoke suite twice: through the gateway and directly against Burst. The two result sets are compared. A difference is a defect in whichever layer introduced it. This makes the gateway's contribution observable and turns Barbacane regressions into a visible diff.
- Burst tracks the latest Barbacane release within one minor version. The version bump is a routine roadmap item, not a blocker for Burst releases.

## Consequences

- **Burst contains authentication code again**, in the order of a few hundred lines with one added dependency. ADR-006's "zero auth code" principle is retired. The security surface added is a standard JWKS validator; the security exposure removed is the unguarded trust of identity headers.
- **The single-binary promise in ADR-003 and ADR-009 becomes true.** Standalone Burst is one binary and one database, plus an identity provider until [ADR-015](015-local-accounts.md) lands.
- **Barbacane upgrades no longer gate Burst availability.** A Barbacane release that breaks the gateway tier is a CI failure to fix, not an outage for the standalone tier.
- **Dogfooding narrows to where it produces findings** (reference deployment and CI) and gains a contract test it does not have today. The gateway tier is pointed at the capabilities Barbacane differentiates on (WAF, multi-node limits, MCP, zero trust) instead of file serving and duplicated ACLs.
- **Amendments to earlier ADRs:** ADR-002 ("Barbacane API gateway as the front door") and ADR-003 ("Authentication: delegated to Barbacane") now describe the gateway tier only. ADR-006 ("Burst does not validate tokens", "Burst contains zero auth code") is superseded for the standalone tier and amended for the gateway tier by the trusted-proxies requirement. ADR-009's topology section gains the standalone tier as the simple deployment.
- **Testing.** Integration tests gain an OIDC-mode harness with a test issuer and JWKS, so the standalone auth path is covered by the same suite that covers the API. The trusted-headers path is covered with and without a matching peer address.
- **Documentation** is restructured around the two tiers: a standalone quick start (compose with two services, or the bare binary) and a production guide for the gateway tier.
- **Bot accounts are gateway-tier only** until standalone bot credentials ship. This is stated in the admin UI and the docs.
