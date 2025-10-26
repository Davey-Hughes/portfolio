import { test, expect } from "@playwright/test";

test.describe("Navigation", () => {
  test("should navigate to About page", async ({ page }) => {
    await page.goto("/");

    // Click on About link
    await page.click('nav a[href="/about"]');

    // Check URL changed
    await expect(page).toHaveURL("/about");

    // Check that about content is visible
    await expect(page.locator(".about-container")).toBeVisible();
  });

  test("should navigate to Contact page", async ({ page }) => {
    await page.goto("/");

    // Click on Contact link
    await page.click('nav a[href="/contact"]');

    // Check URL changed
    await expect(page).toHaveURL("/contact");

    // Check that contact info is visible
    await expect(page.locator(".contact-container")).toBeVisible();
  });

  test("should navigate back to home from About", async ({ page }) => {
    await page.goto("/about");

    // Click on home/brand link
    await page.click('nav a[href="/"]');

    // Check URL is back to home
    await expect(page).toHaveURL("/");
  });

  test("should show dropdown menu for galleries if they exist", async ({ page }) => {
    await page.goto("/");

    // Check if galleries dropdown exists
    const galleriesDropdown = page.locator(".galleries-dropdown");
    
    // If galleries exist, the dropdown should be visible
    const count = await galleriesDropdown.count();
    if (count > 0) {
      await expect(galleriesDropdown).toBeVisible();
    }
  });

  test("should maintain navigation across all pages", async ({ page }) => {
    const pages = ["/", "/about", "/contact"];

    for (const pagePath of pages) {
      await page.goto(pagePath);
      
      // Navigation should always be visible
      await expect(page.locator("nav.navbar")).toBeVisible();
      
      // Brand link should always be present
      await expect(page.locator(".nav-brand")).toBeVisible();
    }
  });
});
