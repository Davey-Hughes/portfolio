# Deployment

Building, running and caching the site in production. Back to the [README](../README.md).

## Building for production

```bash
cargo leptos build --release
```

This produces:

- the server binary at `target/release/portfolio`, and
- the site bundle (JS/WASM/CSS + copied static files) at `target/site`.

To run it on a machine without the Rust toolchain, copy the binary and the
`target/site` directory, then point Leptos at them:

```bash
export LEPTOS_OUTPUT_NAME="portfolio"
export LEPTOS_SITE_ROOT="site"
export LEPTOS_SITE_PKG_DIR="pkg"
export LEPTOS_SITE_ADDR="0.0.0.0:8080"
./portfolio
```

You also need a `public/` directory with the images and content next to the binary,
or the `*_DIR`/`*_PATH` [environment variables](#environment-variables) set to point
at one.

---

## Docker

The [`Dockerfile`](../Dockerfile) is a multi-stage build: it compiles with
`rustlang/rust:nightly-alpine` and `cargo-leptos`, then copies the binary and
`target/site` into a small `alpine:latest` runtime.

Build:

```bash
docker build -t portfolio .
```

The image serves on port 8080 (`LEPTOS_SITE_ADDR=0.0.0.0:8080`) and declares
a volume at `/app/public`, with `IMAGES_DIR` and `README_ASSETS_DIR` pinned to
`/app/public/images` and `/app/public/readme`. Photos and content aren't baked
into the image, so mount them at runtime:

```bash
docker run --rm -p 8080:8080 \
  -v "$(pwd)/public:/app/public" \
  portfolio
```

That mounts your `public/images`, `public/content`, `public/readme` and
`public/cache` into the container. The image doesn't pin `IMAGE_CACHE_DIR`, though,
and the server picks the cache directory once at startup: `public/cache` if it
exists, otherwise `./cache`. On a fresh volume there's no `public/cache` yet, so the
cache ends up in `/app/cache` inside the container and is lost whenever the container
is recreated. Run `mkdir -p public/cache` on the volume before the first start, or
set `IMAGE_CACHE_DIR=/app/public/cache`. Use the
[environment variables](#environment-variables) to change anything else, e.g.
`-e RUST_LOG=debug` or `-e IMAGE_PRESETS=...`.

`.dockerignore` excludes `target/`, `public/`, `end2end/`, `.git/` and `.env`
from the build context, which keeps the build fast and your local photos out of
the image.

---

## Environment variables

All of these are optional, and the defaults work with a standard `public/` layout.
Paths fall back to `public/<x>` and then `./<x>`. The server picks `IMAGE_CACHE_DIR`
and `README_ASSETS_DIR` once at startup, so create those folders under `public/`
before starting it, or set the variables. A folder created later is ignored until
the next restart.

| Variable | Purpose | Default |
| --- | --- | --- |
| `IMAGES_DIR` | gallery images root | `public/images` (`/app/public/images` in Docker) |
| `README_ASSETS_DIR` | images for READMEs, served at `/readme/` | `public/readme` (`/app/public/readme` in Docker) |
| `GALLERY_PATH` | home-gallery directory; the homepage also reads its `gallery.toml` here | `<IMAGES_DIR>/home` |
| `ABOUT_CONTENT_PATH` | content directory (about/profile/open source) | `public/content` |
| `CONFIG_PATH` | path to `config.toml` | `public/content/config.toml` |
| `IMAGE_CACHE_DIR` | compressed-image cache dir | `public/cache` |
| `IMAGE_PRESETS` | allowed `width,quality` pairs (`;`-separated) | `2400,90;4000,90` |
| `RUST_LOG` | log level | — |
| `LEPTOS_SITE_ADDR` | bind address | `127.0.0.1:4000` (dev) |
| `LEPTOS_SITE_ROOT` | compiled site assets dir | `target/site` (`./site` in Docker) |
| `LEPTOS_HASH_FILES` | emit content-hashed `pkg/` filenames (needs `hash.txt`) | `true` in Docker; `false` via local `.env` |

---

## Image cache & compressed endpoint

Thumbnails and the viewer never get the originals. They load
`/images/compressed/<path>?width=<w>&quality=<q>`, which transcodes to WebP and
caches the result under `public/cache/`, with filenames like
`gallery_photo_w2400_q90_l1.webp`. On startup the server pre-warms the cache and
removes orphaned entries, and the filesystem watcher prunes cache files when their
source image changes or is deleted.

The endpoint accepts only a fixed list of `(width, quality)` presets, and anything
else gets a 400. The defaults are `2400×q90`, used for the grid `srcset` and for
requests with no parameters, and `4000×q90`, used for the detail page and the
fullscreen viewer. Override them with the `IMAGE_PRESETS` environment variable:

```bash
IMAGE_PRESETS="1200,80;2400,90;4000,90"
```

The `public/cache/` directory is gitignored and safe to delete. It's regenerated
on demand.

---

## Asset hashing & CDN caching

In production the compiled JS, WASM and CSS under `pkg/` get content-hashed names
(`hash-files = true` in [`Cargo.toml`](../Cargo.toml)), so each build emits
`portfolio.<hash>.{js,wasm,css}` at URLs of its own. A CDN such as Cloudflare can
then cache `/pkg/*` as immutable without ever serving stale JS next to freshly
deployed WASM. That `wasm-bindgen` mismatch would otherwise break hydration after a
deploy, and with hashed names there's no cache to purge either.

For the server to emit the hashed names at runtime:

- In production, the [`Dockerfile`](../Dockerfile) sets `LEPTOS_HASH_FILES=true`
  and copies `hash.txt` next to the binary. The server reads the hashes from that
  file, and without it the first render panics.
- Local dev opts out. `.env` sets `LEPTOS_HASH_FILES=false`, so `cargo leptos
  watch/serve` serve plain `portfolio.{js,wasm,css}` and the filenames don't change
  on every hot reload. `.env` is gitignored and excluded from the Docker build, so it
  never affects production.
