# Content

Adding photos, configuring the site, its galleries and single photos, and hosting
images for READMEs. Back to the [README](../README.md).

## Adding & removing photos

A gallery is any subdirectory of `IMAGES_DIR` (default `public/images/`). The
directory name is the gallery's display name, and also its URL slug once it's
lowercased with spaces turned into dashes. Create a folder and put images in it:

```
public/images/
├── home/          # special: shown on the landing page, not in the nav
├── city/
├── nature/
├── portraits/
└── film/
```

- `home` is special: its photos appear on the landing page, and it isn't listed as
  a gallery in the navigation.
- Every other non-empty directory shows up in the nav as a gallery, except one named
  `categories`, which is skipped.
- To remove a gallery or photo, delete the directory or file. The running server
  watches `IMAGES_DIR` and picks up changes without a restart, and it prunes the
  matching cache entries too.

### Filenames, titles, and ordering

The filename, minus its extension, becomes the photo's title after a little tidying:

- a leading `N - ` or `N.N - ` ordering prefix is stripped
  (`3 - space needle.jpg` → "space needle"),
- `-` and `_` become spaces (`my_photo.jpg` → "my photo").

Photos sort alphabetically by filename, so prefix files with numbers
(`1 - …`, `2 - …`) to set the order. Files in a subfolder sort after the
top-level files of the same gallery.

### Supported formats & multiple variants

Recognized extensions: jpg/jpeg, png, webp, gif, jxl, avif.

Files that share a basename, such as `sunset.jpg` and `sunset.avif`, are grouped
into one photo. The browser gets them as `<picture>` sources in order of preference
(avif → jxl → webp → png → jpg), with JPEG as the fallback that works everywhere.

### Image sets (subfolders)

A photo can also be a subdirectory holding one image and its TOML file. That's a
good way to keep a per-photo title or focal-point override next to its image (see
`public/images/film/rent_a_car/`). Images in nested subdirectories are found
recursively.

### Automatic metadata

When the server finds a photo, it reads the EXIF, with XMP as a fallback for lens
names, to fill in the dimensions, date taken, camera make and model, lens, focal
length, aperture, shutter speed, ISO, copyright and film stock.

Film stock is read from the EXIF `UserComment`/`ImageDescription`, in either of two
export formats:

- Pipe-delimited (current): from `Camera: … | Film: Kodak Ektar 100, 35mm | …` it
  takes the `Film:` value and drops the trailing gauge note, giving `Kodak Ektar 100`.
- LensTagger (legacy, recognized by a `LensTaggerVer` marker): the stock is put
  together from `Film Make:`, `Film Type:` and the `-ISO=` annotation.

---

## Configuration (TOML files)

There are four kinds of TOML config, each with its own scope. Example files
(`*.example`) are committed, so copy one and edit it.

### Site configuration

`public/content/config.toml` holds the site-wide settings. Copy
[`public/content/config.toml.example`](../public/content/config.toml.example) to
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

# Optional: licensing shown on the About page. Omit and it states all rights
# reserved over the photographs (from site_name + year) plus the code's GPLv3.
# [license]
# images      = "All rights reserved."
# contact     = "you@example.com"
# source      = "https://github.com/you/portfolio"
# footer_text = "License"                  # footer link label; "" hides the link

# Arbitrary key/value pairs surfaced on the contact/about pages.
[sections]
email     = "you@example.com"
location  = "Seattle, WA"
instagram = "@yourhandle"
github    = "https://github.com/you/"
```

The about page also uses two files in `public/content/`:

- `about.txt`: the about text. Blank lines separate paragraphs, and each one becomes
  a `<p>`. If the file is missing, the page shows placeholder text.
- `profile.{jpg,jpeg,png,webp}`: the profile photo. The first matching file is
  used, with `/images/profile.jpg` as the fallback.

### Open source page

`public/content/opensource.toml` lists your own projects and your upstream
contributions, and the site renders it at `/open-source` (`/os` redirects there,
for short links). Copy
[`public/content/opensource.toml.example`](../public/content/opensource.toml.example)
to start. The page, and an "Open source" link under the About text, appear only
while this file exists, parses and lists at least one entry. Otherwise
`/open-source` shows "Page not found." and the About page has no link.

```toml
intro = "Things I've built, and projects I've contributed to."   # optional

