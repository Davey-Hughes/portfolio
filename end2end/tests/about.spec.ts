import { test, expect } from "@playwright/test";

test.describe("About Page", () => {
  test("should load and display about page", async ({ page }) => {
    await page.goto("/about", { waitUntil: "networkidle" });

    // Check page title
    await expect(page).toHaveTitle("Photography Portfolio");

    // Check about container is visible
    await expect(page.locator(".about-container")).toBeVisible();
  });

  test("should display about content text", async ({ page }) => {
    await page.goto("/about", { waitUntil: "networkidle" });

    // Wait for content to load
    await page.waitForSelector(".about-content", { timeout: 5000 });

    const aboutContent = page.locator(".about-content");
    await expect(aboutContent).toBeVisible();
    
    // Should have some text content
    const text = await aboutContent.textContent();
    expect(text).toBeTruthy();
    expect(text!.length).toBeGreaterThan(0);
  });

  test("should display profile image if available", async ({ page }) => {
    await page.goto("/about", { waitUntil: "networkidle" });

    // Check if profile image exists
    const profileImage = page.locator(".profile-image");
    const count = await profileImage.count();
    
    if (count > 0) {
      await expect(profileImage).toBeVisible();
      
      // Image should have src attribute
      const src = await profileImage.getAttribute("src");
      expect(src).toBeTruthy();
    }
  });

  test("should have footer on about page", async ({ page }) => {
    await page.goto("/about", { waitUntil: "networkidle" });

    // Footer should be visible
    const footer = page.locator("footer");
    await expect(footer).toBeVisible();
  });

  test("should display the license section", async ({ page }) => {
    await page.goto("/about", { waitUntil: "networkidle" });

    const license = page.locator(".about-license");
    await expect(license).toBeVisible();
    await expect(
      license.getByRole("heading", { name: "License", exact: true }),
    ).toBeVisible();
  });

  test("should state terms for the photographs and the code separately", async ({
    page,
  }) => {
    await page.goto("/about", { waitUntil: "networkidle" });

    const license = page.locator(".about-license");

    // The fixture config sets no [license] table, so this is the default path:
    // an all-rights-reserved line generated from site_name.
    await expect(license).toContainText("Photographs");
    await expect(license).toContainText("Photography Portfolio");
    await expect(license).toContainText("All rights reserved");
    await expect(license).toContainText("not licensed for reuse");

    await expect(license).toContainText("Site code");
    await expect(license).toContainText("General Public License");
  });

  test("should link the license text and the source", async ({ page }) => {
    await page.goto("/about", { waitUntil: "networkidle" });

    const gpl = page.locator(".about-license a[href*='gnu.org']");
    await expect(gpl).toHaveAttribute("href", /gpl-3\.0/);

    const source = page.locator(".about-license a", { hasText: "Source" });
    await expect(source).toHaveAttribute("href", /^https?:\/\//);
  });

  test("should omit the contact sentence when none is configured", async ({
    page,
  }) => {
    await page.goto("/about", { waitUntil: "networkidle" });

    // The fixture config has no [license].contact. A dangling "email ." sentence
    // would mean the optional branch rendered anyway.
    await expect(page.locator(".about-license")).not.toContainText(
      "commercial use, email",
    );
  });
});
