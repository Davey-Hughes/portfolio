//! Image caching and processing utilities
//!
//! This module handles:
//! - Processing images and converting to WebP
//! - Caching processed images
//! - Cleaning up orphaned cache files
//! - Pre-warming the cache on startup

use leptos::logging::log;
use std::path::PathBuf;

/// Process and cache a single image with the given parameters
/// Returns the WebP data or None if processing failed
pub fn process_and_cache_image(
    images_dir: &str,
    cache_dir: &str,
    path_without_ext: &str,
    width: u32,
    quality: u8,
) -> Option<Vec<u8>> {
    let supported_extensions = ["jpg", "jpeg", "jxl", "avif", "webp", "png", "gif"];

    // Find the actual file with any supported extension
    let full_path = find_image_file(images_dir, path_without_ext, &supported_extensions)?;

    // Cache filename without timestamp - makes cache persistent. The `_l1`
    // suffix is an encoder-version tag: it changed when we switched from
    // the `image` crate's lossless WebP encoder to libwebp lossy. Bumping
    // this string invalidates older cache entries automatically (they
    // don't match the new name; the orphan sweep removes them in time).
    let cache_filename = format!(
        "{}_w{}_q{}_l1.webp",
        path_without_ext.replace(['/', '\\'], "_"),
        width,
        quality
    );

    let cache_path = std::path::PathBuf::from(cache_dir).join(&cache_filename);

    // Check if cached version exists and is newer than source
    if let Some(cached_data) = try_use_cached_image(&full_path, &cache_path) {
        return Some(cached_data);
    }

    // Process the image
    let webp_data = process_image(&full_path, width, quality)?;

    // Save to cache
    save_to_cache(cache_dir, &cache_path, &webp_data);

    Some(webp_data)
}

/// Find an image file with any of the supported extensions, refusing any
/// path that resolves outside `images_dir` (defends against `..`-style
/// traversal in the URL path).
fn find_image_file(
    images_dir: &str,
    path_without_ext: &str,
    extensions: &[&str],
) -> Option<PathBuf> {
    let images_root = std::path::PathBuf::from(images_dir).canonicalize().ok()?;
    extensions
        .iter()
        .filter_map(|ext| {
            let candidate = images_root.join(format!("{}.{}", path_without_ext, ext));
            // canonicalize() requires the file to exist; that doubles as an
            // existence check.
            let resolved = candidate.canonicalize().ok()?;
            resolved.starts_with(&images_root).then_some(resolved)
        })
        .next()
}

/// Try to use a cached image if it exists and is up-to-date
fn try_use_cached_image(source_path: &PathBuf, cache_path: &PathBuf) -> Option<Vec<u8>> {
    use std::fs;

    if !cache_path.exists() {
        return None;
    }

    // Compare modification times: only regenerate if source is newer than cache
    let source_mtime = source_path.metadata().ok().and_then(|m| m.modified().ok());
    let cache_mtime = cache_path.metadata().ok().and_then(|m| m.modified().ok());

    if let (Some(source_time), Some(cache_time)) = (source_mtime, cache_mtime) {
        // If cache is newer than or equal to source, use it
        if cache_time >= source_time {
            return fs::read(cache_path).ok();
        }
    } else {
        // If we can't get timestamps, just use the cache if it exists
        return fs::read(cache_path).ok();
    }

    None
}

/// Process an image: load, resize, and convert to WebP
fn process_image(full_path: &PathBuf, width: u32, quality: u8) -> Option<Vec<u8>> {
    // Load the image (handle JXL specially)
    let img = if full_path.extension().and_then(|e| e.to_str()) == Some("jxl") {
        load_jxl_image(full_path)?
    } else {
        load_standard_image(full_path)?
    };

    // Resize if needed
    let img = if img.width() > width {
        img.resize(width, u32::MAX, image::imageops::FilterType::Lanczos3)
    } else {
        img
    };

    // Convert to WebP
    convert_to_webp(&img, quality)
}

