//! Generate the e2e fixture gallery.
//!
//! `public/images/*` is gitignored, so a fresh checkout — CI's included — has no photos
//! at all. Most of the Playwright suite needs some: every test in photo-detail.spec.ts
//! and three in photo-gallery.spec.ts open with an unguarded
//! `waitForSelector(".photo-hero-link")`, which throws on an empty gallery. (The
//! `if (count > 0)` checks that follow are unreachable in that case, so they do not
//! save us.) Three of those tests additionally guard on `count > 1`, so one photo is not
//! enough either.
//!
//! Writes two galleries: `home` (the homepage grid) and a second one, which is required
//! rather than decorative — see the `gallery` field below.
//!
//! Run: `FIXTURE_IMAGES_DIR=target/e2e-fixtures/images cargo run --release --example gen_fixtures --features ssr`
//! (docs/development.md has the matching server command). Without `FIXTURE_IMAGES_DIR`
//! the photos go into public/images, beside any real ones.
//!
//! The EXIF values below are lifted from real frames in the production gallery so the
//! server's discovery path parses something representative rather than something
//! synthetic — a D850 with two different zooms and a Z f with a manual prime. Only the
//! values are copied here; no photo is committed to the repo.

use std::fs;
use std::path::{Path, PathBuf};

use image::{Rgb, RgbImage, Rgba, RgbaImage};
use little_exif::exif_tag::ExifTag;
use little_exif::metadata::Metadata;
use little_exif::rational::uR64;

/// One fixture photo. Dimensions vary so the mosaic bin-packer in `src/mosaic.rs` has
/// real aspect-ratio variety to work with instead of three identical squares.
struct Fixture {
    /// Which gallery directory under the images root this photo belongs to.
    ///
    /// At least one gallery other than `home` is required, not decorative: discovery
    /// deliberately omits `home` from the nav list (src/gallery.rs:869 — it is the
    /// homepage), so with only a `home` gallery the nav has no gallery links at all.
    /// Specs that go looking for one then fall through to the photo thumbnails, which
    /// share the `/gallery/` href prefix, and end up on a photo detail page instead of
    /// a gallery index. That is exactly how CI run 113 failed while passing locally,
    /// where an untracked `city` gallery happened to supply a real nav link.
    gallery: &'static str,
    /// Gallery-relative name; also the visible photo title.
    name: &'static str,
    /// The file's extension, as spelled. `JPG` for one photo, the way cameras name
    /// their files: the site has to find it and keep the real name in its addresses.
    extension: &'static str,
    /// `true` puts the file at `<gallery>/<name>/<name>.<extension>` (the nested form
    /// the production gallery mostly uses), `false` puts it at
    /// `<gallery>/<name>.<extension>`. Both layouts appear in production, so cover both
    /// discovery paths.
    nested: bool,
    width: u32,
    height: u32,
    /// Base colour, so a human looking at a failure screenshot can tell them apart.
    rgb: [u8; 3],
    make: &'static str,
    model: &'static str,
    lens: &'static str,
    /// Focal length in mm.
    focal_mm: u32,
    /// f-number as (numerator, denominator) — f/2.8 is 28/10.
    f_number: (u32, u32),
    /// Shutter speed as a fraction of a second, e.g. (1, 500) for 1/500.
    exposure: (u32, u32),
    iso: u16,
    /// EXIF datetime, "YYYY:MM:DD HH:MM:SS".
    taken: &'static str,
    description: &'static str,
    /// LensTagger-style `-Key=Value` override lines. `src/gallery.rs` parses these out
    /// of `UserComment` to decide whether an EXIF value came from the photographer or
    /// from a scanning camera's auto-exposure, so one fixture carries them to keep
    /// `LenstaggerOverrides::from_user_comment` exercised end to end.
    lenstagger: Option<&'static str>,
}

