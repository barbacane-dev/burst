/**
 * OIDC configuration. At runtime, values come from /env.js (injected by the
 * Docker entrypoint). In dev mode, falls back to Vite's VITE_OIDC_* env vars.
 * When not configured, the login page shows the local credential form.
 */

declare global {
  interface Window {
    __BURST_ENV__?: Record<string, string>;
  }
}

function env(key: string): string | undefined {
  return window.__BURST_ENV__?.[key] ?? import.meta.env[`VITE_${key}`];
}

export interface OidcConfig {
  authority: string; // OIDC issuer URL (e.g. https://accounts.google.com)
  clientId: string; // OAuth client ID
  redirectUri: string; // Callback URL (e.g. https://burst.example.com/callback)
  scope: string; // OAuth scopes
}

export function getOidcConfig(): OidcConfig | null {
  const authority = env("OIDC_AUTHORITY");
  const clientId = env("OIDC_CLIENT_ID");

  if (!authority || !clientId) return null;

  return {
    authority,
    clientId,
    redirectUri:
      env("OIDC_REDIRECT_URI") ?? `${window.location.origin}/callback`,
    scope: env("OIDC_SCOPE") ?? "openid email profile",
  };
}

/**
 * Build the OIDC authorization URL for the redirect flow.
 */
export function buildAuthorizationUrl(config: OidcConfig, state: string): string {
  const params = new URLSearchParams({
    response_type: "code",
    client_id: config.clientId,
    redirect_uri: config.redirectUri,
    scope: config.scope,
    state,
    // PKCE is recommended for public clients but requires crypto.subtle.
    // For v1, we use the authorization code flow without PKCE.
    // Google supports both with and without PKCE.
  });

  // Google-specific: use the well-known authorization endpoint
  const authEndpoint = config.authority.includes("accounts.google.com")
    ? "https://accounts.google.com/o/oauth2/v2/auth"
    : `${config.authority}/authorize`;

  return `${authEndpoint}?${params}`;
}

/**
 * Exchange an authorization code for tokens via the OIDC token endpoint.
 */
export async function exchangeCodeForToken(
  config: OidcConfig,
  code: string,
): Promise<string> {
  const tokenEndpoint = config.authority.includes("accounts.google.com")
    ? "https://oauth2.googleapis.com/token"
    : `${config.authority}/token`;

  const res = await fetch(tokenEndpoint, {
    method: "POST",
    headers: { "Content-Type": "application/x-www-form-urlencoded" },
    body: new URLSearchParams({
      grant_type: "authorization_code",
      client_id: config.clientId,
      redirect_uri: config.redirectUri,
      code,
    }),
  });

  if (!res.ok) {
    const err = await res.text();
    throw new Error(`Token exchange failed: ${err}`);
  }

  const data = await res.json();
  // Google returns id_token (JWT) which Barbacane validates.
  return data.id_token ?? data.access_token;
}