/// Load a JXL image using jxl-oxide
fn load_jxl_image(path: &PathBuf) -> Option<image::DynamicImage> {
    use image::DynamicImage;
    use jxl_oxide::integration::JxlDecoder;

    std::fs::File::open(path)
        .ok()
        .and_then(|file| JxlDecoder::new(file).ok())
        .and_then(|decoder| DynamicImage::from_decoder(decoder).ok())
}

/// Load a standard image format
fn load_standard_image(path: &PathBuf) -> Option<image::DynamicImage> {
    image::ImageReader::open(path)
        .ok()
        .map(|mut reader| {
            reader.limits(image::Limits::no_limits());
            reader.decode()
        })
        .and_then(|r| r.ok())
}

/// Convert an image to lossy WebP at the given quality (0-100). Uses
/// libwebp via the `webp` crate because the `image` crate's built-in
/// WebP encoder is lossless-only.
///
/// `#[doc(hidden)] pub` so `benches/image_pipeline.rs` and
/// `examples/alloc_profile.rs` can measure the encode step directly; not part
/// of the public API.
#[doc(hidden)]
pub fn convert_to_webp(img: &image::DynamicImage, quality: u8) -> Option<Vec<u8>> {
    // libwebp expects RGB or RGBA; convert to RGB8 for opaque images and
    // RGBA8 only when there's an alpha channel.
    let encoder = if img.color().has_alpha() {
        let rgba = img.to_rgba8();
        webp::Encoder::from_rgba(rgba.as_raw(), rgba.width(), rgba.height())
            .encode(f32::from(quality))
    } else {
        let rgb = img.to_rgb8();
        webp::Encoder::from_rgb(rgb.as_raw(), rgb.width(), rgb.height())
            .encode(f32::from(quality))
    };
    Some(encoder.to_vec())
}

/// Save processed image data to cache. Failures (full disk, read-only
/// cache dir, …) are logged but otherwise non-fatal: the image was still
/// produced for this request, only the cache speedup is lost.
fn save_to_cache(cache_dir: &str, cache_path: &PathBuf, data: &[u8]) {
    use std::fs;
    use std::io::Write;

    if let Err(err) = fs::create_dir_all(cache_dir) {
        log!("cache: create_dir_all({}) failed: {err}", cache_dir);
        return;
    }
    match fs::File::create(cache_path) {
        Ok(mut file) => {
            if let Err(err) = file.write_all(data) {
                log!("cache: write {} failed: {err}", cache_path.display());
            }
        }
        Err(err) => log!("cache: create {} failed: {err}", cache_path.display()),
    }
}

/// Clean up orphaned cache files that no longer have corresponding source images
/// Also removes old cache files that haven't been accessed in a while
pub fn cleanup_cache(images_dir: &str, cache_dir: &str) {
    use std::fs;
    use std::time::{Duration, SystemTime};

    log!("Starting cache cleanup...");

    let cache_path = std::path::Path::new(cache_dir);
    if !cache_path.exists() {
        log!("Cache directory does not exist, skipping cleanup");
        return;
    }

    // Enumerate every source image and compute the path-encoded prefix
    // `process_and_cache_image` would produce. Doing it this way avoids the
    // ambiguous reverse decoding that previously deleted cache files for any
    // source whose name happened to contain `_` or `_w`.
    let valid_prefixes = collect_valid_prefixes(images_dir);

    let mut orphaned_count = 0;
    let mut old_count = 0;
    let mut error_count = 0;

    // Cache files not accessed in 30 days will be removed
    let max_age = Duration::from_secs(30 * 24 * 60 * 60); // 30 days
    let now = SystemTime::now();

    let cache_entries = match fs::read_dir(cache_path) {
        Ok(entries) => entries,
        Err(e) => {
            log!("Failed to read cache directory: {}", e);
            return;
        }
    };

    for entry in cache_entries.flatten() {
        let cache_file = entry.path();

        if !cache_file.is_file() {
            continue;
        }

        let filename = match cache_file.file_name().and_then(|n| n.to_str()) {
            Some(name) => name,
            None => continue,
        };

        let Some(prefix) = cache_file_prefix(filename) else {
            // Unknown filename pattern — leave it alone rather than risk
            // deleting an unrelated file someone dropped in the cache dir.
            continue;
        };

        if !valid_prefixes.contains(prefix) {
            match fs::remove_file(&cache_file) {
                Ok(_) => {
                    log!("Removed orphaned cache file: {}", filename);
                    orphaned_count += 1;
                }
                Err(e) => {
                    log!("Failed to remove cache file {}: {}", filename, e);
                    error_count += 1;
                }
            }
            continue;
        }

        if is_cache_file_old(&cache_file, max_age, now) {
            match fs::remove_file(&cache_file) {
                Ok(_) => {
                    let age_days = get_file_age_days(&cache_file, now);
                    log!("Removed old cache file ({}d old): {}", age_days, filename);
                    old_count += 1;
                }
                Err(e) => {
                    log!("Failed to remove old cache file {}: {}", filename, e);
                    error_count += 1;
                }
            }
        }
    }

    log!(
        "Cache cleanup complete: removed {} orphaned, {} old (30+ days), {} errors",
        orphaned_count,
        old_count,
        error_count
    );
}

