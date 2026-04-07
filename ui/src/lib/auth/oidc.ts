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
  const val = window.__BURST_ENV__?.[key] ?? import.meta.env[`VITE_${key}`];
  return val || undefined;
}

export interface OidcConfig {
  authority: string; // OIDC issuer URL (e.g. https://accounts.google.com)
  clientId: string; // OAuth client ID
  redirectUri: string; // Callback URL (e.g. https://burst.example.com/callback)
  scope: string; // OAuth scopes
}

interface OidcDiscovery {
  authorization_endpoint: string;
  token_endpoint: string;
}

let discoveryCache: OidcDiscovery | null = null;

/** @internal Reset discovery cache (for tests only). */
export function _resetDiscoveryCache() {
  discoveryCache = null;
}

async function discover(authority: string): Promise<OidcDiscovery> {
  if (discoveryCache) return discoveryCache;

  const url = `${authority}/.well-known/openid-configuration`;
  const res = await fetch(url);
  if (!res.ok) throw new Error(`OIDC discovery failed: ${res.status}`);

  const doc = await res.json();
  discoveryCache = {
    authorization_endpoint: doc.authorization_endpoint,
    token_endpoint: doc.token_endpoint,
  };
  return discoveryCache;
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
    scope: env("OIDC_SCOPE") ?? "openid email profile groups",
  };
}

/** Whether the local username/password login form should be shown. */
export function isLocalLoginEnabled(): boolean {
  return env("LOGIN_LOCAL") !== "false";
}

/**
 * Generate a PKCE code verifier and challenge pair.
 */
async function generatePkce(): Promise<{
  verifier: string;
  challenge: string;
}> {
  const bytes = crypto.getRandomValues(new Uint8Array(32));
  const verifier = btoa(String.fromCharCode(...bytes))
    .replace(/\+/g, "-")
    .replace(/\//g, "_")
    .replace(/=+$/, "");

  const hash = await crypto.subtle.digest(
    "SHA-256",
    new TextEncoder().encode(verifier),
  );
  const challenge = btoa(String.fromCharCode(...new Uint8Array(hash)))
    .replace(/\+/g, "-")
    .replace(/\//g, "_")
    .replace(/=+$/, "");

  return { verifier, challenge };
}

/**
 * Build the OIDC authorization URL for the redirect flow.
 * Uses OIDC discovery and PKCE (S256). Stores the code_verifier in sessionStorage.
 */
export async function buildAuthorizationUrl(
  config: OidcConfig,
  state: string,
): Promise<string> {
  const { authorization_endpoint } = await discover(config.authority);
  const { verifier, challenge } = await generatePkce();

  sessionStorage.setItem("oidc_code_verifier", verifier);

  const params = new URLSearchParams({
    response_type: "code",
    client_id: config.clientId,
    redirect_uri: config.redirectUri,
    scope: config.scope,
    state,
    code_challenge: challenge,
    code_challenge_method: "S256",
  });

  return `${authorization_endpoint}?${params}`;
}

/**
 * Exchange an authorization code for tokens via the OIDC token endpoint.
 * Uses OIDC discovery to resolve the token endpoint.
 */
export async function exchangeCodeForToken(
  config: OidcConfig,
  code: string,
): Promise<string> {
  const { token_endpoint } = await discover(config.authority);

  const codeVerifier = sessionStorage.getItem("oidc_code_verifier");
  sessionStorage.removeItem("oidc_code_verifier");

  const body: Record<string, string> = {
    grant_type: "authorization_code",
    client_id: config.clientId,
    redirect_uri: config.redirectUri,
    code,
  };
  if (codeVerifier) body.code_verifier = codeVerifier;

  const res = await fetch(token_endpoint, {
    method: "POST",
    headers: { "Content-Type": "application/x-www-form-urlencoded" },
    body: new URLSearchParams(body),
  });

  if (!res.ok) {
    const err = await res.text();
    throw new Error(`Token exchange failed: ${err}`);
  }

  const data = await res.json();
  return data.id_token ?? data.access_token;
}
