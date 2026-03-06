import { Outlet } from "react-router-dom";
import { Sidebar } from "./sidebar";
import { MessageSquare } from "lucide-react";

export function MainLayout() {
  return (
    <div className="flex h-screen bg-white dark:bg-gray-950">
      <Sidebar />
      <main className="flex flex-1 flex-col">
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
