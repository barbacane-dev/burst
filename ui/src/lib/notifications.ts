export function showBrowserNotification(title: string, body: string) {
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
    tag: "burst-message",
  });

  notification.onclick = () => {
    window.focus();
    notification.close();
  };
}
