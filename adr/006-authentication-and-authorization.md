# ADR-006: Authentication & Authorization

**Status:** Amended (2026-03-12)
**Date:** 2026-03-06

## Context

Per [ADR-002](002-core-feature-set.md), Burst requires LDAP/SSO authentication and role-based permissions — both in the open-source core, not gated. Per [ADR-003](003-technology-stack.md), Burst's API sits behind the Barbacane API gateway.

Barbacane already ships a comprehensive set of security plugins:

| Plugin | Capability |
|--------|-----------|
| **jwt-auth** | Bearer JWT validation, claims extraction |
| **oidc-auth** | OIDC auto-discovery, JWKS rotation, signature verification |
| **oauth2-auth** | RFC 7662 token introspection |
| **basic-auth** | Username/password validation |
| **apikey-auth** | API key validation |
| **acl** | Route-level access control via `x-auth-consumer` / `x-auth-consumer-groups` |
| **rate-limit** | Per-consumer rate limiting |
| **ip-restriction** | IP allowlist/blocklist |

Rather than reimplementing auth logic in Burst, we should maximise delegation to Barbacane. This is the core dogfooding proposition: if Barbacane can't handle auth for a real application like Burst, it can't handle it for customers either.

## Decision

### Design principle: Burst trusts Barbacane headers

Burst does not validate tokens. Burst reads identity from HTTP headers set by Barbacane's auth plugins after successful validation. Every authenticated request that reaches Burst carries trusted headers:

- `X-Auth-Consumer` — the authenticated user or bot ID.
- `X-Auth-Consumer-Groups` — comma-separated list of roles/groups from the token claims.
- `X-Auth-Claims` — full JWT claims (JSON-encoded), available for fine-grained decisions.

Burst treats these headers as the source of truth. Since Barbacane sits in front and strips any client-supplied values for these headers before applying its own, they cannot be spoofed.

### Authentication: fully delegated to Barbacane

#### OIDC / SSO (recommended for production)

The primary authentication path. Users authenticate directly with an external IdP (Keycloak, Okta, Azure AD, etc.).

```
Client → IdP login (standard OIDC flow)
       → IdP issues access token
       → Client sends requests with Bearer token
       → Barbacane oidc-auth validates token (auto-discovery, JWKS, signature)
       → Barbacane sets X-Auth-Consumer, X-Auth-Consumer-Groups
       → Burst reads headers, JIT-provisions local user profile if needed
```

Burst does not participate in the OIDC flow at all. The IdP configuration lives entirely in the Barbacane spec (`x-barbacane-*` extensions on the security scheme). This means adding SSO to Burst is a gateway configuration change, not a code change.

#### ~~Local password (simple deployments)~~ — Removed (2026-03-12)

Local password authentication has been removed from Burst. All authentication is now fully delegated to Barbacane. For teams that don't have an IdP, options include:

- Placing an OIDC bridge (e.g. Keycloak) in front of an identity source and using Barbacane's `oidc-auth` plugin.
- Using Barbacane's `basic-auth` plugin for simple deployments.

Burst contains **zero auth code** — no JWT issuance, no password hashing, no token validation.

#### Bot / integration auth

```
Integration → Request with API key or Basic auth credentials
            → Barbacane apikey-auth or basic-auth validates
            → Barbacane sets X-Auth-Consumer (mapped to bot user ID)
            → Burst reads header, maps to bot user account
```

Bot credentials are managed in the Barbacane configuration. Burst stores the bot user profiles (display name, avatar, permissions) but not the credentials themselves.

#### WebSocket authentication (amended 2026-03-12)

WebSocket connections are routed through Barbacane's `ws-upstream` dispatcher plugin. Authentication happens on the HTTP Upgrade request — the same as any REST endpoint:

```
Client → WS upgrade to /ws with Bearer token in Authorization header
       → Barbacane jwt-auth validates token on the HTTP Upgrade request
       → Barbacane sets X-Auth-Consumer, X-Auth-Consumer-Groups headers
       → Barbacane ws-upstream proxies the connection to Burst
       → Burst reads X-Auth-Consumer from the upgrade request headers
       → Connection is associated with the authenticated user
       → Frames are relayed transparently (no per-frame auth)
```

The optional `lastEventId` query parameter enables gap-fill on reconnect (per ADR-004). No first-frame auth handshake is needed.

### Authorization: split between Barbacane and Burst

#### Barbacane handles: coarse-grained route protection

Using the `acl` plugin, Barbacane enforces route-level rules declared in the OpenAPI spec:

- `/admin/*` routes — restricted to `admin` group.
- `POST /channels` — restricted to `member` or higher (guests cannot create channels).
- `DELETE` operations — restricted to `moderator` or `admin` depending on the resource.

These rules are part of the OpenAPI spec and enforced at the gateway before requests reach Burst. Unauthorized requests get a 403 from Barbacane and never hit the application.

#### Burst handles: fine-grained resource-level permissions

For decisions that require application state (channel membership, message ownership):

- Can this user post in this specific channel? (requires checking channel membership)
- Can this user edit this message? (requires checking message ownership)
- Can this user see this private channel? (requires checking channel access)

These checks happen in Burst's request handlers, reading the user identity from `X-Auth-Consumer` and checking against the database.

### Role model

| Role | Scope | Permissions |
|------|-------|------------|
| **Admin** | Instance-wide | Full control: user management, settings, all channels |
| **Moderator** | Per-channel | Manage channel settings, pin/delete messages, mute users |
| **Member** | Per-channel | Send messages, upload files, create channels |
| **Guest** | Per-channel (restricted) | Read and send in explicitly invited channels only |

Roles are carried in the JWT `groups` claim and forwarded by Barbacane as `X-Auth-Consumer-Groups`. The ACL plugin uses these for route-level checks. Burst uses them for resource-level checks.

### JIT user provisioning

When a user authenticates via OIDC/SSO for the first time, Burst automatically creates a local user profile from the token claims:

- `sub` → Burst user ID (or mapped via configurable claim).
- `name` / `preferred_username` → display name.
- `email` → email address.
- `groups` → role mapping.

Profile data is re-synced on each login to pick up changes from the IdP (name changes, group membership changes). The local profile is the authoritative source for Burst-specific data (avatar, custom status, notification preferences).

## Consequences

- **Burst contains zero auth code.** No JWT issuance, no password hashing, no token validation. This dramatically reduces the security surface area.
- **SSO is a configuration change, not a code change.** Switching IdP providers is a matter of updating the Barbacane spec — no Burst deployment or code change needed.
- **Barbacane gets stress-tested on real auth scenarios.** OIDC discovery, JWKS rotation, token introspection, ACL enforcement, WebSocket upgrade auth — all exercised in production by Burst. Bugs found here benefit all Barbacane users.
- **Burst trusts Barbacane completely.** If Barbacane is compromised or misconfigured, Burst's auth is compromised. This is acceptable because Barbacane is the security boundary by design — the same trust model as any API gateway.
- **WebSocket auth uses the same path as REST.** Barbacane's `ws-upstream` dispatcher runs the full middleware chain (including jwt-auth) on the HTTP Upgrade request. No special loopback or first-frame validation needed.
- **The role model is deliberately simple (4 roles).** Fine-grained permissions can be added later without re-architecting, but we start simple per ADR-001.
- **LDAP is a gap.** Barbacane does not currently have an LDAP auth plugin. For LDAP-only deployments, two options: (a) place an OIDC bridge like Keycloak in front of LDAP and use Barbacane's oidc-auth, or (b) build LDAP support as a Barbacane plugin — which is a contribution back to the gateway. Option (b) is preferred as it extends Barbacane's capabilities.
