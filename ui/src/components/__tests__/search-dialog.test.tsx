import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { SearchDialog } from "../search-dialog";
import { searchMessages } from "../../lib/api/search";
import type { SearchResult } from "../../lib/api/types";

vi.mock("../../lib/api/search", () => ({ searchMessages: vi.fn() }));

vi.mock("../../lib/api/users", () => ({
  listUsers: vi.fn().mockResolvedValue({
    items: [
      { id: "usr_alice", displayName: "Alice", username: "alice" },
      { id: "usr_bob", displayName: "Bob", username: "bob" },
    ],
    cursor: undefined,
  }),
}));

function result(id: string, extra: Partial<SearchResult> = {}): SearchResult {
  return {
    id,
    channelId: "ch_general",
    userId: "usr_alice",
    content: `content ${id}`,
    headline: `headline ${id}`,
    createdAt: "2026-09-10T10:00:00Z",
    ...extra,
  } as SearchResult;
}

function renderDialog() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={client}>
      <MemoryRouter>
        <SearchDialog onClose={() => {}} />
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

function type(text: string) {
  fireEvent.change(screen.getByPlaceholderText("Search messages..."), { target: { value: text } });
}

function lastOptions() {
  const calls = vi.mocked(searchMessages).mock.calls;
  return calls[calls.length - 1][1]!;
}

describe("SearchDialog", () => {
  beforeEach(() => {
    vi.mocked(searchMessages).mockReset();
    vi.mocked(searchMessages).mockResolvedValue({ items: [], cursor: undefined });
  });

  it("searches with no filters by default", async () => {
    renderDialog();
    type("report");
    await waitFor(() => expect(searchMessages).toHaveBeenCalled());
    const options = lastOptions();
    expect(vi.mocked(searchMessages).mock.calls[0][0]).toBe("report");
    expect(options.from).toBeUndefined();
    expect(options.after).toBeUndefined();
    expect(options.before).toBeUndefined();
    expect(options.hasFile).toBe(false);
  });

  it("sends the author, the dates and has-file", async () => {
    renderDialog();
    await screen.findByRole("option", { name: "Bob" });
    fireEvent.change(screen.getByLabelText("Author"), { target: { value: "usr_bob" } });
    fireEvent.change(screen.getByLabelText("On or after"), { target: { value: "2026-09-01" } });
    fireEvent.change(screen.getByLabelText("On or before"), { target: { value: "2026-09-30" } });
    fireEvent.click(screen.getByLabelText("Has file"));
    type("report");

    await waitFor(() => expect(lastOptions().hasFile).toBe(true));
    const options = lastOptions();
    expect(options.from).toBe("usr_bob");
    expect(options.after).toBe(new Date(2026, 8, 1).toISOString());
    expect(options.before).toBe(new Date(2026, 9, 1).toISOString());
  });

  it("searches again when a filter changes after results are shown", async () => {
    renderDialog();
    type("report");
    await waitFor(() => expect(searchMessages).toHaveBeenCalledTimes(1));

    fireEvent.click(screen.getByLabelText("Has file"));
    await waitFor(() => expect(searchMessages).toHaveBeenCalledTimes(2));
    expect(lastOptions().hasFile).toBe(true);
  });

  it("does not search a range that ends before it starts", async () => {
    renderDialog();
    fireEvent.change(screen.getByLabelText("On or after"), { target: { value: "2026-09-30" } });
    fireEvent.change(screen.getByLabelText("On or before"), { target: { value: "2026-09-01" } });
    type("report");

    expect(await screen.findByText("The start date is after the end date")).toBeTruthy();
    await new Promise((r) => setTimeout(r, 400));
    expect(searchMessages).not.toHaveBeenCalled();
  });

  it("names the file that matched", async () => {
    vi.mocked(searchMessages).mockResolvedValue({
      items: [result("msg_1", { matchedFile: "Q3-report.pdf" }), result("msg_2")],
      cursor: undefined,
    });
    renderDialog();
    type("report");

    expect(await screen.findByText("Q3-report.pdf")).toBeTruthy();
    expect(screen.getAllByText(/^headline/)).toHaveLength(2);
  });

  it("loads the next page from the cursor", async () => {
    vi.mocked(searchMessages)
      .mockResolvedValueOnce({ items: [result("msg_1")], cursor: "0.5_next" })
      .mockResolvedValueOnce({ items: [result("msg_2")], cursor: undefined });
    renderDialog();
    type("report");

    fireEvent.click(await screen.findByText("Show more results"));
    expect(await screen.findByText("headline msg_2")).toBeTruthy();
    expect(screen.getByText("headline msg_1")).toBeTruthy();
    expect(lastOptions().cursor).toBe("0.5_next");
    expect(screen.queryByText("Show more results")).toBeNull();
  });
});
