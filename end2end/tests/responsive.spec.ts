import { test, expect } from "@playwright/test";

test.describe("Responsive Design", () => {
  const viewports = [
    { name: "Mobile", width: 375, height: 667 },
    { name: "Tablet", width: 768, height: 1024 },
    { name: "Desktop", width: 1920, height: 1080 },
  ];

  for (const viewport of viewports) {
    test(`should render correctly on ${viewport.name}`, async ({ page }) => {
      await page.setViewportSize({ width: viewport.width, height: viewport.height });
      
      await page.goto("/");

      // Navigation should be visible
      await expect(page.locator("nav.navbar")).toBeVisible();

      // Photo grid should be visible
      await page.waitForSelector(".photo-grid-home", { timeout: 10000 });
      await expect(page.locator(".photo-grid-home")).toBeVisible();

      // Footer should be visible
      await expect(page.locator("footer")).toBeVisible();
    });

    test(`should navigate correctly on ${viewport.name}`, async ({ page }) => {
      await page.setViewportSize({ width: viewport.width, height: viewport.height });
      
      await page.goto("/");

      // Try navigating to About
      await page.click('nav a[href="/about"]');
      await expect(page).toHaveURL("/about");

      // About content should be visible
      await expect(page.locator(".about-container")).toBeVisible();
    });
  }

  test("should have mobile-friendly navigation on small screens", async ({ page }) => {
    await page.setViewportSize({ width: 375, height: 667 });
    
    await page.goto("/");

    // Navigation should still be accessible
    const nav = page.locator("nav.navbar");
    await expect(nav).toBeVisible();

    // Links should be clickable
    const aboutLink = page.locator('nav a[href="/about"]');
    await expect(aboutLink).toBeVisible();
  });

  test("should display photo grid responsively", async ({ page }) => {
    const sizes = [375, 768, 1920];

    for (const width of sizes) {
      await page.setViewportSize({ width, height: 1080 });
      await page.goto("/");

      await page.waitForSelector(".photo-grid-home", { timeout: 10000 });
      
      const photoGrid = page.locator(".photo-grid-home");
      await expect(photoGrid).toBeVisible();

      // Grid should adapt to different screen sizes
      const photos = page.locator(".photo-hero-link");
      const count = await photos.count();
      
      if (count > 0) {
        await expect(photos.first()).toBeVisible();
      }
    }
  });
});
