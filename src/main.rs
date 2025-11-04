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
    use portfolio::image_cache::{cleanup_cache, prewarm_cache, process_and_cache_image};
    use portfolio::image_params::ImageParams;
    use tower::ServiceBuilder;
    use tower_http::{
        compression::CompressionLayer, services::ServeDir, set_header::SetResponseHeaderLayer,
    };

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

    // Clean up orphaned cache files and pre-generate cache in background (async, non-blocking)
    let images_dir_clone = images_dir.clone();
    let cache_dir_clone = cache_dir.clone();
    tokio::spawn(async move {
        // Run cleanup first, then prewarming
        cleanup_cache(&images_dir_clone, &cache_dir_clone);
        prewarm_cache(&images_dir_clone, &cache_dir_clone);

        // Pre-warm the all-photos cache for faster photo detail page loads
        portfolio::server::prewarm_all_photos_cache();
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
        .with_state(leptos_options)
        .layer(CompressionLayer::new());

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
