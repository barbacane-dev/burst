import { apiFetch, apiFetchFormData, getAccessToken } from "./client";
import type { PaginatedResponse } from "./types";

export interface CustomEmoji {
  id: string;
  shortcode: string;
  imageUrl: string;
  createdBy: string;
  createdAt: string;
}

/**
 * Where an `<img>` loads a custom emoji from. An image request cannot send the
 * bearer header, so the token travels as `access_token`, as for attachments.
 */
export function emojiImageSrc(emoji: Pick<CustomEmoji, "imageUrl">): string {
  const token = getAccessToken();
  return token ? `${emoji.imageUrl}?access_token=${encodeURIComponent(token)}` : emoji.imageUrl;
}

export async function listEmojis(): Promise<PaginatedResponse<CustomEmoji>> {
  return apiFetch<PaginatedResponse<CustomEmoji>>("/emojis");
}

export async function listAdminEmojis(): Promise<PaginatedResponse<CustomEmoji>> {
  return apiFetch<PaginatedResponse<CustomEmoji>>("/admin/emojis");
}

export async function createEmoji(shortcode: string, image: File): Promise<CustomEmoji> {
  const formData = new FormData();
  formData.append("shortcode", shortcode);
  formData.append("image", image);
  return apiFetchFormData<CustomEmoji>("/admin/emojis", formData);
}

export async function deleteEmoji(emojiId: string): Promise<void> {
  await apiFetch<void>(`/admin/emojis/${emojiId}`, { method: "DELETE" });
}
