# ADR-005: API Design Conventions

**Status:** Accepted
**Date:** 2026-03-06

## Context

Per [ADR-002](002-core-feature-set.md), Burst exposes a REST API for all core features, designed spec-first with OpenAPI. Per [ADR-003](003-technology-stack.md), this API sits behind the Barbacane API gateway, which validates requests against the OpenAPI spec.

This means the OpenAPI spec is not just documentation — it is the runtime configuration. Sloppy specs produce sloppy gateways. We need strict, enforceable conventions.

We inherit experience from a previous API governance ruleset used in a Go/Node.js ecosystem. Most of those conventions are language-agnostic and carry over directly. Some need adaptation for Burst's context (single-product API vs. multi-domain platform).

## Decision

### Spec format

- **OpenAPI 3.2.x** — required. OAS 3.2 (released September 2025) is backward-compatible with 3.1 and adds structured tags (`summary`, `parent`, `kind` fields), streaming support (`itemSchema`, `prefixEncoding` for SSE/JSON Lines), and extended HTTP methods via `additionalOperations`. The streaming support is particularly relevant for Burst's real-time event endpoints. OAS 3.1 or older is not accepted.
- **JSON Schema 2020-12** — all component schemas must declare `$schema: https://json-schema.org/draft/2020-12/schema`. This remains the latest stable JSON Schema draft.
- **Semantic versioning** for `info.version` — `MAJOR.MINOR.PATCH` format.

### URL conventions

- **kebab-case for path segments** — e.g., `/channels/{channelId}/pinned-messages`. This is the standard REST convention, unlike the previous camelCase choice which was project-specific.
- **No URI versioning** — no `/v1/` prefix. Versioning is handled via `info.version` and content negotiation if needed.
- **No `/api` prefix** — the gateway handles routing; the paths start directly with the resource.
- **Normalised paths** — no trailing slashes, no empty segments (`//`).
- **Max 3 levels of sub-resources** — e.g., `/channels/{channelId}/messages/{messageId}/reactions` is fine; deeper nesting should be flattened.

### Naming conventions

| Element | Convention | Example |
|---------|-----------|---------|
| Path segments | kebab-case | `/pinned-messages` |
| Path parameters | camelCase | `{channelId}` |
| Query parameters | camelCase | `?fromUser=...&hasFile=true` |
| Request/response body fields | camelCase | `channelName`, `createdAt` |
| HTTP headers | Hyphenated-Pascal-Case | `Content-Type`, `X-Request-Id` |
| Schema names (components) | UpperCamelCase | `ChannelMessage`, `UserProfile` |
| Operation IDs | camelCase | `listChannelMessages`, `createChannel` |
| Enum values | UPPER_SNAKE_CASE | `CHANNEL_PUBLIC`, `ROLE_ADMIN` |
| Tags | Capitalised, max 3 words, with OAS 3.2 structured fields | `Channels`, `Direct Messages` |

### Operations

- Every operation MUST have: `operationId`, `summary`, `description`, `tags`.
- Operation tags MUST be defined in the global `tags` array with descriptions.
- Security MUST be declared at operation level, not root level (allows per-route auth control via Barbacane).

### HTTP methods and status codes

- Only standard HTTP status codes are allowed.
- Status codes must be appropriate for the HTTP method:

| Code | Allowed methods |
|------|----------------|
| `200` | All |
| `201` | POST, PUT |
| `204` | PUT, DELETE, PATCH |
| `400` | All |
| `401` | All |
| `403` | All |
| `404` | All |
| `409` | POST, PUT, DELETE, PATCH |
| `429` | All |
| `500` | All |

### Error responses

- All error responses MUST use `application/problem+json` (RFC 9457).
- The `type` field MUST use URN format: `urn:burst:error:<error-type>`, following the same convention as Barbacane (`urn:barbacane:error:<error-type>`). This provides stable, machine-readable error identifiers that clients can match on without parsing human-readable messages.
- Standard error types:

| URN | Status | Description |
|-----|--------|-------------|
| `urn:burst:error:bad-request` | 400 | Malformed or invalid request |
| `urn:burst:error:validation` | 400 | Request body/params failed validation (includes `errors` array) |
| `urn:burst:error:unauthorized` | 401 | Missing or invalid authentication |
| `urn:burst:error:forbidden` | 403 | Authenticated but insufficient permissions |
| `urn:burst:error:not-found` | 404 | Resource does not exist |
| `urn:burst:error:conflict` | 409 | Resource already exists or is in use |
| `urn:burst:error:rate-limited` | 429 | Too many requests |
| `urn:burst:error:internal-error` | 500 | Unexpected server error (no detail leaked) |

- Validation errors (400) MUST include an `errors` array with individual issues, each containing `code`, `message`, and optional `location` fields — consistent with Barbacane's `ValidationIssue` structure.
- Error schema names in the OpenAPI spec MUST end with `Error` (e.g., `NotFoundError`, `ValidationError`).
- The `default` response on every operation MUST also use `application/problem+json`.

### Request/response naming

- Request body `$ref` MUST follow: `#/components/requestBodies/{OperationId}Request`
- Success response `$ref` MUST follow: `#/components/responses/{OperationId}{StatusCode}Response` or `{Resource}2XXResponse` for shared responses.

### Schema discipline

- All schemas MUST declare `$schema` and `title`.
- Number types MUST define a `format` (`float`, `double`, `decimal`).
- Integer types MUST define a `format` (`int32`, `int64`).
- String types SHOULD declare `maxLength` (OWASP best practice).
- Array types SHOULD declare `maxItems`.

### Pagination

- List endpoints MUST support cursor-based pagination.
- Query parameters: `cursor` (opaque token), `limit` (default: 50, max: 200).
- Response envelope:

```json
{
  "items": [...],
  "cursor": "next-page-token-or-null"
}
```

### Security

- No `X-` prefixed custom headers (per RFC 6648, deprecated).
- Security schemes: `BearerAuth` (JWT via Barbacane).
- Every operation MUST declare its security requirements explicitly.

### Linting

These conventions will be enforced via a Spectral/Vacuum ruleset committed to the repository. The ruleset extends `vacuum:oas` (recommended) and `vacuum:owasp` (all) as a baseline, with Burst-specific rules layered on top.

## Consequences

- The Spectral/Vacuum ruleset acts as a CI gate: specs that violate conventions cannot be merged. This prevents drift between conventions and implementation.
- Strict naming conventions reduce bikeshedding. Contributors don't need to decide how to name things — the linter tells them.
- `application/problem+json` for all errors gives clients a consistent error model across every endpoint.
- Cursor-based pagination is more complex to implement than offset-based but handles real-time data correctly (no skipped/duplicated items when new messages arrive during pagination).
- Kebab-case URLs are a deliberate departure from the inherited conventions. This aligns with broader REST community standards and is more readable in logs and documentation.
- Since Barbacane validates requests against this spec at runtime, any mistake in the spec is a runtime bug. The linting rules are our first line of defence against this.
