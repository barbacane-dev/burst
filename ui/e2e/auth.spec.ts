import { test, expect } from "@playwright/test";
import { login } from "./helpers";

test.describe("Authentication", () => {
  test("login redirects to main layout", async ({ page }) => {
    await login(page, "alice");
    await expect(page).toHaveURL("/");
    // Sidebar heading should be visible.
    await expect(
      page.locator("h1", { hasText: "Burst" }),
    ).toBeVisible();
  });

  test("unauthenticated user is redirected to login", async ({ page }) => {
    await page.goto("/");
    await expect(page).toHaveURL(/\/login/);
  });

  test("login persists across page reload", async ({ page }) => {
    await login(page, "alice");
    await page.reload();
    // Should still be on main layout, not redirected to login.
    await expect(
      page.locator("h1", { hasText: "Burst" }),
    ).toBeVisible({ timeout: 15_000 });
  });

  test("logout returns to login page", async ({ page }) => {
    await login(page, "alice");
    await page.click('button[title="Log out"]');
    await expect(page).toHaveURL(/\/login/);
  });
});
