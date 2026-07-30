import { expect, type Locator, type Page } from "@playwright/test";

/**
 * Every grid container the home/gallery view can render.
 *
 * Which one appears depends on the gallery's `use_mosaic` setting and the viewport:
 * `.photo-grid-home` when mosaic is off (src/app.rs:505), `.photo-grid-mosaic` with
 * `-desktop` and `-tablet` variants when it is on (src/app.rs:438), and
 * `.photo-grid-mobile` on narrow screens (src/app.rs:481). Several are present in the
 * DOM at once, with CSS media queries deciding which is shown.
 */
export const PHOTO_GRID =
  ".photo-grid-home, .photo-grid-mosaic, .photo-grid-mobile";

/**
 * A photo detail page: `/gallery/<gallery>/<photo>`.
 *
 * See the route at src/app.rs:83-91 and the thumbnail hrefs at src/app.rs:283/378.
 * There is no `/photo/` route — the specs used to wait on one, so every navigation
 * assertion timed out. Three path segments are what separate a detail page from the
 * two-segment gallery index.
 */
export const PHOTO_DETAIL_URL = /\/gallery\/[^/]+\/[^/]+/;

/**
 * Wait for whichever grid container is visible at the current viewport, and return it.
 *
 * Deliberately NOT `page.waitForSelector(PHOTO_GRID)`. That resolves the union, keeps
 * the *first* match — the desktop mosaic — and then waits for that one element to
 * become visible, so it times out at any viewport where the desktop grid is hidden:
 *
 *     locator resolved to 2 elements. Proceeding with the first one:
 *       <div class="photo-grid-mosaic photo-grid-mosaic-desktop">
 *
 * Going through `expect(...).toBeVisible()` instead re-resolves the `visible` filter on
 * every poll, so it settles on whichever container the media queries actually show.
 */
export async function visibleGrid(page: Page, timeout = 10000): Promise<Locator> {
  const grid = page.locator(PHOTO_GRID).filter({ visible: true }).first();
  await expect(grid).toBeVisible({ timeout });
  return grid;
}

/**
 * The photo thumbnail links inside the currently visible grid.
 *
 * `.photo-hero-link` on its own is not safe: the desktop, tablet and mobile layouts are
 * all in the DOM simultaneously, each containing its own copy of every photo. So a bare
 * `page.locator(".photo-hero-link").first()` can resolve to a link in a container the
 * media queries have hidden — which passes at the default desktop viewport and then
 * fails the moment a test narrows the window, because clicking or asserting visibility
 * on a hidden element times out. Scoping to the visible grid also stops `.count()` from
 * multiplying each photo by the number of layouts present.
 */
export async function visiblePhotoLinks(
  page: Page,
  timeout = 10000
): Promise<Locator> {
  const grid = await visibleGrid(page, timeout);
  return grid.locator(".photo-hero-link");
}
