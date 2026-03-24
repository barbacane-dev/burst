import { useEffect, useRef, useState } from "react";
import { wsClient } from "./client";

/**
 * Subscribe to a WS event type with a stable-ref handler.
 * The handler is called on every matching event but never causes re-subscription.
 */
export function useWsEvent<T>(type: string, handler: (event: T) => void): void {
  const ref = useRef(handler);
  useEffect(() => {
    ref.current = handler;
  });

  useEffect(() => {
    return wsClient.on(type, (e) => ref.current(e as T));
  }, [type]);
}

/**
 * Track which users are currently typing in a channel.
 * Sends typing.start/stop to the server and returns a set of user IDs typing.
 */
export function useTypingIndicator(channelId: string, currentUserId?: string): {
  typingUsers: Set<string>;
  sendTypingStart: () => void;
  sendTypingStop: () => void;
} {
  const [typingUsers, setTypingUsers] = useState<Set<string>>(new Set());
  const timers = useRef(new Map<string, ReturnType<typeof setTimeout>>());

  useEffect(() => {
    const timerMap = timers.current;
    const offStart = wsClient.on("typing.start", (e) => {
      const ev = e as { type: string; channelId: string; userId: string };
      if (ev.channelId !== channelId) return;
      if (currentUserId && ev.userId === currentUserId) return;

      setTypingUsers((prev) => {
        const next = new Set(prev);
        next.add(ev.userId);
        return next;
      });

      // Auto-clear after 5s in case stop is missed
      const existing = timerMap.get(ev.userId);
      if (existing) clearTimeout(existing);
      timerMap.set(
        ev.userId,
        setTimeout(() => {
          setTypingUsers((prev) => {
            const next = new Set(prev);
            next.delete(ev.userId);
            return next;
          });
          timerMap.delete(ev.userId);
        }, 5_000),
      );
    });

    const offStop = wsClient.on("typing.stop", (e) => {
      const ev = e as { type: string; channelId: string; userId: string };
      if (ev.channelId !== channelId) return;

      const existing = timerMap.get(ev.userId);
      if (existing) {
        clearTimeout(existing);
        timerMap.delete(ev.userId);
      }
      setTypingUsers((prev) => {
        const next = new Set(prev);
        next.delete(ev.userId);
        return next;
      });
    });

    return () => {
      offStart();
      offStop();
      timerMap.forEach(clearTimeout);
      timerMap.clear();
    };
  }, [channelId, currentUserId]);

  const sendTypingStart = () => wsClient.send({ type: "typing.start", channelId });
  const sendTypingStop = () => wsClient.send({ type: "typing.stop", channelId });

  return { typingUsers, sendTypingStart, sendTypingStop };
}
