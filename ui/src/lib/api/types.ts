export interface TokenResponse {
  accessToken: string;
  tokenType: string;
  expiresIn: number;
}

export interface User {
  id: string;
  username: string;
  displayName: string;
  email?: string;
  avatarUrl?: string;
  role: "admin" | "moderator" | "member" | "guest";
  status: "online" | "away" | "offline" | "dnd";
  statusText?: string;
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
  createdAt: string;
  updatedAt: string;
}

export interface Message {
  id: string;
  channelId: string;
  userId: string;
  threadId?: string;
  content: string;
  editedAt?: string;
  deletedAt?: string;
  createdAt: string;
}
