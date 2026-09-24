import { useState, type FormEvent } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useAuth } from "../lib/auth/use-auth";
import { useTheme } from "../lib/use-theme";
import { clearMyStatus, setMyStatus, updateMe } from "../lib/api/users";
import {
  activeStatus,
  applyToUserList,
  CLEAR_AFTER_LABELS,
  expiryFor,
  type ClearAfter,
} from "../lib/status";
import { Avatar } from "../components/ui/avatar";
import { Button } from "../components/ui/button";
import { Input } from "../components/ui/input";
import type { PaginatedResponse, User } from "../lib/api/types";

export function SettingsPage() {
  const { user } = useAuth();
  const { theme, setTheme } = useTheme();

  if (!user) return null;

  return (
    <div className="flex-1 overflow-y-auto">
      <div className="mx-auto max-w-2xl p-8 space-y-8">
        <h1 className="text-2xl font-bold text-gray-900 dark:text-gray-100">Settings</h1>

        <ProfileSection user={user} />
        <StatusSection user={user} />
        <AppearanceSection theme={theme} setTheme={setTheme} />
        <NotificationSection />
      </div>
    </div>
  );
}

function ProfileSection({ user }: { user: User }) {
  const { logout } = useAuth();
  const queryClient = useQueryClient();
  const [displayName, setDisplayName] = useState(user.displayName);
  const [email, setEmail] = useState(user.email ?? "");
  const [saved, setSaved] = useState(false);

  const mutation = useMutation({
    mutationFn: () =>
      updateMe({
        displayName: displayName !== user.displayName ? displayName : undefined,
        email: email !== (user.email ?? "") ? email || undefined : undefined,
      }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["users"] });
      setSaved(true);
      setTimeout(() => setSaved(false), 2000);
    },
  });

  function handleSubmit(e: FormEvent) {
    e.preventDefault();
    mutation.mutate();
  }

  return (
    <section>
      <h2 className="text-lg font-semibold text-gray-900 dark:text-gray-100 mb-4">Profile</h2>
      <div className="rounded-lg border border-gray-200 bg-white p-6 dark:border-gray-700 dark:bg-gray-900">
        <div className="mb-4 flex items-center gap-4">
          <Avatar name={user.displayName} src={user.avatarUrl} size="lg" />
          <div>
            <p className="font-medium text-gray-900 dark:text-gray-100">@{user.username}</p>
            <p className="text-sm text-gray-500 dark:text-gray-400 capitalize">{user.role}</p>
          </div>
        </div>

        <form onSubmit={handleSubmit} className="space-y-4">
          <Input
            id="display-name"
            label="Display name"
            value={displayName}
            onChange={(e) => setDisplayName(e.target.value)}
          />
          <Input
            id="email"
            label="Email"
            type="email"
            value={email}
            onChange={(e) => setEmail(e.target.value)}
          />
          <div className="flex items-center gap-3">
            <Button type="submit" disabled={mutation.isPending}>
              {mutation.isPending ? "Saving..." : "Save changes"}
            </Button>
            {saved && <span className="text-sm text-green-600 dark:text-green-400">Saved</span>}
            {mutation.isError && (
              <span className="text-sm text-red-600 dark:text-red-400">
                {mutation.error.message}
              </span>
            )}
          </div>
        </form>

        <div className="mt-6 border-t border-gray-200 pt-4 dark:border-gray-700">
          <Button variant="secondary" onClick={logout}>
            Log out
          </Button>
        </div>
      </div>
    </section>
  );
}

const PRESETS: { emoji: string; text: string; clearAfter: ClearAfter }[] = [
  { emoji: "📅", text: "In a meeting", clearAfter: "1h" },
  { emoji: "🚆", text: "Commuting", clearAfter: "30m" },
  { emoji: "🏠", text: "Working remotely", clearAfter: "today" },
  { emoji: "🤒", text: "Out sick", clearAfter: "today" },
  { emoji: "🌴", text: "On vacation", clearAfter: "never" },
];

