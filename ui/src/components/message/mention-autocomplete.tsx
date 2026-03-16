import { useEffect, useRef } from "react";
import { Avatar } from "../ui/avatar";
import type { User } from "../../lib/api/types";

export function MentionAutocomplete({
  query,
  users,
  onSelect,
  onClose,
}: {
  query: string;
  users: User[];
  onSelect: (username: string) => void;
  onClose: () => void;
}) {
  const ref = useRef<HTMLDivElement>(null);

  const filtered = users.filter(
    (u) =>
      u.username.toLowerCase().includes(query.toLowerCase()) ||
      u.displayName.toLowerCase().includes(query.toLowerCase()),
  ).slice(0, 8);

  useEffect(() => {
    function handleClick(e: MouseEvent) {
      if (ref.current && !ref.current.contains(e.target as Node)) {
        onClose();
      }
    }
    document.addEventListener("mousedown", handleClick);
    return () => document.removeEventListener("mousedown", handleClick);
  }, [onClose]);

  if (filtered.length === 0) return null;

  return (
    <div
      ref={ref}
      className="absolute bottom-full left-0 mb-1 w-64 rounded-md border border-gray-200 bg-white shadow-lg dark:border-gray-700 dark:bg-gray-800"
      role="listbox"
      aria-label="Mention suggestions"
    >
      {filtered.map((u) => (
        <button
          key={u.id}
          role="option"
          onClick={() => onSelect(u.username)}
          className="flex w-full items-center gap-2 px-3 py-2 text-left text-sm hover:bg-gray-100 dark:hover:bg-gray-700"
        >
          <Avatar name={u.displayName} src={u.avatarUrl} size="sm" />
          <div className="min-w-0">
            <p className="truncate font-medium text-gray-900 dark:text-gray-100">
              {u.displayName}
            </p>
            <p className="truncate text-xs text-gray-500">@{u.username}</p>
          </div>
        </button>
      ))}
    </div>
  );
}
