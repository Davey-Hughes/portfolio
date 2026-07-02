# Photography Portfolio

A self-hosted photography portfolio website. Drop your photos into a folder and
they show up automatically — galleries, titles, EXIF/film metadata, responsive
thumbnails, and a fullscreen viewer are all generated from the files on disk.

Built with [Leptos](https://leptos.dev/) (server-side rendered + hydrated WASM)
on an [Axum](https://github.com/tokio-rs/axum) server. Images are transcoded to
WebP and cached on disk on first request.

## Highlights

- **Zero-code content** — galleries are just directories under `public/images/`;
  add or remove a folder/photo and the site updates (no recompile).
- **Automatic metadata** — camera, lens, focal length, aperture, shutter, ISO,
  date, and film stock are read from EXIF/XMP. Filenames become titles.
- **On-the-fly image processing** — originals stay full-res; the server serves
  compressed WebP variants and caches them under `public/cache/`.
- **Per-gallery and per-photo overrides** via small TOML files.

---

## Prerequisites

The Rust toolchain is pinned by [`rust-toolchain.toml`](rust-toolchain.toml)
(nightly + the `wasm32-unknown-unknown` target), so `rustup` installs the right
toolchain automatically the first time you build. You also need:

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
and serves the site with live reload at **http://127.0.0.1:4000** (reload port
`4001`). These addresses are configured in the `[package.metadata.leptos]`
section of [`Cargo.toml`](Cargo.toml).

Other useful commands (also listed in `CLAUDE.md`):

```bash
cargo leptos build              # one-off build; good for checking it compiles
cargo leptos build --release    # optimized production build
cargo test --lib --features ssr # Rust unit tests
cargo leptos end-to-end         # Playwright e2e tests (see Testing)
```

Out of the box the site reads its content from `public/`. That directory is
**gitignored** (except the example/README files) so your photos and personal
config never get committed — see the layout below.

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
│   ├── image_cache.rs        # on-disk WebP cache: process / prewarm / cleanup
│   ├── image_params.rs       # width/quality preset validation for the image endpoint
│   ├── mosaic.rs             # mosaic gallery layout generation
│   ├── server.rs             # server functions, in-memory caches, watcher plumbing
│   └── types.rs              # shared types (PhotoInfo, GalleryConfig, PhotoConfig, …)
├── style/main.scss           # all site styles (compiled to app.css by cargo-leptos)
├── public/                   # runtime content (GITIGNORED except examples/READMEs)
│   ├── images/               # one subdirectory per gallery (see "Photos")
│   ├── content/              # config.toml, about.txt, profile image
│   └── cache/                # generated compressed WebP images (safe to delete)
├── assets/                   # optional static assets copied to the site root
├── end2end/                  # Playwright end-to-end tests
├── Dockerfile                # multi-stage build → minimal Alpine runtime
├── Cargo.toml                # dependencies + [package.metadata.leptos] config
├── focal-points.toml.example # reference for per-photo focal_point overrides
└── rust-toolchain.toml       # pins nightly + wasm32 target
```

> **Note on `.env`:** the only thing the app reads from `.env` (via `dotenvy`)
> is `LEPTOS_HASH_FILES` — set to `false` locally so `cargo leptos watch/serve`
> serves un-hashed `pkg/` filenames (see [Asset hashing & CDN caching](#asset-hashing--cdn-caching)).
> Site configuration lives in `public/content/config.toml` (see
> [Site configuration](#site-configuration)), **not** in `.env`.

---

## Adding & removing photos

A **gallery** is any subdirectory of `public/images/`. The directory name becomes
the gallery's display name and its URL slug (lowercased, spaces → dashes). Just
create a folder and drop images in:

```
public/images/
├── home/          # special: shown on the landing page, not in the nav
├── city/
├── nature/
├── portraits/
└── film/
```

- **`home`** is special — its photos appear on the landing page and it is *not*
  listed as a regular gallery in the navigation.
- Every other non-empty directory becomes a gallery in the nav automatically.
- To **remove** a gallery or photo, delete the directory or file. The running
  server watches `public/images/` and picks up changes without a restart (it
  also prunes the matching cache entries).

### Filenames, titles, and ordering

The filename (minus extension) becomes the photo title, with light formatting:

- a leading `N - ` or `N.N - ` ordering prefix is stripped
  (`3 - space needle.jpg` → "space needle"),
- `-` and `_` become spaces (`my_photo.jpg` → "my photo").

Photos sort alphabetically by filename, so prefix files with numbers
(`1 - …`, `2 - …`) to control order. Files in a subfolder sort after the
top-level files of the same gallery.

### Supported formats & multiple variants

Recognized extensions: **jpg/jpeg, png, webp, gif, jxl, avif**.

If several files share a basename (e.g. `sunset.jpg` + `sunset.avif`), they are
grouped into one photo and offered as `<picture>` sources in modern-format
priority order (avif → jxl → webp → png → jpg), with JPEG as the universal
fallback.

### Image sets (subfolders)

A photo can be a subdirectory containing one image plus its sibling TOML — this
is the natural place to keep a per-photo title/focal-point override next to a
single image (see `public/images/film/rent_a_car/`). Images nested in
subdirectories are discovered recursively.

### Automatic metadata

On discovery the server reads EXIF (and XMP as a fallback for lens names) to
populate: dimensions, date taken, camera make/model, lens, focal length,
aperture, shutter speed, ISO, copyright, and **film stock**.

Film stock is parsed from the EXIF `UserComment`/`ImageDescription` in two
supported export formats:

- **Pipe-delimited** (current): `Camera: … | Film: Kodak Ektar 100, 35mm | …`
  → the `Film:` value with the trailing gauge note dropped (`Kodak Ektar 100`).
- **LensTagger** (legacy, detected by a `LensTaggerVer` marker): assembled from
  `Film Make:`, `Film Type:`, and the `-ISO=` annotation.

---

## Configuration (TOML files)

There are three kinds of TOML config, each scoped differently. Example files
(`*.example`) are committed; copy them and edit.

### Site configuration

**`public/content/config.toml`** — site-wide settings. Copy
[`public/content/config.toml.example`](public/content/config.toml.example) to
get started.

```toml
# Required
site_name = "Davey Hughes"
site_tagline = "Photography"

# Optional
# site_title    = "Davey Hughes — Photography"   # browser tab; defaults to site_name
# site_copyright = "© 2025 Davey Hughes."        # auto-generated from site_name + year if omitted

# Optional: nav order. List gallery slugs first-to-last; galleries not listed
# fall through alphabetically afterwards. Slug = lowercased dir name, spaces→dashes.
gallery_order = ["film", "city", "nature", "portraits"]

# Arbitrary key/value pairs surfaced on the contact/about pages.
[sections]
email     = "you@example.com"
location  = "Seattle, WA"
instagram = "@yourhandle"
github    = "https://github.com/you/"
```

The about page also uses two files in `public/content/`:

- **`about.txt`** — about text; blank lines separate paragraphs (each becomes a
  `<p>`). Falls back to placeholder text if missing.
- **`profile.{jpg,jpeg,png,webp}`** — profile photo; the first matching file is
  used (falls back to `/images/profile.jpg`).

### Per-gallery layout

**`public/images/<gallery>/gallery.toml`** — controls how that gallery's grid is
rendered. All fields are optional:

```toml
use_mosaic            = true   # mosaic layout instead of a uniform grid (default false)
mosaic_cache_duration = 3600   # seconds to cache the computed mosaic layout (default 3600)
columns               = 4      # grid columns       (default 6; ignored when use_mosaic)
row_height            = 320    # grid row height px  (default 280; ignored when use_mosaic)
gap                   = 12      # gap between items px (default 8)
```

### Per-photo overrides

A TOML file **named after the photo** (sibling file, same basename) overrides
metadata for that one photo. For `portrait.jpg`, create `portrait.toml`:

```toml
title       = "A Better Title"   # overrides the filename-derived title
focal_point = "top-center"       # which third stays visible when cropped to a thumbnail
lens_model  = "Nikon 50mm f/1.8" # override when EXIF lens is missing/ambiguous
```

`focal_point` uses a rule-of-thirds grid — one of: `top-left`, `top-center`,
`top-right`, `center-left`, `center` (default), `center-right`, `bottom-left`,
`bottom-center`, `bottom-right`. See
[`focal-points.toml.example`](focal-points.toml.example) for the full reference.

---

## Image cache & compressed endpoint

Originals are never served to thumbnails or the viewer. Instead, the server
exposes `/images/compressed/<path>?width=<w>&quality=<q>`, which transcodes to
WebP and caches the result under `public/cache/` (filenames like
`gallery_photo_w2400_q90_l1.webp`). On startup the server pre-warms the cache and
removes orphaned entries; the filesystem watcher prunes cache files when their
source image changes or is deleted.

Only specific `(width, quality)` **presets** are accepted (requests outside the
list get a 400). The defaults are `2400×q90` (grid `srcset` / default) and
`4000×q90` (detail page + fullscreen viewer). Override them with the
`IMAGE_PRESETS` environment variable:

```bash
IMAGE_PRESETS="1200,80;2400,90;4000,90"
```

The `public/cache/` directory is gitignored and safe to delete — it will be
regenerated on demand.

---

## Environment variables

All optional — defaults work for a standard `public/` layout. Paths fall back to
`public/<x>` and then `./<x>`.

| Variable | Purpose | Default |
| --- | --- | --- |
| `IMAGES_DIR` | gallery images root | `public/images` |
| `GALLERY_PATH` | home-gallery directory | `public/images/home` |
| `ABOUT_CONTENT_PATH` | content directory (about/profile) | `public/content` |
| `CONFIG_PATH` | path to `config.toml` | `public/content/config.toml` |
| `IMAGE_CACHE_DIR` | compressed-image cache dir | `public/cache` |
| `IMAGE_PRESETS` | allowed `width,quality` pairs (`;`-separated) | `2400,90;4000,90` |
| `RUST_LOG` | log level | — |
| `LEPTOS_SITE_ADDR` | bind address | `127.0.0.1:4000` (dev) |
| `LEPTOS_SITE_ROOT` | compiled site assets dir | `target/site` (`./site` in Docker) |
| `LEPTOS_HASH_FILES` | emit content-hashed `pkg/` filenames (needs `hash.txt`) | `true` in Docker; `false` via local `.env` |

---

## Building for production

```bash
cargo leptos build --release
```

This produces:

- the server binary at `target/release/portfolio`, and
- the site bundle (JS/WASM/CSS + copied static files) at `target/site`.

To run it on a machine without the Rust toolchain, copy both the binary and the
`target/site` directory, then point Leptos at them:

```bash
export LEPTOS_OUTPUT_NAME="portfolio"
export LEPTOS_SITE_ROOT="site"
export LEPTOS_SITE_PKG_DIR="pkg"
export LEPTOS_SITE_ADDR="0.0.0.0:8080"
./portfolio
```

You also need a `public/` directory (images + content) alongside the binary, or
set the `*_DIR`/`*_PATH` env vars above to point at one.

---

## Docker

The [`Dockerfile`](Dockerfile) is a multi-stage build: it compiles with
`rustlang/rust:nightly-alpine` + `cargo-leptos`, then copies the binary and
`target/site` into a minimal `alpine:latest` runtime.

Build:

```bash
docker build -t portfolio .
```

The image serves on port **8080** (`LEPTOS_SITE_ADDR=0.0.0.0:8080`) and declares
a volume at `/app/public`. Photos and content are **not** baked into the image —
mount them at runtime:

```bash
docker run --rm -p 8080:8080 \
  -v "$(pwd)/public:/app/public" \
  portfolio
```

That mounts your `public/images`, `public/content`, and `public/cache` into the
container. The cache is written to the mounted volume so it persists across
restarts. Adjust with the environment variables above as needed (e.g.
`-e RUST_LOG=debug`, `-e IMAGE_PRESETS=...`).

`.dockerignore` excludes `target/`, `public/`, `end2end/`, `.git/`, and `.env`
from the build context, so the build is fast and your local photos aren't copied
in.

---

## Asset hashing & CDN caching

The compiled JS/WASM/CSS under `pkg/` are **content-hashed** in production
(`hash-files = true` in [`Cargo.toml`](Cargo.toml)): each build emits
`portfolio.<hash>.{js,wasm,css}` with unique URLs. This lets a CDN such as
Cloudflare cache `/pkg/*` immutably with no risk of serving a stale JS against a
freshly deployed WASM — the `wasm-bindgen` mismatch that otherwise breaks
hydration after a deploy (and needs no cache purge).

For the server to emit the hashed names at runtime:

- **Production** (the [`Dockerfile`](Dockerfile)) sets `LEPTOS_HASH_FILES=true`
  and copies `hash.txt` next to the binary — the server reads the hashes from it,
  and without it the first render panics.
- **Local dev** opts out: `.env` sets `LEPTOS_HASH_FILES=false`, so `cargo leptos
  watch/serve` serve plain `portfolio.{js,wasm,css}` — nothing to wire up and no
  filename churn on hot reload. `.env` is gitignored and excluded from the Docker
  build, so it never affects production.

---

## Testing

Rust unit tests (config parsing, metadata extraction, cache logic, …):

```bash
cargo test --lib --features ssr
```

End-to-end tests use [Playwright](https://playwright.dev/) and live in
`end2end/tests/`:

```bash
cd end2end && npm install   # first time only
cargo leptos end-to-end             # builds the site, then runs Playwright
cargo leptos end-to-end --release   # against a release build
```

---

## Performance

The per-request hot paths — the on-the-fly WebP transcode, EXIF/film-stock
parsing, and mosaic layout — have a measurement suite under
[`scripts/perf/`](scripts/perf/):

- **Benchmarks** (criterion): `cargo bench --features ssr`
- **Allocation profile** (dhat): `cargo run --release --example alloc_profile --features ssr`
- **WASM bundle-size budget**: `scripts/perf/wasm_size.sh` (after `cargo leptos build --release`)
- **SSR load test / flamegraph**: `scripts/perf/load_test.sh`, `scripts/perf/flamegraph.sh`

See [`scripts/perf/README.md`](scripts/perf/README.md) for what each measures,
how to install the tools, and how to read the output.

---

## License

See [`LICENSE.md`](LICENSE.md).
