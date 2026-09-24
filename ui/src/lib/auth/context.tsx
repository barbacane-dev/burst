import { useCallback, useEffect, useState, type ReactNode } from "react";
import { apiFetch, setAccessToken, getAccessToken } from "../api/client";
import type { TokenResponse, User } from "../api/types";
import { queryClient } from "../query-client";
import { wsClient } from "../ws/client";
import { AuthContext } from "./auth-context";
import { withStatus, type StatusChangedEvent } from "../status";
import { withDnd, type DndChangedEvent } from "../dnd";

export { AuthContext } from "./auth-context";

export function AuthProvider({ children }: { children: ReactNode }) {
  const [user, setUser] = useState<User | null>(null);
  const [isLoading, setIsLoading] = useState(() => Boolean(getAccessToken()));

  const fetchMe = useCallback(async () => {
    const me = await apiFetch<User>("/users/me");
    setUser(me);
  }, []);

  // On mount: if a token exists in sessionStorage (set by setAccessToken),
  // try to restore the session by fetching the current user.
  const hasToken = Boolean(getAccessToken());
  useEffect(() => {
    if (!hasToken) {
      return;
    }

    let stopHeartbeat: (() => void) | null = null;
    let cancelled = false;

    // eslint-disable-next-line react-hooks/set-state-in-effect -- setState is in async callbacks, not synchronous
    fetchMe()
      .then(() => {
        if (!cancelled) {
          wsClient.connect();
          stopHeartbeat = wsClient.startHeartbeat();
        }
      })
      .catch(() => {
        if (!cancelled) {
          setAccessToken(null);
          setUser(null);
        }
      })
      .finally(() => {
        if (!cancelled) setIsLoading(false);
      });

    return () => {
      cancelled = true;
      stopHeartbeat?.();
    };
  }, [fetchMe, hasToken]);

  // Keeps the signed-in user's own status current when it is changed elsewhere.
  useEffect(
    () =>
      wsClient.on("user.status_changed", (e) => {
        const event = e as StatusChangedEvent;
        setUser((u) => (u && u.id === event.userId ? withStatus(u, event) : u));
      }),
    [],
  );

  // Keeps the signed-in user's own do-not-disturb current when it is changed elsewhere.
  useEffect(
    () =>
      wsClient.on("user.dnd_changed", (e) => {
        const event = e as DndChangedEvent;
        setUser((u) => (u && u.id === event.userId ? withDnd(u, event) : u));
      }),
    [],
  );

  const login = useCallback(
    async (username: string, password: string) => {
      const params = new URLSearchParams({
        grant_type: "password",
        username,
        password,
        client_id: "burst",
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

  const loginWithToken = useCallback(
    async (token: string) => {
      setAccessToken(token);
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
    <AuthContext.Provider
      value={{ user, isLoading, login, loginWithToken, logout, updateUser: setUser }}
    >
      {children}
    </AuthContext.Provider>
  );
}
