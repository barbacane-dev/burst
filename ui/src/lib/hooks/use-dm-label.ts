import type { ChannelMember } from "../api/types";

/**
 * Resolves a DM partner's display name from the channel member list.
 *
 * Used by both Sidebar (per DM channel) and ChannelPage (for the header title).
 */
export function resolveDmPartnerName(
  members: ChannelMember[] | undefined,
  currentUserId: string | undefined,
  usersById: Map<string, string>,
): string {
  const partner = members?.find((m) => m.userId !== currentUserId);
  if (!partner) return "Direct Message";
  return (
    usersById.get(partner.userId) ??
    partner.userId.replace("usr_", "").slice(0, 8)
  );
}
