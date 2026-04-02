import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { getOidcConfig } from "../oidc";

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
    expect(config!.scope).toBe("openid email profile");
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
    expect(config!.scope).toBe("openid email profile");
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
