import { test, expect, type Locator, type Page } from "@playwright/test";

// Relies on the opensource.toml that `cargo run --example gen_fixtures` writes next
// to the fixture config; see FIXTURE_OPEN_SOURCE in examples/gen_fixtures.rs. The
// screenshot tests also need README_ASSETS_DIR, which serves the card's image.
test.describe("Open Source Page", () => {
  test("renders both sections in order", async ({ page }) => {
    await page.goto("/open-source", { waitUntil: "networkidle" });

    await expect(page.locator(".open-source-page h1")).toHaveText("Open Source");
    await expect(page.locator(".oss-intro")).toContainText(
      "Fixture intro for the end-to-end suite.",
    );
    await expect(page.locator(".oss-section h2")).toHaveText([
      "Projects",
      "Contributions",
    ]);
  });

  test("links a card name when it has a url", async ({ page }) => {
    await page.goto("/open-source", { waitUntil: "networkidle" });

    const link = page.locator(".oss-card h3 a", { hasText: "Fixture Project" });
    await expect(link).toHaveAttribute("href", "https://example.com/fixture-project");
    await expect(link).toHaveAttribute("target", "_blank");
    await expect(link).toHaveAttribute("rel", "noopener");
  });

  test("renders a card name as text when it has no url", async ({ page }) => {
    await page.goto("/open-source", { waitUntil: "networkidle" });

    const card = page.locator(".oss-card", { hasText: "Unlinked Upstream" });
    await expect(card.locator("h3")).toHaveText("Unlinked Upstream");
    await expect(card.locator("h3 a")).toHaveCount(0);
    await expect(card.locator(".oss-website")).toHaveCount(0);
  });

  test("links a card's website by its bare domain", async ({ page }) => {
    await page.goto("/open-source", { waitUntil: "networkidle" });

    const card = page.locator(".oss-card", { hasText: "Fixture Project" });
    const site = card.locator(".oss-website a");
    await expect(site).toHaveText("fixture-project.example");
    await expect(site).toHaveAttribute("href", "https://www.fixture-project.example/");
    await expect(site).toHaveAttribute("target", "_blank");
    await expect(site).toHaveAttribute("rel", "noopener");
    // The website sits beside the heading, not in it: the heading stays the name.
    await expect(card.locator("h3")).toHaveText("Fixture Project");
  });

  test("renders tags and highlights", async ({ page }) => {
    await page.goto("/open-source", { waitUntil: "networkidle" });

    const project = page.locator(".oss-card", { hasText: "Fixture Project" });
    await expect(project.locator(".oss-tags li")).toHaveText(["Rust", "Leptos"]);

    const highlight = page.locator(".oss-highlights a", {
      hasText: "#1 Fixture highlight",
    });
    await expect(highlight).toHaveAttribute(
      "href",
      "https://example.com/fixture-upstream/pull/1",
    );
  });

  test("/os permanently redirects to the page", async ({ page, request }) => {
    // The short link must be a real HTTP redirect, not a second copy of the page,
    // so the page keeps one canonical URL.
    const res = await request.get("/os", { maxRedirects: 0 });
    expect(res.status()).toBe(308);
    expect(res.headers()["location"]).toBe("/open-source");

    await page.goto("/os", { waitUntil: "networkidle" });
    await expect(page).toHaveURL("/open-source");
  });

  test("About links to the page", async ({ page }) => {
    await page.goto("/about", { waitUntil: "networkidle" });

    await page.locator(".about-open-source a").click();

    await expect(page).toHaveURL("/open-source");
    await expect(page.locator(".open-source-page")).toBeVisible();
  });

  test("keeps the 900px measure however narrow the content", async ({ page }) => {
    // .main-content is a column flexbox, so a child with auto margins shrinks to
    // its content unless told otherwise; short card summaries would then leave a
    // narrow column whose width moves with every edit. The fixture's long token
    // alone would push the column to full width, so strip everything but the
    // headings before measuring.
    await page.setViewportSize({ width: 1440, height: 900 });
    await page.goto("/open-source", { waitUntil: "networkidle" });
    await page.evaluate(() =>
      document
        .querySelectorAll(".oss-intro, .oss-card")
        .forEach((el) => el.remove()),
    );

    const box = await page.locator(".open-source-page").boundingBox();
    expect(box?.width).toBe(900);
  });

  // The fixture's long unbroken token (intro, card name, tag, highlight) is what
  // gives this test teeth; see FIXTURE_OPEN_SOURCE.
  test("no horizontal scroll at phone width", async ({ page }) => {
    await page.setViewportSize({ width: 375, height: 667 });
    await page.goto("/open-source", { waitUntil: "networkidle" });
    await expect(page.locator(".oss-card").first()).toBeVisible();

    const overflows = await page.evaluate(
      () =>
        document.documentElement.scrollWidth >
        document.documentElement.clientWidth,
    );
    expect(overflows).toBe(false);
  });
});

