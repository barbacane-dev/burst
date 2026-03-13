import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useState,
  type ReactNode,
} from "react";
import { apiFetch, setAccessToken, getAccessToken } from "../api/client";
import type { TokenResponse, User } from "../api/types";
import { queryClient } from "../query-client";
import { wsClient } from "../ws/client";

interface AuthState {
  user: User | null;
  isLoading: boolean;
  login: (email: string, password: string) => Promise<void>;
  logout: () => void;
}

const AuthContext = createContext<AuthState | null>(null);

export function AuthProvider({ children }: { children: ReactNode }) {
  const [user, setUser] = useState<User | null>(null);
  const [isLoading, setIsLoading] = useState(true);

  const fetchMe = useCallback(async () => {
    const me = await apiFetch<User>("/users/me");
    setUser(me);
  }, []);

  // On mount: if a token exists in sessionStorage (set by setAccessToken),
  // try to restore the session by fetching the current user.
  useEffect(() => {
    const token = getAccessToken();
    if (!token) {
      setIsLoading(false);
      return;
    }

    fetchMe()
      .then(() => {
        wsClient.connect();
      })
      .catch(() => {
        setAccessToken(null);
        setUser(null);
      })
      .finally(() => setIsLoading(false));

    const stopHeartbeat = wsClient.startHeartbeat();
    return () => {
      stopHeartbeat();
    };
  }, [fetchMe]);

  const login = useCallback(
    async (email: string, password: string) => {
      // Exchange credentials for a JWT via the mock OIDC server's password grant.
      const params = new URLSearchParams({
        grant_type: "password",
        username: email,
        password,
        client_id: "burst",
        client_secret: "secret",
        scope: "openid",
      });

      const res = await fetch("/oauth/burst/token", {
        method: "POST",
        headers: { "Content-Type": "application/x-www-form-urlencoded" },
        body: params,
      });

      if (!res.ok) {
        throw new Error("Authentication failed");
      }

      const data: TokenResponse = await res.json();
      setAccessToken(data.access_token);
      wsClient.connect();
      await fetchMe();
    },
    [fetchMe],
  );

  const logout = useCallback(() => {
    wsClient.disconnect();
    wsClient.reset();
    queryClient.clear();
    setAccessToken(null);
    setUser(null);
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