const FIXTURES: &[Fixture] = &[
    Fixture {
        gallery: "home",
        name: "laika",
        extension: "jpg",
        nested: true,
        width: 1200,
        height: 800,
        rgb: [92, 74, 58],
        make: "NIKON CORPORATION",
        model: "NIKON D850",
        lens: "70.0-200.0 mm f/2.8",
        focal_mm: 165,
        f_number: (28, 10),
        exposure: (1, 500),
        iso: 2200,
        taken: "2023:06:29 21:15:56",
        description: "Fixture: telephoto, wide open, high ISO",
        lenstagger: None,
    },
    Fixture {
        gallery: "home",
        name: "blue-angels",
        extension: "jpg",
        nested: true,
        width: 1000,
        height: 1000,
        rgb: [58, 92, 128],
        make: "NIKON CORPORATION",
        model: "NIKON D850",
        lens: "200.0-500.0 mm f/5.6",
        focal_mm: 700,
        f_number: (110, 10),
        exposure: (1, 1000),
        iso: 200,
        taken: "2023:08:06 16:08:42",
        description: "Fixture: super-telephoto, stopped down, fast shutter",
        lenstagger: None,
    },
    Fixture {
        // Flat layout on purpose — production has a bare contrail.jpg beside the
        // nested directories, so the discovery code must handle both.
        gallery: "home",
        name: "contrail",
        extension: "jpg",
        nested: false,
        width: 800,
        height: 1200,
        rgb: [128, 128, 140],
        make: "NIKON CORPORATION",
        model: "NIKON Z f",
        lens: "85mm f/1.4D",
        focal_mm: 85,
        f_number: (110, 10),
        exposure: (1, 2000),
        iso: 1250,
        taken: "2026:05:11 10:59:17",
        description: "Fixture: manual prime, portrait orientation",
        // Mirrors what LensTagger writes when the values were entered by hand.
        lenstagger: Some("-FNumber=11.0\n-ExposureTime=1/2000\n-LensModel=85mm f/1.4D"),
    },
    // A second gallery, so `get_galleries()` returns something and the nav renders a
    // real gallery link. Named "nature" to match one of the production galleries. A
    // gallery other than `home` also has to exist before a missing gallery or photo
    // counts as missing (the not-found page, 404) rather than as an outage.
    Fixture {
        gallery: "nature",
        name: "bee",
        extension: "jpg",
        nested: true,
        width: 1400,
        height: 900,
        rgb: [70, 104, 62],
        make: "NIKON CORPORATION",
        model: "NIKON D850",
        lens: "70.0-200.0 mm f/2.8",
        focal_mm: 200,
        f_number: (28, 10),
        exposure: (1, 320),
        iso: 640,
        taken: "2023:06:29 21:16:40",
        description: "Fixture: second gallery, telephoto close-up",
        lenstagger: None,
    },
    Fixture {
        gallery: "nature",
        name: "bird",
        // Upper case, as a camera writes it (photo-gallery.spec.ts). In `nature`, so the
        // home gallery the not-found page draws from stays as it was.
        extension: "JPG",
        nested: false,
        width: 900,
        height: 1400,
        rgb: [104, 88, 62],
        make: "NIKON CORPORATION",
        model: "NIKON Z f",
        lens: "85mm f/1.4D",
        focal_mm: 85,
        f_number: (14, 10),
        exposure: (1, 250),
        iso: 400,
        taken: "2026:05:11 11:02:03",
        description: "Fixture: second gallery, wide-aperture prime",
        lenstagger: None,
    },
];

/// Site config for the e2e run.
///
/// The specs assert `toHaveTitle("Photography Portfolio")`, and the app derives its
/// title from the loaded config — falling back to `SiteConfig::default()`, whose
/// `site_name` is the placeholder "Your Name". So the assertion needs a real config to be
/// meaningful; without one it would either fail or, worse, only pass when config loading
/// happened to break.
///
/// Writing this to a dedicated fixture directory (rather than public/content/) keeps a
/// developer's real config.toml untouched; the e2e job points `CONFIG_PATH` at it.
const FIXTURE_CONFIG: &str = r#"# Generated by `cargo run --example gen_fixtures`. Not committed; not your real config.
site_name = "Photography Portfolio"
site_tagline = "Photography"

[sections]
email = "e2e@example.com"
location = "Test City, ST"
instagram = "@e2efixture"
about_title = "About Me"
contact_title = "Get In Touch"
"#;

