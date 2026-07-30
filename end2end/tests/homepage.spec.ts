import { test, expect } from "@playwright/test";
import { visibleGrid } from "./helpers";

test.describe("Homepage", () => {
  test("should load and display the homepage", async ({ page }) => {
    await page.goto("/", { waitUntil: "networkidle" });

    // Check that the page title is correct
    await expect(page).toHaveTitle("Photography Portfolio");

    // Check that navigation is present
    await expect(page.locator("nav.navbar")).toBeVisible();
  });

  test("should display site name in navigation", async ({ page }) => {
    await page.goto("/", { waitUntil: "networkidle" });

    // Wait for navigation to load
    const navBrand = page.locator(".nav-brand");
    await expect(navBrand).toBeVisible();
    
    // Should have some text (loaded from config)
    await expect(navBrand).not.toBeEmpty();
  });

  test("should have working navigation links", async ({ page }) => {
    await page.goto("/", { waitUntil: "networkidle" });

    // Check that About link exists and is clickable
    const aboutLink = page.locator('nav a[href="/about"]');
    await expect(aboutLink).toBeVisible();
    
    // Check that Contact link exists and is clickable
    const contactLink = page.locator('nav a[href="/contact"]');
    await expect(contactLink).toBeVisible();
  });

  test("should display home gallery photos", async ({ page }) => {
    await page.goto("/", { waitUntil: "networkidle" });

    // Whichever grid container this gallery/viewport renders.
    await visibleGrid(page);
  });

  test("should have footer on homepage", async ({ page }) => {
    await page.goto("/", { waitUntil: "networkidle" });

    // Footer should be visible on homepage
    const footer = page.locator("footer");
    await expect(footer).toBeVisible();
  });
});
