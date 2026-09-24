import { useUserStatus } from "../../lib/hooks/use-user-status";

/** A user's status emoji, with the text on hover. A text-only status shows 💬. */
export function UserStatusEmoji({ userId }: { userId: string }) {
  const status = useUserStatus(userId);
  if (!status) return null;
  const label = status.text ?? status.emoji;
  return (
    <span role="img" aria-label={`Status: ${label}`} title={status.text} className="text-sm leading-none">
      {status.emoji ?? "💬"}
    </span>
  );
}

/** A user's status as a line: emoji and text. */
export function UserStatusLine({ userId }: { userId: string }) {
  const status = useUserStatus(userId);
  if (!status) return null;
  return (
    <span className="block truncate text-xs text-gray-500 dark:text-gray-400" title={status.text}>
      {status.emoji && <span className="mr-1">{status.emoji}</span>}
      {status.text}
    </span>
  );
}
