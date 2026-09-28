import { test, expect } from "@playwright/test";
import { login } from "./helpers";

const GATEWAY = "http://localhost:8080";
const MOCK_OAUTH = "http://localhost:9099";

async function getToken(username: string): Promise<string> {
  const res = await fetch(`${MOCK_OAUTH}/burst/token`, {
    method: "POST",
    headers: { "Content-Type": "application/x-www-form-urlencoded" },
    body: `grant_type=password&username=${username}&password=password&client_id=burst&scope=openid`,
  });
  const data = await res.json();
  return data.access_token;
}

test.describe("Webhooks & Bots (integrator)", () => {
  test("integrator sees Webhooks and Bots tabs but not admin tabs", async ({
    page,
  }) => {
    await login(page, "ivy");

    const adminButton = page.locator('button[title="Administration"]');
    await expect(adminButton).toBeVisible({ timeout: 5_000 });
    await adminButton.click();
    await expect(page).toHaveURL("/admin");

    // Integration tabs visible
    await expect(page.locator("button", { hasText: "Webhooks" })).toBeVisible();
    await expect(page.locator("button", { hasText: "Bots" })).toBeVisible();

    // Admin-only tabs hidden
    await expect(
      page.locator("button", { hasText: "Users" }),
    ).not.toBeVisible();
    await expect(
      page.locator("button", { hasText: "Channels" }),
    ).not.toBeVisible();
    await expect(
      page.locator("button", { hasText: "Audit Log" }),
    ).not.toBeVisible();
  });

  test("integrator can create an incoming webhook and see it listed", async ({
    page,
  }) => {
    await login(page, "ivy");

    // Navigate to admin
    await page.click('button[title="Administration"]');
    await expect(page.locator("button", { hasText: "Webhooks" })).toBeVisible();

    // Click create
    await page.click("button:has-text('Create Webhook')");
    await expect(page.locator("text=New Webhook")).toBeVisible();

    // Select a channel from the dropdown
    const channelSelect = page.locator("select").first();
    await channelSelect.selectOption({ index: 1 }); // first real channel

    // Fill name
    await page.fill('input[placeholder="CI Notifications"]', "E2E Test Hook");

    // Create
    await page.click("button:has-text('Create'):not(:has-text('Webhook'))");

    // Token banner should appear
    await expect(page.locator("text=will not be shown again")).toBeVisible({
      timeout: 5_000,
    });

    // Dismiss token banner
    await page.click("button:has-text('Dismiss')");

    // Webhook should appear in the list
    await expect(page.locator("text=E2E Test Hook")).toBeVisible();
    await expect(page.locator("text=incoming").first()).toBeVisible();
  });

  test("incoming webhook trigger posts a message to the channel", async ({
    page,
  }) => {
    // First, create a webhook via API as ivy
    const ivyToken = await getToken("ivy");

    // Get ivy's channels
    const channelsRes = await fetch(
      `${GATEWAY}/api/channels?joined=true`,
      { headers: { Authorization: `Bearer ${ivyToken}` } },
    );
    const channels = await channelsRes.json();
    const generalChannel = channels.items.find(
      (ch: { name: string }) => ch.name === "general",
    );
    expect(generalChannel).toBeTruthy();

    // Create incoming webhook
    const createRes = await fetch(
      `${GATEWAY}/api/channels/${generalChannel.id}/webhooks`,
      {
        method: "POST",
        headers: {
          Authorization: `Bearer ${ivyToken}`,
          "Content-Type": "application/json",
        },
        body: JSON.stringify({ kind: "incoming", name: "E2E Trigger Test" }),
      },
    );
    expect(createRes.status).toBe(201);
    const webhook = await createRes.json();

    // Trigger the webhook (no OIDC auth, just bearer token)
    const triggerRes = await fetch(
      `${GATEWAY}/api/webhooks/${webhook.id}/trigger`,
      {
        method: "POST",
        headers: {
          Authorization: `Bearer ${webhook.token}`,
          "Content-Type": "application/json",
        },
        body: JSON.stringify({
          content: "Automated deploy #42 succeeded",
        }),
      },
    );
    expect(triggerRes.status).toBe(201);

    // Now log in as ivy and check the message appeared in general
    await login(page, "ivy");

    // Navigate to general channel
    await page.click("text=general");
    await expect(
      page.locator('[role="log"] [role="listitem"]', {
        hasText: "Automated deploy #42 succeeded",
      }),
    ).toBeVisible({ timeout: 10_000 });
  });

  test("admin sees all tabs including Webhooks and Bots", async ({ page }) => {
    await login(page, "alice");

    await page.click('button[title="Administration"]');

    await expect(page.locator("button", { hasText: "Users" })).toBeVisible();
    await expect(
      page.locator("button", { hasText: "Channels" }),
    ).toBeVisible();
    await expect(
      page.locator("button", { hasText: "Webhooks" }),
    ).toBeVisible();
    await expect(page.locator("button", { hasText: "Bots" })).toBeVisible();
    await expect(page.locator("button", { hasText: "Emojis" })).toBeVisible();
    await expect(
      page.locator("button", { hasText: "Audit Log" }),
    ).toBeVisible();
  });

  test("integrator can create a bot user", async ({ page }) => {
    await login(page, "ivy");
    await page.click('button[title="Administration"]');
    await page.click("button:has-text('Bots')");

    await page.click("button:has-text('Create Bot')");
    await expect(page.locator("text=New Bot")).toBeVisible();

    await page.fill('input[placeholder="ci-bot"]', `e2e-bot-${Date.now()}`);
    await page.fill('input[placeholder="CI Bot"]', "E2E Test Bot");
    await page.click("button:has-text('Create'):not(:has-text('Bot'))");

    // Bot should appear in list
    await expect(page.locator("text=E2E Test Bot")).toBeVisible({
      timeout: 5_000,
    });
    await expect(page.locator("text=Bot").first()).toBeVisible();
  });

  test("regular member cannot see admin link", async ({ page }) => {
    await login(page, "bob");
    await expect(
      page.locator('button[title="Administration"]'),
    ).not.toBeVisible();
  });
});
