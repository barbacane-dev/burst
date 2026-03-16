import { Outlet } from "react-router-dom";
import { Sidebar } from "./sidebar";
import { MessageSquare } from "lucide-react";
import { useWsEvent } from "../../lib/ws/hooks";
import { showBrowserNotification } from "../../lib/notifications";
import type { Message } from "../../lib/api/types";

export function MainLayout() {
  useWsEvent<{ type: string; channelId: string; message: Message }>(
    "message.created",
    (ev) => {
      showBrowserNotification("New message", ev.message.content);
    },
  );

  return (
    <div className="flex h-screen bg-white dark:bg-gray-950">
      <a
        href="#main-content"
        className="sr-only focus:not-sr-only focus:absolute focus:z-50 focus:rounded-md focus:bg-indigo-600 focus:px-4 focus:py-2 focus:text-white"
      >
        Skip to content
      </a>
      <Sidebar />
      <main id="main-content" className="flex flex-1 flex-col">
        <Outlet />
      </main>
    </div>
  );
}

export function WelcomeView() {
  return (
    <div className="flex flex-1 items-center justify-center">
      <div className="text-center text-gray-400 dark:text-gray-500">
        <MessageSquare className="mx-auto mb-3 h-12 w-12" />
        <p className="text-lg font-medium">Welcome to Burst</p>
        <p className="text-sm">
          Select a channel or create one to get started.
        </p>
      </div>
    </div>
  );
}
