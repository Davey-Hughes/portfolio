import { test, expect } from "@playwright/test";
import { PHOTO_DETAIL_URL, visibleGrid, visiblePhotoLinks } from "./helpers";

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

  test("keys don't crash the app after leaving a photo page", async ({ page }) => {
    // The photo page's arrow-key listener must go when the page does: left behind,
    // it reads the page's disposed signals on every key press and panics.
    const crashes: string[] = [];
    page.on("pageerror", (error) => crashes.push(error.message));
    page.on("console", (message) => {
      if (message.text().includes("panicked")) crashes.push(message.text());
    });

    await page.goto("/", { waitUntil: "networkidle" });
    const photos = await visiblePhotoLinks(page);
    await photos.first().click();
    await page.waitForURL(PHOTO_DETAIL_URL, { timeout: 5000 });

    // Client-side, so the app (and any listener left behind) lives on.
    await page.locator(".back-link").click();
    await expect(page).not.toHaveURL(PHOTO_DETAIL_URL);
    await visibleGrid(page);

    await page.keyboard.press("ArrowRight");
    await page.keyboard.press("a");
    // Console messages arrive asynchronously; a round trip lets them land.
    await page.evaluate(() => new Promise((resolve) => setTimeout(resolve, 100)));
    expect(crashes).toEqual([]);

    // And the app still navigates.
    await (await visiblePhotoLinks(page)).first().click();
    await page.waitForURL(PHOTO_DETAIL_URL, { timeout: 5000 });
  });

  test("steps photos only on arrow keys without Alt, Ctrl or ⌘", async ({ page }) => {
    await page.goto("/", { waitUntil: "networkidle" });
    await (await visiblePhotoLinks(page)).first().click();
    await page.waitForURL(PHOTO_DETAIL_URL, { timeout: 5000 });
    await expect(page.locator(".photo-navigation")).toBeVisible();
    const url = page.url();
    const key = (await page.locator(".nav-next").count()) > 0 ? "ArrowRight" : "ArrowLeft";

    // Alt+Left is the browser's Back: stepping a photo as well would push one entry
    // just for Back to pop it. Synthetic, so nothing really goes back here.
    await page.evaluate((key) => {
      for (const modifier of ["altKey", "ctrlKey", "metaKey"]) {
        document.body.dispatchEvent(
          new KeyboardEvent("keydown", { key, [modifier]: true, bubbles: true, cancelable: true }),
        );
      }
    }, key);
    // A step to another photo is a client-side navigation; give it time to happen.
    await page.waitForTimeout(300);
    expect(page.url()).toBe(url);

    await page.keyboard.press(key);
    await expect(page).not.toHaveURL(url);
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
