import { Hash } from "lucide-react";

export function ChannelHeader() {
  return (
    <header className="flex h-14 items-center border-b border-gray-200 px-4 dark:border-gray-700">
      <Hash className="mr-2 h-5 w-5 text-gray-400" />
      <h2 className="text-base font-semibold text-gray-900 dark:text-gray-100">
        general
      </h2>
    </header>
  );
}
