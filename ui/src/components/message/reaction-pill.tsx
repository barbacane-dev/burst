import type { ReactionCount } from "../../lib/api/types";

export function ReactionPill({
  reaction,
  currentUserId,
  onClick,
}: {
  reaction: ReactionCount;
  currentUserId: string;
  onClick: () => void;
}) {
  const hasReacted = reaction.userIds.includes(currentUserId);
  return (
    <button
      onClick={onClick}
      className={`flex items-center gap-1 rounded-full border px-2 py-0.5 text-xs transition-colors ${
        hasReacted
          ? "border-indigo-400 bg-indigo-50 text-indigo-700 dark:border-indigo-600 dark:bg-indigo-900/30 dark:text-indigo-300"
          : "border-gray-200 bg-gray-50 text-gray-700 hover:border-gray-300 dark:border-gray-700 dark:bg-gray-800 dark:text-gray-300"
      }`}
    >
      <span>{reaction.emoji}</span>
      <span>{reaction.count}</span>
    </button>
  );
}
