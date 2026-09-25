/// <reference types="node" />
import { describe, it, expect } from "vitest";
import { execFileSync } from "node:child_process";
import { mkdtempSync, readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { runInNewContext } from "node:vm";

// The images generate env.js at startup with docker/env.sh. A script error
// there leaves the SPA without its OIDC settings, so the real script is run and
// its output evaluated the way a browser would.
const script = resolve(__dirname, "../../../../docker/env.sh");

function generate(env: Record<string, string>): Record<string, string> {
  const target = join(mkdtempSync(join(tmpdir(), "burst-env-")), "env.js");
  execFileSync("sh", [script], { env: { PATH: process.env.PATH ?? "", BURST_ENV_JS: target, ...env } });
  const window: { __BURST_ENV__?: Record<string, string> } = {};
  runInNewContext(readFileSync(target, "utf8"), { window });
  return window.__BURST_ENV__ ?? {};
}

describe("docker/env.sh", () => {
  it("writes valid JavaScript carrying the settings", () => {
    expect(
      generate({ OIDC_AUTHORITY: "https://id.example", OIDC_CLIENT_ID: "burst", LOGIN_LOCAL: "true" }),
    ).toEqual({
      OIDC_AUTHORITY: "https://id.example",
      OIDC_CLIENT_ID: "burst",
      OIDC_REDIRECT_URI: "",
      OIDC_SCOPE: "",
      LOGIN_LOCAL: "true",
    });
  });

  it("defaults to no OIDC and no local login form", () => {
    expect(generate({})).toMatchObject({ OIDC_AUTHORITY: "", LOGIN_LOCAL: "false" });
  });

  it("fails rather than serving no configuration when it cannot write", () => {
    expect(() =>
      execFileSync("sh", [script], { env: { PATH: process.env.PATH ?? "", BURST_ENV_JS: "/nonexistent/env.js" }, stdio: "ignore" }),
    ).toThrow();
  });
});
