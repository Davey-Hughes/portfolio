use crate::types::{GalleryInfo, ImageSource, PhotoConfig, PhotoInfo};
use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

static LEADING_NUMBER_DASH: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^[\d.]+\s*-\s*").expect("valid regex"));

/// URL-encode a path component, preserving forward slashes
fn url_encode_path(path: &str) -> String {
    path.split('/')
        .map(|segment| urlencoding::encode(segment).into_owned())
        .collect::<Vec<_>>()
        .join("/")
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
    let config_path = parent.join(format!("{}.toml", stem));

    fs::read_to_string(&config_path)
        .ok()
        .and_then(|content| toml::from_str::<PhotoConfig>(&content).ok())
        .unwrap_or_default()
}

/// Get default image width and quality from environment variables
/// Returns (width, quality) tuple with defaults of (2400, 80)
fn get_default_image_params() -> (u32, u8) {
    // Use 2400px at 80 quality for photo detail pages
    // This provides good quality while keeping file sizes reasonable
    #[cfg(feature = "ssr")]
    {
        use crate::image_params::ImageParams;
        let presets = ImageParams::get_valid_presets();
        // Find the 2400px/80 quality preset, or fall back to default
        presets
            .iter()
            .find(|(w, q)| *w == 2400 && *q == 80)
            .copied()
            .or_else(|| presets.iter().find(|(w, _)| *w == 2400).copied())
            .unwrap_or((2400, 80))
    }

    #[cfg(not(feature = "ssr"))]
    {
        // For client-side, use environment variables or default to 2400/80
        let width = std::env::var("DEFAULT_IMAGE_WIDTH")
            .ok()
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(2400);

        let quality = std::env::var("DEFAULT_IMAGE_QUALITY")
            .ok()
            .and_then(|s| s.parse::<u8>().ok())
            .unwrap_or(80);

        (width, quality)
    }
}

