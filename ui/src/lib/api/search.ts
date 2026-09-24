import { apiFetch } from "./client";
import type { PaginatedResponse, SearchResult } from "./types";

export interface SearchOptions {
  channelId?: string;
  /** Only messages written by this user. */
  from?: string;
  /** RFC 3339; inclusive. */
  after?: string;
  /** RFC 3339; exclusive. */
  before?: string;
  /** Only messages carrying at least one file. */
  hasFile?: boolean;
  cursor?: string;
  limit?: number;
}

export async function searchMessages(
  q: string,
  options: SearchOptions = {},
): Promise<PaginatedResponse<SearchResult>> {
  const params = new URLSearchParams({ q, limit: String(options.limit ?? 50) });
  if (options.channelId) params.set("channelId", options.channelId);
  if (options.from) params.set("from", options.from);
  if (options.after) params.set("after", options.after);
  if (options.before) params.set("before", options.before);
  if (options.hasFile) params.set("hasFile", "true");
  if (options.cursor) params.set("cursor", options.cursor);
  return apiFetch<PaginatedResponse<SearchResult>>(`/search/messages?${params}`);
}
