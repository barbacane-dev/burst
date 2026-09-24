export interface NotificationOptions {
  /**
   * Notifications sharing a tag replace each other. One tag per channel keeps
   * the latest from each channel on screen rather than only the latest overall.
   */
  tag?: string;
  /** Runs after the window is focused, when the notification is clicked. */
  onClick?: () => void;
}

export function showBrowserNotification(
  title: string,
  body: string,
  options: NotificationOptions = {},
) {
  if (
    typeof Notification === "undefined" ||
    Notification.permission !== "granted" ||
    document.visibilityState === "visible"
  ) {
    return;
  }

  const notification = new Notification(title, {
    body,
    icon: "/favicon.ico",
    tag: options.tag ?? "burst-message",
  });

  notification.onclick = () => {
    window.focus();
    options.onClick?.();
    notification.close();
  };
}