/// Deterministic about-page copy. `load_about_content` would otherwise fall back to its
/// built-in default text, which is fine but leaves the page content dependent on a
/// default that could change.
const FIXTURE_ABOUT: &str =
    "Fixture about text for the end-to-end suite. Generated, not authored.\n";

/// The `/open-source` page's content. Its presence is also what makes the About page
/// render its "Open source" link. One linked project, one contribution with a
/// highlight, one contribution with no `url` so the plain-text name path is
/// exercised too, and one long unbroken token (no spaces, hyphens or slashes, so
/// the browser has no break opportunity) in the intro, a card name, a tag and a
/// highlight. Without that token every string fits at phone width and the
/// no-horizontal-scroll spec passes whether or not the wrapping CSS exists.
///
/// The project card has a screenshot: the `shot.webp` that `write_readme_fixtures`
/// writes. So the e2e server needs `README_ASSETS_DIR`, and renaming that file breaks
/// open-source.spec.ts as well as readme-assets.spec.ts. "Fixture Upstream" opens
/// `art.webp`, white art on a transparent background, full screen, and shows
/// `art-card.webp`, the same art at half the size, on its card through `image_card`,
/// with `image_invert`.
const FIXTURE_OPEN_SOURCE: &str = r##"# Generated by `cargo run --example gen_fixtures`. Not committed.
intro = "Fixture intro for the end-to-end suite. LongUnbrokenTokenLongUnbrokenTokenLongUnbrokenTokenLongUnbrokenTokenLongUnbrokenTokenLongUnbrokenTokenLongUnbrokenToken"

[[projects]]
name = "Fixture Project"
url = "https://example.com/fixture-project"
website = "https://www.fixture-project.example/"
summary = "A fixture project."
tags = ["Rust", "Leptos"]
image = "/readme/fixture/shot.webp"
image_alt = "The Fixture Project home page"

[[contributions]]
name = "Fixture Upstream"
url = "https://example.com/fixture-upstream"
summary = "A fixture contribution."
image = "/readme/fixture/art.webp"
image_card = "/readme/fixture/art-card.webp"
image_invert = true

[[contributions.highlights]]
title = "#1 Fixture highlight"
url = "https://example.com/fixture-upstream/pull/1"

[[contributions]]
name = "Unlinked Upstream"
summary = "An entry with no url."

[[contributions]]
name = "LongUnbrokenTokenLongUnbrokenTokenLongUnbrokenTokenLongUnbrokenTokenLongUnbrokenTokenLongUnbrokenTokenLongUnbrokenToken"
url = "https://example.com/long"
summary = "An entry whose name, tag and highlight are one unbroken token."
tags = ["LongUnbrokenTokenLongUnbrokenTokenLongUnbrokenTokenLongUnbrokenTokenLongUnbrokenTokenLongUnbrokenTokenLongUnbrokenToken"]

[[contributions.highlights]]
title = "LongUnbrokenTokenLongUnbrokenTokenLongUnbrokenTokenLongUnbrokenTokenLongUnbrokenTokenLongUnbrokenTokenLongUnbrokenToken"
url = "https://example.com/long/pull/1"
"##;

/// Refused by the `/readme` mount: `.toml` is not on its media allowlist. The marker is
/// what readme-assets.spec.ts checks never comes back in a 404's body.
const FIXTURE_README_NOTES: &str = "# Generated by `cargo run --example gen_fixtures`. The /readme mount must refuse this file.\nmarker = \"readme-notes-fixture\"\n";

/// Sits inside `album.webp/`, a directory named like an image. The name passes the
/// extension allowlist, so only the file server's refusal to serve directories keeps
/// this page from going out in place of a 404.
const FIXTURE_README_ALBUM_INDEX: &str = "<!doctype html><title>readme-album-fixture</title>\n";

/// Size of `clip.mp4`: far above the compression layer's 32-byte floor, so the file is
/// never skipped for being small.
const FIXTURE_CLIP_LEN: usize = 1024;