/// Parse the source-path prefix out of a cache filename.
///
/// Cache files are named `{prefix}_w{W}_q{Q}_l{V}.webp`. The prefix encodes
/// the source path with `/` (and `\`) replaced by `_`, so it can itself
/// contain underscores and even `_w` substrings (e.g. `home_walking`). We
/// strip the deterministic right-hand `_w<digits>_q<digits>_l<digits>.webp`
/// tail instead of trying to reverse the path encoding, which is lossy.
fn cache_file_prefix(filename: &str) -> Option<&str> {
    let stripped = filename.strip_suffix(".webp")?;
    let head = strip_underscore_digit_segment(stripped, "_l")?;
    let head = strip_underscore_digit_segment(head, "_q")?;
    let prefix = strip_underscore_digit_segment(head, "_w")?;
    (!prefix.is_empty()).then_some(prefix)
}

/// Strip a trailing `<sep><digits>` segment (e.g. `_l1`, `_q80`, `_w2400`)
/// from `s` and return the remainder. None if the tail doesn't match.
fn strip_underscore_digit_segment<'a>(s: &'a str, sep: &str) -> Option<&'a str> {
    let pos = s.rfind(sep)?;
    let (head, tail) = s.split_at(pos);
    let digits = &tail[sep.len()..];
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    Some(head)
}

/// Walk `images_dir` recursively and collect the set of valid cache-file
/// prefixes — the same encoding `process_and_cache_image` produces for each
/// source file. Used by `cleanup_cache` to decide whether a cache file's
/// source still exists.
fn collect_valid_prefixes(images_dir: &str) -> std::collections::HashSet<String> {
    let mut prefixes = std::collections::HashSet::new();
    let base = std::path::Path::new(images_dir);
    collect_valid_prefixes_recursive(base, base, &mut prefixes);
    prefixes
}

fn collect_valid_prefixes_recursive(
    dir: &std::path::Path,
    base: &std::path::Path,
    out: &mut std::collections::HashSet<String>,
) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_valid_prefixes_recursive(&path, base, out);
            continue;
        }
        let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
            continue;
        };
        let ext = ext.to_ascii_lowercase();
        if !matches!(
            ext.as_str(),
            "jpg" | "jpeg" | "png" | "webp" | "gif" | "jxl" | "avif"
        ) {
            continue;
        }
        let Ok(relative) = path.strip_prefix(base) else {
            continue;
        };
        let no_ext = relative.with_extension("");
        let encoded = no_ext.to_string_lossy().replace(['/', '\\'], "_");
        if !encoded.is_empty() {
            out.insert(encoded);
        }
    }
}

