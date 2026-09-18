# ADR-015: Local Accounts

**Status:** Accepted
**Date:** 2026-09-15

## Context

[ADR-006](006-authentication-and-authorization.md) removed local password authentication on 2026-03-12. Since then the only way to sign in to Burst is through an external OIDC identity provider. Teams without an identity provider are directed to deploy one (Keycloak, Authelia) in front of Burst.

Three facts make this the wrong default for the product described in [ADR-001](001-project-vision-and-scope.md):

- The target user is a small-to-medium team that wants a focused, self-hosted messaging tool with minimal operational overhead. Requiring an identity provider before the first login adds a second product to install, configure and back up.
- The competing open-source editions are moving the other way. Mattermost v11 caps the free Team Edition at 250 users and removes SSO from it; Rocket.Chat Community is capped at 100 concurrent users. The differentiating position for Burst is local accounts and SSO, both in the open-source core, with no user cap.
- The frontend already ships a username/password form (`LOGIN_LOCAL`, on by default in the Docker images). In production it points at an OAuth password grant on the identity provider, which most providers do not enable. The form exists; it has no backend.

[ADR-014](014-gateway-optional-deployment.md) reintroduces token validation into Burst. Local accounts build on that same code path.

## Decision

Burst provides built-in local accounts alongside OIDC. Both produce the same `AuthUser`; the rest of the application does not distinguish them.

### Accounts and credentials

- Users may have a local password, an external identity (`external_id` from OIDC), or both. The existing `users.password_hash` column is used.
- Passwords are hashed with Argon2id using library defaults for memory, iterations and parallelism. A minimum length of 12 characters is enforced; no composition rules.
- Registration is invite-only by default. Admins create users or issue invitation links; open self-registration is an explicit instance setting, off by default.
- **First-run bootstrap.** When the `users` table is empty, the first local account created through the bootstrap endpoint or the `burst admin create` command becomes an admin. The endpoint is disabled as soon as one user exists.

### Sessions

- `POST /api/auth/login` verifies credentials and returns a short-lived access token (signed JWT, 15 minutes by default) and a refresh token (opaque, stored hashed in a `sessions` table, 7 days by default, rotated on use).
- `POST /api/auth/refresh` and `POST /api/auth/logout` manage the session. Logout revokes the session row; admin deactivation revokes all sessions for the user.
- The access token is a standard JWT carrying `sub`, `groups`, `iat`, `exp`, signed with an Ed25519 key generated on first start and stored in the database. Burst publishes the public key at `/.well-known/jwks.json`.
- The same bearer token is accepted on `/ws?access_token=`.

### Login protection

- Login attempts are rate limited per account and per client address using the ADR-014 limiter.
- Failed attempts return a single generic error; success and failure take comparable time.

### Frontend

- The existing local login form targets `/api/auth/login`. `LOGIN_LOCAL` is replaced by an instance setting exposed through `/env.js` that reflects whether local accounts are enabled, so the form appears only when it has a backend.
- Sign-in with an identity provider remains available when configured. Both options appear on the same login page.

### Gateway tier

In the gateway tier the gateway validates tokens. Because Burst publishes a JWKS and acts as an issuer for its own access tokens, the gateway's `oidc-auth` middleware can be configured with Burst as the issuer, and `/api/auth/*` is declared as an unauthenticated route in the spec. Supporting an external identity provider and local accounts simultaneously behind the gateway requires two issuers on one route, which the current `oidc-auth` plugin does not support; that combination is a gateway-tier follow-up and is documented as a limitation until then.

### Out of scope for this decision

- Multi-factor authentication.
- Self-service password reset by email (requires outbound mail). Admins can reset passwords through the admin API in the meantime.
- Account linking between a local password and an external identity beyond sharing an email address.

## Consequences

- **Standalone Burst is one binary and one database, full stop.** A team can be chatting within minutes of `docker compose up`, with no identity provider.
- **The product's positioning becomes concrete:** local accounts and SSO, both free, no cap. It is a direct answer to the free-tier changes in the alternatives.
- **Burst owns credential security.** Password hashing, session revocation and login throttling are now Burst's responsibility. These are well-understood components with mature Rust implementations, and they are covered by the integration test suite.
- **The dead login form becomes a working one**, and the `LOGIN_LOCAL` toggle disappears in favour of a setting that reflects reality.
- **ADR-006 is amended:** local authentication is reinstated; "Burst contains zero auth code" no longer holds. The JIT provisioning and role-mapping sections remain in force for OIDC users.
- **Schema changes:** a `sessions` table, a `signing_keys` table (or a single-row settings entry), and an instance setting for open registration. The `password_hash` column stops being dead.
- **Gateway-tier users with both an external IdP and local accounts** have a documented limitation until the gateway supports multiple issuers on one route.
