import { apiFetch } from "./client";
import type { PaginatedResponse, SearchResult } from "./types";

export async function searchMessages(
  q: string,
  channelId?: string,
  cursor?: string,
  limit = 50,
): Promise<PaginatedResponse<SearchResult>> {
  const params = new URLSearchParams({ q, limit: String(limit) });
  if (channelId) params.set("channelId", channelId);
  if (cursor) params.set("cursor", cursor);
  return apiFetch<PaginatedResponse<SearchResult>>(
    `/search/messages?${params}`,
  );
}
