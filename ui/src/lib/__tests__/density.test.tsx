/// <reference types="node" />
import { describe, it, expect, beforeEach, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter } from "react-router-dom";
import { applyDensity, DENSITIES, storedDensity } from "../density";
import { STORAGE_KEY_DENSITY } from "../constants";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";

vi.mock("../auth/use-auth", () => ({
  useAuth: () => ({
    user: {
      id: "usr_bob",
      username: "bob",
      displayName: "Bob",
      role: "member",
      status: "online",
      isBot: false,
      createdAt: "2026-01-01T00:00:00Z",
    },
    logout: vi.fn(),
  }),
}));

// Node's own global `localStorage` is undefined without a storage file and
// hides jsdom's, so the tests provide one.
function memoryStorage(): Storage {
  const items = new Map<string, string>();
  return {
    get length() {
      return items.size;
    },
    clear: () => items.clear(),
    getItem: (key) => items.get(key) ?? null,
    key: (index) => [...items.keys()][index] ?? null,
    removeItem: (key) => void items.delete(key),
    setItem: (key, value) => void items.set(key, String(value)),
  };
}

beforeEach(() => {
  vi.stubGlobal("localStorage", memoryStorage());
  delete document.documentElement.dataset.density;
});

describe("storedDensity", () => {
  it("is normal until one is saved", () => {
    expect(storedDensity()).toBe("normal");
  });

  it("reads a saved size and ignores anything else", () => {
    localStorage.setItem(STORAGE_KEY_DENSITY, "compact");
    expect(storedDensity()).toBe("compact");
    localStorage.setItem(STORAGE_KEY_DENSITY, "huge");
    expect(storedDensity()).toBe("normal");
  });

  it("falls back when storage is unavailable", () => {
    vi.stubGlobal("localStorage", {
      getItem: () => {
        throw new Error("blocked");
      },
    });
    expect(storedDensity()).toBe("normal");
  });
});

describe("applyDensity", () => {
  it("marks the document root", () => {
    applyDensity("dense");
    expect(document.documentElement.dataset.density).toBe("dense");
  });
});

describe("the stylesheet", () => {
  // jsdom does not apply CSS, so the rules are read as text. A size with no
  // rule would change nothing on screen.
  const css = readFileSync(resolve(__dirname, "../../index.css"), "utf8");

  it("sizes every non-default choice, in percent", () => {
    for (const { value } of DENSITIES.filter((d) => d.value !== "normal")) {
      expect(css).toMatch(new RegExp(`html\\[data-density="${value}"\\]\\s*\\{\\s*font-size:\\s*[0-9.]+%;`));
    }
  });

  it("orders the sizes from largest to smallest", () => {
    const size = (v: string) =>
      Number(new RegExp(`data-density="${v}"\\]\\s*\\{\\s*font-size:\\s*([0-9.]+)%`).exec(css)?.[1] ?? 100);
    const sizes = DENSITIES.map((d) => size(d.value));
    expect(sizes).toEqual([...sizes].sort((a, b) => b - a));
    expect(size("normal")).toBe(100);
  });
});

describe("the settings control", () => {
  async function renderSettings() {
    const { SettingsPage } = await import("../../pages/settings");
    render(
      <QueryClientProvider client={new QueryClient()}>
        <MemoryRouter>
          <SettingsPage />
        </MemoryRouter>
      </QueryClientProvider>,
    );
  }

  it("applies and remembers the chosen size", async () => {
    await renderSettings();
    expect(screen.getByRole("radio", { name: /Normal/ }).getAttribute("aria-checked")).toBe("true");

    fireEvent.click(screen.getByRole("radio", { name: /Compact/ }));
    expect(document.documentElement.dataset.density).toBe("compact");
    expect(localStorage.getItem(STORAGE_KEY_DENSITY)).toBe("compact");
    expect(screen.getByRole("radio", { name: /Compact/ }).getAttribute("aria-checked")).toBe("true");
    expect(screen.getByRole("radio", { name: /Normal/ }).getAttribute("aria-checked")).toBe("false");
  });

  it("shows the saved size as selected", async () => {
    localStorage.setItem(STORAGE_KEY_DENSITY, "large");
    await renderSettings();
    expect(screen.getByRole("radio", { name: /Large/ }).getAttribute("aria-checked")).toBe("true");
  });
});