/// An MP4 in structure only: an `ftyp` box naming the `isom` brand, then a `free` box of
/// zeros out to [`FIXTURE_CLIP_LEN`]. It holds no media and will not play; the spec only
/// checks how it is served, and the zeros would compress to almost nothing, so a
/// compression layer that took it would show.
fn fixture_clip() -> Vec<u8> {
    // Box size 24, "ftyp", major brand "isom", minor version 0x200, then the compatible
    // brands "isom" and "mp41".
    let mut clip = b"\0\0\0\x18ftypisom\0\0\x02\0isommp41".to_vec();
    let free = u32::try_from(FIXTURE_CLIP_LEN - clip.len()).expect("the clip length fits in a u32");
    clip.extend_from_slice(&free.to_be_bytes());
    clip.extend_from_slice(b"free");
    clip.resize(FIXTURE_CLIP_LEN, 0);
    clip
}

/// Write the `/readme` mount's fixtures into `<content_dir>/readme/fixture/`, and return
/// `<content_dir>/readme`, which the e2e server's `README_ASSETS_DIR` points at.
///
/// Also writes `<content_dir>/readme-outside.webp`, beside the mount's folder rather than
/// in it: the traversal test needs a target that really exists, with an allowed name,
/// one level outside, and `/content` serves it so the spec can check it is there.
fn write_readme_fixtures(content_dir: &Path) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let readme_root = content_dir.join("readme");
    let dir = readme_root.join("fixture");
    let album = dir.join("album.webp");
    fs::create_dir_all(&album)?;

    // A real WebP rather than arbitrary bytes, so the file is what its name says.
    let shot = RgbImage::from_pixel(8, 8, Rgb([200, 120, 40]));
    let webp = webp::Encoder::from_rgb(shot.as_raw(), shot.width(), shot.height()).encode(80.0);
    write(dir.join("shot.webp"), &*webp)?;
    write(content_dir.join("readme-outside.webp"), &*webp)?;
    // What `image_invert` is for: white art on a transparent background. 16:10 like
    // the card, so the card shows all of it, with a solid block in the middle that
    // open-source.spec.ts samples. Lossless, so the alpha stays exactly 0 or 255.
    let art = |scale: u32| {
        let art = RgbaImage::from_fn(16 * scale, 10 * scale, |x, y| {
            let inside =
                (4 * scale..12 * scale).contains(&x) && (3 * scale..7 * scale).contains(&y);
            Rgba(if inside { [255; 4] } else { [0; 4] })
        });
        webp::Encoder::from_rgba(art.as_raw(), art.width(), art.height()).encode_lossless()
    };
    write(dir.join("art.webp"), &*art(2))?;
    // The same art at half the size, for `image_card`: what the card shows.
    write(dir.join("art-card.webp"), &*art(1))?;
    write(dir.join("clip.mp4"), fixture_clip())?;
    write(dir.join("notes.toml"), FIXTURE_README_NOTES)?;
    write(album.join("index.html"), FIXTURE_README_ALBUM_INDEX)?;
    Ok(readme_root)
}

/// The homepage's grid settings: the values of the checked-in
/// `public/images/home/gallery.toml`, mosaic on. A fixture tree outside public/images
/// has no copy of that file, and without one the homepage falls back to the plain grid,
/// so the suite would stop covering the mosaic layout.
const FIXTURE_HOME_GALLERY: &str = r"# Generated by `cargo run --example gen_fixtures`. Not committed.
# Mirrors public/images/home/gallery.toml.
use_mosaic = true
mosaic_cache_duration = 3600
columns = 4
row_height = 320
gap = 12
";

/// EXIF `UserComment` is an UNDEFINED field whose first 8 bytes name the character
/// encoding. `read_lenstagger_overrides` in src/gallery.rs only accepts the ASCII form,
/// so emit exactly that prefix followed by the raw text.
fn ascii_user_comment(text: &str) -> Vec<u8> {
    let mut bytes = b"ASCII\0\0\0".to_vec();
    bytes.extend_from_slice(text.as_bytes());
    bytes
}

