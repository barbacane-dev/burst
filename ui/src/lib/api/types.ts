export interface TokenResponse {
  access_token: string;
  token_type: string;
  expires_in: number;
}

export interface User {
  id: string;
  username: string;
  displayName: string;
  email?: string;
  avatarUrl?: string;
  role: "admin" | "integrator" | "moderator" | "member" | "guest";
  status: "online" | "away" | "offline" | "dnd";
  statusText?: string;
  statusEmoji?: string;
  /** RFC 3339. Once past, the status is no longer shown. */
  statusExpiresAt?: string;
  /** RFC 3339: when the user's current do-not-disturb period ends. Absent when not quiet. */
  doNotDisturbUntil?: string;
  isBot: boolean;
  createdAt: string;
}

export interface PaginatedResponse<T> {
  items: T[];
  cursor?: string;
}

export interface Channel {
  id: string;
  kind: "public" | "private" | "dm" | "group_dm";
  name?: string;
  slug?: string;
  topic?: string;
  description?: string;
  createdBy: string;
  isArchived: boolean;
  isReadonly: boolean;
  unreadCount: number;
  createdAt: string;
  updatedAt: string;
}

export interface ChannelMember {
  userId: string;
  role: "owner" | "moderator" | "member";
  joinedAt: string;
}

export interface ReactionCount {
  emoji: string;
  count: number;
  userIds: string[];
}

export interface Attachment {
  id: string;
  fileName: string;
  fileSize: number;
  contentType: string;
  metadata: Record<string, unknown>;
  createdAt: string;
}

export interface Message {
  id: string;
  channelId: string;
  userId: string;
  threadId?: string;
  content: string;
  editedAt?: string;
  deletedAt?: string;
  replyCount: number;
  reactions: ReactionCount[];
  attachments: Attachment[];
  createdAt: string;
}

/**
 * A notification addressed to the current user. The server decides who is
 * notified, from each member's channel preference and the message's mentions,
 * and sends this only to the recipient.
 */
export interface NotificationEvent {
  type: "notification.created";
  notificationId: string;
  recipientId: string;
  channelId: string;
  messageId: string;
  reason: "mention" | "message";
  authorName: string;
  /** Absent for direct messages, which have no name. */
  channelName?: string;
  preview: string;
}

export interface SearchResult {
  id: string;
  channelId: string;
  userId: string;
  threadId?: string;
  content: string;
  headline: string;
  createdAt: string;
  /** The attached file whose name matched, when one did. */
  matchedFile?: string;
}

export type Weekday = "mon" | "tue" | "wed" | "thu" | "fri" | "sat" | "sun";

export interface DoNotDisturbSchedule {
  /** HH:MM, local to `timeZone`. */
  start: string;
  end: string;
  days: Weekday[];
  /** IANA name. */
  timeZone: string;
}

export interface DoNotDisturb {
  snoozeUntil?: string;
  schedule?: DoNotDisturbSchedule;
  /** When the current quiet period ends; absent when not quiet. */
  quietUntil?: string;
}
