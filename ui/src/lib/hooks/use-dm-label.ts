import type { ChannelMember } from "../api/types";

/**
 * Resolves a DM label from the channel member list.
 *
 * For 1-to-1 DMs: returns the other participant's display name.
 * For group DMs: returns comma-separated names ("Alice, Bob, Charlie").
 *
 * Used by both Sidebar (per DM channel) and ChannelPage (for the header title).
 */
export function resolveDmPartnerName(
  members: ChannelMember[] | undefined,
  currentUserId: string | undefined,
  usersById: Map<string, string>,
): string {
  const others = members?.filter((m) => m.userId !== currentUserId) ?? [];
  if (others.length === 0) return "Direct Message";

  const names = others.map(
    (m) => usersById.get(m.userId) ?? m.userId.replace("usr_", "").slice(0, 8),
  );

  if (names.length <= 3) return names.join(", ");
  return `${names.slice(0, 3).join(", ")} +${names.length - 3}`;
}
