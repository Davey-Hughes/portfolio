use crate::types::{GalleryInfo, PhotoInfo};
use std::fs;
use std::path::Path;

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

/// Count images recursively in a directory
pub fn count_images_recursive(dir: &Path, count: &mut usize) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                count_images_recursive(&path, count);
            } else if let Some(extension) = path.extension() {
                let ext = extension.to_string_lossy().to_lowercase();
                if matches!(ext.as_ref(), "jpg" | "jpeg" | "png" | "webp" | "gif") {
                    *count += 1;
                }
            }
        }
    }
}

/// Find all images recursively in a directory for display on home page
pub fn find_images_recursive(dir: &Path, gallery_root: &Path, photos: &mut Vec<PhotoInfo>) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();

            if path.is_dir() {
                // Recursively search subdirectories
                find_images_recursive(&path, gallery_root, photos);
            } else if let Some(extension) = path.extension() {
                let ext = extension.to_string_lossy().to_lowercase();
                if matches!(ext.as_ref(), "jpg" | "jpeg" | "png" | "webp" | "gif") {
                    if let Some(filename) = path.file_name() {
                        let filename_str = filename.to_string_lossy().to_string();

                        // Calculate the relative path from gallery root
                        let relative_path = path
                            .strip_prefix(gallery_root)
                            .unwrap_or(&path)
                            .to_string_lossy()
                            .to_string();

                        // Create slug from relative path (unique identifier)
                        let slug = relative_path
                            .trim_end_matches(&format!(".{}", ext))
                            .to_lowercase()
                            .replace(['/', '\\', ' '], "-");

                        let title = filename_str
                            .trim_end_matches(&format!(".{}", ext))
                            .replace(['-', '_'], " ");

                        // Extract EXIF data
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
                        ) = extract_exif_data(&path);

                        photos.push(PhotoInfo {
                            url: format!("/images/{}", relative_path),
                            title,
                            filename: filename_str,
                            slug,
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
            }
        }
    }
}

/// Find images for a specific gallery (with different base path handling)
pub fn find_images_for_gallery(dir: &Path, base_root: &Path, photos: &mut Vec<PhotoInfo>) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();

            if path.is_dir() {
                find_images_for_gallery(&path, base_root, photos);
            } else if let Some(extension) = path.extension() {
                let ext = extension.to_string_lossy().to_lowercase();
                if matches!(ext.as_ref(), "jpg" | "jpeg" | "png" | "webp" | "gif") {
                    if let Some(filename) = path.file_name() {
                        let filename_str = filename.to_string_lossy().to_string();

                        // Calculate the relative path from base gallery root
                        let relative_path = path
                            .strip_prefix(base_root)
                            .unwrap_or(&path)
                            .to_string_lossy()
                            .to_string();

                        // Create slug from relative path (unique identifier)
                        let slug = relative_path
                            .trim_end_matches(&format!(".{}", ext))
                            .to_lowercase()
                            .replace(['/', '\\', ' '], "-");

                        let title = filename_str
                            .trim_end_matches(&format!(".{}", ext))
                            .replace(['-', '_'], " ");

                        // Extract EXIF data
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
                        ) = extract_exif_data(&path);

                        photos.push(PhotoInfo {
                            url: format!("/images/{}", relative_path),
                            title,
                            filename: filename_str,
                            slug,
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
            }
        }
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
        if let Ok(reader) = image::ImageReader::open(path) {
            if let Ok(dimensions) = reader.into_dimensions() {
                width = Some(dimensions.0);
                height = Some(dimensions.1);
            }
        }
    }

    let date_taken = exif_reader
        .get_field(exif::Tag::DateTime, exif::In::PRIMARY)
        .or_else(|| exif_reader.get_field(exif::Tag::DateTimeOriginal, exif::In::PRIMARY))
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
        .map(|f| format!("ISO {}", f.display_value().to_string()));

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
            find_images_recursive(gallery_path_buf, images_base, &mut photos);
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

    crate::types::AboutContent {
        image_url,
        content,
    }
}

fn default_about_text() -> String {
    "Hello! I'm a passionate photographer specializing in capturing the beauty of everyday moments.\n\n\
    With over 10 years of experience, I've worked on various projects ranging from landscapes to portraits.\n\n\
    My photography style focuses on natural lighting and authentic emotions. \
    I believe every photograph tells a unique story, and I'm here to help you tell yours.".to_string()
}
