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
source image changes or is deleted. Each cache file is given its source's
modification time, and is made again whenever the two differ, so a photo replaced by
an older file still gets new sizes when the watcher misses the change. The cache
directory therefore needs a filesystem that keeps modification times to the second or
finer. On FAT, which keeps them to two seconds, every request would make its size
again.

A photo changed in the last 10 seconds may still be being written, and a half-written
JPEG still decodes. Until it has been left alone that long, its sizes are made for each
request and not cached, and they and the original are sent with `no-store`, so nobody
keeps half a picture under an address that won't change. That relies on the file
server's clock being no more than nine seconds behind this server's and no more than a
minute ahead. A file dated more than a minute ahead, as a copy from a camera card whose
clock is ahead would be, is trusted at once, since waiting for it could take days.

Transcoding doesn't block the rest of the site. The pre-warm runs on a thread of its
own with at most four workers, fewer on a smaller host, and requests make at most as
many sizes again at a time, each waiting its turn otherwise. A transcode takes about
8 MB of memory per megapixel at its peak, about 2.2 GB for a 283-megapixel photo, and
about 10–35 ms of CPU per megapixel, a few seconds for the largest. With eight running
at once on the largest photos that is about 18 GB, so a small host could run out of
memory.

The first start of a server that dates its cache files this way makes every size
again, since files made by an older server don't carry their source's time. The
pre-warm remakes every photo's 2400px size, and each 4000px size is made on its first
request. With several photos of 250 megapixels or more, expect that to take minutes;
the site keeps answering meanwhile.

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

Files under `/images` and `/content` are cached for a year too. The site's addresses
for them end in `?v=` and the file's modification time, so a replaced file gets a new
address. That only works while the CDN keeps the query string in its cache key, as
Cloudflare's default (Standard) caching level does.

For the server to emit the hashed names at runtime:

- In production, the [`Dockerfile`](../Dockerfile) sets `LEPTOS_HASH_FILES=true`
  and copies `hash.txt` next to the binary. The server reads the hashes from that
  file, and without it the first render panics.
- Local dev opts out. `.env` sets `LEPTOS_HASH_FILES=false`, so `cargo leptos
  watch/serve` serve plain `portfolio.{js,wasm,css}` and the filenames don't change
  on every hot reload. `.env` is gitignored and excluded from the Docker build, so it
  never affects production.
