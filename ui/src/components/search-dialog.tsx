import { useEffect, useRef, useState } from "react";
import { useNavigate } from "react-router-dom";
import { Paperclip, Search, X } from "lucide-react";
import { useInfiniteQuery } from "@tanstack/react-query";
import { searchMessages } from "../lib/api/search";
import { searchWindow } from "../lib/search-dates";
import { Spinner } from "./ui/spinner";
import { useUsersById } from "../lib/hooks/use-users-by-id";
import type { SearchResult } from "../lib/api/types";

export function SearchDialog({ onClose }: { onClose: () => void }) {
  const navigate = useNavigate();
  const inputRef = useRef<HTMLInputElement>(null);
  const [query, setQuery] = useState("");
  const [debouncedQuery, setDebouncedQuery] = useState("");
  const [from, setFrom] = useState("");
  const [fromDay, setFromDay] = useState("");
  const [toDay, setToDay] = useState("");
  const [hasFile, setHasFile] = useState(false);

  // Debounce the search query by 300ms.
  useEffect(() => {
    const timer = setTimeout(() => setDebouncedQuery(query.trim()), 300);
    return () => clearTimeout(timer);
  }, [query]);

  // Auto-focus the input on mount.
  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  // Close on Escape.
  useEffect(() => {
    function handleKeyDown(e: KeyboardEvent) {
      if (e.key === "Escape") onClose();
    }
    document.addEventListener("keydown", handleKeyDown);
    return () => document.removeEventListener("keydown", handleKeyDown);
  }, [onClose]);

  const filters = { from: from || undefined, ...searchWindow(fromDay, toDay), hasFile };
  const backwards = fromDay !== "" && toDay !== "" && fromDay > toDay;

  const { data, isLoading, hasNextPage, fetchNextPage, isFetchingNextPage } = useInfiniteQuery({
    queryKey: ["search", debouncedQuery, filters],
    queryFn: ({ pageParam }) =>
      searchMessages(debouncedQuery, { ...filters, cursor: pageParam, limit: 20 }),
    initialPageParam: undefined as string | undefined,
    getNextPageParam: (last) => last.cursor ?? undefined,
    enabled: debouncedQuery.length > 0 && !backwards,
  });

  const { usersById, users } = useUsersById();

  function handleResultClick(result: SearchResult) {
    navigate(`/channels/${result.channelId}`);
    onClose();
  }

  const items = data?.pages.flatMap((page) => page.items) ?? [];
  const filterClass =
    "rounded border border-gray-300 bg-transparent px-1.5 py-0.5 text-xs text-gray-700 dark:border-gray-600 dark:text-gray-300";

  return (
    <div
      className="fixed inset-0 z-50 flex items-start justify-center bg-black/50 pt-[15vh]"
      onClick={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <div className="w-full max-w-lg rounded-lg bg-white shadow-2xl dark:bg-gray-800">
        {/* Search input */}
        <div className="flex items-center gap-3 border-b border-gray-200 px-4 py-3 dark:border-gray-700">
          <Search className="h-5 w-5 shrink-0 text-gray-400" />
          <input
            ref={inputRef}
            type="text"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Search messages..."
            className="flex-1 bg-transparent text-sm text-gray-900 outline-none placeholder:text-gray-400 dark:text-gray-100 dark:placeholder:text-gray-500"
          />
          {query && (
            <button
              onClick={() => setQuery("")}
              className="rounded p-0.5 text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
            >
              <X className="h-4 w-4" />
            </button>
          )}
        </div>

        {/* Filters */}
        <div className="flex flex-wrap items-center gap-2 border-b border-gray-200 px-4 py-2 text-xs text-gray-500 dark:border-gray-700 dark:text-gray-400">
          <select
            aria-label="Author"
            value={from}
            onChange={(e) => setFrom(e.target.value)}
            className={filterClass}
          >
            <option value="">Anyone</option>
            {users.map((u) => (
              <option key={u.id} value={u.id}>
                {u.displayName}
              </option>
            ))}
          </select>
          <label className="flex items-center gap-1">
            Sent
            <input
              type="date"
              aria-label="On or after"
              value={fromDay}
              onChange={(e) => setFromDay(e.target.value)}
              className={filterClass}
            />
          </label>
          <label className="flex items-center gap-1">
            to
            <input
              type="date"
              aria-label="On or before"
              value={toDay}
              onChange={(e) => setToDay(e.target.value)}
              className={filterClass}
            />
          </label>
          <label className="flex items-center gap-1">
            <input
              type="checkbox"
              checked={hasFile}
              onChange={(e) => setHasFile(e.target.checked)}
            />
            Has file
          </label>
        </div>

        {/* Results */}
        <div className="max-h-80 overflow-y-auto">
          {backwards ? (
            <p className="px-4 py-6 text-center text-sm text-gray-400 dark:text-gray-500">
              The start date is after the end date
            </p>
          ) : debouncedQuery.length === 0 ? (
            <p className="px-4 py-6 text-center text-sm text-gray-400 dark:text-gray-500">
              Type to search messages across all your channels
            </p>
          ) : isLoading ? (
            <div className="flex justify-center py-6">
              <Spinner />
            </div>
          ) : items.length === 0 ? (
            <p className="px-4 py-6 text-center text-sm text-gray-400 dark:text-gray-500">
              No results found for &ldquo;{debouncedQuery}&rdquo;
            </p>
          ) : (
            <ul>
              {items.map((result) => {
                const authorName = usersById.get(result.userId);
                return (
                  <li key={result.id}>
                    <button
                      onClick={() => handleResultClick(result)}
                      className="w-full px-4 py-3 text-left hover:bg-gray-50 dark:hover:bg-gray-700/50"
                    >
                      <div className="mb-1 flex items-center gap-2 text-xs text-gray-500 dark:text-gray-400">
                        <span className="font-medium">
                          {authorName ?? "Unknown"}
                        </span>
                        <span>&middot;</span>
                        <time>
                          {new Date(result.createdAt).toLocaleDateString(
                            undefined,
                            { month: "short", day: "numeric", year: "numeric" },
                          )}
                        </time>
                      </div>
                      <p
                        className="text-sm text-gray-700 dark:text-gray-300 [&_mark]:rounded [&_mark]:bg-yellow-200/60 [&_mark]:px-0.5 dark:[&_mark]:bg-yellow-500/30"
                        dangerouslySetInnerHTML={{ __html: result.headline }}
                      />
                      {result.matchedFile && (
                        <p className="mt-1 flex items-center gap-1 text-xs text-gray-500 dark:text-gray-400">
                          <Paperclip className="h-3 w-3 shrink-0" />
                          <span className="truncate">{result.matchedFile}</span>
                        </p>
                      )}
                    </button>
                  </li>
                );
              })}
              {hasNextPage && (
                <li className="px-4 py-2 text-center">
                  <button
                    onClick={() => fetchNextPage()}
                    disabled={isFetchingNextPage}
                    className="text-xs text-blue-600 hover:underline disabled:opacity-50 dark:text-blue-400"
                  >
                    {isFetchingNextPage ? "Loading..." : "Show more results"}
                  </button>
                </li>
              )}
            </ul>
          )}
        </div>

        {/* Footer hint */}
        <div className="border-t border-gray-200 px-4 py-2 dark:border-gray-700">
          <p className="text-xs text-gray-400 dark:text-gray-500">
            <kbd className="rounded border border-gray-300 px-1 py-0.5 text-[10px] dark:border-gray-600">
              Esc
            </kbd>{" "}
            to close
          </p>
        </div>
      </div>
    </div>
  );
}
