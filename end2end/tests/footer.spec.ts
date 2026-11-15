import { test, expect, type Page } from "@playwright/test";
import { PHOTO_DETAIL_URL, visibleGrid, visiblePhotoLinks } from "./helpers";

type Watched = { footer: Element; changes: string[] };
type Probe = { marker: boolean; sameFooter: boolean; changes: string[] };

/**
 * Record every change to the footer from here on.
 *
 * The probe lives on `window`, so a full page load wipes it out and `marker` comes
 * back false: without that check, "no changes" could mean "watched nothing".
 */
async function watchFooter(page: Page) {
  await page.evaluate(() => {
    const footer = document.querySelector("footer");
    if (!footer) throw new Error("no footer to watch");
    const changes: string[] = [];
    (window as unknown as { footerProbe: Watched }).footerProbe = { footer, changes };
    new MutationObserver((records) => {
      for (const r of records) {
        changes.push(
          r.type === "attributes"
            ? `attributes ${r.attributeName}`
            : `${r.type} -> ${JSON.stringify(footer.textContent)}`
        );
      }
    }).observe(footer, { subtree: true, childList: true, characterData: true, attributes: true });
  });
}

/**
 * What the probe saw. `sameFooter` matters as much as `changes`: the observer only
 * sees the element it was given, so a footer replaced by a new one, placeholder
 * and all, records no changes.
 */
async function readProbe(page: Page): Promise<Probe> {
  return page.evaluate(() => {
    const watched = (window as unknown as { footerProbe?: Watched }).footerProbe;
    return {
      marker: watched !== undefined,
      sameFooter: watched !== undefined && document.querySelector("footer") === watched.footer,
      changes: watched?.changes ?? [],
    };
  });
}

test.describe("Footer", () => {
  test("stays as it is across client-side page changes", async ({ page }) => {
    // One footer serves the whole visit: a page change should neither touch it nor
    // swap it for a new one.
    await page.goto("/", { waitUntil: "networkidle" });
    await expect(page.locator(".footer-license a")).toBeVisible();
    await watchFooter(page);

    await page.click('nav a[href="/about"]');
    await expect(page).toHaveURL("/about");
    await expect(page.locator(".about-container")).toBeVisible();

    await page.click('nav a[href="/contact"]');
    await expect(page).toHaveURL("/contact");
    await expect(page.locator(".contact-container")).toBeVisible();

    await page.locator('nav a[href^="/gallery/"]').first().click();
    await expect(page).toHaveURL(/\/gallery\/[^/]+$/);
    await visibleGrid(page);

    await page.click("nav a.nav-brand");
    await expect(page).toHaveURL("/");
    await visibleGrid(page);

    const probe = await readProbe(page);
    expect(probe.marker, "navigation reloaded the page").toBe(true);
    expect(probe.sameFooter, "the footer was replaced").toBe(true);
    expect(probe.changes).toEqual([]);
  });

  test("is hidden on a photo page and comes back unchanged", async ({ page }) => {
    await page.goto("/", { waitUntil: "networkidle" });
    await expect(page.locator(".footer-license a")).toBeVisible();
    await watchFooter(page);

    await (await visiblePhotoLinks(page)).first().click();
    await page.waitForURL(PHOTO_DETAIL_URL, { timeout: 5000 });
    await expect(page.locator("footer")).toBeHidden();

    await page.locator(".back-link").click();
    await expect(page).not.toHaveURL(PHOTO_DETAIL_URL);
    await visibleGrid(page);
    await expect(page.locator("footer")).toBeVisible();

    // Hidden and shown again, and nothing else: no placeholder on the way back.
    const probe = await readProbe(page);
    expect(probe.marker, "navigation reloaded the page").toBe(true);
    expect(probe.sameFooter, "the footer was replaced").toBe(true);
    expect(probe.changes).toEqual(["attributes hidden", "attributes hidden"]);
  });

  // The footer used to count slashes, and hid on each of these. Reached through an
  // in-app link, so the router decides what shows, as it does for any click.
  for (const { href, shows } of [
    { href: "/gallery/home/", shows: "gallery" },
    { href: "/gallery/home//laika", shows: "not-found" },
    { href: "/gallery/home/laika/extra", shows: "not-found" },
  ]) {
    test(`shows on ${href}, which is not a photo page`, async ({ page }) => {
      await page.goto("/", { waitUntil: "networkidle" });
      await expect(page.locator("footer")).toBeVisible();
      await page.evaluate((to) => {
        const a = document.createElement("a");
        a.id = "look-alike";
        a.href = to;
        a.textContent = "look-alike";
        document.body.append(a);
      }, href);
      await page.click("#look-alike");
      await expect(page).toHaveURL(href);
      if (shows === "gallery") {
        await expect(page.locator(".hero-simple h1")).toHaveText("HOME");
      } else {
        await expect(page.locator(".not-found-page")).toBeVisible();
      }
      await expect.poll(() => page.locator("main").getAttribute("aria-busy")).toBeNull();
      await expect(page.locator("footer")).toBeVisible();
    });
  }
});
