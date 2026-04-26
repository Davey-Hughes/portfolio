import { test, expect } from "@playwright/test";

// These tests pin defensive behavior of public endpoints against malformed
// or malicious input. They should fail loudly if a refactor opens an
// information-disclosure or DoS vector.

test.describe("Path traversal on /images/compressed/", () => {
  test("rejects ../ traversal that would resolve outside images dir", async ({ request }) => {
    // Encoded `../../Cargo.toml` — would resolve outside `public/images/`
    // if not blocked by canonicalize() containment check in find_image_file.
    const response = await request.get(
      "/images/compressed/..%2F..%2FCargo.toml?width=2400&quality=100"
    );
    expect(response.status()).toBe(404);
  });

  test("rejects double-encoded traversal", async ({ request }) => {
    // %252F is double-encoded `/`. Some routers decode twice and let this
    // sneak through. We expect a hard reject (400 or 404), never 200.
    const response = await request.get(
      "/images/compressed/..%252F..%252FCargo.toml?width=2400&quality=100"
    );
    expect([400, 404]).toContain(response.status());
  });

  test("rejects absolute path attempts", async ({ request }) => {
    const response = await request.get(
      "/images/compressed/%2Fetc%2Fpasswd?width=2400&quality=100"
    );
    expect([400, 404]).toContain(response.status());
  });

  test("rejects bare ../etc/passwd", async ({ request }) => {
    const response = await request.get(
      "/images/compressed/..%2F..%2F..%2Fetc%2Fpasswd?width=2400&quality=100"
    );
    expect([400, 404]).toContain(response.status());
  });
});

test.describe("Image parameter abuse", () => {
  test("rejects extremely large width", async ({ request }) => {
    const response = await request.get(
      "/images/compressed/test.jpg?width=99999&quality=80"
    );
    expect(response.status()).toBe(400);
  });

  test("rejects negative width", async ({ request }) => {
    // Negative numbers fail Query<u32> deserialization.
    const response = await request.get(
      "/images/compressed/test.jpg?width=-1&quality=80"
    );
    expect([400, 422]).toContain(response.status());
  });

  test("rejects non-numeric width", async ({ request }) => {
    const response = await request.get(
      "/images/compressed/test.jpg?width=abc&quality=80"
    );
    expect([400, 422]).toContain(response.status());
  });

  test("rejects out-of-range quality", async ({ request }) => {
    const response = await request.get(
      "/images/compressed/test.jpg?width=2400&quality=200"
    );
    expect(response.status()).toBe(400);
  });
});

test.describe("Invalid gallery / photo routes", () => {
  test("non-existent gallery slug shows error UI without 5xx", async ({ page }) => {
    const response = await page.goto("/gallery/this-gallery-does-not-exist-xyz", {
      waitUntil: "networkidle",
    });
    // SSR returns 200 with the error markup (Leptos resource error path).
    // We just want to confirm we don't 500 and the page renders something.
    expect(response?.status()).toBeLessThan(500);
    const bodyText = await page.locator("body").textContent();
    expect(bodyText).toBeTruthy();
  });

  test("non-existent photo slug shows fallback without crashing", async ({ page }) => {
    const response = await page.goto(
      "/gallery/nature/this-photo-does-not-exist-xyz",
      { waitUntil: "networkidle" }
    );
    expect(response?.status()).toBeLessThan(500);
    // The PhotoNotFound component renders "Photo not found" or a back link.
    const bodyText = await page.locator("body").textContent();
    expect(bodyText).toBeTruthy();
  });
});
