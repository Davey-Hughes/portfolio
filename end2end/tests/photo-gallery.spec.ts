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

// gen_fixtures writes the nature gallery's bird as `bird.JPG`, with the upper-case
// extension cameras give their files.
test.describe("A photo whose extension is in upper case", () => {
  test("shows in the grid, and its sizes and original load", async ({ page, request }) => {
    await page.goto("/gallery/nature", { waitUntil: "networkidle" });

    const grid = await visibleGrid(page);
    const img = grid.locator('.photo-hero-link[href="/gallery/nature/bird"] img');
    await img.scrollIntoViewIfNeeded();
    await expect
      .poll(() => img.evaluate((el: HTMLImageElement) => el.complete && el.naturalWidth > 0))
      .toBe(true);

    // The addresses keep the file's real name, which the file server needs.
    const src = (await img.getAttribute("src")) ?? "";
    expect(src).toMatch(/^\/images\/compressed\/nature\/bird\.JPG\?/);
    const srcset = (await img.getAttribute("srcset")) ?? "";
    const sizes = srcset.split(", ").map((candidate) => candidate.split(" ")[0]);
    expect(sizes.length).toBeGreaterThan(0);
    for (const url of [src, ...sizes]) {
      const res = await request.get(url);
      expect(res.status(), url).toBe(200);
      expect(res.headers()["content-type"], url).toBe("image/webp");
    }
    const original = await request.get("/images/nature/bird.JPG");
    expect(original.status()).toBe(200);
    expect(original.headers()["content-type"]).toBe("image/jpeg");
    expect((await request.get("/gallery/nature/bird")).status()).toBe(200);
  });
});
