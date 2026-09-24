import { describe, it, expect, vi, afterEach, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor, act } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { wsClient } from "../../lib/ws/client";
import { clearMyStatus, setMyStatus } from "../../lib/api/users";
import { UserStatusEmoji, UserStatusLine } from "../ui/user-status";
import type { PaginatedResponse, User } from "../../lib/api/types";

const registeredHandlers = new Map<string, Set<(e: unknown) => void>>();
vi.spyOn(wsClient, "on").mockImplementation((type: string, handler: (e: unknown) => void) => {
  if (!registeredHandlers.has(type)) registeredHandlers.set(type, new Set());
  registeredHandlers.get(type)!.add(handler);
  return () => registeredHandlers.get(type)?.delete(handler);
});

const updateUser = vi.fn();
const me: User = {
  id: "usr_bob",
  displayName: "Bob",
  username: "bob",
  role: "member",
  status: "online",
  isBot: false,
  createdAt: "2026-01-01T00:00:00Z",
};
let currentUser: User = me;

vi.mock("../../lib/auth/use-auth", () => ({
  useAuth: () => ({ user: currentUser, logout: vi.fn(), updateUser }),
}));
vi.mock("../../lib/notifications", () => ({ showBrowserNotification: vi.fn() }));
vi.mock("../../lib/api/channels", () => ({
  listChannels: vi.fn().mockResolvedValue({ items: [], cursor: undefined }),
  browseChannels: vi.fn().mockResolvedValue({ items: [], cursor: undefined }),
  listMembers: vi.fn().mockResolvedValue([]),
  markChannelRead: vi.fn().mockResolvedValue(undefined),
}));
vi.mock("../../lib/api/users", () => ({
  listUsers: vi.fn().mockResolvedValue({ items: [], cursor: undefined }),
  updateMe: vi.fn(),
  setMyStatus: vi.fn(),
  clearMyStatus: vi.fn(),
  getMyDoNotDisturb: vi.fn().mockResolvedValue({}),
  setMyDoNotDisturb: vi.fn(),
}));
vi.mock("../../lib/use-theme", () => ({
  useTheme: () => ({ theme: "light", resolved: "light", setTheme: vi.fn() }),
}));

function alice(extra: Partial<User> = {}): User {
  return { ...me, id: "usr_alice", displayName: "Alice", username: "alice", ...extra };
}

function clientWith(users: User[]) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false, staleTime: Infinity } } });
  client.setQueryData<PaginatedResponse<User>>(["users"], { items: users });
  return client;
}

function fire(type: string, payload: unknown) {
  act(() => registeredHandlers.get(type)?.forEach((h) => h(payload)));
}

beforeEach(() => {
  currentUser = me;
  updateUser.mockReset();
});

afterEach(() => {
  registeredHandlers.clear();
});

describe("status badges", () => {
  it("show the emoji with the text as its label", () => {
    const client = clientWith([alice({ statusText: "In a meeting", statusEmoji: "📅" })]);
    render(
      <QueryClientProvider client={client}>
        <UserStatusEmoji userId="usr_alice" />
        <UserStatusLine userId="usr_alice" />
      </QueryClientProvider>,
    );
    expect(screen.getByRole("img", { name: "Status: In a meeting" }).textContent).toBe("📅");
    expect(screen.getByText("In a meeting")).toBeTruthy();
  });

  it("show nothing once the status has expired", () => {
    const client = clientWith([
      alice({ statusText: "Gone", statusEmoji: "🌴", statusExpiresAt: "2020-01-01T00:00:00Z" }),
    ]);
    const { container } = render(
      <QueryClientProvider client={client}>
        <UserStatusEmoji userId="usr_alice" />
        <UserStatusLine userId="usr_alice" />
      </QueryClientProvider>,
    );
    expect(container.textContent).toBe("");
  });
});

describe("live status changes", () => {
  it("update the shared user list, and the badge with it", async () => {
    const { MainLayout } = await import("../layout/main-layout");
    const client = clientWith([alice()]);
    render(
      <QueryClientProvider client={client}>
        <MemoryRouter>
          <MainLayout />
          <UserStatusEmoji userId="usr_alice" />
        </MemoryRouter>
      </QueryClientProvider>,
    );
    expect(screen.queryByRole("img", { name: /Status/ })).toBeNull();

    fire("user.status_changed", { type: "user.status_changed", userId: "usr_alice", text: "Lunch", emoji: "🍕" });
    expect((await screen.findByRole("img", { name: "Status: Lunch" })).textContent).toBe("🍕");

    fire("user.status_changed", { type: "user.status_changed", userId: "usr_alice" });
    await waitFor(() => expect(screen.queryByRole("img", { name: /Status/ })).toBeNull());
  });
});

describe("the status editor", () => {
  async function renderSettings(client = clientWith([me])) {
    const { SettingsPage } = await import("../../pages/settings");
    render(
      <QueryClientProvider client={client}>
        <MemoryRouter>
          <SettingsPage />
        </MemoryRouter>
      </QueryClientProvider>,
    );
    return client;
  }

  it("sets a preset with its expiry and applies the result", async () => {
    const updated = { ...me, statusText: "In a meeting", statusEmoji: "📅", statusExpiresAt: "2099-01-01T00:00:00Z" };
    vi.mocked(setMyStatus).mockResolvedValue(updated);
    const client = await renderSettings();

    const before = Date.now();
    fireEvent.click(screen.getByText("📅 In a meeting"));
    fireEvent.click(screen.getByText("Set status"));

    await waitFor(() => expect(setMyStatus).toHaveBeenCalled());
    const sent = vi.mocked(setMyStatus).mock.calls[0][0];
    expect(sent.text).toBe("In a meeting");
    expect(sent.emoji).toBe("📅");
    const lasts = Date.parse(sent.expiresAt!) - before;
    expect(lasts).toBeGreaterThanOrEqual(3600_000 - 1000);
    expect(lasts).toBeLessThanOrEqual(3600_000 + 1000);

    await waitFor(() => expect(updateUser).toHaveBeenCalledWith(updated));
    const list = client.getQueryData<PaginatedResponse<User>>(["users"])!;
    expect(list.items[0].statusEmoji).toBe("📅");
  });

  it("cannot set a blank status", async () => {
    await renderSettings();
    expect((screen.getByText("Set status") as HTMLButtonElement).disabled).toBe(true);
  });

  it("clears the current status", async () => {
    currentUser = { ...me, statusText: "Away", statusEmoji: "🌴" };
    vi.mocked(clearMyStatus).mockResolvedValue(me);
    await renderSettings(clientWith([currentUser]));

    expect(screen.getByText(/Now showing/).textContent).toContain("Away");
    fireEvent.click(screen.getByText("Clear status"));
    await waitFor(() => expect(clearMyStatus).toHaveBeenCalled());
    await waitFor(() => expect(updateUser).toHaveBeenCalledWith(me));
  });
});