/// Check if a cache file is older than the maximum age
fn is_cache_file_old(cache_file: &PathBuf, max_age: std::time::Duration, now: std::time::SystemTime) -> bool {
    if let Ok(metadata) = cache_file.metadata() {
        if let Ok(accessed) = metadata.accessed() {
            if let Ok(age) = now.duration_since(accessed) {
                return age > max_age;
            }
        }
    }
    false
}

/// Get the age of a file in days
fn get_file_age_days(cache_file: &PathBuf, now: std::time::SystemTime) -> u64 {
    cache_file
        .metadata()
        .ok()
        .and_then(|m| m.accessed().ok())
        .and_then(|accessed| now.duration_since(accessed).ok())
        .map(|age| age.as_secs() / 86400)
        .unwrap_or(0)
}

/// Pre-generate cache images for all existing photos
pub fn prewarm_cache(images_dir: &str, cache_dir: &str) {
    use crate::image_params::ImageParams;

    log!("Starting cache prewarming...");

    let valid_presets = ImageParams::get_valid_presets();
    // Only use the first (default) preset for prewarming
    let default_preset = match valid_presets.first() {
        Some(preset) => *preset,
        None => {
            log!("No valid presets configured, skipping cache prewarming");
            return;
        }
    };

    log!(
        "Prewarming cache with default preset: {}px @ quality {}",
        default_preset.0,
        default_preset.1
    );

    let mut processed_count = 0;
    let mut error_count = 0;

    // Recursively find all image files
    let image_paths = collect_image_paths(images_dir);

    log!("Found {} images to process", image_paths.len());

    for image_path in image_paths {
        // Strip extension to get base path
        let path_without_ext = if let Some(dot_pos) = image_path.rfind('.') {
            &image_path[..dot_pos]
        } else {
            &image_path
        };

        let (width, quality) = default_preset;

        // Use the shared processing function
        match process_and_cache_image(images_dir, cache_dir, path_without_ext, width, quality) {
            Some(_) => {
                processed_count += 1;
            }
            None => {
                error_count += 1;
            }
        }
    }

    log!(
        "Cache prewarming complete: processed {} images, {} errors",
        processed_count,
        error_count
    );
}

/// Recursively collect all image file paths
fn collect_image_paths(images_dir: &str) -> Vec<String> {
    let mut paths = Vec::new();
    collect_image_paths_recursive(std::path::Path::new(images_dir), images_dir, &mut paths);
    paths
}

