import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { searchMessages } from "../search";

describe("searchMessages", () => {
  beforeEach(() => {
    vi.stubGlobal("fetch", vi.fn());
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  function mockSearchResponse() {
    vi.mocked(fetch).mockResolvedValue(
      new Response(
        JSON.stringify({ items: [], cursor: null }),
        { status: 200, headers: { "Content-Type": "application/json" } },
      ),
    );
  }

  it("builds correct URL with query", async () => {
    mockSearchResponse();

    await searchMessages("hello world");

    const [url] = vi.mocked(fetch).mock.calls[0];
    const parsed = new URL(url as string, "http://localhost");
    expect(parsed.pathname).toBe("/api/search/messages");
    expect(parsed.searchParams.get("q")).toBe("hello world");
  });

  it("includes channelId when provided", async () => {
    mockSearchResponse();

    await searchMessages("test", "ch-42");

    const [url] = vi.mocked(fetch).mock.calls[0];
    const parsed = new URL(url as string, "http://localhost");
    expect(parsed.searchParams.get("channelId")).toBe("ch-42");
  });

  it("includes cursor and limit params", async () => {
    mockSearchResponse();

    await searchMessages("test", undefined, "cursor-abc", 25);

    const [url] = vi.mocked(fetch).mock.calls[0];
    const parsed = new URL(url as string, "http://localhost");
    expect(parsed.searchParams.get("cursor")).toBe("cursor-abc");
    expect(parsed.searchParams.get("limit")).toBe("25");
  });

  it("omits optional params when not provided", async () => {
    mockSearchResponse();

    await searchMessages("test");

    const [url] = vi.mocked(fetch).mock.calls[0];
    const parsed = new URL(url as string, "http://localhost");
    expect(parsed.searchParams.has("channelId")).toBe(false);
    expect(parsed.searchParams.has("cursor")).toBe(false);
    expect(parsed.searchParams.get("q")).toBe("test");
    expect(parsed.searchParams.get("limit")).toBe("50");
  });
});
