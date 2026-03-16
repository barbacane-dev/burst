import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { apiFetchFormData, ApiError, setAccessToken } from "../client";

describe("apiFetchFormData", () => {
  beforeEach(() => {
    vi.stubGlobal("fetch", vi.fn());
    sessionStorage.clear();
    setAccessToken(null);
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("sends POST with FormData body", async () => {
    vi.mocked(fetch).mockResolvedValue(
      new Response(JSON.stringify({ id: "1" }), {
        status: 200,
        headers: { "Content-Type": "application/json" },
      }),
    );

    const formData = new FormData();
    formData.append("content", "hello");
    await apiFetchFormData("/channels/ch-1/messages", formData);

    expect(fetch).toHaveBeenCalledOnce();
    const [url, init] = vi.mocked(fetch).mock.calls[0];
    expect(url).toBe("/api/channels/ch-1/messages");
    expect(init?.method).toBe("POST");
    expect(init?.body).toBe(formData);
  });

  it("does not set Content-Type header", async () => {
    vi.mocked(fetch).mockResolvedValue(
      new Response(JSON.stringify({ id: "1" }), {
        status: 200,
        headers: { "Content-Type": "application/json" },
      }),
    );

    const formData = new FormData();
    formData.append("content", "hello");
    await apiFetchFormData("/channels/ch-1/messages", formData);

    const [, init] = vi.mocked(fetch).mock.calls[0];
    const headers = init?.headers as Record<string, string>;
    expect(headers["Content-Type"]).toBeUndefined();
  });

  it("includes auth token when set", async () => {
    vi.mocked(fetch).mockResolvedValue(
      new Response(JSON.stringify({ id: "1" }), {
        status: 200,
        headers: { "Content-Type": "application/json" },
      }),
    );

    setAccessToken("my-jwt-token");

    const formData = new FormData();
    formData.append("content", "hello");
    await apiFetchFormData("/test", formData);

    const [, init] = vi.mocked(fetch).mock.calls[0];
    const headers = init?.headers as Record<string, string>;
    expect(headers["Authorization"]).toBe("Bearer my-jwt-token");
  });

  it("throws ApiError on non-ok response", async () => {
    const problem = {
      type: "urn:burst:error:validation",
      title: "Validation failed",
      status: 400,
      detail: "Content is required",
    };
    vi.mocked(fetch).mockResolvedValue(
      new Response(JSON.stringify(problem), {
        status: 400,
        headers: { "Content-Type": "application/json" },
      }),
    );

    const formData = new FormData();
    const error = await apiFetchFormData("/channels/ch-1/messages", formData).catch(
      (err: unknown) => err,
    );

    expect(error).toBeInstanceOf(ApiError);
    expect((error as ApiError).problem.status).toBe(400);
    expect((error as ApiError).problem.detail).toBe("Content is required");
  });
});