/// Recursively collect all image file paths (helper function)
fn collect_image_paths_recursive(dir: &std::path::Path, base: &str, paths: &mut Vec<String>) {
    use std::fs;

    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_image_paths_recursive(&path, base, paths);
            } else if let Some(extension) = path.extension() {
                let ext = extension.to_string_lossy().to_lowercase();
                // Support all formats
                if matches!(
                    ext.as_ref(),
                    "jxl" | "avif" | "jpg" | "jpeg" | "webp" | "png" | "gif"
                ) {
                    if let Ok(relative) = path.strip_prefix(base) {
                        paths.push(relative.to_string_lossy().to_string());
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{Duration, SystemTime};
    use tempfile::TempDir;

    fn write_jpeg(path: &std::path::Path, bytes: &[u8]) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, bytes).unwrap();
    }

    /// 2-byte file is enough for `path.exists()` checks; we never decode it
    /// in these tests.
    const STUB: &[u8] = b"\xff\xd8";

    #[test]
    fn find_image_file_finds_existing_extension() {
        let dir = TempDir::new().unwrap();
        let images = dir.path();
        write_jpeg(&images.join("foo/bar.jpg"), STUB);

        let found = find_image_file(images.to_str().unwrap(), "foo/bar", &["jpg", "png"]);
        assert!(found.is_some());
        let found = found.unwrap();
        assert!(found.ends_with("foo/bar.jpg"));
    }

    #[test]
    fn find_image_file_returns_none_for_nonexistent() {
        let dir = TempDir::new().unwrap();
        let found = find_image_file(dir.path().to_str().unwrap(), "missing", &["jpg"]);
        assert!(found.is_none());
    }

    #[test]
    fn find_image_file_rejects_path_traversal() {
        // Set up:  <root>/inside/inside.jpg   (legitimate)
        //         <root>/outside.jpg          (must NOT be reachable)
        let dir = TempDir::new().unwrap();
        let inside = dir.path().join("inside");
        fs::create_dir_all(&inside).unwrap();
        write_jpeg(&inside.join("inside.jpg"), STUB);
        write_jpeg(&dir.path().join("outside.jpg"), STUB);

        // Sanity: legitimate path resolves.
        assert!(find_image_file(inside.to_str().unwrap(), "inside", &["jpg"]).is_some());

        // Traversal: from `<root>/inside` try to reach `../outside` — would
        // resolve to `<root>/outside.jpg` which is OUTSIDE the images_dir.
        let escape = find_image_file(inside.to_str().unwrap(), "../outside", &["jpg"]);
        assert!(escape.is_none(), "traversal escape was not blocked");
    }

    #[test]
    fn try_use_cached_image_returns_data_when_cache_newer() {
        let dir = TempDir::new().unwrap();
        let source = dir.path().join("source.jpg");
        let cache = dir.path().join("source.webp");
        fs::write(&source, b"source").unwrap();
        // Write source first, then cache — cache will have the later mtime.
        std::thread::sleep(Duration::from_millis(20));
        fs::write(&cache, b"cached").unwrap();

        let data = try_use_cached_image(&source, &cache);
        assert_eq!(data.as_deref(), Some(b"cached".as_slice()));
    }

    #[test]
    fn try_use_cached_image_misses_when_source_newer() {
        let dir = TempDir::new().unwrap();
        let source = dir.path().join("source.jpg");
        let cache = dir.path().join("source.webp");
        fs::write(&cache, b"cached").unwrap();
        std::thread::sleep(Duration::from_millis(20));
        fs::write(&source, b"source").unwrap();

        assert!(try_use_cached_image(&source, &cache).is_none());
    }

    #[test]
    fn try_use_cached_image_misses_when_cache_absent() {
        let dir = TempDir::new().unwrap();
        let source = dir.path().join("source.jpg");
        let cache = dir.path().join("source.webp");
        fs::write(&source, b"source").unwrap();
        // No cache file written.
        assert!(try_use_cached_image(&source, &cache).is_none());
    }

    #[test]
    fn save_to_cache_creates_dir_and_writes_bytes() {
        let dir = TempDir::new().unwrap();
        let cache_dir = dir.path().join("nested/cache");
        let cache_path = cache_dir.join("foo.webp");
        save_to_cache(cache_dir.to_str().unwrap(), &cache_path, b"webp-data");
        assert_eq!(fs::read(&cache_path).unwrap(), b"webp-data");
    }

    #[test]
    fn cache_file_prefix_strips_w_q_l_suffix() {
        assert_eq!(
            cache_file_prefix("home_sunset_w2400_q80_l1.webp"),
            Some("home_sunset")
        );
        assert_eq!(
            cache_file_prefix("home_sunset_w4000_q90_l1.webp"),
            Some("home_sunset")
        );
    }

    #[test]
    fn cache_file_prefix_handles_underscore_in_source_name() {
        // Source path `home/space_needle.jpg` encodes to `home_space_needle`.
        // The lossy old code split on the first `_w` and replaced every `_`
        // with `/`, mangling this. The new parser strips from the right.
        assert_eq!(
            cache_file_prefix("home_space_needle_w2400_q80_l1.webp"),
            Some("home_space_needle")
        );
    }

    #[test]
    fn cache_file_prefix_handles_w_substring_in_source_name() {
        // `home/walking.jpg` — prefix `home_walking` contains `_w`.
        assert_eq!(
            cache_file_prefix("home_walking_w800_q80_l1.webp"),
            Some("home_walking")
        );
        // And starts-with: `home/wave.jpg` — prefix begins with `wave`.
        assert_eq!(
            cache_file_prefix("home_wave_w800_q80_l1.webp"),
            Some("home_wave")
        );
    }

    #[test]
    fn cache_file_prefix_rejects_unknown_pattern() {
        assert_eq!(cache_file_prefix("notes.txt"), None);
        assert_eq!(cache_file_prefix("foo.webp"), None);
        assert_eq!(cache_file_prefix("foo_w_q80_l1.webp"), None); // empty digit group
        assert_eq!(cache_file_prefix("_w2400_q80_l1.webp"), None); // empty prefix
    }

    #[test]
    fn collect_valid_prefixes_walks_recursively() {
        use std::collections::HashSet;
        let dir = TempDir::new().unwrap();
        let images = dir.path();
        write_jpeg(&images.join("home/sunset.jpg"), STUB);
        write_jpeg(&images.join("home/space_needle.jpg"), STUB);
        write_jpeg(&images.join("travel/iceland/glacier.png"), STUB);
        // Non-image file is skipped.
        fs::write(images.join("home/notes.txt"), b"x").unwrap();

        let prefixes = collect_valid_prefixes(images.to_str().unwrap());
        let expected: HashSet<String> = [
            "home_sunset",
            "home_space_needle",
            "travel_iceland_glacier",
        ]
        .into_iter()
        .map(String::from)
        .collect();
        assert_eq!(prefixes, expected);
    }

    #[test]
    fn cleanup_cache_keeps_caches_for_underscored_source_names() {
        // Regression: the previous `_` → `/` reverse decoded
        // `home_space_needle` as `home/space/needle`, found no source, and
        // wrongly deleted the cache. The new code looks up the literal prefix
        // in the set built from a directory walk.
        let dir = TempDir::new().unwrap();
        let images = dir.path().join("images");
        let cache = dir.path().join("cache");
        fs::create_dir_all(&images).unwrap();
        fs::create_dir_all(&cache).unwrap();
        write_jpeg(&images.join("home/space_needle.jpg"), STUB);

        let kept = cache.join("home_space_needle_w2400_q80_l1.webp");
        fs::write(&kept, b"webp").unwrap();

        cleanup_cache(images.to_str().unwrap(), cache.to_str().unwrap());

        assert!(
            kept.exists(),
            "cache for an existing source with `_` in its name must survive cleanup"
        );
    }

    #[test]
    fn cleanup_cache_removes_orphan_cache_files() {
        let dir = TempDir::new().unwrap();
        let images = dir.path().join("images");
        let cache = dir.path().join("cache");
        fs::create_dir_all(&images).unwrap();
        fs::create_dir_all(&cache).unwrap();
        // No source images at all → every cache file is orphaned.
        let orphan = cache.join("home_deleted_w2400_q80_l1.webp");
        fs::write(&orphan, b"webp").unwrap();

        cleanup_cache(images.to_str().unwrap(), cache.to_str().unwrap());

        assert!(!orphan.exists());
    }

    #[test]
    fn cleanup_cache_leaves_unknown_filenames_alone() {
        // A stray file someone dropped in the cache dir mustn't be deleted
        // just because its source can't be inferred.
        let dir = TempDir::new().unwrap();
        let images = dir.path().join("images");
        let cache = dir.path().join("cache");
        fs::create_dir_all(&images).unwrap();
        fs::create_dir_all(&cache).unwrap();
        let stray = cache.join("README.txt");
        fs::write(&stray, b"hello").unwrap();

        cleanup_cache(images.to_str().unwrap(), cache.to_str().unwrap());

        assert!(stray.exists());
    }

    #[test]
    fn is_cache_file_old_compares_against_max_age() {
        let dir = TempDir::new().unwrap();
        let f = dir.path().join("x.webp");
        fs::write(&f, b"x").unwrap();
        // File was just created — 1ns "max age" should mark it old.
        assert!(is_cache_file_old(
            &f,
            Duration::from_nanos(1),
            SystemTime::now() + Duration::from_secs(1)
        ));
        // 1-day max age should not.
        assert!(!is_cache_file_old(
            &f,
            Duration::from_secs(86_400),
            SystemTime::now()
        ));
    }
}
