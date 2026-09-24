import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MessageComposer } from "../message-composer";
import type { User } from "../../../lib/api/types";

vi.mock("../../../lib/api/channels", () => ({
  sendMessage: vi.fn(),
}));

const bob: User = {
  id: "usr_bob",
  username: "bob",
  displayName: "Bob",
  role: "member",
} as User;

function renderComposer(users: User[] = []) {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  render(
    <QueryClientProvider client={queryClient}>
      <MessageComposer
        channelId="ch_test"
        onTypingStart={vi.fn()}
        onTypingStop={vi.fn()}
        users={users}
      />
    </QueryClientProvider>,
  );
  return screen.getByRole("textbox") as HTMLTextAreaElement;
}

function type(box: HTMLTextAreaElement, value: string) {
  fireEvent.change(box, { target: { value, selectionStart: value.length } });
}

describe("mention autocomplete offers the broadcasts", () => {
  it("suggests @channel and @here even with no users loaded", () => {
    const box = renderComposer([]);
    type(box, "@");
    const options = screen.getAllByRole("option").map((o) => o.textContent);
    expect(options.some((t) => t?.includes("@channel"))).toBe(true);
    expect(options.some((t) => t?.includes("@here"))).toBe(true);
  });

  it("narrows the broadcasts by what has been typed", () => {
    const box = renderComposer([bob]);
    type(box, "@he");
    const options = screen.getAllByRole("option").map((o) => o.textContent ?? "");
    expect(options.some((t) => t.includes("@here"))).toBe(true);
    expect(options.some((t) => t.includes("@channel"))).toBe(false);
  });

  it("inserts the chosen broadcast into the message", () => {
    const box = renderComposer([bob]);
    type(box, "heads up @ch");
    fireEvent.click(screen.getByRole("option", { name: /@channel/ }));
    expect(box.value).toBe("heads up @channel ");
  });

  it("still suggests users", () => {
    const box = renderComposer([bob]);
    type(box, "@bo");
    expect(screen.getByRole("option", { name: /Bob/ })).toBeTruthy();
  });
});
