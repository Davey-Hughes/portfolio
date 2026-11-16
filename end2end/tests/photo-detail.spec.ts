import { test, expect, type Page } from "@playwright/test";
import { PHOTO_DETAIL_URL, holdPhotos, visibleGrid, visiblePhotoLinks } from "./helpers";

test.describe("Photo Detail Page", () => {
  test("should display photo detail page", async ({ page }) => {
    await page.goto("/", { waitUntil: "networkidle" });

    // Scoped to the visible layout: every layout container holds a copy of each
    // photo, so a bare .photo-hero-link can resolve to a hidden one.
    const photos = await visiblePhotoLinks(page);
    const count = await photos.count();
    
    if (count > 0) {
      await photos.first().click();

      // Wait for navigation to photo detail
      await page.waitForURL(PHOTO_DETAIL_URL, { timeout: 5000 });

      // Photo detail container should be visible
      await expect(page.locator(".photo-detail-container")).toBeVisible();
    }
  });

  test("should display full-size image", async ({ page }) => {
    await page.goto("/", { waitUntil: "networkidle" });

    // Scoped to the visible layout: every layout container holds a copy of each
    // photo, so a bare .photo-hero-link can resolve to a hidden one.
    const photos = await visiblePhotoLinks(page);
    const count = await photos.count();
    
    if (count > 0) {
      await photos.first().click();
      await page.waitForURL(PHOTO_DETAIL_URL, { timeout: 5000 });
      
      // Wait for network to be idle to ensure image has loaded
      await page.waitForLoadState("networkidle");

      // Wait for the image to load - check it's attached to DOM and has a src
      const mainPhotoImg = page.locator(".photo-detail-image img");
      await expect(mainPhotoImg).toBeAttached({ timeout: 15000 });
      await expect(mainPhotoImg).toHaveAttribute("src", /\/images\//);
    }
  });

  test("should show EXIF data if available", async ({ page }) => {
    await page.goto("/", { waitUntil: "networkidle" });

    // Scoped to the visible layout: every layout container holds a copy of each
    // photo, so a bare .photo-hero-link can resolve to a hidden one.
    const photos = await visiblePhotoLinks(page);
    const count = await photos.count();
    
    if (count > 0) {
      await photos.first().click();
      await page.waitForURL(PHOTO_DETAIL_URL, { timeout: 5000 });

      // Check if EXIF data section exists
      const exifData = page.locator(".photo-exif");
      const exifCount = await exifData.count();
      
      if (exifCount > 0) {
        await expect(exifData).toBeVisible();
      }
    }
  });

  test("should not show footer on photo detail page", async ({ page }) => {
    await page.goto("/", { waitUntil: "networkidle" });

    // Scoped to the visible layout: every layout container holds a copy of each
    // photo, so a bare .photo-hero-link can resolve to a hidden one.
    const photos = await visiblePhotoLinks(page);
    const count = await photos.count();
    
    if (count > 0) {
      await photos.first().click();
      await page.waitForURL(PHOTO_DETAIL_URL, { timeout: 5000 });

      // Footer should not be visible on photo detail page
      const footer = page.locator("footer");
      const footerCount = await footer.count();
      
      if (footerCount > 0) {
        await expect(footer).not.toBeVisible();
      }
    }
  });

  test("should have navigation controls", async ({ page }) => {
    await page.goto("/", { waitUntil: "networkidle" });

    // Scoped to the visible layout: every layout container holds a copy of each
    // photo, so a bare .photo-hero-link can resolve to a hidden one.
    const photos = await visiblePhotoLinks(page);
    const count = await photos.count();
    
    if (count > 1) {
      await photos.first().click();
      await page.waitForURL(PHOTO_DETAIL_URL, { timeout: 5000 });

      // Check for next/previous navigation
      const navControls = page.locator(".photo-navigation");
      const navCount = await navControls.count();
      
      if (navCount > 0) {
        await expect(navControls).toBeVisible();
      }
    }
  });

  test("should support keyboard navigation", async ({ page }) => {
    await page.goto("/", { waitUntil: "networkidle" });

    // Scoped to the visible layout: every layout container holds a copy of each
    // photo, so a bare .photo-hero-link can resolve to a hidden one.
    const photos = await visiblePhotoLinks(page);
    const count = await photos.count();
    
    if (count > 1) {
      await photos.first().click();
      await page.waitForURL(PHOTO_DETAIL_URL, { timeout: 5000 });

      const currentUrl = page.url();

      // Try pressing arrow key for navigation
      await page.keyboard.press("ArrowRight");
      
      // Wait a bit for potential navigation
      await page.waitForTimeout(500);

      // URL might change if keyboard navigation is implemented
      // This is a soft check - we're just verifying nothing breaks
      await expect(page).toHaveTitle("Photography Portfolio");
    }
  });

  test("keys don't crash the app after leaving a photo page", async ({ page }) => {
    // The photo page's arrow-key listener must go when the page does: left behind,
    // it reads the page's disposed signals on every key press and panics.
    const crashes: string[] = [];
    page.on("pageerror", (error) => crashes.push(error.message));
    page.on("console", (message) => {
      if (message.text().includes("panicked")) crashes.push(message.text());
    });

    await page.goto("/", { waitUntil: "networkidle" });
    const photos = await visiblePhotoLinks(page);
    await photos.first().click();
    await page.waitForURL(PHOTO_DETAIL_URL, { timeout: 5000 });

    // Client-side, so the app (and any listener left behind) lives on.
    await page.locator(".back-link").click();
    await expect(page).not.toHaveURL(PHOTO_DETAIL_URL);
    await visibleGrid(page);

    await page.keyboard.press("ArrowRight");
    await page.keyboard.press("a");
    // Console messages arrive asynchronously; a round trip lets them land.
    await page.evaluate(() => new Promise((resolve) => setTimeout(resolve, 100)));
    expect(crashes).toEqual([]);

    // And the app still navigates.
    await (await visiblePhotoLinks(page)).first().click();
    await page.waitForURL(PHOTO_DETAIL_URL, { timeout: 5000 });
  });

  test("steps photos only on arrow keys without Alt, Ctrl or ⌘", async ({ page }) => {
    await page.goto("/", { waitUntil: "networkidle" });
    await (await visiblePhotoLinks(page)).first().click();
    await page.waitForURL(PHOTO_DETAIL_URL, { timeout: 5000 });
    await expect(page.locator(".photo-navigation")).toBeVisible();
    const url = page.url();
    const key = (await page.locator(".nav-next").count()) > 0 ? "ArrowRight" : "ArrowLeft";

    // Alt+Left is the browser's Back: stepping a photo as well would push one entry
    // just for Back to pop it. Synthetic, so nothing really goes back here.
    await page.evaluate((key) => {
      for (const modifier of ["altKey", "ctrlKey", "metaKey"]) {
        document.body.dispatchEvent(
          new KeyboardEvent("keydown", { key, [modifier]: true, bubbles: true, cancelable: true }),
        );
      }
    }, key);
    // A step to another photo is a client-side navigation; give it time to happen.
    await page.waitForTimeout(300);
    expect(page.url()).toBe(url);

    await page.keyboard.press(key);
    await expect(page).not.toHaveURL(url);
  });

  test("should support swipe gestures on mobile viewports", async ({ page }) => {
    // Set mobile viewport
    await page.setViewportSize({ width: 375, height: 667 });
    
    await page.goto("/", { waitUntil: "networkidle" });

    // Scoped to the visible layout: every layout container holds a copy of each
    // photo, so a bare .photo-hero-link can resolve to a hidden one.
    const photos = await visiblePhotoLinks(page);
    const count = await photos.count();
    
    if (count > 1) {
      await photos.first().click();
      await page.waitForURL(PHOTO_DETAIL_URL, { timeout: 5000 });

      // Photo should be visible
      const photoImage = page.locator(".photo-detail-image");
      await expect(photoImage).toBeVisible();

      // Swipe gesture would be tested here if implemented
      // For now, just verify the page renders correctly on mobile
    }
  });
});

type PhotoPageLayout = Record<"photo" | "info", number[]>;

/** The boxes of the photo and of the title and details beside (or below) it. */
async function photoPageLayout(page: Page): Promise<PhotoPageLayout> {
  return page.evaluate(() => {
    const box = (selector: string) => {
      const r = document.querySelector(selector)!.getBoundingClientRect();
      return [r.x, r.y, r.width, r.height];
    };
    return { photo: box(".photo-detail-image img"), info: box(".photo-detail-info") };
  });
}

/** Check that two layouts match to within a pixel, naming each edge that doesn't. */
function expectSameLayout(actual: PhotoPageLayout, expected: PhotoPageLayout, message: string) {
  const differences = (["photo", "info"] as const).flatMap((box) =>
    ["x", "y", "width", "height"].flatMap((edge, i) =>
      Math.abs(actual[box][i] - expected[box][i]) > 1
        ? [`${box} ${edge}: ${expected[box][i].toFixed(1)} → ${actual[box][i].toFixed(1)}`]
        : [],
    ),
  );
  expect(differences, message).toEqual([]);
}

/**
 * Check that nothing is drawn where the held photo will go, neither a shadow nor
 * alt text: a screenshot of its box, with room for the shadow, must match one taken
 * with the photo hidden.
 */
async function expectNothingDrawnInThePhotosPlace(page: Page) {
  const img = page.locator(".photo-detail-image img");
  await expect(img, "the photo's box is off screen, so this can't check it").toBeInViewport();
  const box = (await img.boundingBox())!;
  const viewport = page.viewportSize()!;
  // The shadow reaches about 70px below the photo: 10px down, then the blur.
  const margin = 80;
  const x = Math.max(0, box.x - margin);
  const y = Math.max(0, box.y - margin);
  const clip = {
    x,
    y,
    width: Math.min(viewport.width, box.x + box.width + margin) - x,
    height: Math.min(viewport.height, box.y + box.height + margin) - y,
  };
  const shown = await page.screenshot({ clip });
  await img.evaluate((i: HTMLImageElement) => (i.style.visibility = "hidden"));
  const hidden = await page.screenshot({ clip });
  await img.evaluate((i: HTMLImageElement) => (i.style.visibility = ""));
  expect(shown.equals(hidden), "something is drawn where the photo will go").toBe(true);
}

/**
 * Measure the photo page while its photo is held, let the photo through, and check
 * that nothing moved once it arrived, and that the photo ends up the size it had
 * before the page told the stylesheet its size. Unless the last photo is still on
 * screen, as it is after Next, nothing may be drawn in the photo's place meanwhile.
 */
async function expectLayoutToHoldAsThePhotoArrives(
  page: Page,
  release: () => void,
  { lastPhotoShowing = false } = {},
) {
  const img = page.locator(".photo-detail-image img");
  await expect(page.locator(".photo-detail-info h1")).toBeVisible();
  // The page's `load` waits for the held photo, so wait for the stylesheet instead.
  await page.waitForFunction(() =>
    [...document.querySelectorAll<HTMLLinkElement>("link[rel=stylesheet]")].every((l) => l.sheet),
  );
  expect(await img.evaluate((i: HTMLImageElement) => i.complete), "the photo already arrived").toBe(
    false,
  );
  const loading = await photoPageLayout(page);
  expect(loading.photo[2], "the photo's box has no width before it arrives").toBeGreaterThan(0);
  if (!lastPhotoShowing) await expectNothingDrawnInThePhotosPlace(page);

  release();
  await expect
    .poll(() => img.evaluate((i: HTMLImageElement) => i.complete && i.naturalWidth > 0))
    .toBe(true);
  const loaded = await photoPageLayout(page);
  expectSameLayout(loaded, loading, "the layout moved when the photo arrived");

  await img.evaluate((i) => i.removeAttribute("style"));
  expectSameLayout(await photoPageLayout(page), loaded, "the photo isn't the size it was before");
}

const desktop = { width: 1280, height: 720 };
const tablet = { width: 1024, height: 768 };
const phone = { width: 390, height: 844 };

// Every fixture has its size on record, so the page can make room for the photo
// before the file arrives. Without that, a slow photo starts at no size and pushes
// the title and details aside when it lands.
test.describe("Photo Detail Page while the photo loads", () => {
  for (const [label, path, viewport] of [
    ["a landscape photo", "/gallery/home/laika", desktop],
    ["a portrait photo", "/gallery/home/contrail", desktop],
    ["a landscape photo on a tablet", "/gallery/home/laika", tablet],
    ["a landscape photo on a phone", "/gallery/home/laika", phone],
    ["a portrait photo on a phone", "/gallery/home/contrail", phone],
  ] as const) {
    test(`keeps the layout still for ${label}`, async ({ page }) => {
      await page.setViewportSize(viewport);
      const release = await holdPhotos(page);
      await page.goto(path, { waitUntil: "domcontentloaded" });
      await expectLayoutToHoldAsThePhotoArrives(page, release);
    });
  }

  test("keeps the layout still after a click from the gallery", async ({ page }) => {
    await page.goto("/", { waitUntil: "networkidle" });
    const release = await holdPhotos(page);
    await (await visiblePhotoLinks(page)).first().click();
    await page.waitForURL(PHOTO_DETAIL_URL, { timeout: 5000 });
    await expectLayoutToHoldAsThePhotoArrives(page, release);
  });

  // A photo that fails to load stops sizing like an image, so its reserved width no
  // longer shrinks to fit the screen by itself.
  test("keeps a photo that fails to load within a phone's width", async ({ page }) => {
    await page.setViewportSize(phone);
    await page.route(/\/images\//, (route) => route.abort());
    await page.goto("/gallery/home/laika", { waitUntil: "domcontentloaded" });
    const img = page.locator(".photo-detail-image img");
    await expect
      .poll(() => img.evaluate((i: HTMLImageElement) => i.complete && i.naturalWidth === 0))
      .toBe(true);
    const box = (await img.boundingBox())!;
    expect(box.x, "the photo's box starts off the left edge").toBeGreaterThanOrEqual(0);
    expect(box.x + box.width, "the photo's box ends off the right edge").toBeLessThanOrEqual(
      phone.width,
    );
    const sideways = await page.evaluate(() =>
      [document.documentElement, document.querySelector(".photo-detail-page")!].map(
        (e) => e.scrollWidth - e.clientWidth,
      ),
    );
    expect(sideways, "the page scrolls sideways").toEqual([0, 0]);
  });

  // Next and Previous keep the page's <img> and change its file, so until the new
  // file arrives the browser still has the last photo's shape for it.
  for (const [label, viewport] of [
    ["", desktop],
    [" on a phone", phone],
  ] as const) {
    test(`keeps the layout still when Next goes from a landscape to a portrait${label}`, async ({
      page,
    }) => {
      await page.setViewportSize(viewport);
      await page.goto("/gallery/home/laika", { waitUntil: "networkidle" });
      // A client-side change keeps the marker, which shows Next didn't load a new page.
      await page.evaluate(() => ((window as unknown as { stayed: boolean }).stayed = true));
      const release = await holdPhotos(page);
      await page.locator(".nav-next").click();
      await expect(page.locator(".photo-detail-image img")).toHaveAttribute("src", /\/bird\./);
      // The browser keeps showing laika, fitted inside bird's box, until bird arrives.
      await expectLayoutToHoldAsThePhotoArrives(page, release, { lastPhotoShowing: true });
      expect(await page.evaluate(() => (window as unknown as { stayed?: boolean }).stayed)).toBe(
        true,
      );
    });
  }
});

// The details beside the photo sit in the space between the header and the navigation.
// On a short window they are taller than that space, and they used to spill over both
// ends of it, where the page cut them off with no way to reach them.
test.describe("Photo Detail Page on a short window", () => {
  const short = { width: 916, height: 480 };

  /** The part of the screen where the details can be seen, top and bottom. */
  function detailsView(page: Page) {
    return page.evaluate(() => {
      const area = document.querySelector(".photo-detail-content")!.getBoundingClientRect();
      const panel = document.querySelector(".photo-detail-info")!.getBoundingClientRect();
      return { top: Math.max(area.top, panel.top), bottom: Math.min(area.bottom, panel.bottom) };
    });
  }

  test("shows the whole title", async ({ page }) => {
    await page.setViewportSize(short);
    await page.goto("/gallery/home/laika", { waitUntil: "networkidle" });
    const view = await detailsView(page);
    const title = (await page.locator(".photo-detail-info h1").boundingBox())!;
    expect(title.y, "the top of the title is cut off").toBeGreaterThanOrEqual(view.top);
    expect(title.y + title.height, "the title is cut off below").toBeLessThanOrEqual(view.bottom);
  });

  test("scrolls the details to the last one", async ({ page }) => {
    await page.setViewportSize(short);
    await page.goto("/gallery/home/laika", { waitUntil: "networkidle" });
    const last = page.locator(".photo-detail-info .exif-section").last();
    await page.locator(".photo-detail-info").hover();
    await expect
      .poll(
        async () => {
          // Firefox scrolls at most a panel's height per turn of the wheel.
          await page.mouse.wheel(0, 200);
          const view = await detailsView(page);
          const box = (await last.boundingBox())!;
          // Scrolled to the end, the last detail's bottom meets the panel's to within
          // a fraction of a pixel, either way.
          return box.y >= view.top && box.y + box.height <= view.bottom + 1;
        },
        { message: "the last detail can't be scrolled into view" },
      )
      .toBe(true);
  });

  // Each edge fades by as much as is out of view beyond it, up to 2.5rem (40px). The
  // fade runs on a scroll timeline, which Firefox doesn't have; there the edge is plain.
  // Checked on a long scroll and on one shorter than 40px, where a fade that reached
  // full depth too soon would cover the title after a few pixels.
  for (const [label, viewport, range] of [
    ["a long scroll", short, (max: number) => max > 80],
    ["a short scroll", { width: 1280, height: 710 }, (max: number) => max > 0 && max < 40],
  ] as const) {
    test(`fades each edge by as much as is out of view on ${label}`, async ({ page }) => {
      await page.setViewportSize(viewport);
      await page.goto("/gallery/home/laika", { waitUntil: "networkidle" });
      const panel = page.locator(".photo-detail-info");
      test.skip(
        !(await page.evaluate(() => CSS.supports("animation-timeline: scroll()"))),
        "no scroll timelines in this browser",
      );
      const fades = (scrollTop: number) =>
        panel.evaluate((el, top) => {
          el.scrollTop = top;
          const max = el.scrollHeight - el.clientHeight;
          // The timeline catches up with the scroll position by the next frame.
          return new Promise<number[]>((done) =>
            requestAnimationFrame(() =>
              requestAnimationFrame(() => {
                const style = getComputedStyle(el);
                done([
                  parseFloat(style.getPropertyValue("--photo-info-fade-top")),
                  parseFloat(style.getPropertyValue("--photo-info-fade-bottom")),
                  max,
                  // Where the browser scrolled to, which can be a whole pixel
                  // rather than the half asked for.
                  el.scrollTop,
                ]);
              }),
            ),
          );
        }, scrollTop);
      const [, , max] = await fades(0);
      expect(range(max), `the details scroll ${max}px, the wrong distance to test`).toBe(true);
      for (const asked of [0, 10, max / 2, max - 10, max].filter((t) => t >= 0 && t <= max)) {
        const [fadeTop, fadeBottom, , scrollTop] = await fades(asked);
        expect(fadeTop, `top fade at scrollTop ${scrollTop}`).toBeCloseTo(
          Math.min(scrollTop, 40),
          0,
        );
        expect(fadeBottom, `bottom fade at scrollTop ${scrollTop}`).toBeCloseTo(
          Math.min(max - scrollTop, 40),
          0,
        );
      }
    });
  }

  // Next and Previous keep the page's elements, the details panel among them, so it
  // would keep its scroll position from the last photo.
  test("shows the next photo's title after scrolling to the end", async ({ page }) => {
    await page.setViewportSize(short);
    await page.goto("/gallery/home/laika", { waitUntil: "networkidle" });
    await page
      .locator(".photo-detail-info")
      .evaluate((panel) => (panel.scrollTop = panel.scrollHeight));
    await page.keyboard.press("ArrowRight");
    await expect(page).not.toHaveURL(/\/laika$/);
    const title = page.locator(".photo-detail-info h1");
    await expect(title).not.toHaveText(/laika/i);
    await expect
      .poll(async () => (await title.boundingBox())!.y - (await detailsView(page)).top, {
        message: "the title is scrolled out of view",
      })
      .toBeGreaterThanOrEqual(0);
  });

  // When the details scroll, the panel is as tall as the photo may be, so it lines up
  // with a photo that reaches that height, as a portrait one does. Below 1025px wide
  // the photo may reach into the padding around it, and the panel does too.
  for (const viewport of [
    { width: 1280, height: 600 },
    { width: 916, height: 551 },
  ]) {
    test(`lines the details up with the photo at ${viewport.width}×${viewport.height}`, async ({
      page,
    }) => {
      await page.setViewportSize(viewport);
      await page.goto("/gallery/home/contrail", { waitUntil: "networkidle" });
      const scrolls = await page
        .locator(".photo-detail-info")
        .evaluate((panel) => panel.scrollHeight > panel.clientHeight);
      expect(scrolls, "the details don't scroll here, so there's nothing to line up").toBe(true);
      const view = await detailsView(page);
      const photo = (await page.locator(".photo-detail-image img").boundingBox())!;
      expect(Math.abs(view.top - photo.y), "the details' top isn't level").toBeLessThanOrEqual(1);
      expect(
        Math.abs(view.bottom - (photo.y + photo.height)),
        "the details' bottom isn't level",
      ).toBeLessThanOrEqual(1);
    });
  }

  test("still centres the details when they fit", async ({ page }) => {
    await page.setViewportSize({ width: 1280, height: 800 });
    await page.goto("/gallery/home/laika", { waitUntil: "networkidle" });
    const area = (await page.locator(".photo-detail-content").boundingBox())!;
    const title = (await page.locator(".photo-detail-info h1").boundingBox())!;
    const last = (await page.locator(".photo-detail-info .exif-section").last().boundingBox())!;
    const middle = (title.y + last.y + last.height) / 2;
    expect(
      Math.abs(middle - (area.y + area.height / 2)),
      "the details aren't centred beside the photo",
    ).toBeLessThanOrEqual(1);
  });
});
