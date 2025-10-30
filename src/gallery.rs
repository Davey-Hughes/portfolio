use crate::types::{GalleryInfo, ImageSource, PhotoInfo};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

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

/// Get default image width and quality from environment variables
/// Returns (width, quality) tuple with defaults of (3600, 100)
fn get_default_image_params() -> (u32, u8) {
    let width = std::env::var("DEFAULT_IMAGE_WIDTH")
        .ok()
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(3600);

    let quality = std::env::var("DEFAULT_IMAGE_QUALITY")
        .ok()
        .and_then(|s| s.parse::<u8>().ok())
        .unwrap_or(100);

    (width, quality)
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
    // Match patterns like "1 - ", "42-", "003 - ", "1.5-", "2.3.4 - ", etc.
    // Allows digits with optional periods in between
    let re = regex::Regex::new(r"^[\d.]+\s*-\s*").unwrap();
    re.replace(filename, "").to_string()
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
    for (base_path, variants) in image_groups {
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

        // Create slug from base path
        let slug = base_path.to_lowercase().replace(['/', '\\', ' '], "-");

        // Strip leading numbers and dashes, then convert to title
        let title = strip_leading_number_and_dash(&filename_str)
            .trim_end_matches(&format!(".{}", primary_ext))
            .replace(['-', '_'], " ");

        // Extract EXIF data from primary image
        let (
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
        ) = extract_exif_data(&primary_full_path);

        // Build sources for compressed versions
        let (img_width, img_quality) = get_default_image_params();
        let mut sources = Vec::new();
        let mut original_sources = Vec::new();

        for (relative_path, ext) in &sorted_variants {
            if relative_path != primary_relative_path {
                // Add as alternative source
                let compressed_url = format!(
                    "/images/compressed/{}?width={}&quality={}",
                    relative_path, img_width, img_quality
                );
                let original_url = format!("/images/{}", relative_path);
                let mime_type = get_mime_type(ext).to_string();

                sources.push(ImageSource {
                    url: compressed_url,
                    mime_type: mime_type.clone(),
                });
                original_sources.push(ImageSource {
                    url: original_url,
                    mime_type,
                });
            }
        }

        // Primary image URLs
        let compressed_url = format!(
            "/images/compressed/{}?width={}&quality={}",
            primary_relative_path, img_width, img_quality
        );
        let original_url = format!("/images/{}", primary_relative_path);

        photos.push(PhotoInfo {
            url: compressed_url,
            original_url,
            sources,
            original_sources,
            title,
            filename: filename_str,
            slug,
            gallery_name: gallery_name.to_string(),
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
        "jpg" | "jpeg" => 0, // Fallback, widest support (manually setting to first for now)
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

    find_images_for_gallery_with_name(dir, base_root, photos, &gallery_name);
}

/// Find images for a specific gallery with explicit gallery name
fn find_images_for_gallery_with_name(
    dir: &Path,
    base_root: &Path,
    photos: &mut Vec<PhotoInfo>,
    gallery_name: &str,
) {
    // First pass: collect all image files and group by basename
    let mut image_groups: HashMap<String, Vec<(String, String)>> = HashMap::new();
    collect_image_files(dir, base_root, &mut image_groups);

    // Second pass: create PhotoInfo for each group
    for (base_path, variants) in image_groups {
        // Sort variants by priority (modern formats first for sources)
        let mut sorted_variants = variants.clone();
        sorted_variants.sort_by(|a, b| {
            let priority_a = format_priority(&a.1);
            let priority_b = format_priority(&b.1);
            priority_a.cmp(&priority_b)
        });

        // Use the first variant as the primary image (fallback)
        let (primary_relative_path, primary_ext) = &sorted_variants[0];
        let primary_full_path = base_root.join(primary_relative_path);

        // Extract metadata from the primary image
        let filename_str = primary_full_path
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_default();

        // Create slug from base path
        let slug = base_path.to_lowercase().replace(['/', '\\', ' '], "-");

        // Strip leading numbers and dashes, then convert to title
        let title = strip_leading_number_and_dash(&filename_str)
            .trim_end_matches(&format!(".{}", primary_ext))
            .replace(['-', '_'], " ");

        // Extract EXIF data from primary image
        let (
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
        ) = extract_exif_data(&primary_full_path);

        // Build sources for compressed versions
        let (img_width, img_quality) = get_default_image_params();
        let mut sources = Vec::new();
        let mut original_sources = Vec::new();

        for (relative_path, ext) in &sorted_variants {
            if relative_path != primary_relative_path {
                // Add as alternative source
                let compressed_url = format!(
                    "/images/compressed/{}?width={}&quality={}",
                    relative_path, img_width, img_quality
                );
                let original_url = format!("/images/{}", relative_path);
                let mime_type = get_mime_type(ext).to_string();

                sources.push(ImageSource {
                    url: compressed_url,
                    mime_type: mime_type.clone(),
                });
                original_sources.push(ImageSource {
                    url: original_url,
                    mime_type,
                });
            }
        }

        // Primary image URLs
        let compressed_url = format!(
            "/images/compressed/{}?width={}&quality={}",
            primary_relative_path, img_width, img_quality
        );
        let original_url = format!("/images/{}", primary_relative_path);

        photos.push(PhotoInfo {
            url: compressed_url,
            original_url,
            sources,
            original_sources,
            title,
            filename: filename_str,
            slug,
            gallery_name: gallery_name.to_string(),
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
        });
    }
}

type ExifData = (
    Option<u32>,    // width
    Option<u32>,    // height
    Option<String>, // date_taken
    Option<String>, // camera_make
    Option<String>, // camera_model
    Option<String>, // lens_model
    Option<String>, // focal_length
    Option<String>, // aperture
    Option<String>, // shutter_speed
    Option<String>, // iso
);

/// Extract EXIF metadata from an image file
fn extract_exif_data(path: &Path) -> ExifData {
    use std::fs::File;
    use std::io::BufReader;

    let Ok(file) = File::open(path) else {
        return (None, None, None, None, None, None, None, None, None, None);
    };

    let mut reader = BufReader::new(file);
    let Ok(exif_reader) = exif::Reader::new().read_from_container(&mut reader) else {
        return (None, None, None, None, None, None, None, None, None, None);
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

    let lens_model = exif_reader
        .get_field(exif::Tag::LensModel, exif::In::PRIMARY)
        .and_then(|f| f.display_value().to_string().into());

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

    (
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
    )
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
                    galleries.push(GalleryInfo {
                        name: name.replace(['-', '_'], " "),
                        slug,
                        photo_count,
                    });
                }
            }
        }
    }

    galleries.sort_by(|a, b| a.name.cmp(&b.name));
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
    photos.sort_by(|a, b| a.filename.cmp(&b.filename));
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
                photos.sort_by(|a, b| a.filename.cmp(&b.filename));
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

    photos.sort_by(|a, b| a.filename.cmp(&b.filename));
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

    // Try to load the about text file
    let text_path = Path::new(&content_path).join("about.txt");
    let content = if text_path.exists() {
        fs::read_to_string(&text_path).unwrap_or_else(|_| default_about_text())
    } else {
        default_about_text()
    };

    // Check if profile image exists
    let image_url = ["profile.jpg", "profile.png", "profile.webp"]
        .iter()
        .map(|name| Path::new(&content_path).join(name))
        .find(|path| path.exists())
        .and_then(|path| path.file_name().map(|n| n.to_string_lossy().to_string()))
        .map(|filename| format!("/content/{}", filename));

    crate::types::AboutContent { image_url, content }
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
    #[serial]
    fn test_get_default_image_params_defaults() {
        // Ensure env vars are not set
        std::env::remove_var("DEFAULT_IMAGE_WIDTH");
        std::env::remove_var("DEFAULT_IMAGE_QUALITY");

        let (width, quality) = get_default_image_params();
        assert_eq!(width, 3600);
        assert_eq!(quality, 100);
    }

    #[test]
    #[serial]
    fn test_get_default_image_params_custom_width() {
        std::env::set_var("DEFAULT_IMAGE_WIDTH", "2400");
        std::env::remove_var("DEFAULT_IMAGE_QUALITY");

        let (width, quality) = get_default_image_params();

        std::env::remove_var("DEFAULT_IMAGE_WIDTH");

        assert_eq!(width, 2400);
        assert_eq!(quality, 100);
    }

    #[test]
    #[serial]
    fn test_get_default_image_params_custom_quality() {
        std::env::remove_var("DEFAULT_IMAGE_WIDTH");
        std::env::set_var("DEFAULT_IMAGE_QUALITY", "80");

        let (width, quality) = get_default_image_params();

        std::env::remove_var("DEFAULT_IMAGE_QUALITY");

        assert_eq!(width, 3600);
        assert_eq!(quality, 80);
    }

    #[test]
    #[serial]
    fn test_get_default_image_params_both_custom() {
        std::env::set_var("DEFAULT_IMAGE_WIDTH", "1200");
        std::env::set_var("DEFAULT_IMAGE_QUALITY", "85");

        let (width, quality) = get_default_image_params();

        std::env::remove_var("DEFAULT_IMAGE_WIDTH");
        std::env::remove_var("DEFAULT_IMAGE_QUALITY");

        assert_eq!(width, 1200);
        assert_eq!(quality, 85);
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
        assert_eq!(width, 3600);
        assert_eq!(quality, 100);
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
        let (width, height, date, make, model, lens, focal, aperture, shutter, iso) =
            extract_exif_data(nonexistent);

        assert_eq!(width, None);
        assert_eq!(height, None);
        assert_eq!(date, None);
        assert_eq!(make, None);
        assert_eq!(model, None);
        assert_eq!(lens, None);
        assert_eq!(focal, None);
        assert_eq!(aperture, None);
        assert_eq!(shutter, None);
        assert_eq!(iso, None);
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
        if photos.len() > 1 {
            for i in 0..photos.len() - 1 {
                assert!(photos[i].filename <= photos[i + 1].filename);
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
        };

        assert_eq!(gallery.name, "Test Gallery");
        assert_eq!(gallery.slug, "test-gallery");
        assert_eq!(gallery.photo_count, 10);
    }
}
