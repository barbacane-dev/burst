import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen } from "@testing-library/react";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { wsClient } from "../../lib/ws/client";

const hoisted = vi.hoisted(() => ({
  navigateSpy: vi.fn(),
  role: "member" as string,
}));

vi.mock("react-router-dom", async (importOriginal) => {
  const actual = await importOriginal<typeof import("react-router-dom")>();
  return { ...actual, useNavigate: () => hoisted.navigateSpy };
});

vi.mock("../../lib/auth/use-auth", () => ({
  useAuth: () => ({
    user: { id: "usr_bob", displayName: "Bob", username: "bob", role: hoisted.role },
    logout: vi.fn(),
  }),
}));

vi.mock("../../lib/api/channels", () => ({
  listChannels: vi.fn().mockResolvedValue({ items: [], cursor: undefined }),
  browseChannels: vi.fn().mockResolvedValue({ items: [], cursor: undefined }),
  listMembers: vi.fn().mockResolvedValue([]),
  createChannel: vi.fn(),
  createDm: vi.fn(),
  createGroupDm: vi.fn(),
  joinChannel: vi.fn(),
}));

vi.mock("../../lib/api/users", () => ({
  listUsers: vi.fn().mockResolvedValue({ items: [], cursor: undefined }),
}));

vi.mock("../../lib/use-theme", () => ({
  useTheme: () => ({ resolved: "light", setTheme: vi.fn() }),
}));

const handlers = new Map<string, Set<(e: unknown) => void>>();
const fire = (type: string, payload: unknown) => handlers.get(type)?.forEach((h) => h(payload));

async function renderSidebar(path = "/channels/ch_room") {
  const { Sidebar } = await import("../layout/sidebar");
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  render(
    <QueryClientProvider client={qc}>
      <MemoryRouter initialEntries={[path]}>
        <Routes>
          <Route path="/channels/:channelId" element={<Sidebar />} />
          <Route path="/" element={<Sidebar />} />
        </Routes>
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

describe("the sidebar follows the caller's role and memberships", () => {
  beforeEach(() => {
    vi.spyOn(wsClient, "on").mockImplementation((type: string, handler: (e: unknown) => void) => {
      if (!handlers.has(type)) handlers.set(type, new Set());
      handlers.get(type)!.add(handler);
      return () => handlers.get(type)?.delete(handler);
    });
  });

  afterEach(() => {
    handlers.clear();
    vi.clearAllMocks();
    hoisted.role = "member";
  });

  it("offers a member browsing, creating channels and starting conversations", async () => {
    hoisted.role = "member";
    await renderSidebar();
    expect(screen.getByTitle("Browse channels")).toBeTruthy();
    expect(screen.getByTitle("Create channel")).toBeTruthy();
    expect(screen.getByTitle("New direct message")).toBeTruthy();
  });

  it("offers a guest none of those", async () => {
    hoisted.role = "guest";
    await renderSidebar();
    expect(screen.queryByTitle("Browse channels")).toBeNull();
    expect(screen.queryByTitle("Create channel")).toBeNull();
    expect(screen.queryByTitle("New direct message")).toBeNull();
  });

  it("leaves the channel on screen when its user is removed from it", async () => {
    await renderSidebar("/channels/ch_room");
    fire("channel.left", { type: "channel.left", channelId: "ch_room", userId: "usr_bob" });
    expect(hoisted.navigateSpy).toHaveBeenCalledWith("/");
  });

  it("stays put when someone else leaves, or when they leave another channel", async () => {
    await renderSidebar("/channels/ch_room");
    fire("channel.left", { type: "channel.left", channelId: "ch_room", userId: "usr_carol" });
    fire("channel.left", { type: "channel.left", channelId: "ch_other", userId: "usr_bob" });
    expect(hoisted.navigateSpy).not.toHaveBeenCalled();
  });
});
