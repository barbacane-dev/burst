import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, fireEvent, waitFor, act } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { wsClient } from "../../lib/ws/client";
import { getMyDoNotDisturb, setMyDoNotDisturb } from "../../lib/api/users";
import { DndIndicator } from "../ui/dnd-indicator";
import type { DoNotDisturb, DoNotDisturbSchedule, PaginatedResponse, User } from "../../lib/api/types";

const handlers = new Map<string, Set<(e: unknown) => void>>();
vi.spyOn(wsClient, "on").mockImplementation((type: string, handler: (e: unknown) => void) => {
  if (!handlers.has(type)) handlers.set(type, new Set());
  handlers.get(type)!.add(handler);
  return () => handlers.get(type)?.delete(handler);
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

vi.mock("../../lib/auth/use-auth", () => ({
  useAuth: () => ({ user: me, logout: vi.fn(), updateUser }),
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
  getMyDoNotDisturb: vi.fn(),
  setMyDoNotDisturb: vi.fn(),
  setMyStatus: vi.fn(),
  clearMyStatus: vi.fn(),
}));
vi.mock("../../lib/use-theme", () => ({
  useTheme: () => ({ theme: "light", resolved: "light", setTheme: vi.fn() }),
}));

const IN_AN_HOUR = new Date(Date.now() + 3600_000).toISOString();
const SCHEDULE: DoNotDisturbSchedule = { start: "22:00", end: "07:00", days: ["mon", "fri"], timeZone: "Europe/Paris" };

function clientWith(users: User[]) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false, staleTime: Infinity } } });
  client.setQueryData<PaginatedResponse<User>>(["users"], { items: users });
  return client;
}

beforeEach(() => {
  updateUser.mockReset();
  vi.mocked(setMyDoNotDisturb).mockReset();
});

afterEach(() => handlers.clear());

describe("the indicator", () => {
  it("shows while a user is quiet and not after", () => {
    const client = clientWith([
      { ...me, id: "usr_quiet", doNotDisturbUntil: IN_AN_HOUR },
      { ...me, id: "usr_done", doNotDisturbUntil: "2020-01-01T00:00:00Z" },
    ]);
    render(
      <QueryClientProvider client={client}>
        <DndIndicator userId="usr_quiet" />
        <DndIndicator userId="usr_done" />
      </QueryClientProvider>,
    );
    expect(screen.getAllByRole("img", { name: /Do not disturb until/ })).toHaveLength(1);
  });

  it("follows live changes", async () => {
    const { MainLayout } = await import("../layout/main-layout");
    const client = clientWith([{ ...me, id: "usr_alice" }]);
    render(
      <QueryClientProvider client={client}>
        <MemoryRouter>
          <MainLayout />
          <DndIndicator userId="usr_alice" />
        </MemoryRouter>
      </QueryClientProvider>,
    );
    const fire = (payload: unknown) => act(() => handlers.get("user.dnd_changed")?.forEach((h) => h(payload)));

    fire({ userId: "usr_alice", until: IN_AN_HOUR });
    expect(await screen.findByRole("img", { name: /Do not disturb/ })).toBeTruthy();
    fire({ userId: "usr_alice" });
    await waitFor(() => expect(screen.queryByRole("img", { name: /Do not disturb/ })).toBeNull());
  });
});

describe("the settings section", () => {
  async function renderSettings(setting: DoNotDisturb) {
    vi.mocked(getMyDoNotDisturb).mockResolvedValue(setting);
    const { SettingsPage } = await import("../../pages/settings");
    const client = clientWith([me]);
    render(
      <QueryClientProvider client={client}>
        <MemoryRouter>
          <SettingsPage />
        </MemoryRouter>
      </QueryClientProvider>,
    );
    await screen.findByText("Pause notifications");
    return client;
  }

  it("pauses for an hour and keeps the schedule", async () => {
    const next: DoNotDisturb = { snoozeUntil: IN_AN_HOUR, schedule: SCHEDULE, quietUntil: IN_AN_HOUR };
    vi.mocked(setMyDoNotDisturb).mockResolvedValue(next);
    const client = await renderSettings({ schedule: SCHEDULE });

    const before = Date.now();
    fireEvent.click(screen.getByRole("button", { name: "1 hour" }));
    await waitFor(() => expect(setMyDoNotDisturb).toHaveBeenCalled());
    const sent = vi.mocked(setMyDoNotDisturb).mock.calls[0][0];
    expect(sent.schedule).toEqual(SCHEDULE);
    expect(Math.abs(Date.parse(sent.snoozeUntil!) - before - 3600_000)).toBeLessThan(1000);

    await waitFor(() => expect(updateUser).toHaveBeenCalledWith({ ...me, doNotDisturbUntil: IN_AN_HOUR }));
    expect(client.getQueryData<PaginatedResponse<User>>(["users"])!.items[0].doNotDisturbUntil).toBe(IN_AN_HOUR);
    expect(await screen.findByText(/Notifications paused until/)).toBeTruthy();
  });

  it("resumes without dropping the schedule", async () => {
    vi.mocked(setMyDoNotDisturb).mockResolvedValue({ schedule: SCHEDULE });
    await renderSettings({ snoozeUntil: IN_AN_HOUR, quietUntil: IN_AN_HOUR, schedule: SCHEDULE });

    fireEvent.click(screen.getByText("Resume notifications"));
    await waitFor(() => expect(setMyDoNotDisturb).toHaveBeenCalledWith({ schedule: SCHEDULE }));
  });

  it("saves quiet hours with the chosen days and keeps a running snooze", async () => {
    vi.mocked(setMyDoNotDisturb).mockResolvedValue({});
    await renderSettings({ snoozeUntil: IN_AN_HOUR, quietUntil: IN_AN_HOUR });

    fireEvent.click(screen.getByLabelText("Quiet hours"));
    fireEvent.change(screen.getByLabelText("Quiet from"), { target: { value: "21:30" } });
    fireEvent.click(screen.getByRole("button", { name: "Sat" }));
    fireEvent.click(screen.getByRole("button", { name: "Mon" }));
    fireEvent.click(screen.getByText("Save quiet hours"));

    await waitFor(() => expect(setMyDoNotDisturb).toHaveBeenCalled());
    const sent = vi.mocked(setMyDoNotDisturb).mock.calls[0][0];
    expect(sent.snoozeUntil).toBe(IN_AN_HOUR);
    expect(sent.schedule).toMatchObject({ start: "21:30", end: "08:00" });
    expect([...sent.schedule!.days].sort()).toEqual(["fri", "sat", "thu", "tue", "wed"]);
    expect(sent.schedule!.timeZone).toBe(Intl.DateTimeFormat().resolvedOptions().timeZone);
  });

  it("turns quiet hours off", async () => {
    vi.mocked(setMyDoNotDisturb).mockResolvedValue({});
    await renderSettings({ schedule: SCHEDULE });

    fireEvent.click(screen.getByLabelText("Quiet hours"));
    fireEvent.click(screen.getByText("Save quiet hours"));
    await waitFor(() => expect(setMyDoNotDisturb).toHaveBeenCalledWith({ snoozeUntil: undefined, schedule: undefined }));
  });

  it("cannot save quiet hours with no day", async () => {
    await renderSettings({ schedule: { ...SCHEDULE, days: ["mon"] } });
    fireEvent.click(screen.getByRole("button", { name: "Mon" }));
    expect((screen.getByText("Save quiet hours") as HTMLButtonElement).disabled).toBe(true);
  });
});