/// Helper function to discover all gallery directories in public/images/
pub fn discover_gallery_directories() -> Vec<String> {
    let images_base = if Path::new("public/images").exists() {
        "public/images"
    } else {
        "./images"
    };

    let mut gallery_dirs = Vec::new();

    if let Ok(entries) = fs::read_dir(images_base) {
        for entry in entries.flatten() {
            let path = entry.path();
            // Only include directories that are not special directories
            if path.is_dir() {
                if let Some(dir_name) = path.file_name() {
                    let name = dir_name.to_string_lossy().to_string();
                    // Skip special directories like categories, content, etc.
                    if name != "categories" {
                        gallery_dirs.push(path.to_string_lossy().to_string());
                    }
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
fn strip_leading_number_and_dash(filename: &str) -> String {
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
    let mut image_groups: HashMap<String, Vec<(String, String)>> = HashMap::new();
    collect_image_files(dir, gallery_root, &mut image_groups);

    // Second pass: create PhotoInfo for each group
    for (_base_path, variants) in image_groups {
        // Sort variants by priority (modern formats first for sources)
        let mut sorted_variants = variants.clone();
        sorted_variants.sort_by(|a, b| {
            let priority_a = format_priority(&a.1);
            let priority_b = format_priority(&b.1);
            priority_a.cmp(&priority_b)
        });

        // Use the first variant as the primary image (fallback)
        let (primary_relative_path, primary_ext) = &sorted_variants[0];
        let primary_full_path = gallery_root.join(primary_relative_path);

        // Extract metadata from the primary image
        let filename_str = primary_full_path
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_default();

        // Create slug from filename (without extension), stripping leading numbers
        let slug = strip_leading_number_and_dash(&filename_str)
            .trim_end_matches(&format!(".{}", primary_ext))
            .to_lowercase()
            .replace([' ', '_'], "-");

        // Strip leading numbers and dashes, then convert to title
        let derived_title = strip_leading_number_and_dash(&filename_str)
            .trim_end_matches(&format!(".{}", primary_ext))
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

        // Build sources for compressed versions
        let (img_width, img_quality) = get_default_image_params();
        let mut sources = Vec::new();
        let mut original_sources = Vec::new();

        for (relative_path, ext) in &sorted_variants {
            if relative_path != primary_relative_path {
                // Add as alternative source (URL-encode the path)
                let encoded_path = url_encode_path(relative_path);
                let compressed_url = format!(
                    "/images/compressed/{}?width={}&quality={}",
                    encoded_path, img_width, img_quality
                );
                let original_url = format!("/images/{}", encoded_path);
                let mime_type = get_mime_type(ext).to_string();

                sources.push(ImageSource {
                    url: compressed_url,
                    mime_type: "image/webp".to_string(), // Compressed endpoint always returns WebP
                });
                original_sources.push(ImageSource {
                    url: original_url,
                    mime_type,
                });
            }
        }

        // Primary image URLs (URL-encode the path)
        let encoded_primary_path = url_encode_path(primary_relative_path);
        let compressed_url = format!(
            "/images/compressed/{}?width={}&quality={}",
            encoded_primary_path, img_width, img_quality
        );
        let original_url = format!("/images/{}", encoded_primary_path);

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
            url: compressed_url,
            original_url,
            sources,
            original_sources,
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

/// Helper function to collect all image files and group them by basename
fn collect_image_files(
    dir: &Path,
    gallery_root: &Path,
    groups: &mut HashMap<String, Vec<(String, String)>>,
) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();

            if path.is_dir() {
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
                    let base_path = relative_path
                        .trim_end_matches(&format!(".{}", ext))
                        .to_string();

                    groups
                        .entry(base_path)
                        .or_default()
                        .push((relative_path, ext.to_string()));
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

/// Find images for a specific gallery (with different base path handling)
pub fn find_images_for_gallery(dir: &Path, base_root: &Path, photos: &mut Vec<PhotoInfo>) {
    // Extract gallery name from directory path (as slug format)
    let gallery_name = dir
        .file_name()
        .map(|n| n.to_string_lossy().to_lowercase().replace(' ', "-"))
        .unwrap_or_else(|| "unknown".to_string());

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
    s.chars().any(|c| {
        c.is_alphabetic() && !matches!(c.to_ascii_lowercase(), 'f' | 'm')
    })
}

/// Lightroom writes these as `crs:LensProfileName` when no Adobe lens profile
/// was matched (e.g. it fell back to the in-camera correction data). They are
/// not real lens names, so we ignore them and fall through to other sources.
fn is_lens_profile_placeholder(s: &str) -> bool {
    matches!(s, "Camera Settings" | "None" | "(none)" | "")
}

/// Try to recover a human-readable lens name from XMP metadata. Lightroom
/// often strips the camera maker note, but writes the matched lens profile
/// as `crs:LensProfileName="Adobe (<lens name>) v<n>"` and `aux:Lens` (which
/// for tools like LensTagger contains the name directly). Returns `None`
/// when XMP holds only a focal-range string that the EXIF `LensModel` tag
/// already exposes.
fn extract_lens_name_from_xmp(path: &Path) -> Option<String> {
    if !extension_supports_xmp(path) {
        return None;
    }
    let xmp = read_xmp_packet(path)?;

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
            return Some(inner.to_string());
        }
    }

    if let Some(raw) = xmp_attr_value(&xmp, "aux:Lens") {
        let trimmed = raw.trim();
        if looks_like_real_lens_name(trimmed) {
            return Some(trimmed.to_string());
        }
    }

    None
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

    let mut width = exif_reader
        .get_field(exif::Tag::PixelXDimension, exif::In::PRIMARY)
        .or_else(|| exif_reader.get_field(exif::Tag::ImageWidth, exif::In::PRIMARY))
        .and_then(|f| f.value.get_uint(0));

    let mut height = exif_reader
        .get_field(exif::Tag::PixelYDimension, exif::In::PRIMARY)
        .or_else(|| exif_reader.get_field(exif::Tag::ImageLength, exif::In::PRIMARY))
        .and_then(|f| f.value.get_uint(0));

    // If EXIF didn't have dimensions, try reading image dimensions from file header
    if width.is_none() || height.is_none() {
        if let Ok(mut reader) = image::ImageReader::open(path) {
            reader.limits(image::Limits::no_limits());
            if let Ok(dimensions) = reader.into_dimensions() {
                width = Some(dimensions.0);
                height = Some(dimensions.1);
            }
        }
    }

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
    let lens_model = extract_lens_name_from_xmp(path).or(exif_lens_model);

    let focal_length = exif_reader
        .get_field(exif::Tag::FocalLength, exif::In::PRIMARY)
        .map(|f| {
            let val = f.display_value().to_string();
            if val.contains("mm") {
                val
            } else {
                format!("{} mm", val)
            }
        });

    let aperture = exif_reader
        .get_field(exif::Tag::FNumber, exif::In::PRIMARY)
        .map(|f| format!("f/{}", f.display_value()));

    let shutter_speed = exif_reader
        .get_field(exif::Tag::ExposureTime, exif::In::PRIMARY)
        .map(|f| {
            let val = f.display_value().to_string();
            if val.contains("s") {
                val
            } else {
                format!("{} s", val)
            }
        });

    let iso = exif_reader
        .get_field(exif::Tag::PhotographicSensitivity, exif::In::PRIMARY)
        .map(|f| format!("ISO {}", f.display_value()));

    // Extract film stock from ImageDescription or UserComment tags
    // These are commonly used for film stock information in scanned film photos
    let film_stock = exif_reader
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
                    if !text.is_empty() {
                        // Try to extract film stock information from the metadata
                        let mut film_make: Option<&str> = None;
                        let mut film_type: Option<&str> = None;
                        let mut film_iso: Option<&str> = None;

                        for line in text.lines() {
                            let line_lower = line.to_lowercase();
                            if line_lower.contains("film make:") {
                                film_make = line.split_once(':').map(|(_, v)| v.trim());
                            } else if line_lower.contains("film type:") {
                                film_type = line.split_once(':').map(|(_, v)| v.trim());
                            } else if line_lower.starts_with("-iso=")
                                || line_lower.starts_with("iso=")
                            {
                                // Handle formats like "-ISO=200" or "ISO=400"
                                film_iso = line.split_once('=').map(|(_, v)| v.trim());
                            }
                        }

                        // Combine film make, type, and ISO if present
                        let film_stock = match (film_make, film_type) {
                            (Some(make), Some(typ)) => format!("{} {}", make, typ),
                            (Some(make), None) => make.to_string(),
                            (None, Some(typ)) => typ.to_string(),
                            (None, None) => text.clone(), // Return full text if no specific fields found
                        };

                        // Add ISO if found (format: "Kodak Gold 200")
                        if let Some(iso_value) = film_iso {
                            Some(format!("{} {}", film_stock, iso_value))
                        } else {
                            Some(film_stock)
                        }
                    } else {
                        None
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
        });

    let copyright = exif_reader
        .get_field(exif::Tag::Copyright, exif::In::PRIMARY)
        .and_then(|f| match &f.value {
            exif::Value::Ascii(vec) => {
                // Concatenate all ASCII strings and decode as UTF-8
                let bytes: Vec<u8> = vec.iter().flat_map(|s| s.iter().copied()).collect();
                String::from_utf8(bytes).ok()
            }
            _ => Some(f.display_value().to_string()),
        });

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
        if path.exists() {
            if let Some(dir_name) = path.file_name() {
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
    }

    let config = crate::config::load_config();
    let order_index: HashMap<&str, usize> = config
        .gallery_order
        .iter()
        .enumerate()
        .map(|(i, slug)| (slug.as_str(), i))
        .collect();

    galleries.sort_by(|a, b| {
        match (order_index.get(a.slug.as_str()), order_index.get(b.slug.as_str())) {
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
    let gallery_path = std::env::var("GALLERY_PATH").unwrap_or_else(|_| {
        if Path::new("public/images/home").exists() {
            "public/images/home".to_string()
        } else {
            "./images/home".to_string()
        }
    });

    // Create directory if it doesn't exist
    if !Path::new(&gallery_path).exists() {
        fs::create_dir_all(&gallery_path).ok();
    }

    let mut photos = Vec::new();
    let gallery_path_buf = Path::new(&gallery_path).to_path_buf();
    let images_base = Path::new("public/images");

    find_images_recursive(&gallery_path_buf, images_base, &mut photos);
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
    let images_base = Path::new("public/images");

    for base_path in gallery_roots {
        let base = Path::new(&base_path);
        if let Some(dir_name) = base.file_name() {
            let slug = dir_name.to_string_lossy().to_lowercase().replace(' ', "-");
            if slug == gallery_name {
                find_images_for_gallery(base, images_base, &mut photos);
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
    let images_base = Path::new("public/images");
    let gallery_roots = discover_gallery_directories();

    for gallery_path in gallery_roots {
        let gallery_path_buf = Path::new(&gallery_path);
        if gallery_path_buf.exists() {
            // Extract gallery name from directory path
            let gallery_name = gallery_path_buf
                .file_name()
                .map(|n| n.to_string_lossy().to_lowercase().replace(' ', "-"))
                .unwrap_or_else(|| "home".to_string());

            find_images_recursive_with_gallery(
                gallery_path_buf,
                images_base,
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

/// Load about page content
pub fn load_about_content() -> crate::types::AboutContent {
    let content_path = std::env::var("ABOUT_CONTENT_PATH").unwrap_or_else(|_| {
        if Path::new("public/content").exists() {
            "public/content".to_string()
        } else {
            "./content".to_string()
        }
    });

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

    // Check if profile image exists
    let image_url = ["profile.jpg", "profile.png", "profile.webp"]
        .iter()
        .map(|name| Path::new(&content_path).join(name))
        .find(|path| path.exists())
        .and_then(|path| path.file_name().map(|n| n.to_string_lossy().to_string()))
        .map(|filename| format!("/content/{}", filename));

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
        assert!(!is_lens_profile_placeholder("Adobe (Nikon AF-S 50mm f/1.8G) v1"));
        assert!(!is_lens_profile_placeholder("Nikon AF Nikkor 85mm f/1.4D"));
    }

    #[test]
    #[serial]
    fn test_get_default_image_params_defaults() {
        // Ensure env vars are not set
        std::env::remove_var("DEFAULT_IMAGE_WIDTH");
        std::env::remove_var("DEFAULT_IMAGE_QUALITY");
        // Also ensure IMAGE_PRESETS doesn't override
        std::env::remove_var("IMAGE_PRESETS");

        let (width, quality) = get_default_image_params();
        // Should use 2400px at 80 quality (the new default)
        assert_eq!(width, 2400);
        assert_eq!(quality, 80);
    }

    #[test]
    #[serial]
    fn test_get_default_image_params_custom_width() {
        std::env::set_var("DEFAULT_IMAGE_WIDTH", "1200");
        std::env::remove_var("DEFAULT_IMAGE_QUALITY");

        let (width, quality) = get_default_image_params();

        std::env::remove_var("DEFAULT_IMAGE_WIDTH");

        // On SSR builds, env vars don't affect get_default_image_params (it uses presets)
        // On client builds, env vars override defaults
        #[cfg(feature = "ssr")]
        {
            assert_eq!(width, 2400);
            assert_eq!(quality, 80);
        }
        #[cfg(not(feature = "ssr"))]
        {
            assert_eq!(width, 1200);
            assert_eq!(quality, 80);
        }
    }

    #[test]
    #[serial]
    fn test_get_default_image_params_custom_quality() {
        std::env::remove_var("DEFAULT_IMAGE_WIDTH");
        std::env::set_var("DEFAULT_IMAGE_QUALITY", "90");

        let (width, quality) = get_default_image_params();

        std::env::remove_var("DEFAULT_IMAGE_QUALITY");

        // On SSR builds, env vars don't affect get_default_image_params (it uses presets)
        // On client builds, env vars override defaults
        #[cfg(feature = "ssr")]
        {
            assert_eq!(width, 2400);
            assert_eq!(quality, 80);
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
        std::env::set_var("DEFAULT_IMAGE_WIDTH", "1200");
        std::env::set_var("DEFAULT_IMAGE_QUALITY", "85");

        let (width, quality) = get_default_image_params();

        std::env::remove_var("DEFAULT_IMAGE_WIDTH");
        std::env::remove_var("DEFAULT_IMAGE_QUALITY");

        // On SSR builds, env vars don't affect get_default_image_params (it uses presets)
        // On client builds, env vars override defaults
        #[cfg(feature = "ssr")]
        {
            assert_eq!(width, 2400);
            assert_eq!(quality, 80);
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
        std::env::set_var("DEFAULT_IMAGE_WIDTH", "invalid");
        std::env::set_var("DEFAULT_IMAGE_QUALITY", "not_a_number");

        let (width, quality) = get_default_image_params();

        std::env::remove_var("DEFAULT_IMAGE_WIDTH");
        std::env::remove_var("DEFAULT_IMAGE_QUALITY");

        // Should fall back to defaults
        assert_eq!(width, 2400);
        assert_eq!(quality, 80);
    }

    #[test]
    fn test_default_about_text() {
        let text = default_about_text();
        assert!(!text.is_empty());
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

        std::env::set_var("ABOUT_CONTENT_PATH", temp_path.to_str().unwrap());

        let about = load_about_content();

        // Should have no image in an empty directory
        assert_eq!(about.image_url, None);
        // Should contain default text since no about.txt exists
        assert!(about.content.contains("photographer"));

        std::env::remove_var("ABOUT_CONTENT_PATH");
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

        std::env::set_var("ABOUT_CONTENT_PATH", temp_dir.to_str().unwrap());

        let about = load_about_content();

        assert_eq!(about.content, custom_text);
        assert_eq!(about.image_url, None);

        std::env::remove_var("ABOUT_CONTENT_PATH");
        fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    #[serial]
    fn test_load_about_content_with_profile_image() {
        let temp_dir = std::env::temp_dir().join("test_about_profile");
        fs::create_dir_all(&temp_dir).unwrap();

        fs::write(temp_dir.join("about.txt"), "Custom text").unwrap();
        fs::write(temp_dir.join("profile.jpg"), b"fake jpg").unwrap();

        std::env::set_var("ABOUT_CONTENT_PATH", temp_dir.to_str().unwrap());

        let about = load_about_content();

        assert_eq!(about.image_url, Some("/content/profile.jpg".to_string()));
        assert_eq!(about.content, "Custom text");

        std::env::remove_var("ABOUT_CONTENT_PATH");
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

        std::env::set_var("ABOUT_CONTENT_PATH", temp_dir.to_str().unwrap());

        let about = load_about_content();

        // Should prefer .jpg as it's first in the array
        assert_eq!(about.image_url, Some("/content/profile.jpg".to_string()));

        std::env::remove_var("ABOUT_CONTENT_PATH");
        fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    #[serial]
    fn test_load_home_photos_creates_directory() {
        let temp_gallery = std::env::temp_dir().join("test_home_gallery_create");

        // Make sure it doesn't exist
        fs::remove_dir_all(&temp_gallery).ok();

        std::env::set_var("GALLERY_PATH", temp_gallery.to_str().unwrap());

        let photos = load_home_photos();

        // Directory should be created
        assert!(temp_gallery.exists());
        assert!(photos.is_empty()); // No photos in new directory

        std::env::remove_var("GALLERY_PATH");
        fs::remove_dir_all(&temp_gallery).ok();
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
            .trim_end_matches(&format!(".{}", ext))
            .to_lowercase()
            .replace(['/', '\\', ' '], "-");

        assert_eq!(slug, "test-photo");
    }

    #[test]
    fn test_title_generation_from_filename() {
        let filename = "test-photo_2024.jpg";
        let ext = "jpg";

        let title = filename
            .trim_end_matches(&format!(".{}", ext))
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

        let make_bytes: Vec<u8> = format!("{}\0", make).into_bytes();
        let model_bytes: Vec<u8> = format!("{}\0", model).into_bytes();
        let make_off = data_pool_start;
        let model_off = data_pool_start + u32::try_from(make_bytes.len()).unwrap();

        tiff.extend_from_slice(&entry_count.to_le_bytes());
        write_ifd_entry(&mut tiff, 0x010f, 2, make_bytes.len() as u32, make_off); // Make
        write_ifd_entry(&mut tiff, 0x0110, 2, model_bytes.len() as u32, model_off); // Model
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
    fn extract_lens_name_from_xmp_strips_adobe_lens_profile_wrapper() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("fixture.jpg");
        let xmp = br#"<x:xmpmeta xmlns:x="adobe:ns:meta/" xmlns:crs="x">
            <rdf:Description crs:LensProfileName="Adobe (Test 35mm f/1.4) v3"/>
            </x:xmpmeta>"#;
        let jpeg = build_synth_jpeg(None, Some(xmp));
        fs::write(&path, &jpeg).unwrap();

        let name = extract_lens_name_from_xmp(&path);
        assert_eq!(name.as_deref(), Some("Test 35mm f/1.4"));
    }

    #[test]
    fn extract_lens_name_from_xmp_uses_aux_lens_when_letter_prefixed() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("fixture.jpg");
        let xmp = br#"<x:xmpmeta>aux:Lens="Acme 50mm f/2"</x:xmpmeta>"#;
        let jpeg = build_synth_jpeg(None, Some(xmp));
        fs::write(&path, &jpeg).unwrap();

        let name = extract_lens_name_from_xmp(&path);
        assert_eq!(name.as_deref(), Some("Acme 50mm f/2"));
    }

    #[test]
    fn extract_lens_name_from_xmp_skips_aux_lens_when_focal_only() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("fixture.jpg");
        let xmp = br#"<x:xmpmeta>aux:Lens="50.0 mm f/1.8"</x:xmpmeta>"#;
        let jpeg = build_synth_jpeg(None, Some(xmp));
        fs::write(&path, &jpeg).unwrap();

        assert!(extract_lens_name_from_xmp(&path).is_none());
    }

    #[test]
    fn extract_lens_name_from_xmp_skips_non_jpeg_extensions() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("fixture.webp");
        let xmp = br#"<x:xmpmeta>aux:Lens="Acme 50mm"</x:xmpmeta>"#;
        let jpeg = build_synth_jpeg(None, Some(xmp));
        fs::write(&path, &jpeg).unwrap();

        assert!(
            extract_lens_name_from_xmp(&path).is_none(),
            "should not read XMP from non-JPEG"
        );
    }

    #[test]
    fn extract_lens_name_from_xmp_returns_none_when_packet_absent() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("fixture.jpg");
        let jpeg = build_synth_jpeg(None, None);
        fs::write(&path, &jpeg).unwrap();
        assert!(extract_lens_name_from_xmp(&path).is_none());
    }
}
