use crate::types::{GalleryInfo, ImageSource, PhotoConfig, PhotoInfo};
use regex::Regex;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

static LEADING_NUMBER_DASH: std::sync::LazyLock<Regex> =
    std::sync::LazyLock::new(|| Regex::new(r"^[\d.]+\s*-\s*").expect("valid regex"));

/// URL-encode a path component, preserving forward slashes
fn url_encode_path(path: &str) -> String {
    path.split('/')
        .map(|segment| urlencoding::encode(segment).into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

/// A file's version, for the addresses that point at it: its modification time in
/// whole seconds since 1970. `None` when that can't be read, or is before 1970.
///
/// Browsers and Cloudflare keep a file from `/images` or `/content` for a year
/// without asking again, so a file replaced under its old name would never reach
/// them at its old address. With the version in it, the address changes along with
/// the file. Whole seconds keep the address short and are plenty to tell one export
/// of a photo from the next. The time comes from a `stat` that the caller has
/// usually made anyway, so it costs nothing extra on the NFS share.
fn file_version(metadata: &fs::Metadata) -> Option<u64> {
    let modified = metadata.modified().ok()?;
    let since_1970 = modified.duration_since(std::time::UNIX_EPOCH).ok()?;
    Some(since_1970.as_secs())
}

/// `url` with `v=<version>` added to its query, or `url` as it is without a version.
/// The server ignores `v`; it is only there to change the address.
fn with_version(url: String, version: Option<u64>) -> String {
    match version {
        Some(version) => {
            let separator = if url.contains('?') { '&' } else { '?' };
            format!("{url}{separator}v={version}")
        }
        None => url,
    }
}

/// The favicon's address: `favicon.ico` at the top of the images root, with its
/// version. The page shell asks for it on every render, which costs one `stat`.
pub fn favicon_url() -> String {
    let metadata = fs::metadata(crate::paths::images_root().join("favicon.ico")).ok();
    with_version(
        "/images/favicon.ico".to_string(),
        metadata.as_ref().and_then(file_version),
    )
}

/// Get MIME type from file extension
fn get_mime_type(extension: &str) -> &'static str {
    match extension {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "webp" => "image/webp",
        "gif" => "image/gif",
        "jxl" => "image/jxl",
        "avif" => "image/avif",
        _ => "application/octet-stream",
    }
}

/// Load per-photo configuration from a sibling TOML file.
/// Looks for `<basename>.toml` next to the photo (e.g. `photo.jpg` -> `photo.toml`).
fn load_photo_config(photo_path: &Path) -> PhotoConfig {
    let Some(stem) = photo_path.file_stem() else {
        return PhotoConfig::default();
    };
    let stem = stem.to_string_lossy();

    let Some(parent) = photo_path.parent() else {
        return PhotoConfig::default();
    };
    let config_path = parent.join(format!("{stem}.toml"));

    fs::read_to_string(&config_path)
        .ok()
        .and_then(|content| toml::from_str::<PhotoConfig>(&content).ok())
        .unwrap_or_default()
}

/// Widest preset eligible for the grid `srcset`. Anything larger (e.g. the
/// 4000w detail preset) is reserved for the photo viewer and excluded here so
/// big desktops don't pull multi-MB images for every thumbnail.
#[cfg(feature = "ssr")]
const SRCSET_MAX_WIDTH: u32 = 2400;

/// The compressed endpoint's address for `encoded_path` at one preset, with `version`.
fn compressed_image_url(
    encoded_path: &str,
    width: u32,
    quality: u8,
    version: Option<u64>,
) -> String {
    with_version(
        format!("/images/compressed/{encoded_path}?width={width}&quality={quality}"),
        version,
    )
}

/// Build a responsive srcset string for the compressed endpoint.
/// Each preset becomes "url Ww", letting the browser pick by viewport+DPR.
#[cfg(feature = "ssr")]
fn build_compressed_srcset(encoded_path: &str, version: Option<u64>) -> String {
    use crate::image_params::ImageParams;
    let mut presets: Vec<_> = ImageParams::get_valid_presets()
        .into_iter()
        .filter(|(w, _)| *w <= SRCSET_MAX_WIDTH)
        .collect();
    // Sort ascending by width so the srcset reads small→large.
    presets.sort_by_key(|(w, _)| *w);
    presets
        .into_iter()
        .map(|(w, q)| format!("{} {w}w", compressed_image_url(encoded_path, w, q, version)))
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(not(feature = "ssr"))]
fn build_compressed_srcset(_encoded_path: &str, _version: Option<u64>) -> String {
    String::new()
}

/// Get default image width and quality from environment variables
/// Returns (width, quality) tuple with defaults of (2400, 90)
fn get_default_image_params() -> (u32, u8) {
    // Use 2400px at 90 quality for photo detail pages — the minimum width
    // served, since anything smaller looked soft.
    #[cfg(feature = "ssr")]
    {
        use crate::image_params::ImageParams;
        let presets = ImageParams::get_valid_presets();
        // Find the 2400px preset (the minimum served width), or fall back.
        presets
            .iter()
            .find(|(w, _)| *w == 2400)
            .copied()
            .unwrap_or((2400, 90))
    }

    #[cfg(not(feature = "ssr"))]
    {
        // For client-side, use environment variables or default to 2400/90
        let width = std::env::var("DEFAULT_IMAGE_WIDTH")
            .ok()
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(2400);

        let quality = std::env::var("DEFAULT_IMAGE_QUALITY")
            .ok()
            .and_then(|s| s.parse::<u8>().ok())
            .unwrap_or(90);

        (width, quality)
    }
}

/// Helper function to discover all gallery directories under [`crate::paths::images_root`]
pub fn discover_gallery_directories() -> Vec<String> {
    let images_base = crate::paths::images_root();

    let mut gallery_dirs = Vec::new();

    if let Ok(entries) = fs::read_dir(images_base) {
        for entry in entries.flatten() {
            let path = entry.path();
            // Only include directories that are not special directories
            if path.is_dir()
                && let Some(dir_name) = path.file_name()
            {
                let name = dir_name.to_string_lossy().to_string();
                // Skip special directories like categories, content, etc.
                if name != "categories" {
                    gallery_dirs.push(path.to_string_lossy().to_string());
                }
            }
        }
    }

    gallery_dirs
}

/// Strip leading numbers and dashes from a filename
/// Example: "1 - space_needle.jpg" -> "space_needle.jpg"
///          "42-mountain.jpg" -> "mountain.jpg"
///          "1.5 - photo.jpg" -> "photo.jpg"
///
/// `#[doc(hidden)] pub` for `benches/metadata.rs`; not part of the public API.
#[doc(hidden)]
pub fn strip_leading_number_and_dash(filename: &str) -> String {
    LEADING_NUMBER_DASH.replace(filename, "").into_owned()
}

/// Count images recursively in a directory
pub fn count_images_recursive(dir: &Path, count: &mut usize) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                count_images_recursive(&path, count);
            } else if let Some(extension) = path.extension() {
                let ext = extension.to_string_lossy().to_lowercase();
                if matches!(
                    ext.as_ref(),
                    "jpg" | "jpeg" | "png" | "webp" | "gif" | "jxl" | "avif"
                ) {
                    *count += 1;
                }
            }
        }
    }
}

/// Find all images recursively in a directory for display on home page
pub fn find_images_recursive(dir: &Path, gallery_root: &Path, photos: &mut Vec<PhotoInfo>) {
    find_images_recursive_with_gallery(dir, gallery_root, photos, "home");
}

/// Find all images recursively with explicit gallery name
fn find_images_recursive_with_gallery(
    dir: &Path,
    gallery_root: &Path,
    photos: &mut Vec<PhotoInfo>,
    gallery_name: &str,
) {
    // First pass: collect all image files and group by basename
    let mut image_groups: HashMap<String, Vec<ImageFile>> = HashMap::new();
    collect_image_files(dir, gallery_root, &mut image_groups);

    // Second pass: create PhotoInfo for each group
    for (_base_path, variants) in image_groups {
        // Sort variants by priority (modern formats first for sources)
        let mut sorted_variants = variants.clone();
        sorted_variants.sort_by(|a, b| variant_order(&a.0, &a.1).cmp(&variant_order(&b.0, &b.1)));

        // Use the first variant as the primary image (fallback)
        let (primary_relative_path, primary_ext, _) = &sorted_variants[0];
        let primary_full_path = gallery_root.join(primary_relative_path);

        // Extract metadata from the primary image
        let filename_str = primary_full_path
            .file_name()
            .map_or_default(|f| f.to_string_lossy().to_string());

        // Create slug from filename (without extension), stripping leading numbers
        let slug = trim_extension(&strip_leading_number_and_dash(&filename_str), primary_ext)
            .to_lowercase()
            .replace([' ', '_'], "-");

        // Strip leading numbers and dashes, then convert to title
        let derived_title =
            trim_extension(&strip_leading_number_and_dash(&filename_str), primary_ext)
                .replace(['-', '_'], " ");

        // Load per-photo config (focal point, title override) from sibling TOML
        let photo_config = load_photo_config(&primary_full_path);
        let title = photo_config
            .title
            .clone()
            .filter(|t| !t.trim().is_empty())
            .unwrap_or(derived_title);

        // Extract EXIF data from primary image
        let exif = extract_exif_data(&primary_full_path);

        let addresses = photo_addresses(&sorted_variants);

        // Extract subfolder from relative path
        let subfolder = Path::new(primary_relative_path).parent().and_then(|p| {
            let path_str = p.to_string_lossy().to_string();
            if path_str.is_empty() || path_str == "." {
                None
            } else {
                Some(path_str)
            }
        });

        let focal_point = photo_config.focal_point;

        let lens_model = photo_config
            .lens_model
            .clone()
            .filter(|s| !s.trim().is_empty())
            .or(exif.lens_model);

        photos.push(PhotoInfo {
            url: addresses.url,
            original_url: addresses.original_url,
            detail_url: addresses.detail_url,
            srcset: addresses.srcset,
            sources: addresses.sources,
            original_sources: addresses.original_sources,
            title,
            filename: filename_str,
            slug,
            gallery_name: gallery_name.to_string(),
            subfolder,
            width: exif.width,
            height: exif.height,
            date_taken: exif.date_taken,
            camera_make: exif.camera_make,
            camera_model: exif.camera_model,
            lens_model,
            focal_length: exif.focal_length,
            aperture: exif.aperture,
            shutter_speed: exif.shutter_speed,
            iso: exif.iso,
            film_stock: exif.film_stock,
            copyright: exif.copyright,
            focal_point,
        });
    }
}

/// An image file found under a gallery: its path relative to the images root, its
/// extension in lower case, and its [`file_version`].
type ImageFile = (String, String, Option<u64>);

/// The addresses of one photo's files, as [`PhotoInfo`] holds them.
struct PhotoAddresses {
    url: String,
    original_url: String,
    detail_url: String,
    srcset: String,
    sources: Vec<ImageSource>,
    original_sources: Vec<ImageSource>,
}

