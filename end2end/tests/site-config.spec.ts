import { test, expect } from "@playwright/test";
import { PHOTO_DETAIL_URL, visibleGrid, visiblePhotoLinks } from "./helpers";

test.describe("Site config", () => {
  test("is fetched once per visit, not on every page change", async ({ page }) => {
    // Every component that made its own config resource fetched it again when it
    // was rebuilt, and showed a placeholder until the fetch landed.
    const fetches: string[] = [];
    page.on("request", (r) => {
      if (r.url().includes("/api/get_site_config")) fetches.push(new URL(page.url()).pathname);
    });

    await page.goto("/", { waitUntil: "networkidle" });
    await page.evaluate(() => ((window as unknown as { marker: boolean }).marker = true));
    fetches.length = 0;

    await page.click('nav a[href="/about"]');
    await expect(page.locator(".about-container")).toBeVisible();
    await page.click('nav a[href="/contact"]');
    await expect(page.locator(".contact-container")).toBeVisible();
    await page.click("nav a.nav-brand");
    await visibleGrid(page);
    await (await visiblePhotoLinks(page)).first().click();
    await page.waitForURL(PHOTO_DETAIL_URL, { timeout: 5000 });
    await expect(page.locator(".photo-nav-copyright")).toBeVisible();

    expect(
      await page.evaluate(() => (window as unknown as { marker?: boolean }).marker),
      "navigation reloaded the page"
    ).toBe(true);
    expect(fetches).toEqual([]);
  });

  test("keeps the photo page's copyright line steady from photo to photo", async ({ page }) => {
    await page.goto("/", { waitUntil: "networkidle" });
    await (await visiblePhotoLinks(page)).first().click();
    await page.waitForURL(PHOTO_DETAIL_URL, { timeout: 5000 });
    const line = page.locator(".photo-nav-copyright");
    await expect(line).toBeVisible();
    const copyright = (await line.textContent()) ?? "";

    // The line is rebuilt with the navigation buttons on every photo, so watch the
    // page rather than one element, and note any moment it reads differently.
    await page.evaluate((expected) => {
      const seen: string[] = [];
      (window as unknown as { copyrightSeen: string[] }).copyrightSeen = seen;
      new MutationObserver(() => {
        const text = document.querySelector(".photo-nav-copyright")?.textContent;
        if (text !== undefined && text !== expected) seen.push(JSON.stringify(text));
      }).observe(document.body, { subtree: true, childList: true, characterData: true });
    }, copyright);

    const step = (await page.locator(".nav-next").count()) > 0 ? ".nav-next" : ".nav-prev";
    const before = page.url();
    await page.locator(step).click();
    await expect(page).not.toHaveURL(before);
    await expect(page).toHaveURL(PHOTO_DETAIL_URL);
    await expect(line).toHaveText(copyright);

    const seen = await page.evaluate(
      () => (window as unknown as { copyrightSeen?: string[] }).copyrightSeen
    );
    expect(seen, "navigation reloaded the page").toBeDefined();
    expect(seen).toEqual([]);
  });
});
