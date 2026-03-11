import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useState,
  type ReactNode,
} from "react";
import { apiFetch, setAccessToken, silentRefresh } from "../api/client";
import type { TokenResponse, User } from "../api/types";
import { queryClient } from "../query-client";
import { wsClient } from "../ws/client";

interface AuthState {
  user: User | null;
  isLoading: boolean;
  login: (email: string, password: string) => Promise<void>;
  logout: () => Promise<void>;
}

const AuthContext = createContext<AuthState | null>(null);

export function AuthProvider({ children }: { children: ReactNode }) {
  const [user, setUser] = useState<User | null>(null);
  const [isLoading, setIsLoading] = useState(true);

  const fetchMe = useCallback(async () => {
    try {
      const me = await apiFetch<User>("/users/me");
      setUser(me);
    } catch {
      setAccessToken(null);
      setUser(null);
    }
  }, []);

  // On mount: try to restore the session via the httpOnly refresh cookie.
  // No localStorage — the cookie is sent automatically by the browser.
  useEffect(() => {
    silentRefresh()
      .then((ok) => {
        if (ok) {
          wsClient.connect();
          return fetchMe();
        }
      })
      .finally(() => setIsLoading(false));

    const stopHeartbeat = wsClient.startHeartbeat();
    return () => {
      stopHeartbeat();
    };
  }, [fetchMe]);

  const login = useCallback(
    async (email: string, password: string) => {
      const data = await apiFetch<TokenResponse>("/auth/login", {
        method: "POST",
        body: JSON.stringify({ email, password }),
      });
      setAccessToken(data.accessToken);
      wsClient.connect();
      await fetchMe();
    },
    [fetchMe],
  );

  const logout = useCallback(async () => {
    try {
      await apiFetch("/auth/logout", { method: "POST" });
    } finally {
      wsClient.disconnect();
      wsClient.reset();
      queryClient.clear();
      setAccessToken(null);
      setUser(null);
    }
  }, []);

  return (
    <AuthContext.Provider value={{ user, isLoading, login, logout }}>
      {children}
    </AuthContext.Provider>
  );
}

export function useAuth() {
  const ctx = useContext(AuthContext);
  if (!ctx) throw new Error("useAuth must be used within AuthProvider");
  return ctx;
}