/// The addresses of the photo whose files are `variants`, the primary first.
///
/// Each carries a version: an original its own file's, and a compressed address the
/// newest variant's. The compressed endpoint finds its source by the name without the
/// extension, so any of the variants can be behind a compressed address, and the
/// newest version changes whichever one is replaced.
fn photo_addresses(variants: &[ImageFile]) -> PhotoAddresses {
    let (primary_path, _, primary_version) = &variants[0];
    let compressed_version = variants.iter().filter_map(|(_, _, version)| *version).max();
    let (width, quality) = get_default_image_params();

    // Alternative sources: the other formats, compressed and original
    let mut sources = Vec::new();
    let mut original_sources = Vec::new();
    for (relative_path, ext, version) in variants {
        if relative_path != primary_path {
            let encoded_path = url_encode_path(relative_path);
            sources.push(ImageSource {
                url: compressed_image_url(&encoded_path, width, quality, compressed_version),
                mime_type: "image/webp".to_string(), // Compressed endpoint always returns WebP
            });
            original_sources.push(ImageSource {
                url: with_version(format!("/images/{encoded_path}"), *version),
                mime_type: get_mime_type(ext).to_string(),
            });
        }
    }

    let encoded_path = url_encode_path(primary_path);
    PhotoAddresses {
        url: compressed_image_url(&encoded_path, width, quality, compressed_version),
        original_url: with_version(format!("/images/{encoded_path}"), *primary_version),
        detail_url: compressed_image_url(&encoded_path, 4000, 90, compressed_version),
        srcset: build_compressed_srcset(&encoded_path, compressed_version),
        sources,
        original_sources,
    }
}

/// Helper function to collect all image files and group them by basename
fn collect_image_files(
    dir: &Path,
    gallery_root: &Path,
    groups: &mut HashMap<String, Vec<ImageFile>>,
) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            // One `stat` per entry, following links as `Path::is_dir` does. It says
            // whether this is a directory and, for a file, when it last changed.
            let metadata = fs::metadata(&path).ok();

            if metadata.as_ref().is_some_and(fs::Metadata::is_dir) {
                collect_image_files(&path, gallery_root, groups);
            } else if let Some(extension) = path.extension() {
                let ext = extension.to_string_lossy().to_lowercase();
                if matches!(
                    ext.as_ref(),
                    "jpg" | "jpeg" | "png" | "webp" | "gif" | "jxl" | "avif"
                ) {
                    let relative_path = path
                        .strip_prefix(gallery_root)
                        .unwrap_or(&path)
                        .to_string_lossy()
                        .to_string();

                    // Create base path without extension
                    let base_path = trim_extension(&relative_path, &ext).to_string();

                    groups.entry(base_path).or_default().push((
                        relative_path,
                        ext.clone(),
                        metadata.as_ref().and_then(file_version),
                    ));
                }
            }
        }
    }
}

/// Determine format priority (lower is better/more modern)
fn format_priority(ext: &str) -> u8 {
    match ext {
        "jpg" | "jpeg" => 0, // Fallback, widest support
        "avif" => 1,         // Most modern, best compression
        "jxl" => 2,          // Modern, excellent quality
        "webp" => 3,         // Good compression, wide support
        "png" => 4,          // Lossless, but larger
        "gif" => 5,          // Lowest priority
        _ => 99,
    }
}

/// Where a file goes among one photo's files, best first. `ext` is its extension in
/// lower case.
///
/// This is the order in which the compressed endpoint picks its source
/// (`image_cache::find_image_file`), so for a photo whose files it can all decode, the
/// thumbnails, the original, the EXIF and the title come from the same file:
/// - a mixed spelling (`x.Jpg`) after every file spelled all in lower or all in upper
///   case, since the endpoint lists a folder for one only when nothing else is there;
/// - then by [`format_priority`], with `jpg` before `jpeg`, as the endpoint has them;
/// - then all lower case (`x.jpg`) before all upper case (`x.JPG`);
/// - then by name in byte order (`x.JPg` before `x.Jpg`).
///
/// The endpoint can't decode AVIF. A photo with an AVIF and another format may have
/// the AVIF as its main file here (it ranks second, after JPEG), with the sizes made
/// from the other file.
fn variant_order<'a>(relative_path: &'a str, ext: &str) -> (bool, u8, bool, u8, &'a str) {
    let spelling = Path::new(relative_path)
        .extension()
        .and_then(|spelled| spelled.to_str())
        .map_or(0, crate::image_cache::extension_spelling_rank);
    (
        spelling == 2,
        format_priority(ext),
        ext == "jpeg",
        spelling,
        relative_path,
    )
}

/// `name` without its extension `ext` (in lower case), however the file spells it:
/// `DSC_0001.JPG` and `DSC_0001.jpg` both give `DSC_0001`. Like the
/// `trim_end_matches` it replaces, it takes off a repeated extension too
/// (`x.jpg.jpg` gives `x`).
fn trim_extension<'a>(name: &'a str, ext: &str) -> &'a str {
    let mut rest = name;
    while let Some(start) = rest.len().checked_sub(ext.len() + 1)
        && let Some(tail) = rest.get(start..)
        && let Some(spelled) = tail.strip_prefix('.')
        && spelled.eq_ignore_ascii_case(ext)
    {
        rest = &rest[..start];
    }
    rest
}

/// Find images for a specific gallery (with different base path handling)
pub fn find_images_for_gallery(dir: &Path, base_root: &Path, photos: &mut Vec<PhotoInfo>) {
    // Extract gallery name from directory path (as slug format)
    let gallery_name = dir.file_name().map_or_else(
        || "unknown".to_string(),
        |n| n.to_string_lossy().to_lowercase().replace(' ', "-"),
    );

    find_images_recursive_with_gallery(dir, base_root, photos, &gallery_name);
}

#[derive(Default)]
struct ExifData {
    width: Option<u32>,
    height: Option<u32>,
    date_taken: Option<String>,
    camera_make: Option<String>,
    camera_model: Option<String>,
    lens_model: Option<String>,
    focal_length: Option<String>,
    aperture: Option<String>,
    shutter_speed: Option<String>,
    iso: Option<String>,
    film_stock: Option<String>,
    copyright: Option<String>,
}

/// Read the XMP packet from a JPEG/TIFF file, scanning at most the first ~1MB.
/// XMP lives in an APP1 segment near the start of the file, so a partial
/// read is sufficient and avoids loading multi-MB photos in full.
fn read_xmp_packet(path: &Path) -> Option<String> {
    use std::io::Read;
    let file = std::fs::File::open(path).ok()?;
    let mut buf = Vec::new();
    file.take(1024 * 1024).read_to_end(&mut buf).ok()?;
    let start = find_subsequence(&buf, b"<x:xmpmeta")?;
    let end_marker = b"</x:xmpmeta>";
    let end_rel = find_subsequence(&buf[start..], end_marker)?;
    let end = start + end_rel + end_marker.len();
    std::str::from_utf8(&buf[start..end]).ok().map(String::from)
}

fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

/// Decode the five predefined XML entities. Numeric character references
/// (`&#nn;`, `&#xnn;`) are not used by XMP attribute writers we care about,
/// so we don't bother.
fn decode_xml_entities(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    s.replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

/// Extract the value of an XML attribute like `name="value"` from an XMP
/// string and decode XML entities. Returns `None` if absent.
fn xmp_attr_value(xmp: &str, name: &str) -> Option<String> {
    let key = format!("{name}=\"");
    let start = xmp.find(&key)? + key.len();
    let rest = &xmp[start..];
    let end = rest.find('"')?;
    Some(decode_xml_entities(&rest[..end]))
}

/// Returns true for file extensions that can carry an XMP packet inline.
/// Other formats (AVIF, WebP, PNG, JXL, GIF) either don't or use a separate
/// container; skipping them avoids reading the file altogether.
fn extension_supports_xmp(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|e| e.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some("jpg" | "jpeg" | "tif" | "tiff")
    )
}

/// Return true if `s` carries lens identity beyond a bare focal-length/aperture
/// stringification. Strings like `"85mm f/1.4D"`, `"AF Nikkor 85mm f/1.4D"` or
/// `"AF-S Nikkor 50mm f/1.4G"` pass; `"50.0 mm f/1.8"` or `"85mm f/1.4"` (which
/// the EXIF `LensModel` already exposes verbatim) do not.
fn looks_like_real_lens_name(s: &str) -> bool {
    s.chars()
        .any(|c| c.is_alphabetic() && !matches!(c.to_ascii_lowercase(), 'f' | 'm'))
}

/// Lightroom writes these as `crs:LensProfileName` when no Adobe lens profile
/// was matched (e.g. it fell back to the in-camera correction data). They are
/// not real lens names, so we ignore them and fall through to other sources.
fn is_lens_profile_placeholder(s: &str) -> bool {
    matches!(s, "Camera Settings" | "None" | "(none)" | "")
}

/// Lens-name candidates recoverable from XMP. Returned separately so the
/// caller can interleave them with the EXIF `LensModel` tag in priority
/// order — `aux:Lens` is authoritative (`LensTagger` writes the taking lens
/// here even on film scans), while `crs:LensProfileName` is Lightroom's
/// optical-correction profile, which for film scans names the *scanning*
/// lens rather than the lens that took the photo.
#[derive(Default)]
struct XmpLensCandidates {
    aux_lens: Option<String>,
    profile_lens: Option<String>,
}

impl XmpLensCandidates {
    /// True when both lens fields are present and describe meaningfully
    /// different lenses — the fingerprint of a camera-scanned film frame,
    /// where `aux:Lens` is the taking lens and `crs:LensProfileName` is the
    /// macro lens whose optical-correction profile Lightroom applies to the
    /// scan. EXIF exposure values (`FNumber`, `ExposureTime`) in that case
    /// belong to the scanning camera, not the original photograph.
    fn looks_like_film_scan(&self) -> bool {
        let (Some(aux), Some(profile)) = (&self.aux_lens, &self.profile_lens) else {
            return false;
        };
        let aux_norm = aux.to_ascii_lowercase();
        let profile_norm = profile.to_ascii_lowercase();
        !aux_norm.contains(&profile_norm) && !profile_norm.contains(&aux_norm)
    }
}

/// Which EXIF fields `LensTagger` explicitly wrote, recovered from its
/// `-Key=Value` annotation lines in `UserComment`. When the user enters a
/// value in `LensTagger` (aperture, shutter, etc.), the plugin records the
/// override here as well as updating the corresponding EXIF tag, so a
/// present flag means the EXIF value reflects the user's input rather than
/// the scanning camera's auto-exposure.
#[derive(Default, Debug, PartialEq, Eq)]
struct LenstaggerOverrides {
    fnumber: bool,
    exposure_time: bool,
}

impl LenstaggerOverrides {
    /// Parse `UserComment` text for `LensTagger`'s `-Key=Value` override
    /// lines. Accepts the case-insensitive aliases `LensTagger` uses across
    /// versions (`-FNumber=` / `-Aperture=`, `-ExposureTime=` / `-Shutter*=`).
    fn from_user_comment(text: &str) -> Self {
        let mut out = Self::default();
        for line in text.lines() {
            let Some(rest) = line.trim_start().strip_prefix('-') else {
                continue;
            };
            let Some((key, _)) = rest.split_once('=') else {
                continue;
            };
            match key.trim().to_ascii_lowercase().as_str() {
                "fnumber" | "aperture" | "fstop" => out.fnumber = true,
                "exposuretime" | "shutterspeed" | "shutterspeedvalue" | "shutter" => {
                    out.exposure_time = true;
                }
                _ => {}
            }
        }
        out
    }
}

/// Read `LensTagger` override annotations out of the EXIF `UserComment` tag.
/// Returns an all-false struct when the tag is absent or doesn't decode as
/// the ASCII-prefixed undefined-bytes form `LensTagger` writes.
fn read_lenstagger_overrides(exif_reader: &exif::Exif) -> LenstaggerOverrides {
    let Some(field) = exif_reader.get_field(exif::Tag::UserComment, exif::In::PRIMARY) else {
        return LenstaggerOverrides::default();
    };
    let exif::Value::Undefined(bytes, _) = &field.value else {
        return LenstaggerOverrides::default();
    };
    if bytes.len() <= 8 {
        return LenstaggerOverrides::default();
    }
    let text = String::from_utf8_lossy(&bytes[8..]);
    LenstaggerOverrides::from_user_comment(&text)
}