/// A flat colour would compress to almost nothing and give the transcoder nothing to
/// do, so lay down a gentle two-axis gradient around the base colour instead. Still
/// tiny, but a plausible photograph as far as the encoder is concerned.
fn render(fixture: &Fixture) -> RgbImage {
    let mut img = RgbImage::new(fixture.width, fixture.height);
    let [base_r, base_g, base_b] = fixture.rgb;
    for (x, y, pixel) in img.enumerate_pixels_mut() {
        // `x < width` and `y < height`, so the quotients are bounded by 64 and 48.
        let dx = u8::try_from(x * 64 / fixture.width.max(1)).unwrap();
        let dy = u8::try_from(y * 48 / fixture.height.max(1)).unwrap();
        *pixel = Rgb([
            base_r.saturating_add(dx),
            base_g.saturating_add(dy),
            base_b.saturating_add(dx / 2).saturating_add(dy / 2),
        ]);
    }
    img
}

fn write_exif(path: &Path, fixture: &Fixture) -> Result<(), Box<dyn std::error::Error>> {
    let mut meta = Metadata::new();

    meta.set_tag(ExifTag::Make(fixture.make.to_string()));
    meta.set_tag(ExifTag::Model(fixture.model.to_string()));
    meta.set_tag(ExifTag::LensModel(fixture.lens.to_string()));
    meta.set_tag(ExifTag::ImageDescription(fixture.description.to_string()));
    meta.set_tag(ExifTag::Copyright("© David Hughes".to_string()));
    meta.set_tag(ExifTag::DateTimeOriginal(fixture.taken.to_string()));

    meta.set_tag(ExifTag::FocalLength(vec![uR64 {
        nominator: fixture.focal_mm,
        denominator: 1,
    }]));
    meta.set_tag(ExifTag::FNumber(vec![uR64 {
        nominator: fixture.f_number.0,
        denominator: fixture.f_number.1,
    }]));
    meta.set_tag(ExifTag::ExposureTime(vec![uR64 {
        nominator: fixture.exposure.0,
        denominator: fixture.exposure.1,
    }]));
    // little_exif calls this ISO; it writes tag 0x8827, which kamadak-exif (and so
    // src/gallery.rs) reads back as PhotographicSensitivity.
    meta.set_tag(ExifTag::ISO(vec![fixture.iso]));

    if let Some(overrides) = fixture.lenstagger {
        meta.set_tag(ExifTag::UserComment(ascii_user_comment(overrides)));
    }

    meta.write_to_file(path)?;
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Defaults to IMAGES_DIR's default, public/images. CI and the e2e recipe in
    // docs/development.md set it to target/e2e-fixtures/images and point the server's
    // IMAGES_DIR there too, so the fixtures never mix with a developer's own galleries.
    let images_root =
        std::env::var("FIXTURE_IMAGES_DIR").unwrap_or_else(|_| "public/images".to_string());
    let images_root = PathBuf::from(images_root);

    for fixture in FIXTURES {
        let gallery = images_root.join(fixture.gallery);
        fs::create_dir_all(&gallery)?;

        let file_name = format!("{}.{}", fixture.name, fixture.extension);
        let path = if fixture.nested {
            let dir = gallery.join(fixture.name);
            fs::create_dir_all(&dir)?;
            dir.join(file_name)
        } else {
            gallery.join(file_name)
        };

        render(fixture).save(&path)?;
        // EXIF is written as a second pass: the image crate's JPEG encoder does not
        // take an EXIF blob, so little_exif splices the APP1 segment into the file
        // that was just saved.
        write_exif(&path, fixture)?;
        backdate(&path)?;

        let size = fs::metadata(&path)?.len();
        println!(
            "wrote {:<48} {}x{}  {} bytes",
            path.display(),
            fixture.width,
            fixture.height,
            size
        );
    }

    let mut galleries: Vec<&str> = FIXTURES.iter().map(|f| f.gallery).collect();
    galleries.sort_unstable();
    galleries.dedup();
    println!(
        "\n{} fixture photos across {} galleries ({}) in {}",
        FIXTURES.len(),
        galleries.len(),
        galleries.join(", "),
        images_root.display()
    );

    // However FIXTURE_IMAGES_DIR spells it: "./public/images" or a symlink counts too.
    let into_public_images = same_dir(&images_root, Path::new("public/images"));

    // Rewritten every run, so an edit to FIXTURE_HOME_GALLERY reaches a fixture tree that
    // already exists; but never into public/images, where it is the checked-in file.
    if !into_public_images {
        let home_gallery_toml = images_root.join("home").join("gallery.toml");
        write(&home_gallery_toml, FIXTURE_HOME_GALLERY)?;
        println!("wrote {}", home_gallery_toml.display());
    }

    // Content fixtures live outside public/ so this never overwrites a real config.
    let content_dir = PathBuf::from(
        std::env::var("FIXTURE_CONTENT").unwrap_or_else(|_| "target/e2e-fixtures".to_string()),
    );
    fs::create_dir_all(&content_dir)?;
    let config_path = content_dir.join("config.toml");
    write(&config_path, FIXTURE_CONFIG)?;
    write(content_dir.join("about.txt"), FIXTURE_ABOUT)?;
    write(content_dir.join("opensource.toml"), FIXTURE_OPEN_SOURCE)?;
    println!(
        "wrote {}, about.txt and opensource.toml",
        config_path.display()
    );

    let readme_root = write_readme_fixtures(&content_dir)?;
    println!(
        "wrote shot.webp, art.webp, art-card.webp, clip.mp4, notes.toml and album.webp/index.html in {}, and readme-outside.webp beside {}",
        readme_root.join("fixture").display(),
        readme_root.display()
    );

    // Targets for abuse-cases.spec.ts: real JPEGs one level above the images root, in
    // the recipe's layout (`IMAGES_DIR` is `<content_dir>/images`), which `/content`
    // serves so the spec can check they are there. The compressed endpoint must not
    // reach either through `..`. `images-outside.jpg` is a name it tries with a `stat`;
    // `images-outside-mixed.Jpg`, a mixed spelling with no other, is only found by
    // listing a folder, so it tests that path's guard.
    for name in ["images-outside.jpg", "images-outside-mixed.Jpg"] {
        let path = content_dir.join(name);
        RgbImage::from_pixel(8, 8, Rgb([200, 40, 40])).save(&path)?;
        backdate(&path)?;
    }
    println!(
        "wrote images-outside.jpg and images-outside-mixed.Jpg in {}",
        content_dir.display()
    );

    // A tree outside public/images is only served if IMAGES_DIR points at it, and the
    // compressed-image cache has to move with it: on startup the server prunes its cache
    // dir of every entry the images root does not account for, so the default
    // public/cache would lose everything belonging to real photos.
    let images_env = if into_public_images {
        String::new()
    } else {
        format!(
            " IMAGES_DIR={} IMAGE_CACHE_DIR={}",
            images_root.display(),
            content_dir.join("cache").display()
        )
    };
    println!(
        "\nrun the server with:\n  CONFIG_PATH={} ABOUT_CONTENT_PATH={}{images_env} README_ASSETS_DIR={} cargo leptos serve --release",
        config_path.display(),
        content_dir.display(),
        readme_root.display()
    );
    Ok(())
}

/// Whether `a` and `b` are the same existing directory, however each is spelled.
fn same_dir(a: &Path, b: &Path) -> bool {
    matches!((a.canonicalize(), b.canonicalize()), (Ok(a), Ok(b)) if a == b)
}

/// Write `contents` to `path`, then [`backdate`] it.
fn write(path: impl AsRef<Path>, contents: impl AsRef<[u8]>) -> std::io::Result<()> {
    fs::write(&path, contents)?;
    backdate(path.as_ref())
}

/// Date the file at `path` an hour back. The server sends a file changed in the last
/// ten seconds with `no-store`, as it may still be being written (`SETTLE_TIME` in
/// `src/image_cache.rs`), and the specs expect the year-long header on fixtures they
/// ask for moments after this writes them.
fn backdate(path: &Path) -> std::io::Result<()> {
    let an_hour_ago = std::time::SystemTime::now() - std::time::Duration::from_hours(1);
    fs::File::options()
        .write(true)
        .open(path)?
        .set_modified(an_hour_ago)
}
