import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import {
  getOidcConfig,
  buildAuthorizationUrl,
  exchangeCodeForToken,
  _resetDiscoveryCache,
} from "../oidc";

describe("getOidcConfig", () => {
  beforeEach(() => {
    delete window.__BURST_ENV__;
    // Clear any OIDC vars from .env.local so tests start clean
    delete (import.meta.env as Record<string, unknown>).VITE_OIDC_AUTHORITY;
    delete (import.meta.env as Record<string, unknown>).VITE_OIDC_CLIENT_ID;
    delete (import.meta.env as Record<string, unknown>).VITE_OIDC_REDIRECT_URI;
    delete (import.meta.env as Record<string, unknown>).VITE_OIDC_SCOPE;
  });

  afterEach(() => {
    vi.unstubAllEnvs();
  });

  it("returns null when no OIDC config is provided", () => {
    expect(getOidcConfig()).toBeNull();
  });

  it("reads from Vite env vars (dev mode)", () => {
    vi.stubEnv("VITE_OIDC_AUTHORITY", "https://accounts.google.com");
    vi.stubEnv("VITE_OIDC_CLIENT_ID", "my-client-id");

    const config = getOidcConfig();
    expect(config).not.toBeNull();
    expect(config!.authority).toBe("https://accounts.google.com");
    expect(config!.clientId).toBe("my-client-id");
    expect(config!.redirectUri).toBe("http://localhost:3000/callback");
    expect(config!.scope).toBe("openid email profile groups");
  });

  it("reads from window.__BURST_ENV__ (runtime injection)", () => {
    window.__BURST_ENV__ = {
      OIDC_AUTHORITY: "https://login.example.com",
      OIDC_CLIENT_ID: "runtime-client",
    };

    const config = getOidcConfig();
    expect(config).not.toBeNull();
    expect(config!.authority).toBe("https://login.example.com");
    expect(config!.clientId).toBe("runtime-client");
  });

  it("runtime env takes precedence over Vite env", () => {
    vi.stubEnv("VITE_OIDC_AUTHORITY", "https://vite-issuer.com");
    vi.stubEnv("VITE_OIDC_CLIENT_ID", "vite-client");
    window.__BURST_ENV__ = {
      OIDC_AUTHORITY: "https://runtime-issuer.com",
      OIDC_CLIENT_ID: "runtime-client",
    };

    const config = getOidcConfig();
    expect(config!.authority).toBe("https://runtime-issuer.com");
    expect(config!.clientId).toBe("runtime-client");
  });

  it("uses defaults for optional fields", () => {
    window.__BURST_ENV__ = {
      OIDC_AUTHORITY: "https://login.example.com",
      OIDC_CLIENT_ID: "test-client",
    };

    const config = getOidcConfig();
    expect(config!.redirectUri).toBe("http://localhost:3000/callback");
    expect(config!.scope).toBe("openid email profile groups");
  });

  it("allows overriding optional fields via runtime env", () => {
    window.__BURST_ENV__ = {
      OIDC_AUTHORITY: "https://login.example.com",
      OIDC_CLIENT_ID: "test-client",
      OIDC_REDIRECT_URI: "https://burst.prod.com/callback",
      OIDC_SCOPE: "openid email",
    };

    const config = getOidcConfig();
    expect(config!.redirectUri).toBe("https://burst.prod.com/callback");
    expect(config!.scope).toBe("openid email");
  });

  it("returns null when runtime env has empty strings", () => {
    window.__BURST_ENV__ = {
      OIDC_AUTHORITY: "",
      OIDC_CLIENT_ID: "",
    };

    expect(getOidcConfig()).toBeNull();
  });
});