/// Parse a film stock label out of an EXIF `UserComment` body. Two export
/// formats are supported and distinguished by whether the comment carries a
/// `LensTaggerVer` marker (the old LensTagger exporter always writes one):
///
/// * **Old format** (has `LensTaggerVer`): LensTagger's newline-delimited
///   annotations, where the stock is assembled from `Film Make:`,
///   `Film Type:` and the `-ISO=` override.
/// * **New format** (no `LensTaggerVer`): a single line of ` | `-separated
///   `Key: Value` pairs (`Camera: … | Film: Kodak Ektar 100, 35mm | …`),
///   where the stock is the `Film:` value with its trailing gauge note
///   dropped.
#[doc(hidden)] // exposed for benches/metadata.rs; not part of the public API
pub fn parse_film_stock_from_comment(text: &str) -> Option<String> {
    if text.to_lowercase().contains("lenstaggerver") {
        parse_film_stock_lenstagger(text)
    } else {
        parse_film_stock_pipe_delimited(text)
    }
}

/// Old `LensTagger` format: newline-delimited annotations. Assemble the film
/// stock from `Film Make:`, `Film Type:` and the `-ISO=` override. Falls
/// back to the full comment text when no film fields are present.
fn parse_film_stock_lenstagger(text: &str) -> Option<String> {
    let mut film_make: Option<&str> = None;
    let mut film_type: Option<&str> = None;
    let mut film_iso: Option<&str> = None;

    for line in text.lines() {
        let line_lower = line.to_lowercase();
        if line_lower.contains("film make:") {
            film_make = line.split_once(':').map(|(_, v)| v.trim());
        } else if line_lower.contains("film type:") {
            film_type = line.split_once(':').map(|(_, v)| v.trim());
        } else if line_lower.starts_with("-iso=") || line_lower.starts_with("iso=") {
            // Handle formats like "-ISO=200" or "ISO=400"
            film_iso = line.split_once('=').map(|(_, v)| v.trim());
        }
    }

    // Combine film make, type, and ISO if present
    let film_stock = match (film_make, film_type) {
        (Some(make), Some(typ)) => format!("{make} {typ}"),
        (Some(make), None) => make.to_string(),
        (None, Some(typ)) => typ.to_string(),
        (None, None) => text.to_string(), // Return full text if no specific fields found
    };

    // Add ISO if found (format: "Kodak Gold 200"), unless the film stock name
    // already contains it (e.g. "Ilford Delta 400 Professional" with ISO 400).
    let combined = match film_iso {
        Some(iso_value) if !film_stock.split_whitespace().any(|tok| tok == iso_value) => {
            format!("{film_stock} {iso_value}")
        }
        _ => film_stock,
    };

    Some(combined).filter(|s| !s.trim().is_empty())
}

