import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { sendMessage } from "../channels";

describe("sendMessage", () => {
  beforeEach(() => {
    vi.stubGlobal("fetch", vi.fn());
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("sends JSON for text-only messages", async () => {
    vi.mocked(fetch).mockResolvedValue(
      new Response(
        JSON.stringify({
          id: "msg-1",
          channelId: "ch-1",
          userId: "u-1",
          content: "hello",
          replyCount: 0,
          reactions: [],
          attachments: [],
          createdAt: "2026-01-01T00:00:00Z",
        }),
        { status: 200, headers: { "Content-Type": "application/json" } },
      ),
    );

    await sendMessage("ch-1", "hello");

    expect(fetch).toHaveBeenCalledOnce();
    const [, init] = vi.mocked(fetch).mock.calls[0];
    expect(init?.headers).toHaveProperty("Content-Type", "application/json");
    expect(JSON.parse(init?.body as string)).toEqual({ content: "hello" });
  });

  it("sends FormData when files are present", async () => {
    vi.mocked(fetch).mockResolvedValue(
      new Response(
        JSON.stringify({
          id: "msg-2",
          channelId: "ch-1",
          userId: "u-1",
          content: "with file",
          replyCount: 0,
          reactions: [],
          attachments: [],
          createdAt: "2026-01-01T00:00:00Z",
        }),
        { status: 200, headers: { "Content-Type": "application/json" } },
      ),
    );

    const file = new File(["data"], "test.png", { type: "image/png" });
    await sendMessage("ch-1", "with file", undefined, [file]);

    expect(fetch).toHaveBeenCalledOnce();
    const [, init] = vi.mocked(fetch).mock.calls[0];
    expect(init?.body).toBeInstanceOf(FormData);
    // Content-Type should NOT be set (browser sets multipart boundary)
    expect(init?.headers).not.toHaveProperty("Content-Type");
  });

  it("includes threadId in FormData", async () => {
    vi.mocked(fetch).mockResolvedValue(
      new Response(
        JSON.stringify({
          id: "msg-3",
          channelId: "ch-1",
          userId: "u-1",
          content: "reply",
          threadId: "thread-1",
          replyCount: 0,
          reactions: [],
          attachments: [],
          createdAt: "2026-01-01T00:00:00Z",
        }),
        { status: 200, headers: { "Content-Type": "application/json" } },
      ),
    );

    const file = new File(["data"], "doc.pdf", { type: "application/pdf" });
    await sendMessage("ch-1", "reply", "thread-1", [file]);

    const [, init] = vi.mocked(fetch).mock.calls[0];
    const formData = init?.body as FormData;
    expect(formData).toBeInstanceOf(FormData);
    expect(formData.get("threadId")).toBe("thread-1");
    expect(formData.get("content")).toBe("reply");
  });

  it("does not use FormData for empty files array", async () => {
    vi.mocked(fetch).mockResolvedValue(
      new Response(
        JSON.stringify({
          id: "msg-4",
          channelId: "ch-1",
          userId: "u-1",
          content: "no files",
          replyCount: 0,
          reactions: [],
          attachments: [],
          createdAt: "2026-01-01T00:00:00Z",
        }),
        { status: 200, headers: { "Content-Type": "application/json" } },
      ),
    );

    await sendMessage("ch-1", "no files", undefined, []);

    const [, init] = vi.mocked(fetch).mock.calls[0];
    expect(init?.headers).toHaveProperty("Content-Type", "application/json");
    expect(JSON.parse(init?.body as string)).toEqual({ content: "no files" });
  });
});
