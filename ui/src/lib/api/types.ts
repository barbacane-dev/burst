export interface TokenResponse {
  accessToken: string;
  refreshToken: string;
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
