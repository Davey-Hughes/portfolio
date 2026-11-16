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

  // The tests above aim at files the endpoint would never pick (it only looks for image
  // extensions), so they pass with or without the containment check. These targets are
  // real JPEGs one level above the images root, written by gen_fixtures: one found by
  // the endpoint's `stat`s, one with a mixed-case extension that only its folder
  // listing finds.
  test("rejects traversal to real images outside the images dir", async ({ request }) => {
    // The layout this relies on, which CI and the recipe in docs/development.md set up:
    // `/content` serves the images dir's parent. Then a photo answers with the same
    // bytes under `/content/images/` as under `/images/`, and the targets below, which
    // `/content` serves, are one level above the images dir.
    const viaImages = await request.get("/images/nature/bird.JPG");
    const viaContent = await request.get("/content/images/nature/bird.JPG");
    expect(viaContent.status(), "/content serves the images dir's parent").toBe(200);
    expect(await viaContent.body()).toEqual(await viaImages.body());
    for (const name of ["images-outside.jpg", "images-outside-mixed.Jpg"]) {
      const response = await request.get(`/content/${name}`);
      expect(response.status(), `${name} is there to aim at`).toBe(200);
    }
    for (const path of [
      "..%2Fimages-outside.jpg",
      "..%2Fimages-outside.JPG",
      "..%2Fimages-outside.Jpg",
      "..%2Fimages-outside-mixed.Jpg",
      "%2E%2E%2Fimages-outside.jpg",
      "%2e%2e%2Fimages-outside-mixed.Jpg",
      "home%2F..%2F..%2Fimages-outside.jpg",
      "home/..%2F..%2Fimages-outside-mixed.Jpg",
      "nature%2F..%2F..%2Fimages-outside-mixed.Jpg",
    ]) {
      const url = `/images/compressed/${path}?width=2400&quality=90`;
      const response = await request.get(url);
      expect(response.status(), url).toBe(404);
      expect(response.headers()["content-type"] ?? "", url).not.toContain("image/");
    }
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
