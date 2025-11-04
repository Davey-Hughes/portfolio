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

    // Cache filename without timestamp - makes cache persistent
    let cache_filename = format!(
        "{}_w{}_q{}.webp",
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
    let webp_data = process_image(&full_path, width)?;

    // Save to cache
    save_to_cache(cache_dir, &cache_path, &webp_data);

    Some(webp_data)
}

/// Find an image file with any of the supported extensions
fn find_image_file(
    images_dir: &str,
    path_without_ext: &str,
    extensions: &[&str],
) -> Option<PathBuf> {
    extensions
        .iter()
        .map(|ext| {
            std::path::PathBuf::from(images_dir).join(format!("{}.{}", path_without_ext, ext))
        })
        .find(|path| path.exists())
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
fn process_image(full_path: &PathBuf, width: u32) -> Option<Vec<u8>> {
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
    convert_to_webp(&img)
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

/// Convert an image to WebP format
fn convert_to_webp(img: &image::DynamicImage) -> Option<Vec<u8>> {
    let mut webp_data = Vec::new();
    img.write_to(
        &mut std::io::Cursor::new(&mut webp_data),
        image::ImageFormat::WebP,
    )
    .ok()?;
    Some(webp_data)
}

/// Save processed image data to cache
fn save_to_cache(cache_dir: &str, cache_path: &PathBuf, data: &[u8]) {
    use std::fs;
    use std::io::Write;

    if fs::create_dir_all(cache_dir).is_ok() {
        if let Ok(mut file) = fs::File::create(cache_path) {
            let _ = file.write_all(data);
        }
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

    let mut orphaned_count = 0;
    let mut old_count = 0;
    let mut error_count = 0;

    // Cache files not accessed in 30 days will be removed
    let max_age = Duration::from_secs(30 * 24 * 60 * 60); // 30 days
    let now = SystemTime::now();

    // Read all cache files
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

        // Parse cache filename and check if source exists
        if let Some(first_part) = filename.split("_w").next() {
            let original_path = first_part.replace('_', "/");

            if !source_image_exists(images_dir, &original_path) {
                // Source image doesn't exist, remove cached file
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

            // Check if cache file is too old (based on last access time)
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
    }

    log!(
        "Cache cleanup complete: removed {} orphaned, {} old (30+ days), {} errors",
        orphaned_count,
        old_count,
        error_count
    );
}

/// Check if a source image exists with any supported extension
fn source_image_exists(images_dir: &str, original_path: &str) -> bool {
    let extensions = ["jxl", "avif", "jpg", "jpeg", "webp", "png", "gif"];

    for ext in &extensions {
        let source_path = PathBuf::from(images_dir).join(format!("{}.{}", original_path, ext));
        if source_path.exists() {
            return true;
        }
    }

    false
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
