import { test, expect, type Page } from "@playwright/test";
import { PHOTO_DETAIL_URL, visibleGrid } from "./helpers";

// Not-found cases the e2e fixtures can produce. The fourth, /open-source without a
// usable opensource.toml, is covered by ssr_tests in src/app.rs: gen_fixtures always
// writes that file.
const MISSING = [
  { path: "/no-such-page", why: "unknown address" },
  { path: "/gallery/nope", why: "missing gallery" },
  { path: "/gallery/home/nope", why: "missing photo" },
];

// gen_fixtures' home gallery, slug -> title (the filename with dashes as spaces).
const HOME_PHOTOS: Record<string, string> = {
  laika: "laika",
  "blue-angels": "blue angels",
  contrail: "contrail",
};

/** Assert the not-found page for `path` is showing; return its photo link's href. */
async function expectNotFoundPage(page: Page, path: string): Promise<string> {
  const nf = page.locator(".not-found-page");
  await expect(nf.locator(".not-found-label")).toHaveText("404");
  await expect(nf.locator("h1")).toHaveText("Page not found");
  await expect(nf.locator(".not-found-path")).toHaveText(path);
  const back = nf.locator(".not-found-back a");
  await expect(back).toHaveText("Back to the galleries");
  await expect(back).toHaveAttribute("href", "/");
  const photoLink = nf.locator(".not-found-photo a");
  const href = (await photoLink.getAttribute("href")) ?? "";
  const slug = href.match(/^\/gallery\/home\/([^/]+)$/)?.[1] ?? "";
  expect(Object.keys(HOME_PHOTOS)).toContain(slug);
  await expect(photoLink.locator("img")).toBeVisible();
  await expect(photoLink.locator("img")).toHaveAttribute("src", /^\/images\/compressed\/home\//);
  await expect(nf.locator(".not-found-photo figcaption")).toHaveText(HOME_PHOTOS[slug]);
  await expect(page).toHaveTitle("Photography Portfolio");
  // Never inside .photo-detail-page, whose fixed height and overflow: hidden would
  // clip it. Checked directly: scrolling into view can scroll that wrapper itself,
  // so "in the viewport" alone would pass even when clipped.
  await expect(page.locator(".photo-detail-page .not-found-page")).toHaveCount(0);
  await back.scrollIntoViewIfNeeded();
  await expect(back).toBeInViewport();
  return href;
}

test.describe("Not found page", () => {
  for (const { path, why } of MISSING) {
    test(`${why} answers 404`, async ({ request }) => {
      expect((await request.get(path)).status()).toBe(404);
    });

    test(`${why}: message beside a home photo at desktop width`, async ({ page }) => {
      await page.setViewportSize({ width: 1280, height: 720 });
      const photoFetches: string[] = [];
      page.on("request", (r) => {
        if (r.url().includes("/api/random_home_photo")) photoFetches.push(r.url());
      });
      const res = await page.goto(path, { waitUntil: "networkidle" });
      expect(res?.status()).toBe(404);
      const href = await expectNotFoundPage(page, path);
      // Hydration kept the photo the server picked (Review Focus 1): the same link
      // as in the server's HTML, and no second pick fetched by the browser.
      expect(await res!.text()).toContain(`href="${href}"`);
      expect(photoFetches).toEqual([]);
      const photo = (await page.locator(".not-found-photo").boundingBox())!;
      const text = (await page.locator(".not-found-text").boundingBox())!;
      expect(photo.x + photo.width).toBeLessThanOrEqual(text.x);
    });
  }

  test("phone width: photo above the message, long path wraps, no horizontal scroll", async ({ page }) => {
    await page.setViewportSize({ width: 375, height: 667 });
    const path = `/no-such-page-${"x".repeat(120)}`;
    await page.goto(path, { waitUntil: "networkidle" });
    await expectNotFoundPage(page, path);
    const photo = (await page.locator(".not-found-photo").boundingBox())!;
    const text = (await page.locator(".not-found-text").boundingBox())!;
    expect(photo.y + photo.height).toBeLessThanOrEqual(text.y);
    expect(text.x + text.width).toBeLessThanOrEqual(375);
    const overflows = await page.evaluate(
      () => document.documentElement.scrollWidth > document.documentElement.clientWidth,
    );
    expect(overflows).toBe(false);
  });

  test("the photo opens its own page", async ({ page }) => {
    await page.goto("/no-such-page", { waitUntil: "networkidle" });
    // A client-side change keeps the marker, which shows the server-rendered page
    // hydrated: without it the click would be a full page load.
    await page.evaluate(() => ((window as unknown as { stayed: boolean }).stayed = true));
    await page.locator(".not-found-photo a").click();
    await expect(page).toHaveURL(PHOTO_DETAIL_URL);
    await expect(page.locator(".photo-detail-image img")).toBeVisible();
    await expect(page).toHaveTitle("Photography Portfolio");
    expect(await page.evaluate(() => (window as unknown as { stayed?: boolean }).stayed)).toBe(true);
  });

  // Review Focus 2: client-side fallback; the photo comes through /api, not SSR.
  test("an in-app link to a dead address shows the page with a photo", async ({ page }) => {
    await page.goto("/", { waitUntil: "networkidle" });
    // The marker only survives a client-side change; a full page load would also
    // show the not-found page, and prove nothing about this path.
    await page.evaluate(() => {
      (window as unknown as { stayed: boolean }).stayed = true;
      const a = document.createElement("a");
      a.href = "/dead-link";
      a.id = "dead-link";
      a.textContent = "dead link";
      document.body.append(a);
    });
    const photoFetched = page.waitForResponse(/\/api\/random_home_photo/);
    await page.click("#dead-link");
    expect((await photoFetched).ok()).toBe(true);
    await expect(page).toHaveURL("/dead-link");
    await expectNotFoundPage(page, "/dead-link");
    expect(await page.evaluate(() => (window as unknown as { stayed?: boolean }).stayed)).toBe(true);
  });

  test("pages that exist still answer 200", async ({ request }) => {
    for (const path of ["/", "/gallery/nature", "/gallery/home", "/gallery/home/laika", "/open-source", "/about"]) {
      expect((await request.get(path)).status(), path).toBe(200);
    }
  });

  // A missing gallery reached from another gallery is a param change: it goes
  // through GalleryPage's Transition, not the router.
  test("a link from a gallery to a missing gallery shows the not-found page", async ({ page }) => {
    await page.goto("/gallery/nature", { waitUntil: "networkidle" });
    await visibleGrid(page);
    await page.evaluate(() => {
      const a = document.createElement("a");
      a.id = "to-nope";
      a.href = "/gallery/nope";
      a.textContent = "nope";
      document.body.append(a);
    });
    await page.click("#to-nope");
    await expectNotFoundPage(page, "/gallery/nope");
    await expect(page.locator(".hero-simple")).toHaveCount(0);
    await expect.poll(() => page.locator("main").getAttribute("aria-busy")).toBeNull();
    await expect(page.locator("footer")).toBeVisible();
  });
});
