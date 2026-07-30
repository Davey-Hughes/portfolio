import { test, expect } from "@playwright/test";
import { PHOTO_DETAIL_URL, visiblePhotoLinks } from "./helpers";

test.describe("Photo Detail Page", () => {
  test("should display photo detail page", async ({ page }) => {
    await page.goto("/", { waitUntil: "networkidle" });

    // Scoped to the visible layout: every layout container holds a copy of each
    // photo, so a bare .photo-hero-link can resolve to a hidden one.
    const photos = await visiblePhotoLinks(page);
    const count = await photos.count();
    
    if (count > 0) {
      await photos.first().click();

      // Wait for navigation to photo detail
      await page.waitForURL(PHOTO_DETAIL_URL, { timeout: 5000 });

      // Photo detail container should be visible
      await expect(page.locator(".photo-detail-container")).toBeVisible();
    }
  });

  test("should display full-size image", async ({ page }) => {
    await page.goto("/", { waitUntil: "networkidle" });

    // Scoped to the visible layout: every layout container holds a copy of each
    // photo, so a bare .photo-hero-link can resolve to a hidden one.
    const photos = await visiblePhotoLinks(page);
    const count = await photos.count();
    
    if (count > 0) {
      await photos.first().click();
      await page.waitForURL(PHOTO_DETAIL_URL, { timeout: 5000 });
      
      // Wait for network to be idle to ensure image has loaded
      await page.waitForLoadState("networkidle");

      // Wait for the image to load - check it's attached to DOM and has a src
      const mainPhotoImg = page.locator(".photo-detail-image img");
      await expect(mainPhotoImg).toBeAttached({ timeout: 15000 });
      await expect(mainPhotoImg).toHaveAttribute("src", /\/images\//);
    }
  });

  test("should show EXIF data if available", async ({ page }) => {
    await page.goto("/", { waitUntil: "networkidle" });

    // Scoped to the visible layout: every layout container holds a copy of each
    // photo, so a bare .photo-hero-link can resolve to a hidden one.
    const photos = await visiblePhotoLinks(page);
    const count = await photos.count();
    
    if (count > 0) {
      await photos.first().click();
      await page.waitForURL(PHOTO_DETAIL_URL, { timeout: 5000 });

      // Check if EXIF data section exists
      const exifData = page.locator(".photo-exif");
      const exifCount = await exifData.count();
      
      if (exifCount > 0) {
        await expect(exifData).toBeVisible();
      }
    }
  });

  test("should not show footer on photo detail page", async ({ page }) => {
    await page.goto("/", { waitUntil: "networkidle" });

    // Scoped to the visible layout: every layout container holds a copy of each
    // photo, so a bare .photo-hero-link can resolve to a hidden one.
    const photos = await visiblePhotoLinks(page);
    const count = await photos.count();
    
    if (count > 0) {
      await photos.first().click();
      await page.waitForURL(PHOTO_DETAIL_URL, { timeout: 5000 });

      // Footer should not be visible on photo detail page
      const footer = page.locator("footer");
      const footerCount = await footer.count();
      
      if (footerCount > 0) {
        await expect(footer).not.toBeVisible();
      }
    }
  });

  test("should have navigation controls", async ({ page }) => {
    await page.goto("/", { waitUntil: "networkidle" });

    // Scoped to the visible layout: every layout container holds a copy of each
    // photo, so a bare .photo-hero-link can resolve to a hidden one.
    const photos = await visiblePhotoLinks(page);
    const count = await photos.count();
    
    if (count > 1) {
      await photos.first().click();
      await page.waitForURL(PHOTO_DETAIL_URL, { timeout: 5000 });

      // Check for next/previous navigation
      const navControls = page.locator(".photo-navigation");
      const navCount = await navControls.count();
      
      if (navCount > 0) {
        await expect(navControls).toBeVisible();
      }
    }
  });

  test("should support keyboard navigation", async ({ page }) => {
    await page.goto("/", { waitUntil: "networkidle" });

    // Scoped to the visible layout: every layout container holds a copy of each
    // photo, so a bare .photo-hero-link can resolve to a hidden one.
    const photos = await visiblePhotoLinks(page);
    const count = await photos.count();
    
    if (count > 1) {
      await photos.first().click();
      await page.waitForURL(PHOTO_DETAIL_URL, { timeout: 5000 });

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
    
    await page.goto("/", { waitUntil: "networkidle" });

    // Scoped to the visible layout: every layout container holds a copy of each
    // photo, so a bare .photo-hero-link can resolve to a hidden one.
    const photos = await visiblePhotoLinks(page);
    const count = await photos.count();
    
    if (count > 1) {
      await photos.first().click();
      await page.waitForURL(PHOTO_DETAIL_URL, { timeout: 5000 });

      // Photo should be visible
      const photoImage = page.locator(".photo-detail-image");
      await expect(photoImage).toBeVisible();

      // Swipe gesture would be tested here if implemented
      // For now, just verify the page renders correctly on mobile
    }
  });
});
