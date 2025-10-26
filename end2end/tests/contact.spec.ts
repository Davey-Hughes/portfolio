import { test, expect } from "@playwright/test";

test.describe("Contact Page", () => {
  test("should load and display contact page", async ({ page }) => {
    await page.goto("/contact");

    // Check page title
    await expect(page).toHaveTitle("Photography Portfolio");

    // Check contact container is visible
    await expect(page.locator(".contact-container")).toBeVisible();
  });

  test("should display contact information", async ({ page }) => {
    await page.goto("/contact");

    // Contact info should be visible
    const contactInfo = page.locator(".contact-info");
    await expect(contactInfo).toBeVisible();
  });

  test("should have footer on contact page", async ({ page }) => {
    await page.goto("/contact");

    // Footer should be visible
    const footer = page.locator("footer");
    await expect(footer).toBeVisible();
  });
});
