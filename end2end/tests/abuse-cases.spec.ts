import { test, expect } from "@playwright/test";

// These tests pin defensive behavior of public endpoints against malformed
// or malicious input. They should fail loudly if a refactor opens an
// information-disclosure or DoS vector.

test.describe("Path traversal on /images/compressed/", () => {
  test("rejects ../ traversal that would resolve outside images dir", async ({ request }) => {
    // Encoded `../../Cargo.toml` — would resolve outside `public/images/`
    // if not blocked by canonicalize() containment check in find_image_file.
    // The params must be a real preset (2400/90 — src/image_params.rs:45). With an
    // invalid combination the handler rejects at parameter validation (main.rs:149)
    // before it ever resolves the path, so the request 400s and the containment check
    // this test exists to cover never runs. That is what quality=100 was doing here.
    const response = await request.get(
      "/images/compressed/..%2F..%2FCargo.toml?width=2400&quality=90"
    );
    expect(response.status()).toBe(404);
    // Belt and braces: whatever the status, the file's contents must not come back.
    expect(await response.text()).not.toContain("[package]");
  });

  test("rejects double-encoded traversal", async ({ request }) => {
    // %252F is double-encoded `/`. Some routers decode twice and let this
    // sneak through. Valid preset params so this reaches the path resolution
    // rather than short-circuiting at parameter validation.
    const response = await request.get(
      "/images/compressed/..%252F..%252FCargo.toml?width=2400&quality=90"
    );
    expect(response.status()).toBe(404);
    expect(await response.text()).not.toContain("[package]");
  });

  test("rejects absolute path attempts", async ({ request }) => {
    const response = await request.get(
      "/images/compressed/%2Fetc%2Fpasswd?width=2400&quality=90"
    );
    expect(response.status()).toBe(404);
    expect(await response.text()).not.toContain("root:");
  });

  test("rejects bare ../etc/passwd", async ({ request }) => {
    const response = await request.get(
      "/images/compressed/..%2F..%2F..%2Fetc%2Fpasswd?width=2400&quality=90"
    );
    expect(response.status()).toBe(404);
    expect(await response.text()).not.toContain("root:");
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
