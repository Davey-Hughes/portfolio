# portfolio

The self-hosted photography portfolio behind [daveyhughes.com](https://daveyhughes.com).

[![The home page of daveyhughes.com](https://daveyhughes.com/readme/portfolio/home.webp)](https://daveyhughes.com)

Each folder of photos becomes a gallery. Titles, camera settings and film stocks are
read from the files themselves, and the server makes web-sized copies on demand. To add
a photo, copy the file into its folder; there's nothing to rebuild.

## What it does

- Each directory under `IMAGES_DIR` (default `public/images/`) is a gallery named
  after it, and `home` fills the landing page. The server watches the folder, so a
  photo you add or delete there shows up or disappears without a restart.
- Camera, lens, focal length, aperture, shutter speed, ISO and date come from EXIF,
  with XMP as a fallback for lens names. Film stock comes from EXIF comments, and
  titles from filenames.
- A gallery shows as a uniform grid, or as a mosaic packed by aspect ratio if you
  turn that on for it. Each photo has its own page with its metadata and a fullscreen
  viewer.
- Originals stay full size on disk. The first time a photo is asked for at one of a
  few preset sizes, the server transcodes it to WebP and keeps the result on disk.
- `/open-source` lists projects and upstream contributions from a hand-written
  `opensource.toml`. The page stays hidden unless that file exists, parses and lists
  at least one project or contribution.

## How it's built

It's Rust end to end, on a pinned nightly toolchain. [Leptos](https://leptos.dev/)
renders each page on an [Axum](https://github.com/tokio-rs/axum) server, then hydrates
it in the browser as WebAssembly. EXIF is read with `kamadak-exif`, and the `image` and
`webp` crates transcode images into an on-disk cache that the server pre-warms at
startup. It ships as a multi-stage Docker image with an Alpine runtime. Forgejo CI runs
fmt, clippy and the unit tests under both the server and client feature sets. It also
runs a Playwright suite in Chromium and Firefox, checks a WASM bundle-size budget, and
checks for RustSec advisories.

## Docs

- [Content](docs/content.md): adding photos; the `config.toml`, `opensource.toml`,
  gallery and per-photo TOML files; and images for READMEs.
- [Deployment](docs/deployment.md): production builds, Docker, environment variables,
  the image cache, and asset hashing behind a CDN.
- [Development](docs/development.md): prerequisites, running locally, the directory
  layout, testing, performance tooling, and how changes reach `main`.
- [Contributing](docs/contributing.md): a personal project; forks welcome under the GPL.

## License

The code and the photographs are licensed separately.

- The code is under the GNU General Public License v3.0 or later. See [`LICENSE.md`](LICENSE.md).
- The photographs, including those in the screenshot above, aren't covered by that
  license and aren't distributed with this repository (`public/images/` is gitignored).
  Each deployment states its own terms on its About page. Set them under `[license]`
  in `config.toml` (see [Site configuration](docs/content.md#site-configuration)); if
  you leave that table out, the page says all rights reserved.

If you modify the code, set `[license].source` to your own repository, since the
default points at upstream.
