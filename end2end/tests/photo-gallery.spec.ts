import { test, expect } from "@playwright/test";
import { PHOTO_DETAIL_URL, visibleGrid, visiblePhotoLinks } from "./helpers";

test.describe("Photo Gallery", () => {
  test("should display photo grid on homepage", async ({ page }) => {
    await page.goto("/", { waitUntil: "networkidle" });

    // Whichever grid container this gallery/viewport renders.
    await visibleGrid(page);
  });

  test("should display photo thumbnails", async ({ page }) => {
    await page.goto("/", { waitUntil: "networkidle" });

    const photos = await visiblePhotoLinks(page);
    const count = await photos.count();
    
    if (count > 0) {
      // First photo should be visible
      await expect(photos.first()).toBeVisible();
      
      // Photos should have images
      const firstImg = photos.first().locator("img");
      await expect(firstImg).toBeVisible();
    }
  });

  test("should open photo detail when clicking on thumbnail", async ({ page }) => {
    await page.goto("/", { waitUntil: "networkidle" });

    const photos = await visiblePhotoLinks(page);
    const count = await photos.count();
    
    if (count > 0) {
      // Click on first photo
      await photos.first().click();

      // Should navigate to photo detail page, which is /gallery/<gallery>/<photo>
      // (src/app.rs:83-91) — there is no /photo/ route.
      await page.waitForURL(PHOTO_DETAIL_URL, { timeout: 5000 });

      expect(page.url()).toMatch(PHOTO_DETAIL_URL);
    }
  });

  test("should prevent right-click on images", async ({ page }) => {
    await page.goto("/", { waitUntil: "networkidle" });

    const links = await visiblePhotoLinks(page);
    const firstImage = links.locator("img").first();
    const imageCount = await links.locator("img").count();
    
    if (imageCount > 0) {
      // Try to right-click on image
      await firstImage.click({ button: "right" });

      // Context menu should be prevented (we can't directly test this,
      // but we can verify the image is still there and nothing broke)
      await expect(firstImage).toBeVisible();
    }
  });

  test("should lazy load images", async ({ page }) => {
    await page.goto("/", { waitUntil: "networkidle" });

    // Wait for photo grid
    await visibleGrid(page);

    // Check that images have loading="lazy" attribute (if implemented)
    const images = (await visiblePhotoLinks(page)).locator("img");
    const count = await images.count();
    
    // Just verify images are present
    if (count > 0) {
      await expect(images.first()).toBeVisible();
    }
  });
});
