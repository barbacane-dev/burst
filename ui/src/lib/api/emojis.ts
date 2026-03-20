import { apiFetch, apiFetchFormData } from "./client";
import type { PaginatedResponse } from "./types";

export interface CustomEmoji {
  id: string;
  shortcode: string;
  imageUrl: string;
  createdBy: string;
  createdAt: string;
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
