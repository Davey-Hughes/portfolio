import { test, expect } from "@playwright/test";

test.describe("Image Parameter Validation", () => {
  test("should serve compressed images with valid width parameter", async ({ page, request }) => {
    // Test valid width parameter (2400 is a valid preset)
    const response = await request.get("/images/compressed/test.jpg?width=2400");
    
    // Should return either 200 (if image exists) or 404 (if image doesn't exist)
    // But NOT 400 (bad request) since 2400 is a valid width
    expect([200, 404]).toContain(response.status());
  });

  test("should reject invalid width parameter", async ({ page, request }) => {
    // Test invalid width parameter (1500 is not in the valid presets)
    const response = await request.get("/images/compressed/test.jpg?width=1500");
    
    // Should return 400 Bad Request for invalid width
    expect(response.status()).toBe(400);
    
    const body = await response.text();
    expect(body).toContain("Invalid width");
  });

  test("should serve compressed images with valid width and quality combination", async ({ page, request }) => {
    // Test valid combination (3600, 100 is a valid preset)
    const response = await request.get("/images/compressed/test.jpg?width=3600&quality=100");
    
    // Should return either 200 (if image exists) or 404 (if image doesn't exist)
    // But NOT 400 (bad request) since this is a valid combination
    expect([200, 404]).toContain(response.status());
  });

  test("should reject invalid width and quality combination", async ({ page, request }) => {
    // Test invalid combination (1200, 100 is not a valid preset)
    const response = await request.get("/images/compressed/test.jpg?width=1200&quality=100");
    
    // Should return 400 Bad Request for invalid combination
    expect(response.status()).toBe(400);
    
    const body = await response.text();
    expect(body).toContain("Invalid width/quality combination");
  });

  test("should reject quality parameter without width", async ({ page, request }) => {
    // Test quality only (not allowed)
    const response = await request.get("/images/compressed/test.jpg?quality=90");
    
    // Should return 400 Bad Request
    expect(response.status()).toBe(400);
    
    const body = await response.text();
    expect(body).toContain("Quality must be specified with a width");
  });

  test("should use default parameters when none specified", async ({ page, request }) => {
    // Test no parameters - should use defaults
    const response = await request.get("/images/compressed/test.jpg");
    
    // Should return either 200 (if image exists) or 404 (if image doesn't exist)
    // But NOT 400 (bad request) since defaults should be valid
    expect([200, 404]).toContain(response.status());
  });

  test("should accept all valid preset widths", async ({ page, request }) => {
    const validWidths = [1200, 2400, 3600];
    
    for (const width of validWidths) {
      const response = await request.get(`/images/compressed/test.jpg?width=${width}`);
      
      // Should return either 200 or 404, but not 400
      expect([200, 404]).toContain(response.status());
    }
  });

  test("should accept valid preset combinations", async ({ page, request }) => {
    const validCombinations = [
      { width: 1200, quality: 80 },
      { width: 2400, quality: 100 },
      { width: 3600, quality: 100 },
    ];
    
    for (const combo of validCombinations) {
      const response = await request.get(
        `/images/compressed/test.jpg?width=${combo.width}&quality=${combo.quality}`
      );
      
      // Should return either 200 or 404, but not 400
      expect([200, 404]).toContain(response.status());
    }
  });
});

test.describe("Image Parameter Environment Variables", () => {
  test("should load images with default parameters on gallery pages", async ({ page }) => {
    await page.goto("/", { waitUntil: "networkidle" });
    
    // Wait for photo grid to load
    await page.waitForSelector(".photo-grid-home", { timeout: 10000 });
    
    // Find all image elements in the grid
    const images = page.locator(".photo-grid-home img");
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
      
      // Wait for photos to load
      await page.waitForSelector(".photo-grid-home", { timeout: 10000 });
      
      // Find all image elements
      const images = page.locator(".photo-grid-home img");
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
    const response = await request.get("/images/compressed/nonexistent.jpg?width=2400&quality=100");
    
    // Should return 404 for nonexistent image
    expect(response.status()).toBe(404);
  });

  test("compressed images should have correct content type", async ({ page, request }) => {
    // Make a request to compressed image endpoint
    const response = await request.get("/images/compressed/test.jpg?width=2400&quality=100");
    
    // If the image exists (200), check content type
    if (response.status() === 200) {
      const contentType = response.headers()["content-type"];
      expect(contentType).toBe("image/webp");
    }
  });
});
