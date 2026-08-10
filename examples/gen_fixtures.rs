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
//! Run: `cargo run --release --example gen_fixtures --features ssr`
//!
//! The EXIF values below are lifted from real frames in the production gallery so the
//! server's discovery path parses something representative rather than something
//! synthetic — a D850 with two different zooms and a Z f with a manual prime. Only the
//! values are copied here; no photo is committed to the repo.

use std::fs;
use std::path::{Path, PathBuf};

use image::{Rgb, RgbImage};
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
    /// `true` puts the file at `<gallery>/<name>/<name>.jpg` (the nested form the
    /// production gallery mostly uses), `false` puts it at `<gallery>/<name>.jpg`.
    /// Both layouts appear in production, so cover both discovery paths.
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
    // real gallery link. Named "nature" to match one of the production galleries; it
    // also gives abuse-cases.spec.ts's "non-existent photo slug" case a real gallery to
    // miss inside, which exercises PhotoNotFound rather than a missing gallery.
    Fixture {
        gallery: "nature",
        name: "bee",
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
    // IMAGES_DIR's default. Overridable so a local run can seed a scratch tree instead
    // of public/images — which is how CI's gallery set gets reproduced exactly, without
    // a developer's own galleries in the way.
    let images_root =
        std::env::var("FIXTURE_IMAGES_DIR").unwrap_or_else(|_| "public/images".to_string());
    let images_root = PathBuf::from(images_root);

    for fixture in FIXTURES {
        let gallery = images_root.join(fixture.gallery);
        fs::create_dir_all(&gallery)?;

        let path = if fixture.nested {
            let dir = gallery.join(fixture.name);
            fs::create_dir_all(&dir)?;
            dir.join(format!("{}.jpg", fixture.name))
        } else {
            gallery.join(format!("{}.jpg", fixture.name))
        };

        render(fixture).save(&path)?;
        // EXIF is written as a second pass: the image crate's JPEG encoder does not
        // take an EXIF blob, so little_exif splices the APP1 segment into the file
        // that was just saved.
        write_exif(&path, fixture)?;

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

    // Content fixtures live outside public/ so this never overwrites a real config.
    let content_dir = PathBuf::from(
        std::env::var("FIXTURE_CONTENT").unwrap_or_else(|_| "target/e2e-fixtures".to_string()),
    );
    fs::create_dir_all(&content_dir)?;
    let config_path = content_dir.join("config.toml");
    fs::write(&config_path, FIXTURE_CONFIG)?;
    fs::write(content_dir.join("about.txt"), FIXTURE_ABOUT)?;
    println!("wrote {} and about.txt", config_path.display());

    println!(
        "\nrun the server with:\n  CONFIG_PATH={} ABOUT_CONTENT_PATH={} cargo leptos serve --release",
        config_path.display(),
        content_dir.display()
    );
    Ok(())
}
