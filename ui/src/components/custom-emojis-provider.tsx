import { useMemo, type ReactNode } from "react";
import { useQuery } from "@tanstack/react-query";
import { listEmojis, type CustomEmoji } from "../lib/api/emojis";
import type { PaginatedResponse } from "../lib/api/types";
import { useAuth } from "../lib/auth/use-auth";
import { CustomEmojisContext } from "../lib/custom-emojis";

/** Loads the custom emojis once signed in, for every place that shows one. */
export function CustomEmojisProvider({ children }: { children: ReactNode }) {
  const { user } = useAuth();
  const { data } = useQuery<PaginatedResponse<CustomEmoji>>({
    queryKey: ["emojis"],
    queryFn: () => listEmojis(),
    staleTime: 60_000,
    enabled: Boolean(user),
  });
  const emojis = useMemo(
    () => new Map((data?.items ?? []).map((emoji) => [emoji.shortcode, emoji])),
    [data],
  );
  return <CustomEmojisContext.Provider value={emojis}>{children}</CustomEmojisContext.Provider>;
}
