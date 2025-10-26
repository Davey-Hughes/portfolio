# End-to-End Tests

This directory contains Playwright end-to-end tests for the photography portfolio application.

## Prerequisites

- Rust and Cargo installed
- Leptos CLI (`cargo-leptos`) installed
- Node.js installed (Leptos will manage Playwright installation)

## Running Tests

Leptos provides a built-in command that handles everything automatically:

### Run All Tests

```bash
# From the project root
cargo leptos end-to-end
```

This command will:
- Build the application
- Start the development server
- Install Playwright browsers if needed
- Run all tests
- Shut down the server
- Display results

### Additional Test Options

You can also run tests manually with more control:

```bash
# From the end2end directory
npm test                 # Run all tests
npm run test:headed      # Run with visible browser
npm run test:ui          # Interactive UI mode
npm run test:debug       # Debug mode
npm run test:chromium    # Run on Chromium only
npm run test:firefox     # Run on Firefox only
npm run test:webkit      # Run on WebKit only
npm run report           # View HTML test report
```

**Note:** When running tests manually, make sure the dev server is running:
```bash
cargo leptos watch
```

## Test Structure

The tests are organized by functionality:

- **homepage.spec.ts** - Tests for the homepage and main photo gallery
- **navigation.spec.ts** - Tests for navigation between pages
- **about.spec.ts** - Tests for the About page
- **contact.spec.ts** - Tests for the Contact page
- **photo-gallery.spec.ts** - Tests for photo grid display and interactions
- **photo-detail.spec.ts** - Tests for individual photo detail pages
- **responsive.spec.ts** - Tests for responsive design across different viewports

## Configuration

The Playwright configuration is in `playwright.config.ts`. Key settings:

- Base URL: `http://localhost:4000`
- Timeout: 30 seconds per test
- Browsers: Chromium, Firefox, WebKit
- Parallel execution in local development
- Sequential execution in CI

## Writing New Tests

To add new tests:

1. Create a new `.spec.ts` file in the `tests/` directory
2. Import the test utilities:
   ```typescript
   import { test, expect } from "@playwright/test";
   ```
3. Write your tests using `test.describe()` and `test()` blocks
4. Use Playwright's locators and assertions

Example:

```typescript
import { test, expect } from "@playwright/test";

test.describe("My Feature", () => {
  test("should do something", async ({ page }) => {
    await page.goto("/");
    await expect(page.locator(".my-element")).toBeVisible();
  });
});
```

## CI/CD Integration

These tests can be run in CI/CD pipelines. The configuration automatically:

- Runs tests sequentially in CI (to avoid race conditions)
- Retries failed tests 2 times
- Fails the build if `test.only` is found in the code

## Troubleshooting

### Tests timing out

If tests are timing out, make sure:
1. The app has data to display (photos in the gallery directory)
2. The config file exists at `public/content/config.toml`
3. Network requests are completing successfully

### Browser installation issues

Leptos handles browser installation automatically when you run `cargo leptos end-to-end`. If you need to install browsers manually:
```bash
cd end2end
npx playwright install --with-deps
```

### Debugging test failures

Use the UI mode for interactive debugging:
```bash
cd end2end
npm run test:ui
```

Or use debug mode to step through tests:
```bash
cd end2end
npm run test:debug
```

### Running tests during development

For a faster feedback loop during development, keep the server running in one terminal and run tests manually in another:

Terminal 1:
```bash
cargo leptos watch
```

Terminal 2:
```bash
cd end2end
npm test
```