function StatusSection({ user }: { user: User }) {
  const { updateUser } = useAuth();
  const queryClient = useQueryClient();
  const current = activeStatus(user);
  const [emoji, setEmoji] = useState(current?.emoji ?? "");
  const [text, setText] = useState(current?.text ?? "");
  const [clearAfter, setClearAfter] = useState<ClearAfter>("never");

  function applied(updated: User) {
    updateUser(updated);
    queryClient.setQueryData<PaginatedResponse<User>>(["users"], (list) =>
      applyToUserList(list, {
        userId: updated.id,
        text: updated.statusText,
        emoji: updated.statusEmoji,
        expiresAt: updated.statusExpiresAt,
      }),
    );
  }

  const save = useMutation({
    mutationFn: () =>
      setMyStatus({
        text: text.trim() || undefined,
        emoji: emoji.trim() || undefined,
        expiresAt: expiryFor(clearAfter),
      }),
    onSuccess: applied,
  });

  const clear = useMutation({
    mutationFn: clearMyStatus,
    onSuccess: (updated) => {
      applied(updated);
      setEmoji("");
      setText("");
      setClearAfter("never");
    },
  });

  const blank = !text.trim() && !emoji.trim();
  const error = save.error ?? clear.error;

  function handleSubmit(e: FormEvent) {
    e.preventDefault();
    if (!blank) save.mutate();
  }

  return (
    <section>
      <h2 className="text-lg font-semibold text-gray-900 dark:text-gray-100 mb-4">Status</h2>
      <div className="rounded-lg border border-gray-200 bg-white p-6 dark:border-gray-700 dark:bg-gray-900">
        <p className="mb-3 text-sm text-gray-500 dark:text-gray-400">
          {current ? (
            <>
              Now showing: {current.emoji} {current.text}
              {user.statusExpiresAt && (
                <> until {new Date(user.statusExpiresAt).toLocaleString()}</>
              )}
            </>
          ) : (
            "No status set"
          )}
        </p>
        <div className="mb-4 flex flex-wrap gap-2">
          {PRESETS.map((preset) => (
            <button
              key={preset.text}
              type="button"
              onClick={() => {
                setEmoji(preset.emoji);
                setText(preset.text);
                setClearAfter(preset.clearAfter);
              }}
              className="rounded-full border border-gray-300 px-3 py-1 text-sm text-gray-700 hover:bg-gray-50 dark:border-gray-600 dark:text-gray-300 dark:hover:bg-gray-800"
            >
              {preset.emoji} {preset.text}
            </button>
          ))}
        </div>
        <form onSubmit={handleSubmit} className="space-y-4">
          <div className="flex gap-3">
            <div className="w-20">
              <Input
                id="status-emoji"
                label="Emoji"
                value={emoji}
                onChange={(e) => setEmoji(e.target.value)}
                maxLength={16}
              />
            </div>
            <div className="flex-1">
              <Input
                id="status-text"
                label="Status text"
                value={text}
                onChange={(e) => setText(e.target.value)}
                maxLength={100}
                placeholder="What are you up to?"
              />
            </div>
          </div>
          <label className="block text-sm font-medium text-gray-700 dark:text-gray-300">
            Clear after
            <select
              value={clearAfter}
              onChange={(e) => setClearAfter(e.target.value as ClearAfter)}
              className="mt-1 block rounded-md border border-gray-300 bg-white px-3 py-2 text-sm dark:border-gray-600 dark:bg-gray-800 dark:text-gray-100"
            >
              {(Object.keys(CLEAR_AFTER_LABELS) as ClearAfter[]).map((choice) => (
                <option key={choice} value={choice}>
                  {CLEAR_AFTER_LABELS[choice]}
                </option>
              ))}
            </select>
          </label>
          <div className="flex items-center gap-3">
            <Button type="submit" disabled={blank || save.isPending}>
              {save.isPending ? "Saving..." : "Set status"}
            </Button>
            {current && (
              <Button
                type="button"
                variant="secondary"
                onClick={() => clear.mutate()}
                disabled={clear.isPending}
              >
                Clear status
              </Button>
            )}
            {error && (
              <span className="text-sm text-red-600 dark:text-red-400">{error.message}</span>
            )}
          </div>
        </form>
      </div>
    </section>
  );
}

function AppearanceSection({
  theme,
  setTheme,
}: {
  theme: "light" | "dark" | "system";
  setTheme: (t: "light" | "dark" | "system") => void;
}) {
  const options: { value: "light" | "dark" | "system"; label: string }[] = [
    { value: "light", label: "Light" },
    { value: "dark", label: "Dark" },
    { value: "system", label: "System" },
  ];

  return (
    <section>
      <h2 className="text-lg font-semibold text-gray-900 dark:text-gray-100 mb-4">Appearance</h2>
      <div className="rounded-lg border border-gray-200 bg-white p-6 dark:border-gray-700 dark:bg-gray-900">
        <fieldset>
          <legend className="text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
            Theme
          </legend>
          <div className="flex gap-3">
            {options.map((opt) => (
              <button
                key={opt.value}
                onClick={() => setTheme(opt.value)}
                className={`rounded-md border px-4 py-2 text-sm font-medium transition-colors ${
                  theme === opt.value
                    ? "border-indigo-600 bg-indigo-50 text-indigo-700 dark:border-indigo-400 dark:bg-indigo-900/30 dark:text-indigo-300"
                    : "border-gray-200 bg-white text-gray-700 hover:bg-gray-50 dark:border-gray-600 dark:bg-gray-800 dark:text-gray-300 dark:hover:bg-gray-700"
                }`}
              >
                {opt.label}
              </button>
            ))}
          </div>
        </fieldset>
      </div>
    </section>
  );
}

function NotificationSection() {
  const [browserEnabled, setBrowserEnabled] = useState(
    typeof Notification !== "undefined" && Notification.permission === "granted",
  );

  async function requestPermission() {
    if (typeof Notification === "undefined") return;
    const result = await Notification.requestPermission();
    setBrowserEnabled(result === "granted");
  }

  return (
    <section>
      <h2 className="text-lg font-semibold text-gray-900 dark:text-gray-100 mb-4">
        Notifications
      </h2>
      <div className="rounded-lg border border-gray-200 bg-white p-6 dark:border-gray-700 dark:bg-gray-900">
        <div className="flex items-center justify-between">
          <div>
            <p className="text-sm font-medium text-gray-900 dark:text-gray-100">
              Browser notifications
            </p>
            <p className="text-xs text-gray-500 dark:text-gray-400">
              Get notified when you receive new messages while the tab is unfocused
            </p>
          </div>
          {browserEnabled ? (
            <span className="rounded-full bg-green-100 px-3 py-1 text-xs font-medium text-green-700 dark:bg-green-900/30 dark:text-green-400">
              Enabled
            </span>
          ) : (
            <Button variant="secondary" onClick={requestPermission}>
              Enable
            </Button>
          )}
        </div>
      </div>
    </section>
  );
}
