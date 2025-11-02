#[cfg(feature = "ssr")]
#[tokio::main]
async fn main() {
    use axum::{
        Router,
        extract::{Path, Query},
        http::{StatusCode, header},
        response::{IntoResponse, Response},
    };
    use leptos::logging::log;
    use leptos::prelude::*;
    use leptos_axum::{LeptosRoutes, generate_route_list};
    use portfolio::app::*;
    use portfolio::image_params::ImageParams;
    use std::path::PathBuf;
    use tower::ServiceBuilder;
    use tower_http::{services::ServeDir, set_header::SetResponseHeaderLayer};

    /// Process and cache a single image with the given parameters
    /// Returns the WebP data or None if processing failed
    fn process_and_cache_image(
        images_dir: &str,
        cache_dir: &str,
        path_without_ext: &str,
        width: u32,
        quality: u8,
    ) -> Option<Vec<u8>> {
        use std::fs;
        use std::io::Write;

        let supported_extensions = ["jpg", "jpeg", "jxl", "avif", "webp", "png", "gif"];

        // Find the actual file with any supported extension
        let full_path = supported_extensions
            .iter()
            .map(|ext| {
                std::path::PathBuf::from(images_dir).join(format!("{}.{}", path_without_ext, ext))
            })
            .find(|path| path.exists())?;

        // Cache filename without timestamp - makes cache persistent
        let cache_filename = format!(
            "{}_w{}_q{}.webp",
            path_without_ext.replace(['/', '\\'], "_"),
            width,
            quality
        );

        let cache_path = std::path::PathBuf::from(cache_dir).join(&cache_filename);

        // Check if cached version exists and is newer than source
        if cache_path.exists() {
            // Compare modification times: only regenerate if source is newer than cache
            let source_mtime = full_path.metadata().ok().and_then(|m| m.modified().ok());

            let cache_mtime = cache_path.metadata().ok().and_then(|m| m.modified().ok());

            if let (Some(source_time), Some(cache_time)) = (source_mtime, cache_mtime) {
                // If cache is newer than or equal to source, use it
                if cache_time >= source_time {
                    return fs::read(&cache_path).ok();
                }
                // Otherwise, cache is stale and will be regenerated below
            } else {
                // If we can't get timestamps, just use the cache if it exists
                return fs::read(&cache_path).ok();
            }
        }

        // Load and process image
        let img_result = if full_path.extension().and_then(|e| e.to_str()) == Some("jxl") {
            use image::DynamicImage;
            use jxl_oxide::integration::JxlDecoder;

            std::fs::File::open(&full_path)
                .ok()
                .and_then(|file| JxlDecoder::new(file).ok())
                .and_then(|decoder| DynamicImage::from_decoder(decoder).ok())
        } else {
            image::ImageReader::open(&full_path)
                .ok()
                .map(|mut reader| {
                    reader.limits(image::Limits::no_limits());
                    reader.decode()
                })
                .and_then(|r| r.ok())
        };

        let img = img_result?;

        // Resize if needed
        let img = if img.width() > width {
            img.resize(width, u32::MAX, image::imageops::FilterType::Lanczos3)
        } else {
            img
        };

        // Convert to WebP
        let mut webp_data = Vec::new();
        img.write_to(
            &mut std::io::Cursor::new(&mut webp_data),
            image::ImageFormat::WebP,
        )
        .ok()?;

        // Save to cache
        if fs::create_dir_all(cache_dir).is_ok() {
            if let Ok(mut file) = fs::File::create(&cache_path) {
                let _ = file.write_all(&webp_data);
            }
        }

        Some(webp_data)
    }

    /// Clean up orphaned cache files that no longer have corresponding source images
    /// Also removes old cache files that haven't been accessed in a while
    fn cleanup_cache(images_dir: &str, cache_dir: &str) {
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

            // Parse cache filename format: "{path}_w{width}_q{quality}.webp" (new format)
            // or "{path}_w{width}_q{quality}_{timestamp}.webp" (old format with timestamp)
            // Extract the original image path
            if let Some(first_part) = filename.split("_w").next() {
                // Convert underscores back to path separators
                let original_path = first_part.replace('_', "/");

                // Check if source image exists (try different extensions)
                let extensions = ["jxl", "avif", "jpg", "jpeg", "webp", "png", "gif"];
                let mut source_exists = false;

                for ext in &extensions {
                    let source_path =
                        PathBuf::from(images_dir).join(format!("{}.{}", original_path, ext));

                    if source_path.exists() {
                        source_exists = true;
                        break;
                    }
                }

                if !source_exists {
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
                if let Ok(metadata) = cache_file.metadata() {
                    if let Ok(accessed) = metadata.accessed() {
                        if let Ok(age) = now.duration_since(accessed) {
                            if age > max_age {
                                // Cache file hasn't been accessed in max_age days, remove it
                                match fs::remove_file(&cache_file) {
                                    Ok(_) => {
                                        log!(
                                            "Removed old cache file ({}d old): {}",
                                            age.as_secs() / 86400,
                                            filename
                                        );
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

    /// Pre-generate cache images for all existing photos
    fn prewarm_cache(images_dir: &str, cache_dir: &str) {
        use portfolio::image_params::ImageParams;

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
        let mut image_paths = Vec::new();
        collect_image_paths(
            std::path::Path::new(images_dir),
            images_dir,
            &mut image_paths,
        );

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
    fn collect_image_paths(dir: &std::path::Path, base: &str, paths: &mut Vec<String>) {
        use std::fs;

        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    collect_image_paths(&path, base, paths);
                } else if let Some(extension) = path.extension() {
                    let ext = extension.to_string_lossy().to_lowercase();
                    // Support all formats: jxl, avif, jpg, jpeg, webp, png, gif
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

    async fn serve_compressed_image(
        Path(image_path): Path<String>,
        Query(params): Query<ImageParams>,
    ) -> Response {
        // Validate parameters
        let (width, quality) = match params.validate() {
            Ok(values) => values,
            Err(err_msg) => {
                return (StatusCode::BAD_REQUEST, err_msg).into_response();
            }
        };

        let images_dir = std::env::var("IMAGES_DIR").unwrap_or_else(|_| {
            if std::path::Path::new("public/images").exists() {
                "public/images".to_string()
            } else {
                "./images".to_string()
            }
        });

        let cache_dir = std::env::var("IMAGE_CACHE_DIR").unwrap_or_else(|_| {
            if std::path::Path::new("public/cache").exists() {
                "public/cache".to_string()
            } else {
                "./cache".to_string()
            }
        });

        // Strip extension from image_path to find any matching source file
        let path_without_ext = if let Some(dot_pos) = image_path.rfind('.') {
            &image_path[..dot_pos]
        } else {
            &image_path
        };

        // Use the shared processing function
        match process_and_cache_image(&images_dir, &cache_dir, path_without_ext, width, quality) {
            Some(webp_data) => {
                log!("Serving image: {}", image_path);
                (
                    StatusCode::OK,
                    [
                        (header::CONTENT_TYPE, "image/webp"),
                        (header::CACHE_CONTROL, "public, max-age=31536000, immutable"),
                    ],
                    webp_data,
                )
                    .into_response()
            }
            None => {
                log!("Failed to process image: {}", image_path);
                (
                    StatusCode::NOT_FOUND,
                    "Image not found or failed to process",
                )
                    .into_response()
            }
        }
    }

    let conf = get_configuration(None).unwrap();
    let addr = conf.leptos_options.site_addr;
    let leptos_options = conf.leptos_options;
    // Generate the list of routes in your Leptos App
    let routes = generate_route_list(App);

    // Get images directory path from environment or use default
    let images_dir = std::env::var("IMAGES_DIR").unwrap_or_else(|_| {
        if std::path::Path::new("public/images").exists() {
            "public/images".to_string()
        } else {
            "./images".to_string()
        }
    });

    // Get content directory path from environment or use default
    let content_dir = std::env::var("ABOUT_CONTENT_PATH").unwrap_or_else(|_| {
        if std::path::Path::new("public/content").exists() {
            "public/content".to_string()
        } else {
            "./content".to_string()
        }
    });

    log!("Serving images from: {}", images_dir);
    log!("Serving content from: {}", content_dir);

    // Get cache directory path
    let cache_dir = std::env::var("IMAGE_CACHE_DIR").unwrap_or_else(|_| {
        if std::path::Path::new("public/cache").exists() {
            "public/cache".to_string()
        } else {
            "./cache".to_string()
        }
    });

    // Clean up orphaned cache files on startup
    cleanup_cache(&images_dir, &cache_dir);

    // Pre-generate cache for default size only (async, non-blocking)
    let images_dir_clone = images_dir.clone();
    let cache_dir_clone = cache_dir.clone();
    tokio::spawn(async move {
        prewarm_cache(&images_dir_clone, &cache_dir_clone);
    });

    let app = Router::new()
        .leptos_routes(&leptos_options, routes, {
            let leptos_options = leptos_options.clone();
            move || shell(leptos_options.clone())
        })
        // Compressed image endpoint
        .route(
            "/images/compressed/{*image_path}",
            axum::routing::get(serve_compressed_image),
        )
        .nest_service(
            "/images",
            ServiceBuilder::new()
                .layer(SetResponseHeaderLayer::if_not_present(
                    header::CACHE_CONTROL,
                    header::HeaderValue::from_static("public, max-age=31536000, immutable"),
                ))
                .service(ServeDir::new(&images_dir)),
        )
        .nest_service(
            "/content",
            ServiceBuilder::new()
                .layer(SetResponseHeaderLayer::if_not_present(
                    header::CACHE_CONTROL,
                    header::HeaderValue::from_static("public, max-age=31536000, immutable"),
                ))
                .service(ServeDir::new(&content_dir)),
        )
        .fallback(leptos_axum::file_and_error_handler(shell))
        .with_state(leptos_options);

    // run our app with hyper
    // `axum::Server` is a re-export of `hyper::Server`
    log!("listening on http://{}", &addr);
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app.into_make_service())
        .await
        .unwrap();
}

#[cfg(not(feature = "ssr"))]
pub fn main() {
    // no client-side main function
    // unless we want this to work with e.g., Trunk for pure client-side testing
    // see lib.rs for hydration function instead
}
