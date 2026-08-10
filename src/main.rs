// `#![recursion_limit]` is per-crate; the bin target needs its own bump
// because main.rs references the deeply-nested view tree types from the
// lib via `portfolio::app::*`.
#![recursion_limit = "512"]

#[cfg(feature = "ssr")]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[cfg(feature = "ssr")]
mod cache_middleware {
    use axum::{
        extract::{Request, State},
        middleware::Next,
        response::{IntoResponse, Response},
    };
    use moka::future::Cache;
    use std::sync::Arc;

    pub type HtmlCache = Arc<Cache<String, (axum::http::HeaderMap, axum::body::Bytes)>>;

    pub async fn html_cache_middleware(
        State(cache): State<HtmlCache>,
        req: Request,
        next: Next,
    ) -> Response {
        if req.method() != axum::http::Method::GET || req.uri().path().starts_with("/api") {
            return next.run(req).await;
        }

        let cookie_str = req
            .headers()
            .get(axum::http::header::COOKIE)
            .and_then(|h| h.to_str().ok())
            .unwrap_or("");
        let key = format!("{}|{}", req.uri(), cookie_str);

        if let Some((headers, body_bytes)) = cache.get(&key).await {
            let mut res = body_bytes.into_response();
            *res.headers_mut() = headers;
            return res;
        }

        let res = next.run(req).await;

        if res.status().is_success() {
            let is_html = res
                .headers()
                .get(axum::http::header::CONTENT_TYPE)
                .is_some_and(|ct| ct.as_bytes().starts_with(b"text/html"));

            if is_html {
                let (parts, body) = res.into_parts();
                match axum::body::to_bytes(body, usize::MAX).await {
                    Ok(bytes) => {
                        // Never cache Set-Cookie: stored headers are replayed
                        // to every client sharing this key, so a cached cookie
                        // would leak across users (session fixation). The
                        // originating response below keeps its own headers.
                        let mut cached_headers = parts.headers.clone();
                        cached_headers.remove(axum::http::header::SET_COOKIE);
                        cache.insert(key, (cached_headers, bytes.clone())).await;
                        let mut cached_res = bytes.into_response();
                        *cached_res.headers_mut() = parts.headers;
                        return cached_res;
                    }
                    Err(_) => {
                        return axum::response::Response::from_parts(
                            parts,
                            axum::body::Body::empty(),
                        );
                    }
                }
            }
        }

        res
    }
}

/// Resolve a directory path from an env var, falling back to the first
/// existing path in `fallbacks` (last entry is used if none exist).
#[cfg(feature = "ssr")]
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

/// Bounded in-memory cache for rendered HTML responses.
///
/// The cache key embeds the client-supplied Cookie header, so a flood of distinct
/// cookies could otherwise grow the cache without limit within the TTL window.
/// Entries are weighed by body size and evicted once the total exceeds the ceiling.
#[cfg(feature = "ssr")]
fn build_html_cache() -> cache_middleware::HtmlCache {
    std::sync::Arc::new(
        moka::future::Cache::builder()
            .time_to_live(std::time::Duration::from_secs(15))
            .max_capacity(64 * 1024 * 1024)
            .weigher(
                |_key: &String, value: &(axum::http::HeaderMap, axum::body::Bytes)| {
                    value.1.len().try_into().unwrap_or(u32::MAX)
                },
            )
            .build(),
    )
}

/// Transcode-on-demand image endpoint.
///
/// Image processing is CPU/IO heavy, so the work goes to the blocking pool rather
/// than tying up a tokio worker thread.
#[cfg(feature = "ssr")]
async fn serve_compressed_image(
    images_dir: std::sync::Arc<str>,
    cache_dir: std::sync::Arc<str>,
    image_path: String,
    params: portfolio::image_params::ImageParams,
) -> axum::response::Response {
    use axum::http::{StatusCode, header};
    use axum::response::IntoResponse;
    use leptos::logging::log;

    let (width, quality) = match params.validate() {
        Ok(values) => values,
        Err(err_msg) => return (StatusCode::BAD_REQUEST, err_msg).into_response(),
    };

    let path_without_ext = match image_path.rfind('.') {
        Some(dot) => image_path[..dot].to_string(),
        None => image_path.clone(),
    };

    let result = tokio::task::spawn_blocking(move || {
        portfolio::image_cache::process_and_cache_image(
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
            (StatusCode::INTERNAL_SERVER_ERROR, "Image processing failed").into_response()
        }
    }
}

#[cfg(feature = "ssr")]
#[tokio::main]
async fn main() {
    use axum::{
        Router,
        extract::{Path, Query},
        http::{StatusCode, header},
    };
    use leptos::logging::log;
    use leptos::prelude::*;
    use leptos_axum::{LeptosRoutes, generate_route_list};
    use portfolio::app::{App, shell};
    use portfolio::image_cache::{cleanup_cache, prewarm_cache};
    use portfolio::image_params::ImageParams;
    use std::sync::Arc;
    use tower::ServiceBuilder;
    use tower_http::{
        compression::CompressionLayer, services::ServeDir, set_header::SetResponseHeaderLayer,
    };

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

    let image_handler = {
        let images_dir = Arc::clone(&images_dir_arc);
        let cache_dir = Arc::clone(&cache_dir_arc);
        move |Path(image_path): Path<String>, Query(params): Query<ImageParams>| {
            serve_compressed_image(
                Arc::clone(&images_dir),
                Arc::clone(&cache_dir),
                image_path,
                params,
            )
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

    let html_cache = build_html_cache();

    let app = Router::new()
        .leptos_routes(&leptos_options, routes, {
            let leptos_options = leptos_options.clone();
            move || shell(leptos_options.clone())
        })
        // Lightweight liveness probe for Docker HEALTHCHECK / load-balancers.
        .route(
            "/healthz",
            axum::routing::get(|| async { (StatusCode::OK, "ok") }),
        )
        // Compressed image endpoint
        .route(
            "/images/compressed/{*image_path}",
            axum::routing::get(image_handler),
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
        .layer(axum::middleware::from_fn_with_state(
            html_cache,
            cache_middleware::html_cache_middleware,
        ))
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
