import { LogOut, MessageSquare } from "lucide-react";
import { useAuth } from "../../lib/auth/context";
import { Avatar } from "../ui/avatar";

export function Sidebar() {
  const { user, logout } = useAuth();

  return (
    <aside className="flex h-full w-64 flex-col border-r border-gray-200 bg-gray-50 dark:border-gray-700 dark:bg-gray-900">
      <div className="flex h-14 items-center border-b border-gray-200 px-4 dark:border-gray-700">
        <MessageSquare className="mr-2 h-5 w-5 text-indigo-600" />
        <h1 className="text-lg font-semibold text-gray-900 dark:text-gray-100">
          Burst
        </h1>
      </div>

      <nav className="flex-1 overflow-y-auto p-3" role="navigation">
        <h2 className="mb-2 px-2 text-xs font-semibold uppercase tracking-wider text-gray-500 dark:text-gray-400">
          Channels
        </h2>
        <p className="px-2 text-sm text-gray-400 dark:text-gray-500 italic">
          No channels yet
        </p>
      </nav>

      {user && (
        <div className="border-t border-gray-200 p-3 dark:border-gray-700">
          <div className="flex items-center gap-2">
            <Avatar
              name={user.displayName}
              src={user.avatarUrl}
              size="sm"
            />
            <div className="min-w-0 flex-1">
              <p className="truncate text-sm font-medium text-gray-900 dark:text-gray-100">
                {user.displayName}
              </p>
              <p className="truncate text-xs text-gray-500 dark:text-gray-400">
                {user.username}
              </p>
            </div>
            <button
              onClick={logout}
              className="rounded p-1 text-gray-400 hover:bg-gray-200 hover:text-gray-600 dark:hover:bg-gray-700 dark:hover:text-gray-300"
              title="Log out"
            >
              <LogOut className="h-4 w-4" />
            </button>
          </div>
        </div>
      )}
    </aside>
  );
}
