#[cfg(feature = "ssr")]
#[tokio::main]
async fn main() {
    use axum::{
        extract::{Path, Query},
        http::{header, StatusCode},
        response::{IntoResponse, Response},
        Router,
    };
    use leptos::logging::log;
    use leptos::prelude::*;
    use leptos_axum::{generate_route_list, LeptosRoutes};
    use portfolio::app::*;
    use serde::Deserialize;
    use std::path::PathBuf;
    use tower_http::services::ServeDir;

    #[derive(Deserialize)]
    struct ImageParams {
        #[serde(default)]
        quality: Option<u8>,
        #[serde(default)]
        width: Option<u32>,
    }

    async fn serve_compressed_image(
        Path(image_path): Path<String>,
        Query(params): Query<ImageParams>,
    ) -> Response {
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

        // Generate cache key based on path and parameters
        let width_suffix = params.width.map(|w| format!("_w{}", w)).unwrap_or_default();
        let quality_suffix = params
            .quality
            .map(|q| format!("_q{}", q))
            .unwrap_or_default();

        // Create cache filename: original_name_w1200_q80.webp
        let cache_filename = format!(
            "{}{}{}_{}.webp",
            image_path.replace(['/', '\\'], "_"),
            width_suffix,
            quality_suffix,
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
                return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to load image").into_response();
            }
        };

        // Resize if width is specified
        let img = if let Some(max_width) = params.width {
            if img.width() > max_width {
                img.resize(max_width, u32::MAX, image::imageops::FilterType::Lanczos3)
            } else {
                img
            }
        } else {
            img
        };

        // Convert to WebP with compression
        let mut webp_data = Vec::new();
        let _quality = params.quality.unwrap_or(80).clamp(1, 100);

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
