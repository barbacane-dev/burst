import { test, expect } from "@playwright/test";
import { login, createChannel, sendMessage } from "./helpers";

test.describe("Messaging", () => {
  test.beforeEach(async ({ page }) => {
    await login(page, "alice");
  });

  test("create channel and send a message", async ({ page }) => {
    const channelName = `test-${Date.now()}`;
    await createChannel(page, channelName);

    // Channel header should show the name.
    await expect(page.locator("h2", { hasText: channelName })).toBeVisible();

    await sendMessage(page, "Hello from Playwright!");

    // Message should be visible in the message list.
    await expect(
      page.locator('[role="listitem"]', { hasText: "Hello from Playwright!" }),
    ).toBeVisible();
  });

  test("pin and unpin a message", async ({ page }) => {
    const channelName = `pin-${Date.now()}`;
    await createChannel(page, channelName);
    await sendMessage(page, "Pin this message");

    // The pin button is inside a group-hover toolbar hidden via CSS.
    // Playwright's hover doesn't reliably trigger Tailwind's group-hover,
    // so we click the button directly via JavaScript.
    const message = page.locator('[role="listitem"]', {
      hasText: "Pin this message",
    });

    await message.evaluate((el) => {
      const btn = el.querySelector('button[title="Pin message"]') as HTMLButtonElement | null;
      if (!btn) throw new Error("Pin button not found in message DOM");
      btn.click();
    });

    // Wait for the pin API call to complete.
    await page.waitForTimeout(1_000);

    // Open the pinned messages panel.
    await page.click('button[title="Pinned messages"]');

    // The pinned panel should show the message.
    const pinnedPanel = page.locator('aside[aria-label="Pinned messages"]');
    await expect(pinnedPanel).toBeVisible();
    await expect(
      pinnedPanel.locator("text=Pin this message"),
    ).toBeVisible({ timeout: 5_000 });

    // Close the panel.
    await page.click('button[aria-label="Close pinned messages"]');
  });

  test("send message with @mention", async ({ page }) => {
    const channelName = `mention-${Date.now()}`;
    await createChannel(page, channelName);
    await sendMessage(page, "Hey @alice check this out");

    // The message should appear with the mention text visible.
    await expect(
      page.locator('[role="listitem"]', {
        hasText: "Hey @alice check this out",
      }),
    ).toBeVisible();
  });

  test("message sent by one user is visible to another", async ({
    browser,
  }) => {
    const channelName = `multi-${Date.now()}`;

    // Alice creates the channel.
    const alicePage = await browser.newPage();
    await login(alicePage, "alice");
    await createChannel(alicePage, channelName);

    // Bob logs in and browses channels to join.
    const bobPage = await browser.newPage();
    await login(bobPage, "bob");
    await bobPage.click('button[title="Browse channels"]');

    // In the browse dialog, find the channel row and click "Join".
    const channelRow = bobPage.locator("li", { hasText: channelName });
    await expect(channelRow).toBeVisible({ timeout: 5_000 });
    await channelRow.locator("button", { hasText: "Join" }).click();

    // Wait for Bob to be in the channel page.
    await expect(
      bobPage.locator("h2", { hasText: channelName }),
    ).toBeVisible({ timeout: 5_000 });

    // Alice sends a message.
    await sendMessage(alicePage, "Cross-user test!");

    // Bob reloads to fetch messages via REST.
    // NOTE: WebSocket delivery through the gateway currently fails with 401
    // on WS upgrade, so we rely on reload for now. When WS auth is fixed,
    // this test can be changed to assert real-time delivery without reload.
    await bobPage.reload();
    await expect(
      bobPage.locator("h2", { hasText: channelName }),
    ).toBeVisible({ timeout: 5_000 });

    await expect(
      bobPage.locator('[role="listitem"]', { hasText: "Cross-user test!" }),
    ).toBeVisible({ timeout: 10_000 });

    await alicePage.close();
    await bobPage.close();
  });
});
