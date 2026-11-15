# Commands
- `cargo leptos build`: build the project and check for errors
- `cargo leptos test`: unit tests under both the `ssr` and `hydrate` feature sets
- `cargo ci`: fmt, clippy and those tests, as CI's `rust` and `hydrate` jobs run them;
  `cargo ci --release` adds the release build, fixture generation and bundle-size budget

The Playwright end-to-end suite needs a fixture gallery and a release server. Run it with
the recipe under "Testing" in `docs/development.md`, exactly as written. Two shortcuts
that look reasonable and aren't:
- `cargo leptos end-to-end` reports failure even when every test passes (cargo-leptos
  0.3.7 panics on teardown).
- Without `IMAGE_CACHE_DIR`, the server prunes `public/cache` of every entry the fixtures
  don't account for.

`docs/development.md` also covers setup, the directory layout and how changes reach `main`.
