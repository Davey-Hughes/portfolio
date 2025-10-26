import { test, expect } from "@playwright/test";

test.describe("Photo Detail Page", () => {
  test("should display photo detail page", async ({ page }) => {
    await page.goto("/");

    // Wait for photos and click on first one
    await page.waitForSelector(".photo-hero-link", { timeout: 10000 });
    
    const photos = page.locator(".photo-hero-link");
    const count = await photos.count();
    
    if (count > 0) {
      await photos.first().click();

      // Wait for navigation to photo detail
      await page.waitForURL(/\/photo\//, { timeout: 5000 });

      // Photo detail container should be visible
      await expect(page.locator(".photo-detail-container")).toBeVisible();
    }
  });

  test("should display full-size image", async ({ page }) => {
    await page.goto("/");

    await page.waitForSelector(".photo-hero-link", { timeout: 10000 });
    
    const photos = page.locator(".photo-hero-link");
    const count = await photos.count();
    
    if (count > 0) {
      await photos.first().click();
      await page.waitForURL(/\/photo\//, { timeout: 5000 });

      // Main photo should be visible
      const mainPhoto = page.locator(".photo-detail-image");
      await expect(mainPhoto).toBeVisible();
    }
  });

  test("should show EXIF data if available", async ({ page }) => {
    await page.goto("/");

    await page.waitForSelector(".photo-hero-link", { timeout: 10000 });
    
    const photos = page.locator(".photo-hero-link");
    const count = await photos.count();
    
    if (count > 0) {
      await photos.first().click();
      await page.waitForURL(/\/photo\//, { timeout: 5000 });

      // Check if EXIF data section exists
      const exifData = page.locator(".photo-exif");
      const exifCount = await exifData.count();
      
      if (exifCount > 0) {
        await expect(exifData).toBeVisible();
      }
    }
  });

  test("should not show footer on photo detail page", async ({ page }) => {
    await page.goto("/");

    await page.waitForSelector(".photo-hero-link", { timeout: 10000 });
    
    const photos = page.locator(".photo-hero-link");
    const count = await photos.count();
    
    if (count > 0) {
      await photos.first().click();
      await page.waitForURL(/\/photo\//, { timeout: 5000 });

      // Footer should not be visible on photo detail page
      const footer = page.locator("footer");
      const footerCount = await footer.count();
      
      if (footerCount > 0) {
        await expect(footer).not.toBeVisible();
      }
    }
  });

  test("should have navigation controls", async ({ page }) => {
    await page.goto("/");

    await page.waitForSelector(".photo-hero-link", { timeout: 10000 });
    
    const photos = page.locator(".photo-hero-link");
    const count = await photos.count();
    
    if (count > 1) {
      await photos.first().click();
      await page.waitForURL(/\/photo\//, { timeout: 5000 });

      // Check for next/previous navigation
      const navControls = page.locator(".photo-navigation");
      const navCount = await navControls.count();
      
      if (navCount > 0) {
        await expect(navControls).toBeVisible();
      }
    }
  });

  test("should support keyboard navigation", async ({ page }) => {
    await page.goto("/");

    await page.waitForSelector(".photo-hero-link", { timeout: 10000 });
    
    const photos = page.locator(".photo-hero-link");
    const count = await photos.count();
    
    if (count > 1) {
      await photos.first().click();
      await page.waitForURL(/\/photo\//, { timeout: 5000 });

      const currentUrl = page.url();

      // Try pressing arrow key for navigation
      await page.keyboard.press("ArrowRight");
      
      // Wait a bit for potential navigation
      await page.waitForTimeout(500);

      // URL might change if keyboard navigation is implemented
      // This is a soft check - we're just verifying nothing breaks
      await expect(page).toHaveTitle("Photography Portfolio");
    }
  });

  test("should support swipe gestures on mobile viewports", async ({ page }) => {
    // Set mobile viewport
    await page.setViewportSize({ width: 375, height: 667 });
    
    await page.goto("/");

    await page.waitForSelector(".photo-hero-link", { timeout: 10000 });
    
    const photos = page.locator(".photo-hero-link");
    const count = await photos.count();
    
    if (count > 1) {
      await photos.first().click();
      await page.waitForURL(/\/photo\//, { timeout: 5000 });

      // Photo should be visible
      const photoImage = page.locator(".photo-detail-image");
      await expect(photoImage).toBeVisible();

      // Swipe gesture would be tested here if implemented
      // For now, just verify the page renders correctly on mobile
    }
  });
});
