import { test, expect, type Page } from "@playwright/test";
import { PHOTO_DETAIL_URL, holdApi, visibleGrid, visiblePhotoLinks } from "./helpers";

/** Where the footer's top edge sits relative to the window's bottom edge. */
async function footerBelowWindow(page: Page): Promise<boolean> {
  return page.evaluate(
    () => (document.querySelector("footer")?.getBoundingClientRect().top ?? 0) >= innerHeight
  );
}

/**
 * Give a page change a moment to show anything it is going to show, then check
 * once. A retrying assertion would pass the moment its condition held, even if
 * the new page appeared a few milliseconds later.
 */
async function settle(page: Page) {
  await page.waitForTimeout(250);
}

test.describe("Page loading", () => {
  test("keeps the old page until the new one is ready", async ({ page }) => {
    // Home is tall enough to scroll and keep the footer out of view; the fixture
    // Contact page is not.
    await page.goto("/", { waitUntil: "networkidle" });
    await visibleGrid(page);
    await page.evaluate(() => window.scrollTo(0, 300));
    const scrolled = await page.evaluate(() => window.scrollY);
    expect(scrolled).toBe(300);

    const release = await holdApi(page);
    const requested = page.waitForRequest(/\/api\//, { timeout: 5000 });
    await page.click('nav a[href="/about"]');
    await requested;

    await settle(page);
    expect(await page.locator("main > .home-page").isVisible()).toBe(true);
    expect(await page.locator(".about-page").count()).toBe(0);
    expect(await page.evaluate(() => window.scrollY)).toBe(scrolled);
    expect(await footerBelowWindow(page), "the footer came into view").toBe(true);

    release();
    await expect(page.locator(".about-page")).toBeVisible();
  });

  test("waits for a gallery too", async ({ page }) => {
    await page.goto("/contact", { waitUntil: "networkidle" });

    const release = await holdApi(page);
    const requested = page.waitForRequest(/\/api\//, { timeout: 5000 });
    await page.locator('nav a[href^="/gallery/"]').first().click();
    await requested;

    await settle(page);
    expect(await page.locator(".contact-page").isVisible()).toBe(true);
    expect(await page.locator("main .home-page").count()).toBe(0);

    release();
    await visibleGrid(page);
  });

  test("waits when moving between galleries", async ({ page }) => {
    // A gallery-to-gallery change is a param change on one route. It used to swap
    // the title and keep the old gallery's photos, without fetching anything.
    await page.goto("/gallery/nature", { waitUntil: "networkidle" });
    await visibleGrid(page);
    const title = page.locator(".hero-simple h1");
    await expect(title).toHaveText("NATURE");

    // Only `nature` is in the fixtures' nav, so link to `home` by hand; the router
    // takes clicks on any same-origin anchor.
    await page.evaluate(() => {
      const link = document.createElement("a");
      link.id = "to-home";
      link.href = "/gallery/home";
      link.textContent = "home";
      document.body.append(link);
    });

    const release = await holdApi(page);
    const requested = page.waitForRequest(/\/api\//, { timeout: 5000 });
    await page.click("#to-home");
    await requested;

    // The title and the photos must always be one gallery's: the old ones while the
    // next gallery loads, the new ones after.
    const photoLinks = async () =>
      (await visibleGrid(page))
        .locator(".photo-hero-link")
        .evaluateAll((as) => as.map((a) => a.getAttribute("href") ?? ""));
    const outside = (links: string[], gallery: string) =>
      links.filter((href) => !href.startsWith(`/gallery/${gallery}/`));

    await settle(page);
    expect(await title.textContent()).toBe("NATURE");
    const during = await photoLinks();
    expect(during.length).toBeGreaterThan(0);
    expect(outside(during, "nature")).toEqual([]);

    release();
    await expect(title).toHaveText("HOME");
    const after = await photoLinks();
    expect(after.length).toBeGreaterThan(0);
    expect(outside(after, "home")).toEqual([]);
  });

  test("keeps the footer on a gallery until the photo page shows", async ({ page }) => {
    // The footer hides on photo pages. Hiding it at the click, while the gallery is
    // still on screen, shortens the page and shifts it under the reader.
    await page.goto("/gallery/nature", { waitUntil: "networkidle" });
    const photos = await visiblePhotoLinks(page);
    await page.evaluate(() => window.scrollTo(0, document.body.scrollHeight));
    const scrolled = await page.evaluate(() => window.scrollY);
    // Otherwise the scroll check below proves nothing.
    expect(scrolled).toBeGreaterThan(0);
    const footer = page.locator("footer");
    await expect(footer).toBeVisible();

    const release = await holdApi(page);
    const requested = page.waitForRequest(/\/api\//, { timeout: 5000 });
    // A DOM click, so Playwright doesn't scroll the link into view first.
    await photos.first().evaluate((a) => (a as HTMLElement).click());
    await requested;

    await settle(page);
    expect(await footer.isVisible(), "the footer hid before the photo page showed").toBe(true);
    expect(await page.evaluate(() => window.scrollY)).toBe(scrolled);

    release();
    await page.waitForURL(PHOTO_DETAIL_URL, { timeout: 5000 });
    await expect(footer).toBeHidden();
  });

  test("keeps the footer hidden on a photo page until the gallery shows", async ({ page }) => {
    await page.goto("/", { waitUntil: "networkidle" });
    await (await visiblePhotoLinks(page)).first().click();
    await page.waitForURL(PHOTO_DETAIL_URL, { timeout: 5000 });
    const footer = page.locator("footer");
    await expect(footer).toBeHidden();

    const release = await holdApi(page);
    const requested = page.waitForRequest(/\/api\//, { timeout: 5000 });
    await page.locator(".back-link").click();
    await requested;

    await settle(page);
    expect(await footer.isVisible(), "the footer showed under the photo page").toBe(false);

    release();
    await visibleGrid(page);
    await expect(footer).toBeVisible();
  });

  test("keeps the footer through a page change that overtakes another", async ({ page }) => {
    // A second click while the first page is still loading: the router reports the
    // first one finished as soon as it is abandoned, while the second is still
    // loading. The footer must still wait for the page that actually arrives.
    await page.goto("/gallery/nature", { waitUntil: "networkidle" });
    const photos = await visiblePhotoLinks(page);
    const footer = page.locator("footer");
    await expect(footer).toBeVisible();

    const release = await holdApi(page);
    let requested = page.waitForRequest(/\/api\//, { timeout: 5000 });
    await page.click('nav a[href="/about"]');
    await requested;
    requested = page.waitForRequest(/get_all_gallery_photos/, { timeout: 5000 });
    await photos.first().evaluate((a) => (a as HTMLElement).click());
    await requested;

    await settle(page);
    expect(await page.locator(".photo-detail-page").count()).toBe(0);
    expect(await footer.isVisible(), "the footer hid before the photo page showed").toBe(true);

    release();
    await page.waitForURL(PHOTO_DETAIL_URL, { timeout: 5000 });
    await expect(footer).toBeHidden();
  });
});

/** `<main>`'s aria-busy: "true" while a page change waits, absent otherwise. */
async function mainBusy(page: Page): Promise<string | null> {
  return page.locator("main").getAttribute("aria-busy");
}

/** Click, and resolve once the click has sent a server-function request. */
async function clickAndWaitForApi(page: Page, selector: string, api: RegExp = /\/api\//) {
  const requested = page.waitForRequest(api, { timeout: 5000 });
  await page.locator(selector).first().click();
  await requested;
}

/** Add a link to /gallery/home, which the fixtures leave out of the nav. */
async function addHomeGalleryLink(page: Page) {
  await page.evaluate(() => {
    const link = document.createElement("a");
    link.id = "to-home";
    link.href = "/gallery/home";
    link.textContent = "home";
    document.body.append(link);
  });
}

// Nothing visible marks a wait; screen readers are told through aria-busy.
test.describe("Busy state", () => {
  test("marks the main area busy while a page loads", async ({ page }) => {
    await page.goto("/contact", { waitUntil: "networkidle" });
    expect(await mainBusy(page)).toBeNull();
    const release = await holdApi(page);
    await clickAndWaitForApi(page, 'nav a[href="/about"]');

    await expect.poll(() => mainBusy(page)).toBe("true");

    release();
    await expect(page.locator(".about-page")).toBeVisible();
    await expect.poll(() => mainBusy(page)).toBeNull();
  });

  test("ends when a quick page overtakes a slow one", async ({ page }) => {
    await page.goto("/", { waitUntil: "networkidle" });
    await visibleGrid(page);
    const release = await holdApi(page);
    await clickAndWaitForApi(page, 'nav a[href="/about"]');
    await expect.poll(() => mainBusy(page)).toBe("true");

    // Contact needs nothing new from the server, so the router finishes there at once.
    await page.click('nav a[href="/contact"]');
    await expect(page.locator(".contact-page")).toBeVisible();
    await expect.poll(() => mainBusy(page)).toBeNull();
    release();
  });

  test("stays busy while an overtaking page is still loading", async ({ page }) => {
    await page.goto("/contact", { waitUntil: "networkidle" });
    const release = await holdApi(page);
    await clickAndWaitForApi(page, 'nav a[href="/about"]');
    await clickAndWaitForApi(page, 'nav a[href^="/gallery/"]', /get_gallery_data/);

    await settle(page);
    expect(await mainBusy(page), "busy ended before the gallery arrived").toBe("true");
    expect(await page.locator(".contact-page").isVisible()).toBe(true);

    release();
    await visibleGrid(page);
    await expect.poll(() => mainBusy(page)).toBeNull();
  });

  test("marks a move between galleries busy", async ({ page }) => {
    // One gallery to another is a param change, which the router doesn't treat as
    // a page change; the gallery page reports its own loading.
    await page.goto("/gallery/nature", { waitUntil: "networkidle" });
    await visibleGrid(page);
    await addHomeGalleryLink(page);
    const release = await holdApi(page);
    await clickAndWaitForApi(page, "#to-home");

    await expect.poll(() => mainBusy(page)).toBe("true");

    release();
    await expect(page.locator(".hero-simple h1")).toHaveText("HOME");
    await expect.poll(() => mainBusy(page)).toBeNull();
  });

  test("isn't left busy when the new page needs nothing loaded", async ({ page }) => {
    // Contact reads only the shared site config, so it appears at once.
    await page.goto("/about", { waitUntil: "networkidle" });
    await page.click('nav a[href="/contact"]');
    await expect(page.locator(".contact-page")).toBeVisible();
    await expect.poll(() => mainBusy(page)).toBeNull();
  });

  test("clears when leaving a gallery that is still loading", async ({ page }) => {
    await page.goto("/gallery/nature", { waitUntil: "networkidle" });
    await visibleGrid(page);
    await addHomeGalleryLink(page);
    const release = await holdApi(page);
    await clickAndWaitForApi(page, "#to-home");
    await expect.poll(() => mainBusy(page)).toBe("true");

    // Contact needs nothing new, so the gallery goes away mid-load.
    await page.click('nav a[href="/contact"]');
    await expect(page.locator(".contact-page")).toBeVisible();
    await expect.poll(() => mainBusy(page)).toBeNull();
    release();
  });
});
