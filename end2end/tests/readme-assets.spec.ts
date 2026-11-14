import { test, expect } from "@playwright/test";

// /readme/<repo>/<file> serves screenshots and clips for the owner's repos' READMEs
// from README_ASSETS_DIR. gen_fixtures writes target/e2e-fixtures/readme/fixture/ with
// two allowed files (shot.webp, clip.mp4), one refused file (notes.toml), and a
// directory named like an image (album.webp/, holding an index.html) so the directory
// check cannot lean on the extension allowlist. It also writes readme-outside.webp
// beside readme/, as a target for the traversal test.

const SHOT = "/readme/fixture/shot.webp";

test.describe("README assets", () => {
  test("serves an allowed image with revalidate-every-load headers", async ({ request }) => {
    const response = await request.get(SHOT);
    expect(response.status()).toBe(200);
    const headers = response.headers();
    expect(headers["content-type"]).toBe("image/webp");
    expect(headers["cache-control"]).toBe("no-cache");
    expect(headers["x-content-type-options"]).toBe("nosniff");
    expect(headers["last-modified"]).toBeTruthy();
  });

  test("answers 304 when the file is unchanged", async ({ request }) => {
    const first = await request.get(SHOT);
    expect(first.status()).toBe(200);
    const lastModified = first.headers()["last-modified"];
    expect(lastModified).toBeTruthy();

    const second = await request.get(SHOT, {
      headers: { "If-Modified-Since": lastModified },
    });
    expect(second.status()).toBe(304);
  });

  test("serves video without compressing it", async ({ request }) => {
    // The app compresses responses router-wide. A video is already compressed, so
    // gzipping it again only burns CPU and drops Content-Length and Accept-Ranges.
    const response = await request.get("/readme/fixture/clip.mp4", {
      headers: { "Accept-Encoding": "gzip, br" },
    });
    expect(response.status()).toBe(200);
    const headers = response.headers();
    expect(headers["content-type"]).toBe("video/mp4");
    expect(headers["content-encoding"]).toBeUndefined();
    // What compressing it would cost: the length, and the range support seeking needs.
    expect(headers["content-length"]).toBe("1024");
    expect(headers["accept-ranges"]).toBe("bytes");
  });

  test("judges the decoded name, as the file server does", async ({ request }) => {
    // %2E is an encoded `.`. The file server decodes it and opens shot.webp, so the
    // allowlist must decode it too, or it would refuse a file that is allowed.
    const response = await request.get("/readme/fixture/shot%2Ewebp");
    expect(response.status()).toBe(200);
    expect(response.headers()["content-type"]).toBe("image/webp");
  });

  test("refuses files outside the allowlist", async ({ request }) => {
    // The encoded form must stay refused too. (An allowlist that never decoded would
    // also refuse it, having no `.` to find; the decoded-name test above is what
    // proves the decoding.)
    for (const path of ["/readme/fixture/notes.toml", "/readme/fixture/notes%2Etoml"]) {
      const response = await request.get(path);
      expect(response.status(), path).toBe(404);
      expect(await response.text(), path).not.toContain("readme-notes-fixture");
    }
  });

  test("refuses directories and missing files", async ({ request }) => {
    for (const path of [
      "/readme/fixture/",
      "/readme/fixture",
      "/readme/fixture/album.webp",
      "/readme/fixture/album.webp/",
      "/readme/fixture/missing.webp",
    ]) {
      const response = await request.get(path, { maxRedirects: 0 });
      expect(response.status(), path).toBe(404);
      expect(await response.text(), path).not.toContain("readme-album-fixture");
    }
  });

  test("never serves a file from outside its folder", async ({ request }) => {
    // readme-outside.webp sits beside README_ASSETS_DIR (target/e2e-fixtures/readme),
    // in the content dir that /content serves. Fetching it there first proves the file
    // exists, so the 404 below comes from the refusal, not from a missing target.
    const control = await request.get("/content/readme-outside.webp");
    expect(control.status()).toBe(200);

    // Both names pass the allowlist, and both targets exist: two `..` up from
    // readme/fixture/ is readme-outside.webp, and album.webp/../shot.webp is
    // fixture/shot.webp. Only the file server's refusal of `..` keeps them out.
    for (const path of [
      "/readme/fixture/..%2F..%2Freadme-outside.webp",
      "/readme/fixture/album.webp%2F..%2Fshot.webp",
    ]) {
      const response = await request.get(path, { maxRedirects: 0 });
      expect(response.status(), path).toBe(404);
      expect(response.headers()["content-type"] ?? "", path).not.toContain("image/");
    }
  });
});
