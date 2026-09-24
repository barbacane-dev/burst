import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { wsClient } from "../../lib/ws/client";
import { showBrowserNotification } from "../../lib/notifications";
import type { NotificationEvent } from "../../lib/api/types";

const { navigateSpy } = vi.hoisted(() => ({ navigateSpy: vi.fn() }));

vi.mock("react-router-dom", async (importOriginal) => {
  const actual = await importOriginal<typeof import("react-router-dom")>();
  return { ...actual, useNavigate: () => navigateSpy };
});

vi.mock("../../lib/notifications", () => ({
  showBrowserNotification: vi.fn(),
}));

vi.mock("../../lib/auth/use-auth", () => ({
  useAuth: () => ({
    user: { id: "usr_bob", displayName: "Bob", username: "bob", role: "member" },
    logout: vi.fn(),
  }),
}));

vi.mock("../../lib/api/channels", () => ({
  listChannels: vi.fn().mockResolvedValue({ items: [], cursor: undefined }),
  browseChannels: vi.fn().mockResolvedValue({ items: [], cursor: undefined }),
  listMembers: vi.fn().mockResolvedValue([]),
  markChannelRead: vi.fn().mockResolvedValue(undefined),
}));

vi.mock("../../lib/api/users", () => ({
  listUsers: vi.fn().mockResolvedValue({ items: [], cursor: undefined }),
}));

vi.mock("../../lib/use-theme", () => ({
  useTheme: () => ({ resolved: "light", setTheme: vi.fn() }),
}));

const handlers = new Map<string, Set<(e: unknown) => void>>();

function fire(type: string, payload: unknown) {
  handlers.get(type)?.forEach((h) => h(payload));
}

function notification(overrides: Partial<NotificationEvent> = {}): NotificationEvent {
  return {
    type: "notification.created",
    notificationId: "ntf_1",
    recipientId: "usr_bob",
    channelId: "ch_general",
    messageId: "msg_1",
    reason: "message",
    authorName: "Alice",
    channelName: "general",
    preview: "the build is green",
    ...overrides,
  };
}

async function renderLayout() {
  const { MainLayout } = await import("../layout/main-layout");
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  render(
    <QueryClientProvider client={queryClient}>
      <MemoryRouter initialEntries={["/channels/ch_current"]}>
        <MainLayout />
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

describe("browser notifications follow the server's routing", () => {
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
  });

  it("does not notify on message.created, which every member receives", async () => {
    await renderLayout();
    fire("message.created", {
      type: "message.created",
      channelId: "ch_other",
      message: { id: "msg_1", channelId: "ch_other", userId: "usr_bob", content: "my own message" },
    });
    expect(showBrowserNotification).not.toHaveBeenCalled();
  });

  it("notifies on notification.created, naming the author and channel", async () => {
    await renderLayout();
    fire("notification.created", notification());
    expect(showBrowserNotification).toHaveBeenCalledWith(
      "Alice in #general",
      "the build is green",
      expect.objectContaining({ tag: "ch_general" }),
    );
  });

  it("titles a direct message by its author alone", async () => {
    await renderLayout();
    fire("notification.created", notification({ channelName: undefined, channelId: "ch_dm" }));
    expect(showBrowserNotification).toHaveBeenCalledWith(
      "Alice",
      expect.any(String),
      expect.anything(),
    );
  });

  it("describes a message with no text as a file", async () => {
    await renderLayout();
    fire("notification.created", notification({ preview: "" }));
    expect(showBrowserNotification).toHaveBeenCalledWith(
      expect.any(String),
      "sent a file",
      expect.anything(),
    );
  });

  it("opens the message's channel when clicked", async () => {
    await renderLayout();
    fire("notification.created", notification({ channelId: "ch_deploys" }));
    const options = vi.mocked(showBrowserNotification).mock.calls[0][2];
    options?.onClick?.();
    expect(navigateSpy).toHaveBeenCalledWith("/channels/ch_deploys");
  });
});
