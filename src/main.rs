// `#![recursion_limit]` is per-crate; the bin target needs its own bump
// because main.rs references the deeply-nested view tree types from the
// lib via `portfolio::app::*`.
#![recursion_limit = "512"]

#[cfg(feature = "ssr")]
#[tokio::main]
async fn main() {
    use axum::{
        Router,
        extract::{Path, Query},
        http::{StatusCode, header},
        response::IntoResponse,
    };
    use leptos::logging::log;
    use leptos::prelude::*;
    use leptos_axum::{LeptosRoutes, generate_route_list};
    use portfolio::app::*;
    use portfolio::image_cache::{cleanup_cache, prewarm_cache, process_and_cache_image};
    use portfolio::image_params::ImageParams;
    use std::sync::Arc;
    use tower::ServiceBuilder;
    use tower_http::{
        compression::CompressionLayer, services::ServeDir, set_header::SetResponseHeaderLayer,
    };

    /// Resolve a directory path from an env var, falling back to the first
    /// existing path in `fallbacks` (last entry is used if none exist).
    fn resolve_dir(env_var: &str, fallbacks: &[&str]) -> String {
        if let Ok(val) = std::env::var(env_var) {
            return val;
        }
        for candidate in fallbacks {
            if std::path::Path::new(candidate).exists() {
                return (*candidate).to_string();
            }
        }
        fallbacks.last().copied().unwrap_or(".").to_string()
    }

    // Load `.env` (used locally to set LEPTOS_HASH_FILES=false so dev builds skip
    // content-hashed pkg filenames). Never present in the Docker image
    // (.dockerignore), so production keeps hashing on.
    dotenvy::dotenv().ok();

    let conf = get_configuration(None).unwrap();
    let addr = conf.leptos_options.site_addr;
    let leptos_options = conf.leptos_options;
    let routes = generate_route_list(App);

    let images_dir = resolve_dir("IMAGES_DIR", &["public/images", "./images"]);
    let content_dir = resolve_dir("ABOUT_CONTENT_PATH", &["public/content", "./content"]);
    let cache_dir = resolve_dir("IMAGE_CACHE_DIR", &["public/cache", "./cache"]);

    log!("Serving images from: {}", images_dir);
    log!("Serving content from: {}", content_dir);

    // Closure captures the Arc'd dirs so the handler doesn't re-resolve them
    // on every request. Image processing is CPU/IO heavy; offload to the
    // blocking pool so it doesn't tie up tokio worker threads.
    let images_dir_arc: Arc<str> = Arc::from(images_dir.as_str());
    let cache_dir_arc: Arc<str> = Arc::from(cache_dir.as_str());

    let serve_compressed_image = {
        let images_dir = Arc::clone(&images_dir_arc);
        let cache_dir = Arc::clone(&cache_dir_arc);
        move |Path(image_path): Path<String>, Query(params): Query<ImageParams>| {
            let images_dir = Arc::clone(&images_dir);
            let cache_dir = Arc::clone(&cache_dir);
            async move {
                let (width, quality) = match params.validate() {
                    Ok(values) => values,
                    Err(err_msg) => {
                        return (StatusCode::BAD_REQUEST, err_msg).into_response();
                    }
                };

                let path_without_ext = match image_path.rfind('.') {
                    Some(dot) => image_path[..dot].to_string(),
                    None => image_path.clone(),
                };

                let result = tokio::task::spawn_blocking(move || {
                    process_and_cache_image(
                        &images_dir,
                        &cache_dir,
                        &path_without_ext,
                        width,
                        quality,
                    )
                })
                .await;

                match result {
                    Ok(Some(webp_data)) => {
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
                    Ok(None) => {
                        log!("Failed to process image: {}", image_path);
                        (
                            StatusCode::NOT_FOUND,
                            "Image not found or failed to process",
                        )
                            .into_response()
                    }
                    Err(err) => {
                        log!("Image worker panicked: {err}");
                        (StatusCode::INTERNAL_SERVER_ERROR, "Image processing failed")
                            .into_response()
                    }
                }
            }
        }
    };

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

    // Periodic in-memory cache sweep (mosaic + gallery TTLs). Off the request path.
    portfolio::server::spawn_cache_sweeper();

    // Watch the images directory so newly added/moved/removed photos show up
    // immediately instead of waiting for the TTL to expire. Also prunes
    // matching entries from the on-disk compressed-image cache.
    portfolio::server::spawn_image_watcher(images_dir.clone(), cache_dir.clone());

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
