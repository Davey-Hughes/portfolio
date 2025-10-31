#[cfg(feature = "ssr")]
#[tokio::main]
async fn main() {
    use axum::{
        extract::{Path, Query},
        http::{header, StatusCode},
        response::{IntoResponse, Response},
        Router,
    };
    use jxl_oxide::integration::JxlDecoder;
    use leptos::logging::log;
    use leptos::prelude::*;
    use leptos_axum::{generate_route_list, LeptosRoutes};
    use portfolio::app::*;
    use portfolio::image_params::ImageParams;
    use std::path::PathBuf;
    use tower_http::services::ServeDir;

    /// Clean up orphaned cache files that no longer have corresponding source images
    fn cleanup_cache(images_dir: &str, cache_dir: &str) {
        use std::fs;

        log!("Starting cache cleanup...");

        let cache_path = std::path::Path::new(cache_dir);
        if !cache_path.exists() {
            log!("Cache directory does not exist, skipping cleanup");
            return;
        }

        let mut removed_count = 0;
        let mut error_count = 0;

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

            // Parse cache filename format: "{path}_w{width}_q{quality}_{timestamp}.webp"
            // Extract the original image path
            if let Some(first_part) = filename.split("_w").next() {
                // Convert underscores back to path separators
                let original_path = first_part.replace('_', "/");

                // Check if source image exists (try different extensions)
                let extensions = ["jpg", "jpeg", "png", "webp", "gif", "jxl", "avif"];
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
                            removed_count += 1;
                        }
                        Err(e) => {
                            log!("Failed to remove cache file {}: {}", filename, e);
                            error_count += 1;
                        }
                    }
                }
            }
        }

        log!(
            "Cache cleanup complete: removed {} orphaned files, {} errors",
            removed_count,
            error_count
        );
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
        use std::fs;
        use std::io::Write;

        let images_dir = std::env::var("IMAGES_DIR").unwrap_or_else(|_| {
            if std::path::Path::new("public/images").exists() {
                "public/images".to_string()
            } else {
                "./images".to_string()
            }
        });

        // Cache directory
        let cache_dir = std::env::var("IMAGE_CACHE_DIR").unwrap_or_else(|_| {
            if std::path::Path::new("public/cache").exists() {
                "public/cache".to_string()
            } else {
                "./cache".to_string()
            }
        });

        let full_path = PathBuf::from(&images_dir).join(&image_path);

        if !full_path.exists() {
            return (StatusCode::NOT_FOUND, "Image not found").into_response();
        }

        // Strip extension from image_path for cache key (so different formats map to same cache)
        let path_without_ext = if let Some(dot_pos) = image_path.rfind('.') {
            &image_path[..dot_pos]
        } else {
            &image_path
        };

        // Generate cache key based on validated parameters (without file extension)
        let cache_filename = format!(
            "{}_w{}_q{}_{}.webp",
            path_without_ext.replace(['/', '\\'], "_"),
            width,
            quality,
            full_path
                .metadata()
                .ok()
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0)
        );

        let cache_path = PathBuf::from(&cache_dir).join(&cache_filename);

        // Check if cached version exists
        if cache_path.exists() {
            if let Ok(cached_data) = fs::read(&cache_path) {
                log!("Serving cached image: {}", cache_filename);
                return (
                    StatusCode::OK,
                    [(header::CONTENT_TYPE, "image/webp")],
                    cached_data,
                )
                    .into_response();
            }
        }

        log!("Processing and caching image: {}", image_path);

        let img = if full_path.extension().unwrap() == "jxl" {
            // Read and decode a JPEG XL image.

            use image::DynamicImage;
            let file = match std::fs::File::open(&full_path) {
                Ok(file) => file,
                Err(e) => {
                    log!("Failed to open image: {}, err: {}", full_path.display(), e);
                    return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to load image")
                        .into_response();
                }
            };
            let decoder = match JxlDecoder::new(file) {
                Ok(decoder) => decoder,
                Err(e) => {
                    log!(
                        "Failed to init decoder: {}, err: {}",
                        full_path.display(),
                        e
                    );
                    return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to init decoder")
                        .into_response();
                }
            };
            let img = match DynamicImage::from_decoder(decoder) {
                Ok(image) => image,
                Err(e) => {
                    log!(
                        "Failed to decode image: {}, err: {}",
                        full_path.display(),
                        e
                    );
                    return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to load image")
                        .into_response();
                }
            };
            img
        } else {
            // Load the original image with no limits
            let img = match image::ImageReader::open(&full_path) {
                Ok(mut reader) => {
                    reader.limits(image::Limits::no_limits());

                    match reader.decode() {
                        Ok(img) => img,
                        Err(e) => {
                            log!(
                                "Failed to decode image: {}, err: {}",
                                full_path.display(),
                                e
                            );
                            return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to load image")
                                .into_response();
                        }
                    }
                }
                Err(e) => {
                    log!("Failed to open image: {}, err: {}", full_path.display(), e);
                    return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to load image")
                        .into_response();
                }
            };

            img
        };

        // Resize to validated width
        let img = if img.width() > width {
            log!("Resizing image to {}px", width);
            img.resize(width, u32::MAX, image::imageops::FilterType::Lanczos3)
        } else {
            img
        };

        // Convert to WebP with validated quality
        let mut webp_data = Vec::new();
        let _quality = quality;

        // Note: The image crate's WebP encoder uses default quality settings
        // For more control, a dedicated WebP encoder library would be needed
        if img
            .write_to(
                &mut std::io::Cursor::new(&mut webp_data),
                image::ImageFormat::WebP,
            )
            .is_err()
        {
            return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to encode image").into_response();
        }

        // Save to cache
        if let Err(e) = fs::create_dir_all(&cache_dir) {
            log!("Warning: Failed to create cache directory: {}", e);
        } else if let Ok(mut file) = fs::File::create(&cache_path) {
            if let Err(e) = file.write_all(&webp_data) {
                log!("Warning: Failed to write cache file: {}", e);
            } else {
                log!("Cached image saved: {}", cache_filename);
            }
        }

        (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "image/webp")],
            webp_data,
        )
            .into_response()
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
        .nest_service("/images", ServeDir::new(&images_dir))
        .nest_service("/content", ServeDir::new(&content_dir))
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
