import { createContext } from "react";
import type { User } from "../api/types";

export interface AuthState {
  user: User | null;
  isLoading: boolean;
  login: (username: string, password: string) => Promise<void>;
  loginWithToken: (token: string) => Promise<void>;
  logout: () => void;
  /** Replaces the signed-in user, as returned by a profile or status update. */
  updateUser: (user: User) => void;
}

export const AuthContext = createContext<AuthState | null>(null);
