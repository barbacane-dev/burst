import { useQuery } from "@tanstack/react-query";
import { listUsers } from "../api/users";
import { activeStatus, type ActiveStatus } from "../status";

/** The status `userId` shows now, read from the shared user list. */
export function useUserStatus(userId: string): ActiveStatus | undefined {
  const { data } = useQuery({
    queryKey: ["users"],
    queryFn: () => listUsers(),
    staleTime: 60_000,
    select: (list) => list.items.find((u) => u.id === userId),
  });
  return activeStatus(data);
}
