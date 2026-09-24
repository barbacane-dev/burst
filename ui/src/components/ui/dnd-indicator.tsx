import { BellOff } from "lucide-react";
import { useQuery } from "@tanstack/react-query";
import { listUsers } from "../../lib/api/users";
import { quietUntil } from "../../lib/dnd";
import type { User } from "../../lib/api/types";

function label(until: Date): string {
  return `Do not disturb until ${until.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}`;
}

/** A bell-off mark for a user who is quiet now. Pass `user` when it is at hand. */
export function DndIndicator({ userId, user }: { userId?: string; user?: User }) {
  const { data } = useQuery({
    queryKey: ["users"],
    queryFn: () => listUsers(),
    staleTime: 60_000,
    enabled: !user,
    select: (list) => list.items.find((u) => u.id === userId),
  });
  const until = quietUntil(user ?? data);
  if (!until) return null;
  return (
    <span role="img" aria-label={label(until)} title={label(until)} className="inline-flex text-gray-400">
      <BellOff className="h-3.5 w-3.5" />
    </span>
  );
}
