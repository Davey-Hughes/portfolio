import { test, expect, type Page } from "@playwright/test";
import { PHOTO_DETAIL_URL, visibleGrid } from "./helpers";

// photo-detail.spec.ts never opens the full-screen viewer. This spec pins what the
// viewer shows for a photo and how it closes, so a change to its input (a
// ViewerImage, which /open-source screenshots share) can't swap the phone and
// desktop URLs, or lose the alt, the size or Escape, without a test failing.

async function openFirstPhoto(page: Page) {
  await page.goto("/", { waitUntil: "networkidle" });
  // Not visiblePhotoLinks: at phone width the fixture's mosaic home gallery shows
  // .photo-grid-mobile, whose links are .photo-mobile-item, not .photo-hero-link.
  const grid = await visibleGrid(page);
  await grid.locator(".photo-hero-link, .photo-mobile-item").first().click();
  await page.waitForURL(PHOTO_DETAIL_URL, { timeout: 5000 });
  await page.waitForLoadState("networkidle");
}

test.describe("Photo page full-screen viewer", () => {
  test("desktop: shows the original, and ✕ closes it", async ({ page }) => {
    await openFirstPhoto(page);

    const pageImage = page.locator(".photo-detail-image img");
    const original = await page
      .locator(".photo-detail-image source:not([type])")
      .getAttribute("srcset");
    const detail = await pageImage.getAttribute("src");
    const alt = await pageImage.getAttribute("alt");
    const width = await pageImage.getAttribute("width");
    const height = await pageImage.getAttribute("height");
    expect(original).toBeTruthy();
    expect(original).not.toBe(detail);

    await page.locator(".photo-detail-image").click();
    await expect(page.locator(".fullscreen-overlay")).toBeVisible();

    const image = page.locator(".fullscreen-image");
    await expect(image).toHaveAttribute("src", original!);
    await expect(image).toHaveAttribute("alt", alt!);
    await expect(image).toHaveAttribute("width", width!);
    await expect(image).toHaveAttribute("height", height!);
    await expect(page.locator(".zoom-label").last()).toHaveText("10×");

    // The controls fade 1 s after the pointer last moved over the image.
    await image.hover();
    await page.locator(".fullscreen-close").click();
    await expect(page.locator(".fullscreen-overlay")).toHaveCount(0);
  });

  test("is a modal dialog that takes focus and keeps Tab inside", async ({ page }) => {
    await openFirstPhoto(page);
    const alt = await page.locator(".photo-detail-image img").getAttribute("alt");

    await page.locator(".photo-detail-image").click();
    const dialog = page.getByRole("dialog", { name: alt! });
    await expect(dialog).toBeVisible();
    await expect(dialog).toHaveAttribute("aria-modal", "true");

    const close = dialog.getByRole("button", { name: "Close" });
    const slider = dialog.getByRole("slider", { name: "Zoom" });
    await expect(close).toBeFocused();
    await page.keyboard.press("Tab");
    await expect(slider).toBeFocused();
    await page.keyboard.press("Tab");
    await expect(close).toBeFocused();
    await page.keyboard.press("Shift+Tab");
    await expect(slider).toBeFocused();
    await page.keyboard.press("Shift+Tab");
    await expect(close).toBeFocused();

    // The ✕ works from the keyboard too.
    await page.keyboard.press("Enter");
    await expect(dialog).toHaveCount(0);
  });

  test("arrow keys on the zoom slider zoom without changing the photo", async ({
    page,
  }) => {
    await openFirstPhoto(page);
    const url = page.url();
    // The arrow with a photo behind it: the page steps photos with the arrow keys.
    // Counted once the page has rendered its Previous/Next row.
    await expect(page.locator(".photo-navigation")).toBeVisible();
    const arrow = (await page.locator(".nav-next").count()) > 0 ? "ArrowRight" : "ArrowLeft";

    await page.locator(".photo-detail-image").click();
    const close = page.getByRole("button", { name: "Close" });
    const slider = page.getByRole("slider", { name: "Zoom" });
    await expect(close).toBeFocused();
    await page.keyboard.press("Tab");
    await expect(slider).toBeFocused();

    await page.keyboard.press(arrow);
    // At 1× only a step up shows on the slider.
    if (arrow === "ArrowRight") await expect(slider).not.toHaveValue("1");
    // A step to another photo is a client-side navigation; give it time to happen.
    await page.waitForTimeout(300);
    expect(page.url()).toBe(url);
    await expect(slider).toBeFocused();

    // From the ✕, the arrows still step through the photos.
    await page.keyboard.press("Tab");
    await expect(close).toBeFocused();
    await page.keyboard.press(arrow);
    await expect(page).not.toHaveURL(url);
  });

  test("phone: shows the detail image, and Escape closes it", async ({ page }) => {
    // Before goto: the page measures its width once, on mount.
    await page.setViewportSize({ width: 375, height: 667 });
    await openFirstPhoto(page);

    const detail = await page.locator(".photo-detail-image img").getAttribute("src");
    expect(detail).toBeTruthy();

    await page.locator(".photo-detail-image").click();
    await expect(page.locator(".fullscreen-overlay")).toBeVisible();
    await expect(page.locator(".fullscreen-image")).toHaveAttribute("src", detail!);

    await page.keyboard.press("Escape");
    await expect(page.locator(".fullscreen-overlay")).toHaveCount(0);
  });
});
