import { test, expect } from "@playwright/test";
import { visibleGrid } from "./helpers";


// These request a photo that actually exists — home/contrail.jpg, from the fixture
// gallery written by `cargo run --example gen_fixtures`.
//
// The handler validates query params first (main.rs:149, returning 400) and only then
// looks the image up (main.rs:185, returning 404 when it is missing). That ordering used
// to hide these tests from themselves: they requested a nonexistent test.jpg with
// parameters that were not a valid preset, so every response was a 400 from validation
// and the "200 or 404" assertions passed without the image lookup ever running. Against
// a real image with real presets, a valid request is a genuine 200 and a rejection is a
// genuine parameter rejection.
//
// Valid presets are (2400, 90) and (4000, 90) — src/image_params.rs:45.
const FIXTURE_IMAGE = "/images/compressed/home/contrail.jpg";

test.describe("Image Parameter Validation", () => {
  test("should serve compressed images with valid width parameter", async ({ page, request }) => {
    // 2400 is a preset width; quality defaults to its paired 90.
    const response = await request.get(`${FIXTURE_IMAGE}?width=2400`);

    expect(response.status()).toBe(200);
  });

  test("should reject invalid width parameter", async ({ page, request }) => {
    // Test invalid width parameter (1500 is not in the valid presets)
    const response = await request.get(`${FIXTURE_IMAGE}?width=1500`);
    
    // Should return 400 Bad Request for invalid width
    expect(response.status()).toBe(400);
    
    const body = await response.text();
    expect(body).toContain("Invalid width");
  });

  test("should serve compressed images with valid width and quality combination", async ({ page, request }) => {
    // 4000/90 is a valid preset (src/image_params.rs:45). The fixture image exists, so
    // this is a real 200 rather than the "200 or 404" hedge the file used to need.
    const response = await request.get(`${FIXTURE_IMAGE}?width=4000&quality=90`);

    expect(response.status()).toBe(200);
  });

  test("should reject invalid width and quality combination", async ({ page, request }) => {
    // Test invalid combination (1200, 100 is not a valid preset)
    const response = await request.get(`${FIXTURE_IMAGE}?width=1200&quality=100`);
    
    // Should return 400 Bad Request for invalid combination
    expect(response.status()).toBe(400);
    
    const body = await response.text();
    expect(body).toContain("Invalid width/quality combination");
  });

  test("should reject quality parameter without width", async ({ page, request }) => {
    // Test quality only (not allowed)
    const response = await request.get(`${FIXTURE_IMAGE}?quality=90`);
    
    // Should return 400 Bad Request
    expect(response.status()).toBe(400);
    
    const body = await response.text();
    expect(body).toContain("Quality must be specified with a width");
  });

  test("should use default parameters when none specified", async ({ page, request }) => {
    // No parameters falls back to the first preset, 2400/90.
    const response = await request.get(FIXTURE_IMAGE);

    expect(response.status()).toBe(200);
  });

  test("should accept all valid preset widths", async ({ page, request }) => {
    // The preset widths, per src/image_params.rs:45. 1200 and 3600 were in this list
    // historically and are no longer served — anything below 2400 looked soft, so they
    // were dropped deliberately (see the comment there).
    const validWidths = [2400, 4000];

    for (const width of validWidths) {
      const response = await request.get(`${FIXTURE_IMAGE}?width=${width}`);

      expect(response.status()).toBe(200);
    }
  });

  test("should accept valid preset combinations", async ({ page, request }) => {
    // Every current preset is quality 90; see src/image_params.rs:45.
    const validCombinations = [
      { width: 2400, quality: 90 },
      { width: 4000, quality: 90 },
    ];
    
    for (const combo of validCombinations) {
      const response = await request.get(
        `${FIXTURE_IMAGE}?width=${combo.width}&quality=${combo.quality}`
      );

      expect(response.status()).toBe(200);
    }
  });
});

test.describe("Image Parameter Environment Variables", () => {
  test("should load images with default parameters on gallery pages", async ({ page }) => {
    await page.goto("/", { waitUntil: "networkidle" });
    
    // Scope to the visible grid: mosaic and mobile layouts are both in the DOM, so
    // querying across all grid classes would count each photo more than once.
    const images = (await visibleGrid(page)).locator("img");
    const count = await images.count();
    
    if (count > 0) {
      // Get the src of the first image
      const firstImage = images.first();
      const src = await firstImage.getAttribute("src");
      
      // Image src should contain the compressed endpoint
      expect(src).toContain("/images/compressed/");
      
      // Should have width and quality parameters
      expect(src).toMatch(/[?&]width=\d+/);
      expect(src).toMatch(/[?&]quality=\d+/);
    }
  });

  test("should use compressed images in photo gallery", async ({ page }) => {
    await page.goto("/", { waitUntil: "networkidle" });
    
    // Find a gallery link if it exists
    const galleryLinks = page.locator('a[href^="/gallery/"]');
    const linkCount = await galleryLinks.count();
    
    if (linkCount > 0) {
      // Click the first gallery link
      await galleryLinks.first().click();
      await page.waitForLoadState("networkidle");
      
      // Scope to the visible grid, as above.
      const images = (await visibleGrid(page)).locator("img");
      const imageCount = await images.count();
      
      if (imageCount > 0) {
        const firstImage = images.first();
        const src = await firstImage.getAttribute("src");
        
        // Should use compressed endpoint
        expect(src).toContain("/images/compressed/");
        
        // Should have width and quality parameters
        expect(src).toMatch(/[?&]width=\d+/);
        expect(src).toMatch(/[?&]quality=\d+/);
      }
    }
  });

  test("should handle image loading errors gracefully", async ({ page, request }) => {
    // Test that the page doesn't crash when an image fails to load
    // Valid params on purpose: with an invalid combination this would 400 during
    // parameter validation (main.rs:149) and never reach the lookup, so the test would
    // pass without exercising the missing-image path at all.
    const response = await request.get("/images/compressed/nonexistent.jpg?width=2400&quality=90");

    expect(response.status()).toBe(404);
  });

  test("compressed images should have correct content type", async ({ page, request }) => {
    // width=2400&quality=90 is a real preset, so this genuinely returns 200 — the old
    // quality=100 made it 400, leaving the content-type assertion below unreachable.
    const response = await request.get(`${FIXTURE_IMAGE}?width=2400&quality=90`);

    expect(response.status()).toBe(200);
    expect(response.headers()["content-type"]).toBe("image/webp");
  });
});
