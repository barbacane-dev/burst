import { type Page, expect } from "@playwright/test";

/**
 * Log in via the UI login form.
 *
 * The mock OIDC server accepts any username/password, and the gateway
 * does JIT provisioning, so any username will work.
 */
export async function login(page: Page, username: string) {
  await page.goto("/login");
  await page.fill("input#username", username);
  await page.fill("input#password", "password");
  await page.click('button[type="submit"]');
  // Wait until the main layout appears (sidebar with "Burst" heading).
  await expect(page.locator("h1", { hasText: "Burst" })).toBeVisible({
    timeout: 15_000,
  });
}

/**
 * Create a channel from the sidebar and navigate to it.
 * Returns the channel name.
 */
export async function createChannel(page: Page, name: string) {
  await page.click('button[title="Create channel"]');
  await page.fill('input[placeholder="e.g. engineering"]', name);
  await page.click('button[type="submit"]');
  // Wait for navigation to the new channel.
  await expect(page).toHaveURL(/\/channels\//, { timeout: 5_000 });
}

/**
 * Send a message in the currently open channel and wait for it to appear.
 */
export async function sendMessage(page: Page, content: string) {
  const composer = page.locator('textarea[aria-label="Message input"]');
  await composer.fill(content);
  await page.click('button[aria-label="Send message"]');
  // Wait for the message to appear in the message list.
  await expect(
    page.locator('[role="log"] [role="listitem"]', { hasText: content }),
  ).toBeVisible({ timeout: 5_000 });
}
