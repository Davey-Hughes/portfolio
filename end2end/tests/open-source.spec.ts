import { test, expect } from "@playwright/test";

// Relies on the opensource.toml that `cargo run --example gen_fixtures` writes next
// to the fixture config; see FIXTURE_OPEN_SOURCE in examples/gen_fixtures.rs.
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
    await expect(link).toHaveAttribute("rel", "noopener noreferrer");
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
    await expect(site).toHaveAttribute("rel", "noopener noreferrer");
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
