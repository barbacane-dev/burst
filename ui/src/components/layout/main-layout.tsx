import { Outlet, useNavigate } from "react-router-dom";
import { Sidebar } from "./sidebar";
import { MessageSquare } from "lucide-react";
import { useQueryClient } from "@tanstack/react-query";
import { useWsEvent } from "../../lib/ws/hooks";
import { showBrowserNotification } from "../../lib/notifications";
import type { Message, NotificationEvent } from "../../lib/api/types";

export function MainLayout() {
  const queryClient = useQueryClient();
  const navigate = useNavigate();

  // Keeps unread counts current. It does not notify: every member receives
  // this event, including the author, whatever their channel preference.
  useWsEvent<{ type: string; channelId: string; message: Message }>(
    "message.created",
    () => {
      queryClient.invalidateQueries({ queryKey: ["channels"] });
    },
  );

  // The server sends this only to the member it has decided to notify.
  useWsEvent<NotificationEvent>("notification.created", (ev) => {
    showBrowserNotification(notificationTitle(ev), ev.preview || "sent a file", {
      tag: ev.channelId,
      onClick: () => navigate(`/channels/${ev.channelId}`),
    });
  });

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

/** "Alice in #general", or just "Alice" for a direct message. */
function notificationTitle(ev: NotificationEvent): string {
  return ev.channelName ? `${ev.authorName} in #${ev.channelName}` : ev.authorName;
}