describe("OIDC discovery", () => {
  const discoveryDoc = {
    authorization_endpoint: "https://idp.example.com/authorize",
    token_endpoint: "https://idp.example.com/token",
  };

  beforeEach(() => {
    _resetDiscoveryCache();
    vi.stubGlobal("fetch", vi.fn());
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  describe("buildAuthorizationUrl", () => {
    it("fetches discovery document and builds authorization URL with PKCE", async () => {
      vi.mocked(fetch).mockResolvedValueOnce({
        ok: true,
        json: async () => discoveryDoc,
      } as Response);

      const config = {
        authority: "https://idp.example.com",
        clientId: "my-app",
        redirectUri: "http://localhost:8080/callback",
        scope: "openid email profile groups",
      };

      const url = await buildAuthorizationUrl(config, "state123");

      expect(fetch).toHaveBeenCalledWith(
        "https://idp.example.com/.well-known/openid-configuration",
      );
      expect(url).toContain("https://idp.example.com/authorize?");
      expect(url).toContain("client_id=my-app");
      expect(url).toContain("redirect_uri=http%3A%2F%2Flocalhost%3A8080%2Fcallback");
      expect(url).toContain("response_type=code");
      expect(url).toContain("scope=openid+email+profile");
      expect(url).toContain("state=state123");
      expect(url).toContain("code_challenge=");
      expect(url).toContain("code_challenge_method=S256");

      // Verifier stored in sessionStorage
      expect(sessionStorage.getItem("oidc_code_verifier")).toBeTruthy();
    });

    it("caches discovery document across calls", async () => {
      vi.mocked(fetch).mockResolvedValueOnce({
        ok: true,
        json: async () => discoveryDoc,
      } as Response);

      const config = {
        authority: "https://idp.example.com",
        clientId: "my-app",
        redirectUri: "http://localhost/callback",
        scope: "openid",
      };

      await buildAuthorizationUrl(config, "s1");
      await buildAuthorizationUrl(config, "s2");

      // Only one fetch — second call uses cache
      expect(fetch).toHaveBeenCalledTimes(1);
    });

    it("throws when discovery fails", async () => {
      vi.mocked(fetch).mockResolvedValueOnce({
        ok: false,
        status: 404,
      } as Response);

      const config = {
        authority: "https://bad.example.com",
        clientId: "app",
        redirectUri: "http://localhost/callback",
        scope: "openid",
      };

      await expect(buildAuthorizationUrl(config, "s")).rejects.toThrow(
        "OIDC discovery failed: 404",
      );
    });
  });

  describe("exchangeCodeForToken", () => {
    it("uses discovered token endpoint with PKCE verifier and returns id_token", async () => {
      sessionStorage.setItem("oidc_code_verifier", "test-verifier");

      vi.mocked(fetch)
        .mockResolvedValueOnce({
          ok: true,
          json: async () => discoveryDoc,
        } as Response)
        .mockResolvedValueOnce({
          ok: true,
          json: async () => ({ id_token: "jwt.token.here", access_token: "at" }),
        } as Response);

      const config = {
        authority: "https://idp.example.com",
        clientId: "my-app",
        redirectUri: "http://localhost/callback",
        scope: "openid",
      };

      const token = await exchangeCodeForToken(config, "auth-code-123");

      expect(token).toBe("jwt.token.here");
      expect(fetch).toHaveBeenCalledWith("https://idp.example.com/token", {
        method: "POST",
        headers: { "Content-Type": "application/x-www-form-urlencoded" },
        body: expect.any(URLSearchParams),
      });

      // Verifier was included in the request body
      const body = vi.mocked(fetch).mock.calls[1][1]!.body as URLSearchParams;
      expect(body.get("code_verifier")).toBe("test-verifier");

      // Verifier cleaned up from sessionStorage
      expect(sessionStorage.getItem("oidc_code_verifier")).toBeNull();
    });

    it("falls back to access_token when id_token is absent", async () => {
      vi.mocked(fetch)
        .mockResolvedValueOnce({
          ok: true,
          json: async () => discoveryDoc,
        } as Response)
        .mockResolvedValueOnce({
          ok: true,
          json: async () => ({ access_token: "access-tok" }),
        } as Response);

      const config = {
        authority: "https://idp.example.com",
        clientId: "app",
        redirectUri: "http://localhost/callback",
        scope: "openid",
      };

      const token = await exchangeCodeForToken(config, "code");
      expect(token).toBe("access-tok");
    });

    it("throws on token exchange failure", async () => {
      vi.mocked(fetch)
        .mockResolvedValueOnce({
          ok: true,
          json: async () => discoveryDoc,
        } as Response)
        .mockResolvedValueOnce({
          ok: false,
          text: async () => "invalid_grant",
        } as Response);

      const config = {
        authority: "https://idp.example.com",
        clientId: "app",
        redirectUri: "http://localhost/callback",
        scope: "openid",
      };

      await expect(exchangeCodeForToken(config, "bad-code")).rejects.toThrow(
        "Token exchange failed: invalid_grant",
      );
    });
  });
});
