import { describe, it, expect, vi, afterEach } from "vitest";
import { render, screen } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MessageBubble } from "../message-bubble";
import type { Message } from "../../../lib/api/types";

// Mock API calls so mutations never hit the network
vi.mock("../../../lib/api/channels", () => ({
  addReaction: vi.fn().mockResolvedValue(undefined),
  removeReaction: vi.fn().mockResolvedValue(undefined),
  pinMessage: vi.fn().mockResolvedValue(undefined),
}));

function makeMessage(overrides: Partial<Message> = {}): Message {
  return {
    id: "msg_1",
    channelId: "ch_general",
    userId: "usr_alice",
    content: "Hello, world!",
    replyCount: 0,
    reactions: [],
    attachments: [],
    createdAt: "2026-03-20T12:00:00Z",
    ...overrides,
  };
}

function renderBubble(
  message: Message,
  overrides: Partial<Parameters<typeof MessageBubble>[0]> = {},
) {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });

  const usersById = overrides.usersById ?? new Map([["usr_alice", "Alice"]]);

  return render(
    <QueryClientProvider client={queryClient}>
      <MessageBubble
        message={message}
        currentUserId="usr_bob"
        channelId="ch_general"
        usersById={usersById}
        {...overrides}
      />
    </QueryClientProvider>,
  );
}

afterEach(() => {
  vi.restoreAllMocks();
});

describe("MessageBubble", () => {
  it("renders message content and display name", () => {
    renderBubble(makeMessage({ content: "Hey there!" }));

    expect(screen.getByText("Alice")).toBeInTheDocument();
    expect(screen.getByText("Hey there!")).toBeInTheDocument();
    expect(
      screen.getByRole("listitem", { name: /Message from Alice/ }),
    ).toBeInTheDocument();
  });

  it("falls back to truncated userId when usersById has no entry", () => {
    const usersById = new Map<string, string>();
    renderBubble(makeMessage({ userId: "usr_unknown123" }), { usersById });

    expect(screen.getByText("unknown1")).toBeInTheDocument();
  });

  it("shows reaction pills for existing reactions", () => {
    const message = makeMessage({
      reactions: [
        { emoji: "\u{1F44D}", count: 3, userIds: ["usr_alice", "usr_charlie", "usr_bob"] },
        { emoji: "\u{1F389}", count: 1, userIds: ["usr_alice"] },
      ],
    });

    renderBubble(message);

    expect(screen.getByText("\u{1F44D}")).toBeInTheDocument();
    expect(screen.getByText("3")).toBeInTheDocument();
    expect(screen.getByText("\u{1F389}")).toBeInTheDocument();
    expect(screen.getByText("1")).toBeInTheDocument();
  });

  it("renders deleted state for soft-deleted messages", () => {
    const message = makeMessage({
      content: "This should be hidden",
      deletedAt: "2026-03-20T13:00:00Z",
    });

    renderBubble(message);

    expect(screen.getByText("This message was deleted")).toBeInTheDocument();
    expect(screen.queryByText("This should be hidden")).not.toBeInTheDocument();
  });

  it("shows (edited) indicator when message has editedAt", () => {
    const message = makeMessage({
      editedAt: "2026-03-20T12:05:00Z",
    });

    renderBubble(message);

    expect(screen.getByText("(edited)")).toBeInTheDocument();
  });

  it("shows thread reply count button when replyCount > 0", () => {
    const onOpenThread = vi.fn();
    renderBubble(makeMessage({ replyCount: 5 }), { onOpenThread });

    const button = screen.getByText("5 replies");
    expect(button).toBeInTheDocument();
  });

  it("uses singular 'reply' for replyCount === 1", () => {
    renderBubble(makeMessage({ replyCount: 1 }));

    expect(screen.getByText("1 reply")).toBeInTheDocument();
  });

  it("hides hover actions when message is deleted", () => {
    renderBubble(makeMessage({ deletedAt: "2026-03-20T13:00:00Z" }));

    expect(screen.queryByTitle("Add reaction")).not.toBeInTheDocument();
    expect(screen.queryByTitle("Pin message")).not.toBeInTheDocument();
  });
});
