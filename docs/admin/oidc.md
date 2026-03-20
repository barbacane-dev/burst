## OIDC & SSO

Burst delegates authentication entirely to the Barbacane gateway. The `oidc-auth` plugin validates JWTs issued by your identity provider, and Burst receives pre-authenticated requests with user identity in headers.

## How It Works

1. Your identity provider (Keycloak, Auth0, Okta, etc.) issues a JWT after the user logs in.
2. The client sends the JWT as a `Bearer` token in the `Authorization` header.
3. Barbacane's `oidc-auth` plugin validates the token signature, expiry, audience, and issuer.
4. If valid, Barbacane forwards the request to Burst with an `X-Auth-Consumer` header containing the `sub` claim.
5. Burst uses the subject identifier to look up or create the local user record (JIT provisioning).

## Configuration

Set the following environment variables in your Burst deployment:

| Variable | Required | Description |
|----------|----------|-------------|
| `BURST_OIDC_ISSUER_URL` | Yes | The OIDC discovery URL of your identity provider (e.g., `https://keycloak.example.com/realms/burst`). |
| `BURST_OIDC_ISSUER_OVERRIDE` | No | Override the issuer value in token validation. Useful when the internal issuer URL differs from the public URL. |

The `oidc-auth` plugin in Barbacane is configured in the OpenAPI spec (`specs/burst-api.yaml`) at the global middleware level:

```yaml
x-barbacane-middlewares:
  - name: oidc-auth
    config:
      issuer_url: env://BURST_OIDC_ISSUER_URL
      audience: "burst"
      groups_claim: "roles"
```

## Identity Provider Setup

### Keycloak

1. Create a realm (e.g., `burst`).
2. Create a client with **Client authentication** enabled.
3. Set the **Valid redirect URIs** to your Burst frontend URL.
4. Create roles: `admin`, `moderator`, `member`, `guest`.
5. Assign roles to users. Map realm roles to the `roles` claim in the token using a client scope mapper.
6. Set `BURST_OIDC_ISSUER_URL` to `https://keycloak.example.com/realms/burst`.

### Auth0

1. Create an application of type **Regular Web Application**.
2. Set the **Allowed Callback URLs** to your Burst frontend URL.
3. Create an API with identifier `burst` (this becomes the `audience`).
4. Use Auth0 Rules or Actions to add a `roles` claim to the access token.
5. Set `BURST_OIDC_ISSUER_URL` to `https://your-tenant.auth0.com/`.

### Okta

1. Create an application of type **Web**.
2. Set the **Sign-in redirect URIs** to your Burst frontend URL.
3. Create an Authorization Server with audience `burst`.
4. Add a `roles` claim to the access token using a custom claims policy.
5. Set `BURST_OIDC_ISSUER_URL` to `https://your-org.okta.com/oauth2/default`.

## Role Mapping

The `groups_claim: "roles"` configuration tells Barbacane to extract the `roles` array from the JWT and map it to the `x-auth-consumer-groups` header. Burst reads this header to determine the user's role.

Your JWT should include a claim like:

```json
{
  "sub": "a1b2c3d4",
  "roles": ["admin"],
  "preferred_username": "alice"
}
```

If the `roles` claim contains multiple values, the highest-privilege role is used.

## JIT User Provisioning

When Burst receives a request with an `X-Auth-Consumer` header for a subject it has not seen before, it automatically creates a user record with the **member** role. The username and display name are populated from available JWT claims (`preferred_username`, `name`, `email`).

You can then promote the user to a different role from the [admin panel](users.md).

## Local Development

For local development, Burst provides a mock OIDC server on port `9099`. It accepts any credentials for preconfigured users:

- **alice** — has the `admin` role.
- **bob** — has the `member` role.

The mock server issues valid JWTs that Barbacane accepts, so the full authentication flow works locally without an external identity provider.

Start the mock server as part of the standard `docker compose up` development setup.

## Troubleshooting

| Symptom | Likely cause |
|---------|-------------|
| `401 Unauthorized` on all requests | `BURST_OIDC_ISSUER_URL` is wrong or the identity provider is unreachable. |
| User created with wrong role | The `roles` claim is missing or incorrectly mapped in your identity provider. |
| `403 Forbidden` on admin endpoints | The user's JWT does not include the `admin` role. |
| Issuer mismatch error | The token's `iss` claim does not match the expected issuer. Use `BURST_OIDC_ISSUER_OVERRIDE` if the internal and external URLs differ. |

## See Also

- [User Management](users.md) for managing roles after provisioning.
- [Barbacane Gateway](../guide/barbacane.md) for gateway configuration details.
