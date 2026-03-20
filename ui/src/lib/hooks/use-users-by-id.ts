import { useMemo } from "react";
import { useQuery } from "@tanstack/react-query";
import { listUsers } from "../api/users";
import type { PaginatedResponse, User } from "../api/types";

/**
 * Shared hook that fetches the user list and returns a display-name lookup map.
 *
 * Eliminates the duplicated `useQuery(["users"]) + new Map(...)` pattern
 * that was copy-pasted across Sidebar, ChannelPage, and SearchDialog.
 */
export function useUsersById(): { usersById: Map<string, string>; users: User[] } {
  const { data } = useQuery<PaginatedResponse<User>>({
    queryKey: ["users"],
    queryFn: () => listUsers(),
    staleTime: 60_000,
  });

  const users = data?.items ?? [];

  const usersById = useMemo(
    () => new Map(users.map((u) => [u.id, u.displayName])),
    [users],
  );

  return { usersById, users };
}
