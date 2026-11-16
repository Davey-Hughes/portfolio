import { test, expect } from "@playwright/test";
import { visibleGrid } from "./helpers";

// A page address with a trailing slash is redirected to the one without it. Before,
// the server answered all of these 404: with the page itself showing for a page
// that exists, and with the not-found page, naming the slashed address, for a
// missing one such as /gallery/nope/. ssr_tests in src/app.rs cover the cases;
// these check the server as src/main.rs wires it, file mounts and all.

test.describe("Trailing slash", () => {
  test("a page address with one redirects permanently, query kept", async ({ request }) => {
    for (const [from, to] of [
      ["/gallery/home/", "/gallery/home"],
      ["/gallery/home/laika/", "/gallery/home/laika"],
      ["/gallery/nope/", "/gallery/nope"],
      ["/about/", "/about"],
      ["/open-source/?from=test", "/open-source?from=test"],
    ]) {
      const response = await request.get(from, { maxRedirects: 0 });
      expect(response.status(), from).toBe(308);
      expect(response.headers()["location"], from).toBe(to);
    }
  });

  test("a browser lands on the page at its own address", async ({ page }) => {
    const response = await page.goto("/gallery/home/", { waitUntil: "networkidle" });
    expect(response?.status()).toBe(200);
    await expect(page).toHaveURL("/gallery/home");
    await visibleGrid(page);
    await expect(page.locator("footer")).toBeVisible();
  });

  // /pkg is served by the Leptos fallback, the other two by file mounts. /readme's
  // own directory 404s are in readme-assets.spec.ts. A directory on a file mount is
  // a 404 without its slash too. The file server used to redirect /images/home to
  // /home/, because the mount hands it the path with /images already taken off.
  // /content/readme is the folder gen_fixtures writes the /readme fixtures into.
  test("/pkg and the file mounts are left alone", async ({ request }) => {
    for (const path of [
      "/pkg/",
      "/images/home/",
      "/images/home",
      "/content/",
      "/content/readme",
    ]) {
      const response = await request.get(path, { maxRedirects: 0 });
      expect(response.status(), path).toBe(404);
      expect(response.headers()["location"], path).toBeUndefined();
    }
  });
});
