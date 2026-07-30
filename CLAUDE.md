# Commands
- cargo leptos build: build the project and check for errors
- cargo leptos test: run unit tests under both feature sets. Resolves to
  `cargo test --no-default-features --features ssr` (server) plus
  `cargo test --lib --no-default-features --features hydrate` (client) — together
  exactly what CI's `rust` and `hydrate` jobs run. The crate declares no default
  features, so `--features ssr` alone is equivalent to the server half.
- Playwright end-to-end tests (`end2end/`, 118 tests over chromium + firefox). Run by
  CI's `release-checks` job. Locally, generate the fixture gallery first — most of the
  suite needs photos and `public/images/*` is gitignored, so without them ~30 tests fail
  on a missing selector:
  ```
  cargo run --release --example gen_fixtures --features ssr
  CONFIG_PATH=target/e2e-fixtures/config.toml ABOUT_CONTENT_PATH=target/e2e-fixtures \
    cargo leptos serve --release &
  cd end2end && npm ci && npx playwright install chromium firefox && CI=1 npx playwright test
  ```
  Prefer that over `cargo leptos end-to-end`: in cargo-leptos 0.3.7 it panics on teardown
  after Playwright exits ("ProcessHandle should not have been dropped"), so it reports
  failure even when every test passed. CI starts the server the same way for that reason.
- cargo ci: fmt + clippy + test over both feature sets (xtask/src/main.rs) — matches
  CI's `rust` and `hydrate` jobs. It does not run the e2e suite or the wasm bundle-size
  budget, both of which need a release build; those live in `release-checks`.

