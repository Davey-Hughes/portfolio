# Commands
- cargo leptos build: build the project and check for errors
- cargo leptos test: run unit tests under both feature sets. Resolves to
  `cargo test --no-default-features --features ssr` (server) plus
  `cargo test --lib --no-default-features --features hydrate` (client) — together
  exactly what CI's `rust` and `hydrate` jobs run. The crate declares no default
  features, so `--features ssr` alone is equivalent to the server half.
- cargo leptos end-to-end: run end-to-end tests with Playwright. Not run in CI.
- cargo ci: fmt + clippy + test, ssr feature set only (xtask/src/main.rs). Does not
  cover the hydrate half, so a green `cargo ci` can still fail CI's `hydrate` job.