test.describe("Open Source Page screenshots", () => {
  const SHOT = "/readme/fixture/shot.webp";
  const ALT = "The Fixture Project home page";
  const VIEW = "View the Fixture Project screenshot full screen";
  // A screenshot's address on the page: its path, then its file's version.
  const versioned = (path: string) => new RegExp(`^${path.replace(/\./g, "\\.")}\\?v=\\d+$`);

  const card = (page: Page) => page.locator(".oss-card", { hasText: "Fixture Project" });
  const plain = (page: Page) =>
    page.locator(".oss-card", { hasText: "Unlinked Upstream" });
  const button = (page: Page) => page.getByRole("button", { name: VIEW });
  // The path of the file the browser picked from a card's <picture>, not just the
  // fallback src; empty until it has picked.
  const shown = (l: Locator) =>
    l.evaluate((el: HTMLImageElement) => el.currentSrc && new URL(el.currentSrc).pathname);

  test("shows a screenshot only on the card that sets one", async ({ page }) => {
    await page.goto("/open-source", { waitUntil: "networkidle" });

    const img = card(page).locator(".oss-image img");
    await expect(img).toHaveAttribute("src", versioned(SHOT));
    await expect(img).toHaveAttribute("alt", ALT);
    await expect(img).toHaveAttribute("loading", "lazy");
    await expect(img).toHaveAttribute("decoding", "async");
    // The file is really served, not just referenced.
    await expect
      .poll(() => img.evaluate((el: HTMLImageElement) => el.naturalWidth))
      .toBe(8);
    await expect(card(page).getByRole("button", { name: VIEW })).toBeVisible();

    // Fixture Project and the inverted Fixture Upstream.
    await expect(page.locator(".oss-image")).toHaveCount(2);
    await expect(plain(page).locator("img, .oss-card-text")).toHaveCount(0);
  });

  test("takes a reload's screenshot from the browser's cache", async ({
    page,
    browserName,
  }) => {
    // Firefox's image cache answers the reload without a Resource Timing entry, so
    // there is nothing to read there.
    test.skip(browserName === "firefox", "no timing entry for a cached image");
    // Checked with the server on every load, a screenshot showed up a round trip
    // after the text it sits beside.
    await page.goto("/open-source", { waitUntil: "networkidle" });
    const img = card(page).locator(".oss-image img");
    const width = () => img.evaluate((el: HTMLImageElement) => el.complete && el.naturalWidth);
    await expect.poll(width).toBe(8);

    await page.reload({ waitUntil: "networkidle" });
    await expect.poll(width).toBe(8);
    // Bytes that came over the network for it: none from the cache, a 304's headers
    // from a check with the server.
    const transferred = await img.evaluate((el: HTMLImageElement) =>
      performance
        .getEntriesByName(el.currentSrc)
        .map((entry) => (entry as PerformanceResourceTiming).transferSize),
    );
    expect(transferred).toEqual([0]);
  });

  test("inverts an image_invert screenshot on its card only", async ({ page }) => {
    // Desktop: the card shows image_card (16 px wide), full screen image (32 px).
    await page.setViewportSize({ width: 1280, height: 900 });
    await page.goto("/open-source", { waitUntil: "networkidle" });
    const filter = (l: Locator) => l.evaluate((el) => getComputedStyle(el).filter);
    const loaded = (l: Locator, width: number) =>
      expect.poll(() => l.evaluate((el: HTMLImageElement) => el.naturalWidth)).toBe(width);
    // The [r, g, b] the page shows at `at` (fractions of the element's box), read
    // from a screenshot the browser decodes, so what is checked is what is seen.
    const pixel = async (l: Locator, at: [number, number]) => {
      const png = (await l.screenshot({ animations: "disabled" })).toString("base64");
      return page.evaluate(
        async ([b64, [fx, fy]]) => {
          const shot = new Image();
          shot.src = `data:image/png;base64,${b64}`;
          await shot.decode();
          const canvas = document.createElement("canvas");
          canvas.width = shot.width;
          canvas.height = shot.height;
          const ctx = canvas.getContext("2d")!;
          ctx.drawImage(shot, 0, 0);
          const x = Math.floor(shot.width * fx);
          const y = Math.floor(shot.height * fy);
          return Array.from(ctx.getImageData(x, y, 1, 1).data.slice(0, 3));
        },
        [png, at] as const,
      );
    };
    const art = page
      .locator(".oss-card", { hasText: "Fixture Upstream" })
      .locator(".oss-image img");
    // The image is lazy, so it may not load until it is near the viewport.
    await art.scrollIntoViewIfNeeded();
    await loaded(art, 16); // art-card.webp, through image_card

    expect(await filter(card(page).locator(".oss-image img"))).toBe("none");
    expect(await filter(art)).toBe("invert(1) hue-rotate(180deg)");
    // fixture art: white in the middle, transparent around it. The white comes out
    // black on the white page, and the transparent edge stays page white.
    expect(Math.max(...(await pixel(art, [0.5, 0.5])))).toBeLessThan(40);
    expect(Math.min(...(await pixel(art, [0.1, 0.1])))).toBeGreaterThan(215);

    // Full screen the art shows as it is: white, on the viewer's dark overlay.
    await page
      .getByRole("button", { name: "View the Fixture Upstream screenshot full screen" })
      .click();
    const full = page.locator(".fullscreen-image");
    await loaded(full, 32); // art.webp, the image itself
    expect(await filter(full)).toBe("none");
    expect(Math.min(...(await pixel(full, [0.5, 0.5])))).toBeGreaterThan(215);
  });

  test("shows image_card on the desktop card and opens image full screen", async ({
    page,
  }) => {
    await page.setViewportSize({ width: 1280, height: 900 });
    await page.goto("/open-source", { waitUntil: "networkidle" });
    const art = page
      .locator(".oss-card", { hasText: "Fixture Upstream" })
      .locator(".oss-image img");
    const full = page.locator(".fullscreen-image");

    await art.scrollIntoViewIfNeeded();
    await expect(
      page.locator(".oss-card", { hasText: "Fixture Upstream" }).locator("source"),
    ).toHaveAttribute("srcset", versioned("/readme/fixture/art-card.webp"));
    await expect.poll(() => shown(art)).toBe("/readme/fixture/art-card.webp");
    await page
      .getByRole("button", { name: "View the Fixture Upstream screenshot full screen" })
      .click();
    await expect(full).toHaveAttribute("src", versioned("/readme/fixture/art.webp"));
    await page.keyboard.press("Escape");
    await expect(page.locator(".fullscreen-overlay")).toHaveCount(0);

    // Fixture Project sets no image_card: the card and full screen show the same file.
    const shot = card(page).locator(".oss-image img");
    await expect.poll(() => shown(shot)).toBe(SHOT);
    await expect(card(page).locator("source")).toHaveCount(0);
    await button(page).click();
    await expect(full).toHaveAttribute("src", versioned(SHOT));
  });

  test("shows image on the full-width card at phone width", async ({ page }) => {
    await page.setViewportSize({ width: 375, height: 667 });
    await page.goto("/open-source", { waitUntil: "networkidle" });
    const art = page
      .locator(".oss-card", { hasText: "Fixture Upstream" })
      .locator(".oss-image img");

    // image_card is made for the desktop slot; the full-width phone card at 2x or 3x
    // density needs more pixels than that.
    await art.scrollIntoViewIfNeeded();
    await expect.poll(() => shown(art)).toBe("/readme/fixture/art.webp");
  });

  test("sets the screenshot right of the text on desktop", async ({ page }) => {
    await page.setViewportSize({ width: 1280, height: 900 });
    await page.goto("/open-source", { waitUntil: "networkidle" });

    const img = (await card(page).locator(".oss-image img").boundingBox())!;
    const text = (await card(page).locator(".oss-card-text").boundingBox())!;
    const box = (await card(page).boundingBox())!;
    const plainHead = (await plain(page).locator(".oss-card-head").boundingBox())!;

    expect(img.width).toBeCloseTo(340, 0);
    expect(img.height).toBeCloseTo(212, 0);
    expect(img.x - (text.x + text.width)).toBeCloseTo(28, 0);
    // This card's text is shorter than its screenshot, so both start at the top.
    expect(img.y).toBeCloseTo(box.y + 20, 0);
    expect(text.y).toBeCloseTo(box.y + 20, 0);
    // The text keeps the left edge of cards without an image.
    expect(text.x).toBeCloseTo(plainHead.x, 0);
  });

  test("centers the screenshot on text taller than it", async ({ page }) => {
    await page.setViewportSize({ width: 1280, height: 900 });
    await page.goto("/open-source", { waitUntil: "networkidle" });
    const upstream = page.locator(".oss-card", { hasText: "Fixture Upstream" });

    const img = (await upstream.locator(".oss-image img").boundingBox())!;
    const text = (await upstream.locator(".oss-card-text").boundingBox())!;
    // The fixture's summary is long enough to run past the screenshot.
    expect(text.height).toBeGreaterThan(img.height + 40);
    expect(img.y + img.height / 2).toBeCloseTo(text.y + text.height / 2, 0);
  });

  test("stacks the screenshot under the tags at phone width", async ({ page }) => {
    await page.setViewportSize({ width: 375, height: 667 });
    await page.goto("/open-source", { waitUntil: "networkidle" });

    const img = (await card(page).locator(".oss-image img").boundingBox())!;
    const tags = (await card(page).locator(".oss-tags").boundingBox())!;
    const box = (await card(page).boundingBox())!;

    expect(img.width).toBeCloseTo(box.width, 0);
    expect(img.height).toBeCloseTo(img.width * 0.625, 0);
    expect(img.y - (tags.y + tags.height)).toBeCloseTo(14, 0);
    // Only the card's own padding under the image: the closed viewer adds no row.
    expect(box.y + box.height - (img.y + img.height)).toBeCloseTo(20, 0);
  });

  test("opens full screen and closes back to the same place", async ({ page }) => {
    await page.setViewportSize({ width: 1280, height: 360 });
    await page.goto("/open-source", { waitUntil: "networkidle" });

    await button(page).evaluate((el) => el.scrollIntoView({ block: "center" }));
    const before = await page.evaluate(() => window.scrollY);
    expect(before).toBeGreaterThan(0);

    await button(page).click();
    const overlay = page.locator(".fullscreen-overlay");
    await expect(overlay).toBeVisible();
    const image = page.locator(".fullscreen-image");
    await expect(image).toHaveAttribute("src", versioned(SHOT));
    await expect(image).toHaveAttribute("alt", ALT);
    // No size is known for a screenshot, so it gets the default 10× zoom.
    await expect(page.locator(".zoom-label").last()).toHaveText("10×");
    // A one-finger drag must not scroll the page under the overlay.
    expect(await overlay.evaluate((el) => getComputedStyle(el).touchAction)).toBe(
      "none",
    );

    // The controls fade 1 s after the pointer last moved over the image.
    await image.hover();
    await page.locator(".fullscreen-close").click();
    await expect(overlay).toHaveCount(0);

    expect(await page.evaluate(() => window.scrollY)).toBe(before);
    await expect(button(page)).toBeInViewport();
    await expect(button(page)).toBeFocused();
  });

  test("keeps keyboard focus in the viewer and hands it back on close", async ({
    page,
  }) => {
    await page.goto("/open-source", { waitUntil: "networkidle" });

    await button(page).focus();
    await page.keyboard.press("Enter");
    const dialog = page.getByRole("dialog", { name: ALT });
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

    await page.keyboard.press("Escape");
    await expect(dialog).toHaveCount(0);
    await expect(button(page)).toBeFocused();
  });

  test("marks the zoom slider when the keyboard is on it", async ({ page }) => {
    await page.goto("/open-source", { waitUntil: "networkidle" });
    // Hold the controls up: this is about the mark, not the fade.
    await page.addStyleTag({ content: ".fullscreen-controls.hidden { opacity: 1 !important; }" });

    await button(page).focus();
    await page.keyboard.press("Enter");
    const pill = page.locator(".zoom-slider-container");
    await expect(page.getByRole("button", { name: "Close" })).toBeFocused();
    const before = await pill.screenshot({ animations: "disabled" });

    await page.keyboard.press("Tab");
    await expect(page.getByRole("slider", { name: "Zoom" })).toBeFocused();
    const after = await pill.screenshot({ animations: "disabled" });

    expect(after.equals(before)).toBe(false);
  });

  test("keeps the slider's halo from bulging out of the pill on phones", async ({ page }) => {
    await page.setViewportSize({ width: 375, height: 667 });
    await page.goto("/open-source", { waitUntil: "networkidle" });
    // Hold the controls up: this is about the mark, not the fade.
    await page.addStyleTag({ content: ".fullscreen-controls.hidden { opacity: 1 !important; }" });
    await button(page).focus();
    await page.keyboard.press("Enter");
    await expect(page.getByRole("button", { name: "Close" })).toBeFocused();

    // The pill is short on phones. Three rows above it and three below, starting
    // a pixel clear of its edge, must not change when the halo appears.
    const pill = (await page.locator(".zoom-slider-container").boundingBox())!;
    const strips = [pill.y - 5, pill.y + pill.height + 2].map((y) => ({
      x: pill.x,
      y,
      width: pill.width,
      height: 3,
    }));
    // The pill itself too, so the test fails if the halo is gone altogether.
    const shoot = () =>
      Promise.all(
        [pill, ...strips].map((clip) => page.screenshot({ clip, animations: "disabled" })),
      );
    const before = await shoot();

    await page.keyboard.press("Tab");
    await expect(page.getByRole("slider", { name: "Zoom" })).toBeFocused();
    const after = await shoot();

    // [the pill, the strip above, the strip below]
    expect(after.map((shot, i) => shot.equals(before[i]))).toEqual([false, true, true]);
  });

  test("keeps the keyboard-focused control visible when the controls fade", async ({
    page,
  }) => {
    await page.goto("/open-source", { waitUntil: "networkidle" });

    await button(page).focus();
    await page.keyboard.press("Enter");
    const close = page.getByRole("button", { name: "Close" });
    const controls = page.locator(".fullscreen-controls");
    await expect(close).toBeFocused();

    // The controls fade 1 s after opening; the slider's pill goes, the ✕ stays.
    await expect(controls).toHaveClass(/hidden/);
    await expect(controls).toHaveCSS("opacity", "0");
    await expect(close).toHaveCSS("opacity", "1");

    await page.keyboard.press("Tab");
    await expect(close).toHaveCSS("opacity", "0");
    await expect(controls).toHaveCSS("opacity", "1");
  });

  test("leaves Back and Forward keys to the browser while the viewer is open", async ({
    page,
  }) => {
    await page.goto("/open-source", { waitUntil: "networkidle" });

    await button(page).focus();
    await page.keyboard.press("Enter");
    await expect(page.getByRole("button", { name: "Close" })).toBeFocused();

    // Synthetic events, so a Back that gets through doesn't really go back.
    const prevented = await page.evaluate(() =>
      [
        { key: "ArrowLeft" },
        { key: "ArrowLeft", altKey: true },
        { key: "ArrowRight", altKey: true },
        { key: "ArrowLeft", metaKey: true },
        { key: "ArrowDown", altKey: true },
      ].map((init) => {
        const event = new KeyboardEvent("keydown", { ...init, bubbles: true, cancelable: true });
        document.activeElement!.dispatchEvent(event);
        return event.defaultPrevented;
      }),
    );
    // Alt or ⌘ with Left/Right is Back/Forward. Plain arrows and Alt+Down scroll
    // the page behind, so the viewer keeps them.
    expect(prevented).toEqual([true, false, false, false, true]);
  });

  test("keys don't scroll the page behind the viewer", async ({ page }) => {
    await page.setViewportSize({ width: 1280, height: 360 });
    await page.goto("/open-source", { waitUntil: "networkidle" });

    await button(page).evaluate((el) => el.scrollIntoView({ block: "center" }));
    const before = await page.evaluate(() => window.scrollY);
    expect(before).toBeGreaterThan(0);

    await button(page).focus();
    await page.keyboard.press("Enter");
    await expect(page.locator(".fullscreen-overlay")).toBeVisible();

    // Whatever has focus in the viewer: the ✕ first, then the slider, which keeps
    // its own arrow, Page and Home/End keys, but not Space, and not with Ctrl.
    const scrollKeys = ["PageUp", "Home", "ArrowUp", "PageDown", "End", "ArrowDown"];
    for (const key of scrollKeys) await page.keyboard.press(key);
    // One direction only: a pair could scroll away and back if it got through.
    await page.keyboard.press("Alt+ArrowDown");
    await page.keyboard.press("Tab");
    await page.keyboard.press("Space");
    await page.keyboard.press("Control+Home");
    await page.keyboard.press("Control+End");
    await page.keyboard.press("Control+ArrowDown");
    // Keyboard scrolling can be animated; give it time to show up.
    await page.waitForTimeout(300);
    expect(await page.evaluate(() => window.scrollY)).toBe(before);

    await page.keyboard.press("ArrowRight");
    await expect(page.locator(".zoom-slider")).not.toHaveValue("1");
  });

  test("opens from the keyboard, closes on Escape, and reopens unzoomed", async ({
    page,
  }) => {
    await page.goto("/open-source", { waitUntil: "networkidle" });

    await button(page).focus();
    await page.keyboard.press("Enter");
    const overlay = page.locator(".fullscreen-overlay");
    await expect(overlay).toBeVisible();

    await page.locator(".zoom-slider").fill("3");
    await expect(page.locator(".fullscreen-image")).toHaveAttribute(
      "style",
      /scale\(3\)/,
    );

    await page.keyboard.press("Escape");
    await expect(overlay).toHaveCount(0);

    await button(page).click();
    await expect(page.locator(".zoom-slider")).toHaveValue("1");
  });

  test("keeps the viewer's zoom labels on one line at tablet width", async ({ page }) => {
    // The page's `overflow-wrap: anywhere` must not reach the viewer: at tablet
    // portrait widths the zoom pill is squeezed, and "10×" would break in two.
    await page.setViewportSize({ width: 820, height: 1180 });
    await page.goto("/open-source", { waitUntil: "networkidle" });

    await button(page).click();
    await expect(page.locator(".fullscreen-overlay")).toBeVisible();

    const lines = await page.locator(".zoom-label").evaluateAll((labels) =>
      labels.map((label) => {
        const range = document.createRange();
        range.selectNodeContents(label);
        return range.getClientRects().length;
      }),
    );
    expect(lines).toEqual([1, 1]);
  });
});
