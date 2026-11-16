# Development

Setting up, running and testing the project locally, and how changes reach `main`.
Back to the [README](../README.md).

## Prerequisites

[`rust-toolchain.toml`](../rust-toolchain.toml) pins the Rust toolchain to a dated
nightly plus the `wasm32-unknown-unknown` target, so `rustup` installs the right one the
first time you build. To bump it, change the date and run `cargo ci`, which runs CI's
fmt, clippy and test commands. Fix what it flags, or keep the old date. Then let the
PR's CI run as well, since `release-checks` builds the release bundle. You also need:

| Tool | Why | Install |
| --- | --- | --- |
| [`cargo-leptos`](https://github.com/leptos-rs/cargo-leptos) | build/dev runner | `cargo install cargo-leptos --locked` |
| `dart-sass` | compiles `style/main.scss` | `npm install -g sass` |
| `binaryen` (`wasm-opt`) | shrinks the release WASM bundle | your package manager |
| Node + Playwright | only for end-to-end tests | `cd end2end && npm install` |
| [`leptosfmt`](https://github.com/bram209/leptosfmt) | optional, formats the `view!` macros (used by `rust-analyzer.toml`) | `cargo install leptosfmt` |

---

## Running locally

```bash
cargo leptos watch
```

This compiles the server (SSR) and client (WASM) bundles, watches for changes,
and serves the site with live reload at http://127.0.0.1:4000 (reload port
`4001`). Both addresses are set in the `[package.metadata.leptos]` section of
[`Cargo.toml`](../Cargo.toml).

Other useful commands (also listed in `CLAUDE.md`):

```bash
cargo leptos build              # one-off build; good for checking it compiles
cargo leptos build --release    # optimized production build
cargo leptos test               # Rust unit tests, under both feature sets
cargo ci                        # fmt + clippy + those tests, as CI runs them
cargo ci --release              # ...plus the release build, fixtures and bundle-size budget
```

The Playwright end-to-end tests need a fixture gallery and a running server; see
[Testing](#testing).

By default the site reads its content from `public/`. That directory is
gitignored, apart from the example and README files, so your photos and personal
config never get committed. See the layout below.

---

## Directory layout

```
portfolio/
├── src/                      # Rust source
│   ├── main.rs               # Axum server entry (SSR): routes, /images/compressed
│   │                         #   endpoint, cache warmup + filesystem watcher
│   ├── lib.rs                # WASM hydration entry + module declarations
│   ├── app.rs                # Leptos components, routing, and pages
│   ├── gallery.rs            # gallery/photo discovery, EXIF/XMP + film-stock parsing,
│   │                         #   title/slug derivation
│   ├── config.rs             # loads SiteConfig from content/config.toml
│   ├── open_source.rs        # opensource.toml types + loader for /open-source
│   ├── paths.rs              # where files live: IMAGES_DIR, GALLERY_PATH, env dir lookup
│   ├── image_cache.rs        # on-disk WebP cache: process / prewarm / cleanup
│   ├── image_params.rs       # width/quality preset validation for the image endpoint
│   ├── mosaic.rs             # mosaic gallery layout generation
│   ├── readme_assets.rs      # file types the /readme mount serves (allowlist)
│   ├── server.rs             # server functions, in-memory caches, watcher plumbing
│   ├── test_support.rs       # unit-test helpers (EnvVarGuard for env vars)
│   └── types.rs              # shared types (PhotoInfo, GalleryConfig, PhotoConfig, …)
├── style/main.scss           # all site styles (compiled to app.css by cargo-leptos)
├── public/                   # runtime content (GITIGNORED except examples/READMEs)
│   ├── images/               # one subdirectory per gallery (see docs/content.md)
│   ├── content/              # config.toml, about.txt, profile image, opensource.toml
│   ├── readme/               # images for READMEs, served at /readme/ (see docs/content.md)
│   └── cache/                # generated compressed WebP images (safe to delete)
├── assets/                   # optional static assets copied to the site root
├── end2end/                  # Playwright end-to-end tests
├── docs/                     # content, deployment, development and contributing docs
├── Dockerfile                # multi-stage build → minimal Alpine runtime
├── Cargo.toml                # dependencies + [package.metadata.leptos] config
└── rust-toolchain.toml       # pins a dated nightly + wasm32 target
```

The only thing the app reads from `.env` (via `dotenvy`) is `LEPTOS_HASH_FILES`.
It's set to `false` locally so `cargo leptos watch/serve` serves un-hashed `pkg/`
filenames (see [Asset hashing & CDN caching](deployment.md#asset-hashing--cdn-caching)).
Site configuration lives in `public/content/config.toml`, not in `.env` (see
[Site configuration](content.md#site-configuration)).

---

## Testing

The Rust unit tests cover config parsing, metadata extraction, cache logic and more.
Run them under both the server (`ssr`) and the client (`hydrate`) feature sets:

```bash
cargo leptos test   # cargo test under ssr, plus the library tests under hydrate
cargo ci            # fmt + clippy + the same tests: CI's rust and hydrate jobs
```

`cargo ci` is an alias (in `.cargo/config.toml`) for the small runner in
`xtask/src/main.rs`. By default it skips CI's `release-checks` job, which needs a slow
release build. `cargo ci --release` adds that job's release build, fixture generation and
bundle-size budget. Only the job's Playwright suite is left out, since it needs a server;
see below.

The end-to-end tests use [Playwright](https://playwright.dev/), run over Chromium and
Firefox, and live in `end2end/tests/`. Most of the suite needs photos, and
`public/images/*` is gitignored, so generate the fixture gallery first, then serve a
release build against it and run Playwright. As in CI, everything below stays in
`target/e2e-fixtures/`, so it's safe in a checkout whose `public/` holds real photos.
Don't drop `IMAGE_CACHE_DIR`, though: without it the server prunes every entry in
`public/cache` that the fixtures don't account for. Stop any other server on port 4000
first, or the health check below passes at once against the wrong one. If you skip the
fixtures, the tests that need photos fail on a missing selector. The fixtures are dated
an hour back on purpose: the server sends a file changed in the last ten seconds with
`no-store`, since it may still be being written, and the specs expect the year-long
`immutable` header.

```bash
rm -rf target/e2e-fixtures
FIXTURE_IMAGES_DIR=target/e2e-fixtures/images cargo run --release --example gen_fixtures --features ssr
CONFIG_PATH=target/e2e-fixtures/config.toml ABOUT_CONTENT_PATH=target/e2e-fixtures IMAGES_DIR=target/e2e-fixtures/images IMAGE_CACHE_DIR=target/e2e-fixtures/cache README_ASSETS_DIR=target/e2e-fixtures/readme cargo leptos serve --release &
until curl -sf http://127.0.0.1:4000/healthz >/dev/null; do kill -0 $! 2>/dev/null || break; sleep 1; done   # first build takes minutes
cd end2end && npm ci && npx playwright install chromium firefox && CI=1 npx playwright test
```

That's how CI's `release-checks` job runs the suite. Use it rather than
`cargo leptos end-to-end`: in cargo-leptos 0.3.7 that command panics on teardown
after Playwright exits ("ProcessHandle should not have been dropped"), so it
reports failure even when every test passed.

---

## Performance

[`scripts/perf/`](../scripts/perf/) has tools for measuring the per-request hot
paths: the on-the-fly WebP transcode, EXIF and film-stock parsing, and the mosaic
layout.

- **Benchmarks** (criterion): `cargo bench --features ssr`
- **Allocation profile** (dhat): `cargo run --release --example alloc_profile --features ssr`
- **WASM bundle-size budget**: `scripts/perf/wasm_size.sh` (after `cargo leptos build --release`)
- **SSR load test / flamegraph**: `scripts/perf/load_test.sh`, `scripts/perf/flamegraph.sh`

See [`scripts/perf/README.md`](../scripts/perf/README.md) for what each measures,
how to install the tools, and how to read the output.

---

## How changes reach `main`

`main` has a linear history with no merge commits, and every commit on it builds and
passes the gate on its own. Work happens on a feature branch, which lands as one
squashed commit.

Pull requests are the only way onto `main`; there's no local landing script. Push the
branch, open a PR, and squash-merge it in the Forgejo web UI once CI is green:

```bash
git push -u origin my-branch
# open a PR at forge.daveynet.xyz, squash-merge once CI is green
```

CI runs against the PR head on every push, so its result describes the commit you're about
to merge. The jobs are `rust` (fmt, clippy and tests under `ssr`), `hydrate` (the same tests
recompiled under the client feature set) and `release-checks` (the release build, the
fixture gallery, the Playwright suite across chromium and firefox, and the bundle-size
budget).

One thing a web-UI merge doesn't do is check the merged tree. The retired `scripts/land.sh`
did: it ran the gate on the merged tree before making the commit, and refused to commit if
the gate failed. That ordering is what guaranteed every commit on `main` passed the gate. A
squash-merge builds the commit from the PR head instead, and merging is a decision a person
makes after reading CI's result, not a check the merge re-runs.

Branch protection on `main` (enabled 2026-08-04) stands in for that ordering:

- Direct pushes are off, so a PR is the only way in.
- `ci / rust`, `ci / hydrate` and `ci / release-checks`, in their `(pull_request)` form, are
  required status checks.
- `ci / docker` isn't required, on purpose. It's skipped on pull requests, and Forgejo
  reports a skipped job as `success` on the commit-status API, so requiring it would gate on
  nothing. The three jobs above have no `if:` condition, which is why it's safe to require
  them directly.
- `block_on_outdated_branch` is on, so a PR that went green against an older `main` can't
  merge once `main` has moved.
- `apply_to_admins` is on, so the rule binds the repository owner too. Without it, the rule
  would only be advisory for the one account that merges.

Requiring `release-checks` is the strict choice, and it has a cost. The Playwright suite is
the flakiest thing in this repository, and a flake now blocks a merge until the job is
re-run. Under CI, `playwright.config.ts` sets `retries: 2` and `workers: 1` to absorb the
usual noise. The `docker` job is less strict: it doesn't wait on `release-checks`, because
shipping an image and merging a commit are different decisions.

This still isn't the same as gating the merged tree, since the merge commit itself is never
tested before it exists. But it closes the two ways that gap causes real trouble: a red head
and a stale base.

A plain `git merge --squash` throws away every commit message on the branch, so `land.sh`
used to prefill the message with all of them under a `--- Squashed from N commits ---`
marker. The PR's written description does that job now, and reads better as prose. What
changes is where the intermediate messages live. They're no longer in the commit that lands
on `main`. They're on the PR page and under `refs/pull/N/head`, but only on
`forge.daveynet.xyz`, which the git history didn't depend on before.

Once per clone, set this so a non-fast-forward merge fails instead of quietly creating a
merge commit:

```bash
git config merge.ff only && git config pull.ff only
```

That's only a convenience, since `.git/config` isn't tracked and binds nobody. The real
constraint is the remote, which allows only squash and fast-forward merges and deletes the
branch on merge.
