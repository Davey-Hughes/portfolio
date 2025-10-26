import { test, expect } from "@playwright/test";

test.describe("About Page", () => {
  test("should load and display about page", async ({ page }) => {
    await page.goto("/about", { waitUntil: "networkidle" });

    // Check page title
    await expect(page).toHaveTitle("Photography Portfolio");

    // Check about container is visible
    await expect(page.locator(".about-container")).toBeVisible();
  });

  test("should display about content text", async ({ page }) => {
    await page.goto("/about", { waitUntil: "networkidle" });

    // Wait for content to load
    await page.waitForSelector(".about-content", { timeout: 5000 });

    const aboutContent = page.locator(".about-content");
    await expect(aboutContent).toBeVisible();
    
    // Should have some text content
    const text = await aboutContent.textContent();
    expect(text).toBeTruthy();
    expect(text!.length).toBeGreaterThan(0);
  });

  test("should display profile image if available", async ({ page }) => {
    await page.goto("/about", { waitUntil: "networkidle" });

    // Check if profile image exists
    const profileImage = page.locator(".profile-image");
    const count = await profileImage.count();
    
    if (count > 0) {
      await expect(profileImage).toBeVisible();
      
      // Image should have src attribute
      const src = await profileImage.getAttribute("src");
      expect(src).toBeTruthy();
    }
  });

  test("should have footer on about page", async ({ page }) => {
    await page.goto("/about", { waitUntil: "networkidle" });

    // Footer should be visible
    const footer = page.locator("footer");
    await expect(footer).toBeVisible();
  });
});
