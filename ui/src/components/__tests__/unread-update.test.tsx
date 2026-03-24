import { describe, it, expect, vi, afterEach } from "vitest";
import { render } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { wsClient } from "../../lib/ws/client";
import type { Channel, PaginatedResponse } from "../../lib/api/types";

// Capture handlers registered via wsClient.on
const registeredHandlers = new Map<string, Set<(e: unknown) => void>>();
vi.spyOn(wsClient, "on").mockImplementation((type: string, handler: (e: unknown) => void) => {
  if (!registeredHandlers.has(type)) registeredHandlers.set(type, new Set());
  registeredHandlers.get(type)!.add(handler);
  return () => registeredHandlers.get(type)?.delete(handler);
});

// Mock auth hook to avoid network calls
vi.mock("../../lib/auth/use-auth", () => ({
  useAuth: () => ({
    user: { id: "usr_bob", displayName: "Bob", username: "bob", role: "member" },
    logout: vi.fn(),
  }),
}));

// Mock notifications
vi.mock("../../lib/notifications", () => ({
  showBrowserNotification: vi.fn(),
}));

// Mock the channels API to return seed data
vi.mock("../../lib/api/channels", () => ({
  listChannels: vi.fn().mockResolvedValue({
    items: [
      {
        id: "ch_general",
        kind: "public",
        name: "general",
        slug: "general",
        createdBy: "usr_alice",
        isArchived: false,
        isReadonly: false,
        unreadCount: 0,
        createdAt: "2026-01-01T00:00:00Z",
        updatedAt: "2026-01-01T00:00:00Z",
      },
    ],
    cursor: undefined,
  } satisfies PaginatedResponse<Channel>),
  browseChannels: vi.fn().mockResolvedValue({ items: [], cursor: undefined }),
  listMembers: vi.fn().mockResolvedValue([]),
  markChannelRead: vi.fn().mockResolvedValue(undefined),
}));

vi.mock("../../lib/api/users", () => ({
  listUsers: vi.fn().mockResolvedValue({ items: [], cursor: undefined }),
}));

// Mock theme
vi.mock("../../lib/use-theme", () => ({
  useTheme: () => ({ resolved: "light", setTheme: vi.fn() }),
}));

function fireWsEvent(type: string, payload: unknown) {
  registeredHandlers.get(type)?.forEach((h) => h(payload));
}

describe("Unread count update on message.created", () => {
  afterEach(() => {
    registeredHandlers.clear();
    vi.restoreAllMocks();
  });

  it("invalidates channels query when message.created fires for another channel", async () => {
    // We import MainLayout dynamically so the wsClient mock is in place
    const { MainLayout } = await import("../layout/main-layout");

    const queryClient = new QueryClient({
      defaultOptions: { queries: { retry: false } },
    });
    const invalidateSpy = vi.spyOn(queryClient, "invalidateQueries");

    render(
      <QueryClientProvider client={queryClient}>
        <MemoryRouter initialEntries={["/channels/ch_current"]}>
          <MainLayout />
        </MemoryRouter>
      </QueryClientProvider>,
    );

    // MainLayout registers a message.created handler
    expect(registeredHandlers.has("message.created")).toBe(true);

    // Simulate a message arriving in a different channel
    fireWsEvent("message.created", {
      type: "message.created",
      channelId: "ch_other",
      message: {
        id: "msg_1",
        channelId: "ch_other",
        userId: "usr_alice",
        content: "hello from alice",
        createdAt: "2026-03-18T00:00:00Z",
        reactions: [],
        attachments: [],
        replyCount: 0,
      },
    });

    // The channels query should be invalidated so sidebar refetches unread counts
    expect(invalidateSpy).toHaveBeenCalledWith(
      expect.objectContaining({ queryKey: ["channels"] }),
    );
  });
});
