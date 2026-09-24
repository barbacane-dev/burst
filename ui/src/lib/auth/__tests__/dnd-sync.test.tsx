import { describe, it, expect, vi } from "vitest";
import { render, screen, act } from "@testing-library/react";
import { wsClient } from "../../ws/client";
import { AuthProvider } from "../context";
import { useAuth } from "../use-auth";

const handlers = new Map<string, Set<(e: unknown) => void>>();
vi.spyOn(wsClient, "on").mockImplementation((type: string, handler: (e: unknown) => void) => {
  if (!handlers.has(type)) handlers.set(type, new Set());
  handlers.get(type)!.add(handler);
  return () => handlers.get(type)?.delete(handler);
});
vi.spyOn(wsClient, "connect").mockImplementation(() => {});
vi.spyOn(wsClient, "startHeartbeat").mockImplementation(() => () => {});

vi.mock("../../api/client", () => ({
  getAccessToken: () => "token",
  setAccessToken: vi.fn(),
  apiFetch: vi.fn().mockResolvedValue({
    id: "usr_bob",
    username: "bob",
    displayName: "Bob",
    role: "member",
    status: "online",
    isBot: false,
    createdAt: "2026-01-01T00:00:00Z",
  }),
}));

function Quiet() {
  const { user } = useAuth();
  return <p>{user ? `until:${user.doNotDisturbUntil ?? "none"}` : "loading"}</p>;
}

function fire(payload: unknown) {
  act(() => handlers.get("user.dnd_changed")?.forEach((h) => h(payload)));
}

describe("the signed-in user's do not disturb", () => {
  it("follows events for that user and ignores others", async () => {
    render(
      <AuthProvider>
        <Quiet />
      </AuthProvider>,
    );
    await screen.findByText("until:none");

    fire({ userId: "usr_alice", until: "2099-01-01T00:00:00Z" });
    expect(screen.getByText("until:none")).toBeTruthy();

    fire({ userId: "usr_bob", until: "2099-01-01T00:00:00Z" });
    expect(screen.getByText("until:2099-01-01T00:00:00Z")).toBeTruthy();

    fire({ userId: "usr_bob" });
    expect(screen.getByText("until:none")).toBeTruthy();
  });
});
