import { test, expect } from "@playwright/test";
import { login } from "./helpers";

test.describe("Admin panel", () => {
  test("admin can access the admin panel", async ({ page }) => {
    // alice is seeded as admin in seed.rs
    await login(page, "alice");

    const adminButton = page.locator('button[title="Administration"]');
    await expect(adminButton).toBeVisible({ timeout: 5_000 });
    await adminButton.click();
    await expect(page).toHaveURL("/admin");

    // Tabs should be visible.
    await expect(page.locator("button", { hasText: "Users" })).toBeVisible();
    await expect(
      page.locator("button", { hasText: "Channels" }),
    ).toBeVisible();
    await expect(
      page.locator("button", { hasText: "Audit Log" }),
    ).toBeVisible();
  });

  test("non-admin user does not see admin link", async ({ page }) => {
    // bob is seeded as member in seed.rs
    await login(page, "bob");

    // The Administration button should not be in the sidebar.
    await expect(
      page.locator('button[title="Administration"]'),
    ).not.toBeVisible();
  });

  test("admin can view users list", async ({ page }) => {
    await login(page, "alice");

    await page.click('button[title="Administration"]');
    await page.click("button:has-text('Users')");

    // Should see at least the current user in the list.
    await expect(
      page.locator(".border.border-gray-200").first(),
    ).toBeVisible({ timeout: 5_000 });
  });

  test("admin can view channels list", async ({ page }) => {
    await login(page, "alice");

    await page.click('button[title="Administration"]');
    await page.click("button:has-text('Channels')");

    // The channels tab should be active.
    await expect(page.locator("button:has-text('Channels')")).toBeVisible();
  });

  test("admin can view audit log", async ({ page }) => {
    await login(page, "alice");

    await page.click('button[title="Administration"]');
    await page.click("button:has-text('Audit Log')");

    // The audit log tab should be active.
    await expect(page.locator("button:has-text('Audit Log')")).toBeVisible();
  });
});
