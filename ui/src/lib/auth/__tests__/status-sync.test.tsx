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

function Status() {
  const { user } = useAuth();
  return <p>{user ? `status:${user.statusEmoji ?? ""}${user.statusText ?? ""}` : "loading"}</p>;
}

function fire(payload: unknown) {
  act(() => handlers.get("user.status_changed")?.forEach((h) => h(payload)));
}

describe("the signed-in user's status", () => {
  it("follows status events for that user and ignores others", async () => {
    render(
      <AuthProvider>
        <Status />
      </AuthProvider>,
    );
    await screen.findByText("status:");

    fire({ userId: "usr_alice", text: "Not me" });
    expect(screen.getByText("status:")).toBeTruthy();

    fire({ userId: "usr_bob", text: "Set elsewhere", emoji: "📱" });
    expect(screen.getByText("status:📱Set elsewhere")).toBeTruthy();

    fire({ userId: "usr_bob" });
    expect(screen.getByText("status:")).toBeTruthy();
  });
});
