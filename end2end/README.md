# End-to-end tests

Playwright tests for the site, run over Chromium and Firefox. CI runs them in the
`release-checks` job.

## Running them

The suite needs a fixture gallery and a release build serving it. Follow the recipe
under [Testing](../docs/development.md#testing) in `docs/development.md`. Don't use
`cargo leptos end-to-end`, which reports failure on teardown even when every test
passes, and don't point the tests at `cargo leptos watch`: without the fixtures, the
tests that need photos fail on a missing selector.

With that server running on port 4000, these work from this directory:

```bash
npm test                 # the whole suite
npm run test:chromium    # one browser
npm run test:firefox
npm run test:headed      # with a visible browser
npm run test:ui          # Playwright's interactive UI
npm run test:debug       # step through with the inspector
npm run report           # open the last HTML report
```

Set `CI=1` for CI's settings: two retries, one worker, and a stray `test.only` fails
the run.

## Writing tests

Specs live in `tests/`, one file per area of the site. `tests/helpers.ts` has the shared
pieces, and its comments explain why each one exists:

- `visibleGrid` and `visiblePhotoLinks` wait for whichever photo grid the viewport shows.
  Several grids are in the DOM at once, and CSS decides which is visible.
- `PHOTO_DETAIL_URL` matches a photo page, `/gallery/<gallery>/<photo>`.
- `holdApi` holds API responses, so a test can look at a page while it is still loading.

The photos, `config.toml` and other content the tests see come from
`examples/gen_fixtures.rs`. If a test needs different content, change the generator.