[[projects]]                       # your own work; one block per card
name    = "portfolio"              # required
url     = "https://github.com/you/portfolio"   # optional; the name links here
website = "https://you.example"    # optional; shown beside the name as its domain
summary = "This site."             # required
tags    = ["Rust", "Leptos"]       # optional
image   = "/readme/portfolio/card.webp"  # optional; a screenshot on the card
image_alt = "The portfolio home page"    # optional; defaults to "<name> screenshot"
image_invert = false               # optional; see below

[[contributions]]                  # upstream projects; same fields as [[projects]]
name    = "Some Upstream"
url     = "https://github.com/org/some-upstream"
summary = "What you worked on there."

[[contributions.highlights]]       # optional links under a card; title + url required
title = "#123 A notable change"
url   = "https://github.com/org/some-upstream/pull/123"
```

Cards appear in file order. Unknown keys are rejected, so a typo such as
`highlight` for `highlights` hides the page instead of quietly dropping that
data. The server logs the parse error.

`image` puts a screenshot on a card, and takes any URL or site path. A good home
for one is the `/readme/<repo>/` folder (files under `README_ASSETS_DIR/<repo>/`, see
[README images](#readme-images)): browsers revalidate it on every load, so you can
replace a screenshot in place. `/content/` works too, but browsers cache it for a
year. Blank values count as unset. The screenshot sits right of the card's text at
340×212 (16:10), cropped from the top left. On phones it runs full width at 16:10
under the text. A click opens it in the gallery's full-screen viewer.

`image_invert = true` is for an image that is white line art on a transparent
background, such as a terminal animation. The card draws it inverted, so the art
reads black on the white page, and the full-screen viewer shows it as it is,
white on its dark overlay. Colours keep roughly their hue. A normal screenshot
would come out as its own negative. The key is newer than the page itself, and an
older server rejects it like any unknown key, so update the server before adding
it to the file.

The whole content directory is also served as static files under `/content/`,
which is how the profile image reaches the browser. That means anyone can
download `opensource.toml` as-is from `/content/opensource.toml`, comments and
commented-out entries included, so keep drafts and private notes out of it.

### Per-gallery layout

`<IMAGES_DIR>/<gallery>/gallery.toml` controls how that gallery's grid is laid
out (the homepage's `gallery.toml` lives in `GALLERY_PATH`, default
`<IMAGES_DIR>/home`). Every field is optional:

```toml
use_mosaic            = true   # mosaic layout instead of a uniform grid (default false)
mosaic_cache_duration = 3600   # seconds to cache the computed mosaic layout (default 3600)
columns               = 4      # grid columns       (default 6; ignored when use_mosaic)
row_height            = 320    # grid row height px  (default 280; ignored when use_mosaic)
gap                   = 12      # gap between items px (default 8)
```

### Per-photo overrides

A TOML file next to a photo, with the same basename, overrides that one photo's
metadata. For `portrait.jpg`, create `portrait.toml`:

```toml
title       = "A Better Title"   # overrides the filename-derived title
focal_point = "top-center"       # which third stays visible when cropped to a thumbnail
lens_model  = "Nikon 50mm f/1.8" # override when EXIF lens is missing/ambiguous
```

`focal_point` picks a cell of a rule-of-thirds grid: `top-left`, `top-center`,
`top-right`, `center-left`, `center` (default), `center-right`, `bottom-left`,
`bottom-center`, `bottom-right`. See
[`focal-points.toml.example`](../focal-points.toml.example) for the full reference.

---

## README images

Screenshots and short clips for your repositories' READMEs can live on the site
instead of in git. A file at `<README_ASSETS_DIR>/<repo>/<file>` (default
`public/readme/`) is served at `/readme/<repo>/<file>`. This repository's screenshot,
for example, is `/readme/portfolio/home.webp`. Only webp, png, jpg, jpeg, gif, avif,
mp4 and webm files are served, and anything else, SVG and directory paths included, is
a 404. Responses carry `Cache-Control: no-cache`, so you can replace a file in place
under the same name.

The server picks `README_ASSETS_DIR` once, at startup. When running without Docker,
create `public/readme` before starting the server, or set the variable. If
`public/readme` doesn't exist at startup, the server uses `./readme` instead, and a
`public/readme` created later is ignored until the next restart.