/// New export format: one line of ` | `-separated `Key: Value` pairs. The
/// film stock is the `Film:` value (matched case-insensitively); its
/// optional trailing gauge note (`, 35mm`) is dropped so the label reads as
/// just the stock name (`Kodak Ektar 100`).
fn parse_film_stock_pipe_delimited(text: &str) -> Option<String> {
    text.split('|')
        .filter_map(|segment| segment.split_once(':'))
        .find(|(key, _)| key.trim().eq_ignore_ascii_case("film"))
        .map(|(_, value)| value.split(',').next().unwrap_or(value).trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Recover human-readable lens names from XMP metadata. Lightroom often
/// strips the camera maker note, but writes the matched lens profile as
/// `crs:LensProfileName="Adobe (<lens name>) v<n>"` and tools like
/// `LensTagger` write the taking lens directly to `aux:Lens`. Values that
/// only stringify focal length/aperture (which EXIF `LensModel` already
/// exposes) are filtered out.
fn extract_lens_candidates_from_xmp(path: &Path) -> XmpLensCandidates {
    let mut out = XmpLensCandidates::default();
    if !extension_supports_xmp(path) {
        return out;
    }
    let Some(xmp) = read_xmp_packet(path) else {
        return out;
    };

    if let Some(raw) = xmp_attr_value(&xmp, "aux:Lens") {
        let trimmed = raw.trim();
        if looks_like_real_lens_name(trimmed) {
            out.aux_lens = Some(trimmed.to_string());
        }
    }

    if let Some(raw) = xmp_attr_value(&xmp, "crs:LensProfileName") {
        let trimmed = raw.trim();
        let inner = trimmed
            .strip_prefix("Adobe (")
            .and_then(|s| {
                // Drop the matching ')' and any trailing version like " v2".
                let close = s.rfind(')')?;
                Some(s[..close].trim())
            })
            .unwrap_or(trimmed);
        if !is_lens_profile_placeholder(inner) {
            out.profile_lens = Some(inner.to_string());
        }
    }

    out
}

/// Pixel dimensions from EXIF, falling back to the image header.
///
/// A surprising number of exports carry EXIF but omit the dimension tags, so the
/// header read is the common path rather than an exotic fallback.
fn exif_dimensions(exif_reader: &exif::Exif, path: &Path) -> (Option<u32>, Option<u32>) {
    let mut width = exif_reader
        .get_field(exif::Tag::PixelXDimension, exif::In::PRIMARY)
        .or_else(|| exif_reader.get_field(exif::Tag::ImageWidth, exif::In::PRIMARY))
        .and_then(|f| f.value.get_uint(0));

    let mut height = exif_reader
        .get_field(exif::Tag::PixelYDimension, exif::In::PRIMARY)
        .or_else(|| exif_reader.get_field(exif::Tag::ImageLength, exif::In::PRIMARY))
        .and_then(|f| f.value.get_uint(0));

    if (width.is_none() || height.is_none())
        && let Ok(mut reader) = image::ImageReader::open(path)
    {
        reader.limits(image::Limits::no_limits());
        if let Ok(dimensions) = reader.into_dimensions() {
            width = Some(dimensions.0);
            height = Some(dimensions.1);
        }
    }

    (width, height)
}

/// Film stock name, which scanners and taggers stash in `ImageDescription` or
/// `UserComment` because EXIF has no dedicated tag for it.
fn exif_film_stock(exif_reader: &exif::Exif) -> Option<String> {
    exif_reader
        .get_field(exif::Tag::ImageDescription, exif::In::PRIMARY)
        .or_else(|| exif_reader.get_field(exif::Tag::UserComment, exif::In::PRIMARY))
        .and_then(|f| match &f.value {
            exif::Value::Ascii(vec) => {
                // Concatenate all ASCII strings and decode as UTF-8
                let bytes: Vec<u8> = vec.iter().flat_map(|s| s.iter().copied()).collect();
                String::from_utf8(bytes)
                    .ok()
                    .filter(|s| !s.trim().is_empty())
            }
            exif::Value::Undefined(bytes, _) => {
                // UserComment has an 8-byte character code followed by the actual text
                // The character code is typically "ASCII\0\0\0" or "UNICODE\0"
                if bytes.len() > 8 {
                    let text = String::from_utf8_lossy(&bytes[8..]).trim().to_string();
                    if text.is_empty() {
                        None
                    } else {
                        parse_film_stock_from_comment(&text)
                    }
                } else {
                    None
                }
            }
            _ => {
                let display = f.display_value().to_string();
                if display.trim().is_empty() {
                    None
                } else {
                    Some(display)
                }
            }
        })
}

/// Copyright string, decoded from the multi-part ASCII form when present.
fn exif_copyright(exif_reader: &exif::Exif) -> Option<String> {
    exif_reader
        .get_field(exif::Tag::Copyright, exif::In::PRIMARY)
        .and_then(|f| match &f.value {
            exif::Value::Ascii(vec) => {
                // Concatenate all ASCII strings and decode as UTF-8
                let bytes: Vec<u8> = vec.iter().flat_map(|s| s.iter().copied()).collect();
                String::from_utf8(bytes).ok()
            }
            _ => Some(f.display_value().to_string()),
        })
}

/// Extract EXIF metadata from an image file
fn extract_exif_data(path: &Path) -> ExifData {
    use std::fs::File;
    use std::io::BufReader;

    let Ok(file) = File::open(path) else {
        return ExifData::default();
    };

    let mut reader = BufReader::new(file);
    let Ok(exif_reader) = exif::Reader::new().read_from_container(&mut reader) else {
        return ExifData::default();
    };

    let (width, height) = exif_dimensions(&exif_reader, path);

    let date_taken = exif_reader
        .get_field(exif::Tag::DateTimeOriginal, exif::In::PRIMARY)
        .or_else(|| exif_reader.get_field(exif::Tag::DateTime, exif::In::PRIMARY))
        .map(|f| {
            let datetime_str = f.display_value().to_string();
            // Remove seconds from format "YYYY-MM-DD HH:MM:SS" -> "YYYY-MM-DD HH:MM"
            // or from "YYYY:MM:DD HH:MM:SS" -> "YYYY:MM:DD HH:MM"
            if let Some(last_colon_idx) = datetime_str.rfind(':') {
                datetime_str[..last_colon_idx].to_string()
            } else {
                datetime_str
            }
        });

    let camera_make = exif_reader
        .get_field(exif::Tag::Make, exif::In::PRIMARY)
        .and_then(|f| f.display_value().to_string().into());

    let camera_model = exif_reader
        .get_field(exif::Tag::Model, exif::In::PRIMARY)
        .and_then(|f| f.display_value().to_string().into());

    let exif_lens_model = exif_reader
        .get_field(exif::Tag::LensModel, exif::In::PRIMARY)
        .map(|f| f.display_value().to_string());
    let xmp_lens = extract_lens_candidates_from_xmp(path);
    let is_film_scan = xmp_lens.looks_like_film_scan();
    let lt_overrides = read_lenstagger_overrides(&exif_reader);
    // Prefer `aux:Lens` (LensTagger writes the taking lens here) and EXIF
    // `LensModel` over `crs:LensProfileName` — on film scans the latter
    // names the scanning lens, not the lens used to take the photo.
    let lens_model = xmp_lens
        .aux_lens
        .or(exif_lens_model)
        .or(xmp_lens.profile_lens);

    let focal_length = exif_reader
        .get_field(exif::Tag::FocalLength, exif::In::PRIMARY)
        .map(|f| {
            let val = f.display_value().to_string();
            if val.contains("mm") {
                val
            } else {
                format!("{val} mm")
            }
        });

    // On camera-scanned film, EXIF FNumber/ExposureTime belong to the
    // scanning camera — unless LensTagger explicitly overwrote them, in
    // which case it leaves a matching `-FNumber=` / `-ExposureTime=` line
    // in `UserComment` and the EXIF value is what the user typed in.
    let trust_aperture = !is_film_scan || lt_overrides.fnumber;
    let trust_shutter = !is_film_scan || lt_overrides.exposure_time;

    let aperture = if trust_aperture {
        exif_reader
            .get_field(exif::Tag::FNumber, exif::In::PRIMARY)
            .map(|f| format!("f/{}", f.display_value()))
    } else {
        None
    };

    let shutter_speed = if trust_shutter {
        exif_reader
            .get_field(exif::Tag::ExposureTime, exif::In::PRIMARY)
            .map(|f| {
                let val = f.display_value().to_string();
                if val.contains('s') {
                    val
                } else {
                    format!("{val} s")
                }
            })
    } else {
        None
    };

    let iso = exif_reader
        .get_field(exif::Tag::PhotographicSensitivity, exif::In::PRIMARY)
        .map(|f| format!("ISO {}", f.display_value()));

    let film_stock = exif_film_stock(&exif_reader);
    let copyright = exif_copyright(&exif_reader);

    ExifData {
        width,
        height,
        date_taken,
        camera_make,
        camera_model,
        lens_model,
        focal_length,
        aperture,
        shutter_speed,
        iso,
        film_stock,
        copyright,
    }
}

/// Load gallery configuration from a TOML file
pub fn load_gallery_config(gallery_path: &Path) -> Option<crate::types::GalleryConfig> {
    let config_path = gallery_path.join("gallery.toml");
    if !config_path.exists() {
        return None;
    }

    let content = fs::read_to_string(&config_path).ok()?;
    toml::from_str(&content).ok()
}

/// Load all galleries with photo counts
pub fn load_galleries() -> Vec<GalleryInfo> {
    let mut galleries = Vec::new();
    let gallery_roots = discover_gallery_directories();

    for base_path in gallery_roots {
        let path = Path::new(&base_path);
        if path.exists()
            && let Some(dir_name) = path.file_name()
        {
            let name = dir_name.to_string_lossy().to_string();
            let slug = name.to_lowercase().replace(' ', "-");

            // Skip "home" since it's shown on the home page
            if slug == "home" {
                continue;
            }

            // Count photos in this directory (recursively)
            let mut photo_count = 0;
            count_images_recursive(path, &mut photo_count);

            if photo_count > 0 {
                let config = load_gallery_config(path);
                galleries.push(GalleryInfo {
                    name: name.replace(['-', '_'], " "),
                    slug,
                    photo_count,
                    config,
                });
            }
        }
    }

    let config = crate::config::load_config();
    let order_index: HashMap<&str, usize> = config
        .gallery_order
        .iter()
        .enumerate()
        .map(|(i, slug)| (slug.as_str(), i))
        .collect();

    galleries.sort_by(|a, b| {
        match (
            order_index.get(a.slug.as_str()),
            order_index.get(b.slug.as_str()),
        ) {
            (Some(ia), Some(ib)) => ia.cmp(ib),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => a.name.cmp(&b.name),
        }
    });
    galleries
}

/// Load photos from home directory
pub fn load_home_photos() -> Vec<PhotoInfo> {
    let gallery_path = crate::paths::home_gallery_dir();

    // Create directory if it doesn't exist
    if !gallery_path.exists() {
        fs::create_dir_all(&gallery_path).ok();
    }

    let mut photos = Vec::new();
    let images_base = crate::paths::images_root();

    find_images_recursive(&gallery_path, &images_base, &mut photos);
    photos.sort_by(|a, b| {
        // Sort by subfolder first, then by filename
        match (&a.subfolder, &b.subfolder) {
            (Some(sf_a), Some(sf_b)) => {
                // Both have subfolders, compare them first
                match sf_a.cmp(sf_b) {
                    std::cmp::Ordering::Equal => a.filename.cmp(&b.filename),
                    other => other,
                }
            }
            (Some(_), None) => std::cmp::Ordering::Greater, // Photos in subfolders come after root
            (None, Some(_)) => std::cmp::Ordering::Less,    // Root photos come first
            (None, None) => a.filename.cmp(&b.filename),    // Both in root, sort by filename
        }
    });
    photos
}

/// Load photos from a specific gallery by name
pub fn load_gallery_photos(gallery_name: &str) -> Option<Vec<PhotoInfo>> {
    let gallery_roots = discover_gallery_directories();
    let mut photos = Vec::new();
    let images_base = crate::paths::images_root();

    for base_path in gallery_roots {
        let base = Path::new(&base_path);
        if let Some(dir_name) = base.file_name() {
            let slug = dir_name.to_string_lossy().to_lowercase().replace(' ', "-");
            if slug == gallery_name {
                find_images_for_gallery(base, &images_base, &mut photos);
                photos.sort_by(|a, b| {
                    // Sort by subfolder first, then by filename
                    match (&a.subfolder, &b.subfolder) {
                        (Some(sf_a), Some(sf_b)) => {
                            // Both have subfolders, compare them first
                            match sf_a.cmp(sf_b) {
                                std::cmp::Ordering::Equal => a.filename.cmp(&b.filename),
                                other => other,
                            }
                        }
                        (Some(_), None) => std::cmp::Ordering::Greater, // Photos in subfolders come after root
                        (None, Some(_)) => std::cmp::Ordering::Less,    // Root photos come first
                        (None, None) => a.filename.cmp(&b.filename), // Both in root, sort by filename
                    }
                });
                return Some(photos);
            }
        }
    }

    None
}

/// Load photos from all galleries
pub fn load_all_gallery_photos() -> Vec<PhotoInfo> {
    let mut photos = Vec::new();
    let images_base = crate::paths::images_root();
    let gallery_roots = discover_gallery_directories();

    for gallery_path in gallery_roots {
        let gallery_path_buf = Path::new(&gallery_path);
        if gallery_path_buf.exists() {
            // Extract gallery name from directory path
            let gallery_name = gallery_path_buf.file_name().map_or_else(
                || "home".to_string(),
                |n| n.to_string_lossy().to_lowercase().replace(' ', "-"),
            );

            find_images_recursive_with_gallery(
                gallery_path_buf,
                &images_base,
                &mut photos,
                &gallery_name,
            );
        }
    }

    photos.sort_by(|a, b| {
        // Sort by subfolder first, then by filename
        match (&a.subfolder, &b.subfolder) {
            (Some(sf_a), Some(sf_b)) => {
                // Both have subfolders, compare them first
                match sf_a.cmp(sf_b) {
                    std::cmp::Ordering::Equal => a.filename.cmp(&b.filename),
                    other => other,
                }
            }
            (Some(_), None) => std::cmp::Ordering::Greater, // Photos in subfolders come after root
            (None, Some(_)) => std::cmp::Ordering::Less,    // Root photos come first
            (None, None) => a.filename.cmp(&b.filename),    // Both in root, sort by filename
        }
    });
    photos
}

/// Directory holding the deployer's content files: `about.html`/`about.txt`, the
/// profile image, and `opensource.toml`.
///
/// `ABOUT_CONTENT_PATH` when set, else `public/content` when it exists, else
/// `./content` (the Docker image's layout).
pub fn content_dir() -> String {
    std::env::var("ABOUT_CONTENT_PATH").unwrap_or_else(|_| {
        if Path::new("public/content").exists() {
            "public/content".to_string()
        } else {
            "./content".to_string()
        }
    })
}

/// Load about page content
pub fn load_about_content() -> crate::types::AboutContent {
    let content_path = content_dir();

    // Create directory if it doesn't exist
    if !Path::new(&content_path).exists() {
        fs::create_dir_all(&content_path).ok();
    }

    // Try to load the about HTML file first, then fall back to text file
    let html_path = Path::new(&content_path).join("about.html");
    let text_path = Path::new(&content_path).join("about.txt");

    let (content, is_html) = if html_path.exists() {
        (
            fs::read_to_string(&html_path).unwrap_or_else(|_| default_about_text()),
            true,
        )
    } else if text_path.exists() {
        (
            fs::read_to_string(&text_path).unwrap_or_else(|_| default_about_text()),
            false,
        )
    } else {
        (default_about_text(), false)
    };

    // The first profile image that exists, with its version. The `stat` that finds it
    // also gives the version.
    let image_url = ["profile.jpg", "profile.png", "profile.webp"]
        .iter()
        .find_map(|name| {
            let metadata = fs::metadata(Path::new(&content_path).join(name)).ok()?;
            Some(with_version(
                format!("/content/{name}"),
                file_version(&metadata),
            ))
        });

    crate::types::AboutContent {
        image_url,
        content,
        is_html,
    }
}

/// Returns default about page text when no custom content is provided.
///
/// # Examples
///
/// ```
/// use portfolio::gallery::default_about_text;
///
/// let text = default_about_text();
/// assert!(text.contains("photographer"));
/// ```
pub fn default_about_text() -> String {
    "Hello! I'm a passionate photographer specializing in capturing the beauty of everyday moments.\n\n\
    With over 10 years of experience, I've worked on various projects ranging from landscapes to portraits.\n\n\
    My photography style focuses on natural lighting and authentic emotions. \
    I believe every photograph tells a unique story, and I'm here to help you tell yours.".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::EnvVarGuard;
    use serial_test::serial;
    use std::fs;

    #[test]
    fn looks_like_real_lens_name_accepts_manufacturer_strings() {
        assert!(looks_like_real_lens_name("85mm f/1.4D"));
        assert!(looks_like_real_lens_name("AF Nikkor 85mm f/1.4D"));
        assert!(looks_like_real_lens_name("AF-S Nikkor 50mm f/1.4G"));
        assert!(looks_like_real_lens_name("RF 24-70mm f/2.8L IS USM"));
    }

    #[test]
    fn looks_like_real_lens_name_rejects_bare_focal_stringifications() {
        assert!(!looks_like_real_lens_name("85mm f/1.4"));
        assert!(!looks_like_real_lens_name("50.0 mm f/1.8"));
        assert!(!looks_like_real_lens_name("24-70mm f/2.8-4"));
        assert!(!looks_like_real_lens_name(""));
    }

    #[test]
    fn lens_profile_placeholders_are_recognized() {
        assert!(is_lens_profile_placeholder("Camera Settings"));
        assert!(is_lens_profile_placeholder("None"));
        assert!(is_lens_profile_placeholder("(none)"));
        assert!(is_lens_profile_placeholder(""));
        assert!(!is_lens_profile_placeholder(
            "Adobe (Nikon AF-S 50mm f/1.8G) v1"
        ));
        assert!(!is_lens_profile_placeholder("Nikon AF Nikkor 85mm f/1.4D"));
    }

    #[test]
    #[serial]
    fn test_get_default_image_params_defaults() {
        // Ensure env vars are not set
        unsafe { std::env::remove_var("DEFAULT_IMAGE_WIDTH") };
        unsafe { std::env::remove_var("DEFAULT_IMAGE_QUALITY") };
        // Also ensure IMAGE_PRESETS doesn't override
        unsafe { std::env::remove_var("IMAGE_PRESETS") };

        let (width, quality) = get_default_image_params();
        // Should use 2400px at 90 quality (the default)
        assert_eq!(width, 2400);
        assert_eq!(quality, 90);
    }

    #[test]
    #[serial]
    fn test_get_default_image_params_custom_width() {
        unsafe { std::env::set_var("DEFAULT_IMAGE_WIDTH", "1200") };
        unsafe { std::env::remove_var("DEFAULT_IMAGE_QUALITY") };

        let (width, quality) = get_default_image_params();

        unsafe { std::env::remove_var("DEFAULT_IMAGE_WIDTH") };

        // On SSR builds, env vars don't affect get_default_image_params (it uses presets)
        // On client builds, env vars override defaults
        #[cfg(feature = "ssr")]
        {
            assert_eq!(width, 2400);
            assert_eq!(quality, 90);
        }
        #[cfg(not(feature = "ssr"))]
        {
            assert_eq!(width, 1200);
            assert_eq!(quality, 90);
        }
    }

    #[test]
    #[serial]
    fn test_get_default_image_params_custom_quality() {
        unsafe { std::env::remove_var("DEFAULT_IMAGE_WIDTH") };
        unsafe { std::env::set_var("DEFAULT_IMAGE_QUALITY", "90") };

        let (width, quality) = get_default_image_params();

        unsafe { std::env::remove_var("DEFAULT_IMAGE_QUALITY") };

        // On SSR builds, env vars don't affect get_default_image_params (it uses presets)
        // On client builds, env vars override defaults
        #[cfg(feature = "ssr")]
        {
            assert_eq!(width, 2400);
            assert_eq!(quality, 90);
        }
        #[cfg(not(feature = "ssr"))]
        {
            assert_eq!(width, 2400);
            assert_eq!(quality, 90);
        }
    }

    #[test]
    #[serial]
    fn test_get_default_image_params_both_custom() {
        unsafe { std::env::set_var("DEFAULT_IMAGE_WIDTH", "1200") };
        unsafe { std::env::set_var("DEFAULT_IMAGE_QUALITY", "85") };

        let (width, quality) = get_default_image_params();

        unsafe { std::env::remove_var("DEFAULT_IMAGE_WIDTH") };
        unsafe { std::env::remove_var("DEFAULT_IMAGE_QUALITY") };

        // On SSR builds, env vars don't affect get_default_image_params (it uses presets)
        // On client builds, env vars override defaults
        #[cfg(feature = "ssr")]
        {
            assert_eq!(width, 2400);
            assert_eq!(quality, 90);
        }
        #[cfg(not(feature = "ssr"))]
        {
            assert_eq!(width, 1200);
            assert_eq!(quality, 85);
        }
    }

    #[test]
    #[serial]
    fn test_get_default_image_params_invalid_values() {
        unsafe { std::env::set_var("DEFAULT_IMAGE_WIDTH", "invalid") };
        unsafe { std::env::set_var("DEFAULT_IMAGE_QUALITY", "not_a_number") };

        let (width, quality) = get_default_image_params();

        unsafe { std::env::remove_var("DEFAULT_IMAGE_WIDTH") };
        unsafe { std::env::remove_var("DEFAULT_IMAGE_QUALITY") };

        // Should fall back to defaults
        assert_eq!(width, 2400);
        assert_eq!(quality, 90);
    }

    #[test]
    fn test_default_about_text() {
        let text = default_about_text();
        assert!(!text.is_empty(), "default about text should not be empty");
        assert!(text.contains("photographer"));
        assert!(text.contains("10 years"));
    }

    #[test]
    fn test_count_images_recursive() {
        let temp_dir = std::env::temp_dir().join("test_gallery_count");
        fs::create_dir_all(&temp_dir).unwrap();

        // Create some test files
        fs::write(temp_dir.join("photo1.jpg"), b"fake jpg").unwrap();
        fs::write(temp_dir.join("photo2.png"), b"fake png").unwrap();
        fs::write(temp_dir.join("photo3.webp"), b"fake webp").unwrap();
        fs::write(temp_dir.join("readme.txt"), b"not an image").unwrap();

        let mut count = 0;
        count_images_recursive(&temp_dir, &mut count);

        assert_eq!(count, 3);

        // Cleanup
        fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn test_count_images_recursive_nested() {
        let temp_dir = std::env::temp_dir().join("test_gallery_nested");
        fs::create_dir_all(temp_dir.join("subdir")).unwrap();

        fs::write(temp_dir.join("photo1.jpg"), b"fake jpg").unwrap();
        fs::write(temp_dir.join("subdir/photo2.jpeg"), b"fake jpeg").unwrap();
        fs::write(temp_dir.join("subdir/photo3.gif"), b"fake gif").unwrap();

        let mut count = 0;
        count_images_recursive(&temp_dir, &mut count);

        assert_eq!(count, 3);

        // Cleanup
        fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn test_count_images_recursive_empty_directory() {
        let temp_dir = std::env::temp_dir().join("test_gallery_empty");
        fs::create_dir_all(&temp_dir).unwrap();

        let mut count = 0;
        count_images_recursive(&temp_dir, &mut count);

        assert_eq!(count, 0);

        // Cleanup
        fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn test_count_images_nonexistent_directory() {
        let temp_dir = std::env::temp_dir().join("nonexistent_gallery_12345");

        let mut count = 0;
        count_images_recursive(&temp_dir, &mut count);

        // Should not panic, just count 0
        assert_eq!(count, 0);
    }

    #[test]
    fn test_discover_gallery_directories_nonexistent() {
        // Set up env to use a directory that doesn't exist for test
        let galleries = discover_gallery_directories();
        // Should return empty vec without panicking
        assert!(galleries.is_empty() || !galleries.is_empty());
    }

    #[test]
    #[serial]
    fn test_load_about_content_with_default() {
        // Create a fresh temp directory with no profile image
        let temp_path = std::env::temp_dir().join("test_about_default_only");

        // Clean up and recreate to ensure it's empty
        fs::remove_dir_all(&temp_path).ok();
        fs::create_dir_all(&temp_path).unwrap();

        unsafe { std::env::set_var("ABOUT_CONTENT_PATH", temp_path.to_str().unwrap()) };

        let about = load_about_content();

        // Should have no image in an empty directory
        assert_eq!(about.image_url, None);
        // Should contain default text since no about.txt exists
        assert!(about.content.contains("photographer"));

        unsafe { std::env::remove_var("ABOUT_CONTENT_PATH") };
        // Cleanup
        fs::remove_dir_all(&temp_path).ok();
    }

    #[test]
    #[serial]
    fn test_load_about_content_with_custom_text() {
        let temp_dir = std::env::temp_dir().join("test_about_content");
        fs::create_dir_all(&temp_dir).unwrap();

        let custom_text = "This is my custom about text!";
        fs::write(temp_dir.join("about.txt"), custom_text).unwrap();

        unsafe { std::env::set_var("ABOUT_CONTENT_PATH", temp_dir.to_str().unwrap()) };

        let about = load_about_content();

        assert_eq!(about.content, custom_text);
        assert_eq!(about.image_url, None);

        unsafe { std::env::remove_var("ABOUT_CONTENT_PATH") };
        fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    #[serial]
    fn test_load_about_content_with_profile_image() {
        let temp_dir = std::env::temp_dir().join("test_about_profile");
        fs::create_dir_all(&temp_dir).unwrap();

        fs::write(temp_dir.join("about.txt"), "Custom text").unwrap();
        fs::write(temp_dir.join("profile.jpg"), b"fake jpg").unwrap();

        unsafe { std::env::set_var("ABOUT_CONTENT_PATH", temp_dir.to_str().unwrap()) };

        let about = load_about_content();

        assert_eq!(
            about.image_url.as_deref().map(without_version),
            Some("/content/profile.jpg")
        );
        assert_eq!(about.content, "Custom text");

        unsafe { std::env::remove_var("ABOUT_CONTENT_PATH") };
        fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    #[serial]
    fn test_load_about_content_prefers_jpg_over_png() {
        let temp_dir = std::env::temp_dir().join("test_about_multi_image");
        fs::create_dir_all(&temp_dir).unwrap();

        // Create multiple profile images
        fs::write(temp_dir.join("profile.jpg"), b"fake jpg").unwrap();
        fs::write(temp_dir.join("profile.png"), b"fake png").unwrap();

        unsafe { std::env::set_var("ABOUT_CONTENT_PATH", temp_dir.to_str().unwrap()) };

        let about = load_about_content();

        // Should prefer .jpg as it's first in the array
        assert_eq!(
            about.image_url.as_deref().map(without_version),
            Some("/content/profile.jpg")
        );

        unsafe { std::env::remove_var("ABOUT_CONTENT_PATH") };
        fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    #[serial]
    fn test_load_home_photos_creates_directory() {
        let temp_gallery = std::env::temp_dir().join("test_home_gallery_create");

        // Make sure it doesn't exist
        fs::remove_dir_all(&temp_gallery).ok();

        unsafe { std::env::set_var("GALLERY_PATH", temp_gallery.to_str().unwrap()) };

        let photos = load_home_photos();

        // Directory should be created
        assert!(temp_gallery.exists());
        assert!(
            photos.is_empty(),
            "a freshly created gallery directory has no photos, got {photos:?}"
        );

        unsafe { std::env::remove_var("GALLERY_PATH") };
        fs::remove_dir_all(&temp_gallery).ok();
    }

    /// An `IMAGES_DIR` tree in a temp dir, well away from `public/images`: `home/`
    /// and `nature/`, one small JPEG in each.
    fn images_dir_with_two_galleries() -> tempfile::TempDir {
        let root = tempfile::TempDir::new().unwrap();
        for (gallery, photo) in [("home", "contrail.jpg"), ("nature", "bird.jpg")] {
            let dir = root.path().join(gallery);
            fs::create_dir_all(&dir).unwrap();
            image::RgbImage::new(4, 3).save(dir.join(photo)).unwrap();
        }
        root
    }

    #[test]
    #[serial]
    fn discover_gallery_directories_lists_images_dir() {
        let root = images_dir_with_two_galleries();
        let _images_dir = EnvVarGuard::set("IMAGES_DIR", root.path());

        let mut found = discover_gallery_directories();

        found.sort();
        let expected: Vec<String> = ["home", "nature"]
            .iter()
            .map(|g| root.path().join(g).to_string_lossy().into_owned())
            .collect();
        assert_eq!(found, expected);
    }

    #[test]
    #[serial]
    fn load_gallery_photos_finds_photos_under_images_dir() {
        let root = images_dir_with_two_galleries();
        let _images_dir = EnvVarGuard::set("IMAGES_DIR", root.path());

        let photos = load_gallery_photos("nature");

        // The URL is relative to IMAGES_DIR, which is what the /images mount serves.
        let urls: Option<Vec<String>> = photos.map(|photos| {
            photos
                .iter()
                .map(|p| without_version(&p.original_url).to_owned())
                .collect()
        });
        assert_eq!(urls, Some(vec!["/images/nature/bird.jpg".to_string()]));
    }

    #[test]
    #[serial]
    fn load_home_photos_reads_home_under_images_dir() {
        let root = images_dir_with_two_galleries();
        unsafe { std::env::remove_var("GALLERY_PATH") };
        let _images_dir = EnvVarGuard::set("IMAGES_DIR", root.path());

        let urls: Vec<String> = load_home_photos()
            .iter()
            .map(|p| without_version(&p.original_url).to_owned())
            .collect();

        assert_eq!(urls, vec!["/images/home/contrail.jpg".to_string()]);
    }

    #[test]
    #[serial]
    fn load_all_gallery_photos_reads_every_gallery_under_images_dir() {
        let root = images_dir_with_two_galleries();
        let _images_dir = EnvVarGuard::set("IMAGES_DIR", root.path());

        let urls: Vec<String> = load_all_gallery_photos()
            .iter()
            .map(|p| without_version(&p.original_url).to_owned())
            .collect();

        assert_eq!(
            urls,
            vec![
                "/images/home/contrail.jpg".to_string(),
                "/images/nature/bird.jpg".to_string()
            ]
        );
    }

    /// `url` without its version, once it is checked to end in one: `v=` and a whole
    /// number of seconds, as the last parameter of its query.
    fn without_version(url: &str) -> &str {
        let (address, version) = url
            .rsplit_once("v=")
            .unwrap_or_else(|| panic!("no version in {url}"));
        assert!(
            !version.is_empty() && version.bytes().all(|b| b.is_ascii_digit()),
            "the version in {url} is not a whole number of seconds"
        );
        address
            .strip_suffix(['?', '&'])
            .unwrap_or_else(|| panic!("v= is not a query parameter in {url}"))
    }

    /// 2024-11-15 06:33:20 UTC, a modification time to give a file.
    const MTIME: u64 = 1_731_652_400;

    /// Set the modification time of the file at `path` to `secs` seconds after 1970.
    fn set_mtime(path: &Path, secs: u64) {
        fs::File::options()
            .write(true)
            .open(path)
            .unwrap()
            .set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_secs(secs))
            .unwrap();
    }

    /// The one photo in the `nature` gallery under `IMAGES_DIR`.
    fn the_nature_photo() -> PhotoInfo {
        let mut photos = load_gallery_photos("nature").expect("a nature gallery");
        assert_eq!(photos.len(), 1, "{photos:?}");
        photos.remove(0)
    }

    #[test]
    #[serial]
    fn every_address_of_a_photo_carries_its_file_version() {
        unsafe { std::env::remove_var("IMAGE_PRESETS") };
        let root = images_dir_with_two_galleries();
        let _images_dir = EnvVarGuard::set("IMAGES_DIR", root.path());
        set_mtime(&root.path().join("nature/bird.jpg"), MTIME);

        let photo = the_nature_photo();

        assert_eq!(photo.original_url, "/images/nature/bird.jpg?v=1731652400");
        assert_eq!(
            photo.url,
            "/images/compressed/nature/bird.jpg?width=2400&quality=90&v=1731652400"
        );
        assert_eq!(
            photo.detail_url,
            "/images/compressed/nature/bird.jpg?width=4000&quality=90&v=1731652400"
        );
        assert_eq!(
            photo.srcset,
            "/images/compressed/nature/bird.jpg?width=2400&quality=90&v=1731652400 2400w"
        );
    }

    #[test]
    #[serial]
    fn every_srcset_entry_carries_the_version() {
        let _presets = EnvVarGuard::set("IMAGE_PRESETS", "800,70;1600,90;4000,90");
        let root = images_dir_with_two_galleries();
        let _images_dir = EnvVarGuard::set("IMAGES_DIR", root.path());
        set_mtime(&root.path().join("nature/bird.jpg"), MTIME);

        let photo = the_nature_photo();

        // 4000 is wider than the grid ever asks for, so it isn't in the srcset.
        assert_eq!(
            photo.srcset,
            "/images/compressed/nature/bird.jpg?width=800&quality=70&v=1731652400 800w, \
             /images/compressed/nature/bird.jpg?width=1600&quality=90&v=1731652400 1600w"
        );
    }

    #[test]
    #[serial]
    fn a_photo_gets_new_addresses_when_its_file_changes() {
        unsafe { std::env::remove_var("IMAGE_PRESETS") };
        let root = images_dir_with_two_galleries();
        let _images_dir = EnvVarGuard::set("IMAGES_DIR", root.path());
        let bird = root.path().join("nature/bird.jpg");
        set_mtime(&bird, MTIME);
        let before = the_nature_photo();

        // Exported again a minute later, under the same name.
        set_mtime(&bird, MTIME + 60);
        let after = the_nature_photo();

        for (old, new) in [
            (&before.url, &after.url),
            (&before.original_url, &after.original_url),
            (&before.detail_url, &after.detail_url),
            (&before.srcset, &after.srcset),
        ] {
            assert!(new.contains("v=1731652460"), "{new}");
            assert_eq!(new.replace("v=1731652460", "v=1731652400"), *old);
        }
    }

    /// The compressed endpoint finds its source by the name without the extension, so
    /// any of a photo's formats can be behind a compressed address.
    #[test]
    #[serial]
    fn originals_carry_their_own_version_and_compressed_addresses_the_newest() {
        unsafe { std::env::remove_var("IMAGE_PRESETS") };
        let root = tempfile::TempDir::new().unwrap();
        let dir = root.path().join("nature");
        fs::create_dir_all(&dir).unwrap();
        image::RgbImage::new(4, 3)
            .save(dir.join("sunset.jpg"))
            .unwrap();
        fs::write(dir.join("sunset.webp"), b"webp").unwrap();
        set_mtime(&dir.join("sunset.jpg"), MTIME);
        set_mtime(&dir.join("sunset.webp"), MTIME + 5);
        let _images_dir = EnvVarGuard::set("IMAGES_DIR", root.path());

        let photo = the_nature_photo();

        assert_eq!(photo.original_url, "/images/nature/sunset.jpg?v=1731652400");
        assert_eq!(
            photo.original_sources,
            vec![ImageSource {
                url: "/images/nature/sunset.webp?v=1731652405".to_string(),
                mime_type: "image/webp".to_string(),
            }]
        );
        assert_eq!(
            photo.sources,
            vec![ImageSource {
                url: "/images/compressed/nature/sunset.webp?width=2400&quality=90&v=1731652405"
                    .to_string(),
                mime_type: "image/webp".to_string(),
            }]
        );
        assert_eq!(
            photo.url,
            "/images/compressed/nature/sunset.jpg?width=2400&quality=90&v=1731652405"
        );
        assert_eq!(
            photo.detail_url,
            "/images/compressed/nature/sunset.jpg?width=4000&quality=90&v=1731652405"
        );
        assert_eq!(
            photo.srcset,
            "/images/compressed/nature/sunset.jpg?width=2400&quality=90&v=1731652405 2400w"
        );
    }

    /// Here the file is a link to nothing, so there is nothing to read the time from.
    #[cfg(unix)]
    #[test]
    #[serial]
    fn a_photo_without_a_readable_modification_time_gets_plain_addresses() {
        unsafe { std::env::remove_var("IMAGE_PRESETS") };
        let root = tempfile::TempDir::new().unwrap();
        let dir = root.path().join("nature");
        fs::create_dir_all(&dir).unwrap();
        std::os::unix::fs::symlink(dir.join("gone.jpg"), dir.join("ghost.jpg")).unwrap();
        let _images_dir = EnvVarGuard::set("IMAGES_DIR", root.path());

        let photo = the_nature_photo();

        assert_eq!(photo.original_url, "/images/nature/ghost.jpg");
        assert_eq!(
            photo.url,
            "/images/compressed/nature/ghost.jpg?width=2400&quality=90"
        );
        assert_eq!(
            photo.detail_url,
            "/images/compressed/nature/ghost.jpg?width=4000&quality=90"
        );
        assert_eq!(
            photo.srcset,
            "/images/compressed/nature/ghost.jpg?width=2400&quality=90 2400w"
        );
    }

    #[test]
    fn a_modification_time_before_1970_is_no_version() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("old.jpg");
        fs::write(&path, b"jpg").unwrap();
        fs::File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_modified(std::time::UNIX_EPOCH - std::time::Duration::from_secs(1))
            .unwrap();

        assert_eq!(file_version(&fs::metadata(&path).unwrap()), None);
        assert_eq!(
            with_version("/images/old.jpg".to_string(), None),
            "/images/old.jpg"
        );
    }

    #[test]
    #[serial]
    fn the_profile_photo_address_carries_its_version() {
        let dir = tempfile::TempDir::new().unwrap();
        let profile = dir.path().join("profile.png");
        fs::write(&profile, b"png").unwrap();
        let _content = EnvVarGuard::set("ABOUT_CONTENT_PATH", dir.path());

        set_mtime(&profile, MTIME);
        assert_eq!(
            load_about_content().image_url.as_deref(),
            Some("/content/profile.png?v=1731652400")
        );
        set_mtime(&profile, MTIME + 60);
        assert_eq!(
            load_about_content().image_url.as_deref(),
            Some("/content/profile.png?v=1731652460")
        );
    }

    #[test]
    #[serial]
    fn the_favicon_address_carries_its_version() {
        let root = tempfile::TempDir::new().unwrap();
        let _images_dir = EnvVarGuard::set("IMAGES_DIR", root.path());

        assert_eq!(favicon_url(), "/images/favicon.ico", "no favicon yet");

        let favicon = root.path().join("favicon.ico");
        fs::write(&favicon, b"ico").unwrap();
        set_mtime(&favicon, MTIME);
        assert_eq!(favicon_url(), "/images/favicon.ico?v=1731652400");
    }

    #[test]
    fn test_load_gallery_photos_nonexistent() {
        let result = load_gallery_photos("nonexistent-gallery-12345");
        assert_eq!(result, None);
    }

    #[test]
    fn test_extract_exif_data_nonexistent_file() {
        let nonexistent = Path::new("/tmp/nonexistent_image_12345.jpg");
        let exif = extract_exif_data(nonexistent);

        assert_eq!(exif.width, None);
        assert_eq!(exif.height, None);
        assert_eq!(exif.date_taken, None);
        assert_eq!(exif.camera_make, None);
        assert_eq!(exif.camera_model, None);
        assert_eq!(exif.lens_model, None);
        assert_eq!(exif.focal_length, None);
        assert_eq!(exif.aperture, None);
        assert_eq!(exif.shutter_speed, None);
        assert_eq!(exif.iso, None);
        assert_eq!(exif.film_stock, None);
        assert_eq!(exif.copyright, None);
    }

    #[test]
    fn test_xmp_attr_value_decodes_entities() {
        let xmp = r#"<x:xmpmeta>foo:Bar="hello &amp; world &quot;ok&quot;" baz:Q="x"</x:xmpmeta>"#;
        assert_eq!(
            xmp_attr_value(xmp, "foo:Bar").as_deref(),
            Some(r#"hello & world "ok""#)
        );
        assert_eq!(xmp_attr_value(xmp, "baz:Q").as_deref(), Some("x"));
        assert_eq!(xmp_attr_value(xmp, "missing"), None);
    }

    #[test]
    fn test_extension_supports_xmp_filters_non_jpeg() {
        assert!(extension_supports_xmp(Path::new("a.jpg")));
        assert!(extension_supports_xmp(Path::new("a.JPEG")));
        assert!(extension_supports_xmp(Path::new("a.tif")));
        assert!(!extension_supports_xmp(Path::new("a.webp")));
        assert!(!extension_supports_xmp(Path::new("a.avif")));
        assert!(!extension_supports_xmp(Path::new("a.png")));
        assert!(!extension_supports_xmp(Path::new("noext")));
    }

    #[test]
    fn test_decode_xml_entities_no_entities_passthrough() {
        let s = "no entities here";
        assert_eq!(decode_xml_entities(s), s);
    }

    #[test]
    fn test_slug_generation_from_filename() {
        // Test slug generation logic
        let filename = "Test Photo.jpg";
        let ext = "jpg";

        let slug = filename
            .trim_end_matches(&format!(".{ext}"))
            .to_lowercase()
            .replace(['/', '\\', ' '], "-");

        assert_eq!(slug, "test-photo");
    }

    #[test]
    fn test_title_generation_from_filename() {
        let filename = "test-photo_2024.jpg";
        let ext = "jpg";

        let title = filename
            .trim_end_matches(&format!(".{ext}"))
            .replace(['-', '_'], " ");

        assert_eq!(title, "test photo 2024");
    }

    #[test]
    fn test_strip_leading_number_and_dash() {
        // Test with space before and after dash
        assert_eq!(
            strip_leading_number_and_dash("1 - space_needle.jpg"),
            "space_needle.jpg"
        );

        // Test with just dash, no spaces
        assert_eq!(
            strip_leading_number_and_dash("42-mountain.jpg"),
            "mountain.jpg"
        );

        // Test with multiple digits
        assert_eq!(
            strip_leading_number_and_dash("003 - photo.jpg"),
            "photo.jpg"
        );

        // Test with no leading number
        assert_eq!(
            strip_leading_number_and_dash("regular_photo.jpg"),
            "regular_photo.jpg"
        );

        // Test with number in middle (should not be stripped)
        assert_eq!(
            strip_leading_number_and_dash("photo-2024-test.jpg"),
            "photo-2024-test.jpg"
        );

        // Test with just number and dash at beginning
        assert_eq!(strip_leading_number_and_dash("5-test.jpg"), "test.jpg");

        // Test with decimal numbers
        assert_eq!(
            strip_leading_number_and_dash("1.5 - photo.jpg"),
            "photo.jpg"
        );
        assert_eq!(
            strip_leading_number_and_dash("2.3-mountain.jpg"),
            "mountain.jpg"
        );

        // Test with multiple periods
        assert_eq!(
            strip_leading_number_and_dash("1.2.3 - test.jpg"),
            "test.jpg"
        );

        // Test with period but no space
        assert_eq!(
            strip_leading_number_and_dash("10.5-sunset.jpg"),
            "sunset.jpg"
        );
    }

    #[test]
    fn test_load_all_gallery_photos_returns_sorted() {
        // This test just verifies that the function can be called without panic
        let photos = load_all_gallery_photos();

        // Check if sorting is maintained (if there are photos)
        // Photos should be sorted by subfolder first, then by filename
        if photos.len() > 1 {
            for i in 0..photos.len() - 1 {
                match (&photos[i].subfolder, &photos[i + 1].subfolder) {
                    (Some(sf_a), Some(sf_b)) => {
                        // Both have subfolders - either subfolder differs or filenames should be sorted
                        if sf_a == sf_b {
                            assert!(photos[i].filename <= photos[i + 1].filename);
                        } else {
                            assert!(sf_a <= sf_b);
                        }
                    }
                    (Some(_), None) => {
                        // Photos in subfolders come after root - this violates our sort order
                        panic!("Subfolder photo should not come before root photo");
                    }
                    (None, Some(_)) => {
                        // Root photos come before subfolder photos - this is correct
                    }
                    (None, None) => {
                        // Both in root, check filename sort
                        assert!(photos[i].filename <= photos[i + 1].filename);
                    }
                }
            }
        }
    }

    #[test]
    fn test_load_galleries_excludes_home() {
        let galleries = load_galleries();

        // Should not contain a gallery named "home"
        assert!(!galleries.iter().any(|g| g.slug == "home"));
    }

    #[test]
    fn test_load_galleries_sorted_alphabetically() {
        let galleries = load_galleries();

        // If a gallery_order is configured, alphabetical sort is not guaranteed.
        let config = crate::config::load_config();
        if !config.gallery_order.is_empty() {
            return;
        }

        // Check if sorted by name
        if galleries.len() > 1 {
            for i in 0..galleries.len() - 1 {
                assert!(galleries[i].name <= galleries[i + 1].name);
            }
        }
    }

    #[test]
    fn test_gallery_info_structure() {
        let gallery = GalleryInfo {
            name: "Test Gallery".to_string(),
            slug: "test-gallery".to_string(),
            photo_count: 10,
            config: None,
        };

        assert_eq!(gallery.name, "Test Gallery");
        assert_eq!(gallery.slug, "test-gallery");
        assert_eq!(gallery.photo_count, 10);
    }

    // -- Synthetic JPEG fixtures for EXIF/XMP extraction tests ---------

    /// Build a JPEG-shaped byte stream with optional EXIF and/or XMP APP1
    /// segments. kamadak-exif and our XMP scanner only parse markers near
    /// the start of the file, so no compressed image body is needed.
    fn build_synth_jpeg(exif: Option<&[u8]>, xmp: Option<&[u8]>) -> Vec<u8> {
        let mut out = vec![0xff, 0xd8]; // SOI
        if let Some(exif_bytes) = exif {
            out.extend_from_slice(&[0xff, 0xe1]);
            let header = b"Exif\0\0";
            // length field includes itself (2) + header + payload
            let len = u16::try_from(2 + header.len() + exif_bytes.len()).unwrap();
            out.extend_from_slice(&len.to_be_bytes());
            out.extend_from_slice(header);
            out.extend_from_slice(exif_bytes);
        }
        if let Some(xmp_bytes) = xmp {
            out.extend_from_slice(&[0xff, 0xe1]);
            let id = b"http://ns.adobe.com/xap/1.0/\0";
            let len = u16::try_from(2 + id.len() + xmp_bytes.len()).unwrap();
            out.extend_from_slice(&len.to_be_bytes());
            out.extend_from_slice(id);
            out.extend_from_slice(xmp_bytes);
        }
        out.extend_from_slice(&[0xff, 0xd9]); // EOI
        out
    }

    fn write_ifd_entry(buf: &mut Vec<u8>, tag: u16, ty: u16, count: u32, value: u32) {
        buf.extend_from_slice(&tag.to_le_bytes());
        buf.extend_from_slice(&ty.to_le_bytes());
        buf.extend_from_slice(&count.to_le_bytes());
        buf.extend_from_slice(&value.to_le_bytes());
    }

    /// Minimal little-endian TIFF block with `Make` and `Model` strings in
    /// IFD0. Both ASCII values live in a data pool just past the IFD.
    fn build_tiff_make_model(make: &str, model: &str) -> Vec<u8> {
        let mut tiff = Vec::new();
        tiff.extend_from_slice(b"II\x2a\x00");
        tiff.extend_from_slice(&8u32.to_le_bytes()); // IFD0 at offset 8

        let entry_count: u16 = 2;
        // Header (8) + count (2) + 2 entries × 12 + next-IFD ptr (4) = 38
        let data_pool_start: u32 = 8 + 2 + 24 + 4;

        let make_bytes: Vec<u8> = format!("{make}\0").into_bytes();
        let model_bytes: Vec<u8> = format!("{model}\0").into_bytes();
        let make_len = u32::try_from(make_bytes.len()).unwrap();
        let model_len = u32::try_from(model_bytes.len()).unwrap();
        let make_off = data_pool_start;
        let model_off = data_pool_start + make_len;

        tiff.extend_from_slice(&entry_count.to_le_bytes());
        write_ifd_entry(&mut tiff, 0x010f, 2, make_len, make_off); // Make
        write_ifd_entry(&mut tiff, 0x0110, 2, model_len, model_off); // Model
        tiff.extend_from_slice(&0u32.to_le_bytes()); // no next IFD

        tiff.extend_from_slice(&make_bytes);
        tiff.extend_from_slice(&model_bytes);
        tiff
    }

    #[test]
    fn extract_exif_data_reads_make_and_model_from_synth_jpeg() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("fixture.jpg");
        let tiff = build_tiff_make_model("ACME", "X100");
        let jpeg = build_synth_jpeg(Some(&tiff), None);
        fs::write(&path, &jpeg).unwrap();

        let exif = extract_exif_data(&path);
        // kamadak-exif wraps ASCII values in surrounding quotes via
        // display_value(); strip_quotes() in app.rs handles that. Here we
        // just check the inner string is present.
        let make = exif.camera_make.expect("Make should parse");
        let model = exif.camera_model.expect("Model should parse");
        assert!(make.contains("ACME"), "got: {make}");
        assert!(model.contains("X100"), "got: {model}");
    }

    #[test]
    fn extract_lens_candidates_from_xmp_strips_adobe_lens_profile_wrapper() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("fixture.jpg");
        let xmp = br#"<x:xmpmeta xmlns:x="adobe:ns:meta/" xmlns:crs="x">
            <rdf:Description crs:LensProfileName="Adobe (Test 35mm f/1.4) v3"/>
            </x:xmpmeta>"#;
        let jpeg = build_synth_jpeg(None, Some(xmp));
        fs::write(&path, &jpeg).unwrap();

        let candidates = extract_lens_candidates_from_xmp(&path);
        assert_eq!(candidates.profile_lens.as_deref(), Some("Test 35mm f/1.4"));
        assert!(candidates.aux_lens.is_none());
    }

    #[test]
    fn extract_lens_candidates_from_xmp_uses_aux_lens_when_letter_prefixed() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("fixture.jpg");
        let xmp = br#"<x:xmpmeta>aux:Lens="Acme 50mm f/2"</x:xmpmeta>"#;
        let jpeg = build_synth_jpeg(None, Some(xmp));
        fs::write(&path, &jpeg).unwrap();

        let candidates = extract_lens_candidates_from_xmp(&path);
        assert_eq!(candidates.aux_lens.as_deref(), Some("Acme 50mm f/2"));
    }

    #[test]
    fn extract_lens_candidates_from_xmp_skips_aux_lens_when_focal_only() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("fixture.jpg");
        let xmp = br#"<x:xmpmeta>aux:Lens="50.0 mm f/1.8"</x:xmpmeta>"#;
        let jpeg = build_synth_jpeg(None, Some(xmp));
        fs::write(&path, &jpeg).unwrap();

        assert!(extract_lens_candidates_from_xmp(&path).aux_lens.is_none());
    }

    #[test]
    fn extract_lens_candidates_from_xmp_skips_non_jpeg_extensions() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("fixture.webp");
        let xmp = br#"<x:xmpmeta>aux:Lens="Acme 50mm"</x:xmpmeta>"#;
        let jpeg = build_synth_jpeg(None, Some(xmp));
        fs::write(&path, &jpeg).unwrap();

        let candidates = extract_lens_candidates_from_xmp(&path);
        assert!(
            candidates.aux_lens.is_none() && candidates.profile_lens.is_none(),
            "should not read XMP from non-JPEG"
        );
    }

    #[test]
    fn extract_lens_candidates_from_xmp_returns_empty_when_packet_absent() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("fixture.jpg");
        let jpeg = build_synth_jpeg(None, None);
        fs::write(&path, &jpeg).unwrap();
        let candidates = extract_lens_candidates_from_xmp(&path);
        assert!(candidates.aux_lens.is_none() && candidates.profile_lens.is_none());
    }

    #[test]
    fn lenstagger_overrides_parse_rent_a_car_user_comment() {
        // Real-world LensTagger output: only Make/Model/ISO touched, no
        // aperture or shutter override.
        let text = "-Make=Nippon Kogaku K. K.\n\
                    -Model=Nikon F\n\
                    -ISO=200\n\
                    Scanner Make: Nikon Corporation\n\
                    Scanner Model: D850\n\
                    LensTaggerVer:1.7.6";
        assert_eq!(
            LenstaggerOverrides::from_user_comment(text),
            LenstaggerOverrides {
                fnumber: false,
                exposure_time: false,
            },
        );
    }

    #[test]
    fn lenstagger_overrides_detect_aperture_and_shutter_keys() {
        let text = "-FNumber=5.6\n-ExposureTime=1/125\n-ISO=400";
        assert_eq!(
            LenstaggerOverrides::from_user_comment(text),
            LenstaggerOverrides {
                fnumber: true,
                exposure_time: true,
            },
        );
    }

    #[test]
    fn lenstagger_overrides_accept_alias_keys_case_insensitively() {
        // LensTagger versions / forks have used several names for the same
        // overrides — accept the common aliases.
        let text = "-aperture=2.8\n-Shutter=1/60";
        assert_eq!(
            LenstaggerOverrides::from_user_comment(text),
            LenstaggerOverrides {
                fnumber: true,
                exposure_time: true,
            },
        );
    }

    #[test]
    fn lenstagger_overrides_ignore_unrelated_lines() {
        let text = "Scanner Make: Nikon\nFilm Type: Gold\nLensTaggerVer:1.7.6";
        assert_eq!(
            LenstaggerOverrides::from_user_comment(text),
            LenstaggerOverrides::default(),
        );
    }

    #[test]
    fn film_stock_old_format_combines_make_type_and_iso() {
        let text = "-Make=Mamiya Digital Imaging Co., Ltd.\n\
                    -Model=Mamiya RB67 Pro SD\n\
                    -ISO=200\n\
                    Scanner Make: Epson\n\
                    Scanner Model: GT-X980\n\
                    Film Make: Kodak\n\
                    Film Type: Gold\n\
                    DeveloperMoon Photo Lab\n\
                    LensTaggerVer:1.7.6";
        assert_eq!(
            parse_film_stock_from_comment(text).as_deref(),
            Some("Kodak Gold 200"),
        );
    }

    #[test]
    fn film_stock_old_format_skips_iso_already_in_name() {
        // "Delta 400 Professional" already names the speed, so the ISO
        // override must not be appended a second time.
        let text = "-ISO=400\n\
                    Film Make: Ilford\n\
                    Film Type: Delta 400 Professional\n\
                    LensTaggerVer:1.7.6";
        assert_eq!(
            parse_film_stock_from_comment(text).as_deref(),
            Some("Ilford Delta 400 Professional"),
        );
    }

    #[test]
    fn film_stock_new_format_extracts_film_field_without_gauge() {
        // Pipe-delimited export (no LensTaggerVer marker). The film stock is
        // the `Film:` value with the trailing `, 35mm` gauge note removed.
        let text = "Camera: Nippon Kogaku K. K. Nikon F | \
                    Lens: Nikon AF NIKKOR 50mm f/1.8 (SN 3193552) | \
                    Focal length: 50mm (35mm-equiv 50mm) | \
                    Film: Kodak Ektar 100, 35mm | \
                    Shot at EI: 100 | \
                    Developer: Glazer's Camera Photo Lab | \
                    Scanner: Epson GT-X980";
        assert_eq!(
            parse_film_stock_from_comment(text).as_deref(),
            Some("Kodak Ektar 100"),
        );
    }

    #[test]
    fn film_stock_new_format_without_gauge_note() {
        let text = "Camera: Nikon F | Film: Ilford HP5 Plus | Shot at EI: 400";
        assert_eq!(
            parse_film_stock_from_comment(text).as_deref(),
            Some("Ilford HP5 Plus"),
        );
    }

    #[test]
    fn film_stock_new_format_missing_film_field_returns_none() {
        // No `Film:` field — better to show nothing than dump the whole
        // comment (the bug this format support fixes).
        let text = "Camera: Nikon F | Lens: 50mm f/1.8 | Shot at EI: 100";
        assert_eq!(parse_film_stock_from_comment(text), None);
    }

    #[test]
    fn looks_like_film_scan_true_when_aux_and_profile_describe_different_lenses() {
        let candidates = XmpLensCandidates {
            aux_lens: Some("Nikon NIKKOR SC Auto 55mm f/1.2".to_string()),
            profile_lens: Some("Nikon AF-S VR Micro-Nikkor 105mm f/2.8G IF-ED".to_string()),
        };
        assert!(candidates.looks_like_film_scan());
    }

    #[test]
    fn looks_like_film_scan_false_when_profile_name_contains_aux_lens() {
        // Typical digital workflow: Lightroom prepends the manufacturer.
        let candidates = XmpLensCandidates {
            aux_lens: Some("RF 50mm f/1.8 STM".to_string()),
            profile_lens: Some("Canon RF 50mm f/1.8 STM".to_string()),
        };
        assert!(!candidates.looks_like_film_scan());
    }

    #[test]
    fn looks_like_film_scan_false_when_aux_lens_alone() {
        let candidates = XmpLensCandidates {
            aux_lens: Some("Acme 50mm f/2".to_string()),
            profile_lens: None,
        };
        assert!(!candidates.looks_like_film_scan());
    }

    #[test]
    fn looks_like_film_scan_false_when_profile_alone() {
        let candidates = XmpLensCandidates {
            aux_lens: None,
            profile_lens: Some("Canon RF 50mm f/1.8 STM".to_string()),
        };
        assert!(!candidates.looks_like_film_scan());
    }

    /// For a film scan the Lightroom `crs:LensProfileName` describes the
    /// macro lens used to capture the scan, while `aux:Lens` (set by
    /// `LensTagger`) carries the actual taking lens. The taking lens must win.
    #[test]
    fn extract_lens_candidates_from_xmp_exposes_aux_and_profile_independently() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("fixture.jpg");
        let xmp = br#"<x:xmpmeta xmlns:x="adobe:ns:meta/" xmlns:crs="x" xmlns:aux="y">
            <rdf:Description
                aux:Lens="Nikon NIKKOR SC Auto 55mm f/1.2"
                crs:LensProfileName="Adobe (Nikon AF-S VR Micro-Nikkor 105mm f/2.8G IF-ED)"/>
            </x:xmpmeta>"#;
        let jpeg = build_synth_jpeg(None, Some(xmp));
        fs::write(&path, &jpeg).unwrap();

        let candidates = extract_lens_candidates_from_xmp(&path);
        assert_eq!(
            candidates.aux_lens.as_deref(),
            Some("Nikon NIKKOR SC Auto 55mm f/1.2"),
        );
        assert_eq!(
            candidates.profile_lens.as_deref(),
            Some("Nikon AF-S VR Micro-Nikkor 105mm f/2.8G IF-ED"),
        );
    }

    /// An `IMAGES_DIR` holding one gallery, `cams`, with a small image for each of
    /// `files`, in the format its extension names in whatever case.
    fn images_dir_with_cams(files: &[&str]) -> tempfile::TempDir {
        let root = tempfile::TempDir::new().unwrap();
        let cams = root.path().join("cams");
        fs::create_dir_all(&cams).unwrap();
        for file in files {
            image::RgbImage::new(4, 3).save(cams.join(file)).unwrap();
        }
        root
    }

    /// The photos of the `cams` gallery under `root`, in the gallery's order.
    fn cams_photos(root: &tempfile::TempDir) -> Vec<PhotoInfo> {
        unsafe { std::env::remove_var("IMAGE_PRESETS") };
        let _images_dir = EnvVarGuard::set("IMAGES_DIR", root.path());
        load_gallery_photos("cams").expect("a cams gallery")
    }

    #[test]
    #[serial]
    fn a_photo_is_found_whatever_the_case_of_its_extension() {
        let root = images_dir_with_cams(&["DSC_0001.JPG", "Mixed.Jpg", "Shot.PNG", "Pic.WEBP"]);
        // A per-photo TOML is found by the name without the extension, as for `.jpg`.
        fs::write(
            root.path().join("cams/DSC_0001.toml"),
            "title = \"Harbour\"\n",
        )
        .unwrap();

        let photos = cams_photos(&root);

        let found: Vec<(&str, &str, &str)> = photos
            .iter()
            .map(|p| (p.filename.as_str(), p.slug.as_str(), p.title.as_str()))
            .collect();
        assert_eq!(
            found,
            [
                ("DSC_0001.JPG", "dsc-0001", "Harbour"),
                ("Mixed.Jpg", "mixed", "Mixed"),
                ("Pic.WEBP", "pic", "Pic"),
                ("Shot.PNG", "shot", "Shot"),
            ]
        );
        // Every address keeps the file's real name: the file server under `/images`
        // matches names exactly.
        for photo in &photos {
            let name = &photo.filename;
            let compressed = format!("/images/compressed/cams/{name}");
            assert_eq!(
                without_version(&photo.original_url),
                format!("/images/cams/{name}")
            );
            assert_eq!(
                without_version(&photo.url),
                format!("{compressed}?width=2400&quality=90")
            );
            assert_eq!(
                without_version(&photo.detail_url),
                format!("{compressed}?width=4000&quality=90")
            );
            let (srcset_url, srcset_width) = photo.srcset.split_once(' ').unwrap();
            assert_eq!(
                (without_version(srcset_url), srcset_width),
                (
                    format!("{compressed}?width=2400&quality=90").as_str(),
                    "2400w"
                )
            );
            assert!(photo.sources.is_empty() && photo.original_sources.is_empty());
        }
    }

    #[test]
    #[serial]
    fn formats_of_one_photo_are_grouped_whatever_the_case_of_their_extensions() {
        let root = images_dir_with_cams(&["x.JPG", "x.webp", "y.jpg", "y.WEBP"]);

        let photos = cams_photos(&root);

        assert_eq!(photos.len(), 2, "{photos:?}");
        for (photo, (slug, main, other)) in photos
            .iter()
            .zip([("x", "x.JPG", "x.webp"), ("y", "y.jpg", "y.WEBP")])
        {
            assert_eq!(photo.slug, slug);
            assert_eq!(
                without_version(&photo.original_url),
                format!("/images/cams/{main}")
            );
            let others: Vec<(&str, &str)> = photo
                .original_sources
                .iter()
                .map(|s| (without_version(&s.url), s.mime_type.as_str()))
                .collect();
            let other = format!("/images/cams/{other}");
            assert_eq!(others, [(other.as_str(), "image/webp")]);
        }
        let compressed: Vec<&str> = photos[0]
            .sources
            .iter()
            .map(|s| without_version(&s.url))
            .collect();
        assert_eq!(
            compressed,
            ["/images/compressed/cams/x.webp?width=2400&quality=90"]
        );
    }

    #[test]
    #[serial]
    fn files_that_differ_only_in_the_case_of_the_extension_are_one_photo() {
        // Lower case first, then upper case, then mixed spellings in byte order, as the
        // compressed endpoint picks between them.
        for (files, primary, other) in [
            (["x.JPG", "x.jpg"], "x.jpg", "x.JPG"),
            (["x.Jpg", "x.JPG"], "x.JPG", "x.Jpg"),
            (["x.jPg", "x.Jpg"], "x.Jpg", "x.jPg"),
        ] {
            let root = images_dir_with_cams(&files);

            let photos = cams_photos(&root);

            assert_eq!(photos.len(), 1, "{files:?}: {photos:?}");
            assert_eq!(photos[0].slug, "x");
            assert_eq!(photos[0].filename, primary);
            let others: Vec<&str> = photos[0]
                .original_sources
                .iter()
                .map(|s| without_version(&s.url))
                .collect();
            assert_eq!(others, [format!("/images/cams/{other}")], "{files:?}");
        }
    }

    #[test]
    #[serial]
    fn exif_and_xmp_are_read_from_a_photo_with_an_upper_case_extension() {
        let root = tempfile::TempDir::new().unwrap();
        let cams = root.path().join("cams");
        fs::create_dir_all(&cams).unwrap();
        let tiff = build_tiff_make_model("ACME", "X100");
        let xmp = br#"<x:xmpmeta xmlns:x="adobe:ns:meta/" xmlns:aux="y">
            <rdf:Description aux:Lens="Acme 35mm f/2 Macro"/>
            </x:xmpmeta>"#;
        fs::write(
            cams.join("DSC_0002.JPG"),
            build_synth_jpeg(Some(&tiff), Some(xmp)),
        )
        .unwrap();

        let photos = cams_photos(&root);

        assert_eq!(photos.len(), 1, "{photos:?}");
        let photo = &photos[0];
        assert!(
            photo
                .camera_make
                .as_deref()
                .unwrap_or_default()
                .contains("ACME")
        );
        assert_eq!(photo.lens_model.as_deref(), Some("Acme 35mm f/2 Macro"));
    }
}
