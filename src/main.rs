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

/// Response compression for the whole router: tower-http's default rules (which already
/// leave images alone), plus video.
#[cfg(feature = "ssr")]
fn compression_layer()
-> tower_http::compression::CompressionLayer<impl tower_http::compression::Predicate> {
    use tower_http::compression::{
        CompressionLayer, DefaultPredicate, Predicate, predicate::NotForContentType,
    };

    // Video is compressed already; gzipping it again costs CPU and drops Content-Length and
    // Accept-Ranges, the header a player needs before it will seek.
    CompressionLayer::new()
        .compress_when(DefaultPredicate::new().and(NotForContentType::const_new("video/")))
}

/// Sets Cache-Control on a response from the `/images` and `/content` file servers or
/// from the compressed-image endpoint, going by its status.
///
/// A file at one of these addresses is treated as never changing under the same name,
/// so a success is cached for a year: any 2xx, including the 206 a video player gets for
/// a range. So is a 304, because a cache copies a 304's headers onto the copy it already
/// holds, so the 304 has to carry the same Cache-Control as the 200. A photo replaced in
/// place under its old name won't reach a browser that already holds it.
///
/// Everything else gets `no-store`: a 404, a redirect, a 405 or 416, the endpoint's 400
/// for bad parameters, a 5xx. A missing file may appear later, when a photo is added or
/// the images share comes back after an outage, so nobody should remember its absence.
/// Nor should anyone keep a redirect or an error, which may answer differently next time.
#[cfg(feature = "ssr")]
fn file_cache_control<B>(mut response: axum::http::Response<B>) -> axum::http::Response<B> {
    use axum::http::{HeaderValue, StatusCode, header};

    let status = response.status();
    let value = if status.is_success() || status == StatusCode::NOT_MODIFIED {
        "public, max-age=31536000, immutable"
    } else {
        "no-store"
    };
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static(value));
    response
}

/// What the `/images` and `/content` file servers answer with.
#[cfg(feature = "ssr")]
type FileResponse = axum::http::Response<tower_http::services::fs::ServeFileSystemResponseBody>;

/// The file server for the `/images` and `/content` mounts: the files in `dir`, with
/// Cache-Control set by [`file_cache_control`].
#[cfg(feature = "ssr")]
fn immutable_files(
    dir: &str,
) -> tower::util::MapResponse<tower_http::services::ServeDir, fn(FileResponse) -> FileResponse> {
    use tower::{ServiceBuilder, util::MapResponseLayer};
    use tower_http::services::ServeDir;

    ServiceBuilder::new()
        .layer(MapResponseLayer::new(
            file_cache_control as fn(FileResponse) -> FileResponse,
        ))
        .service(ServeDir::new(dir))
}

/// The `/images` routes: the original files in `images_dir`, and the transcoding
/// endpoint at `/images/compressed/<path>`, which keeps the WebP files it makes in
/// `cache_dir`.
#[cfg(feature = "ssr")]
fn image_routes<S>(images_dir: &str, cache_dir: &str) -> axum::Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    use axum::extract::{Path, Query};
    use portfolio::image_params::ImageParams;
    use std::sync::Arc;
    use tower::util::MapResponseLayer;

    // The closure holds the directories, so a request clones two pointers rather than
    // resolving the paths again.
    let images: Arc<str> = Arc::from(images_dir);
    let cache: Arc<str> = Arc::from(cache_dir);
    let compressed = move |Path(image_path): Path<String>, Query(params): Query<ImageParams>| {
        serve_compressed_image(Arc::clone(&images), Arc::clone(&cache), image_path, params)
    };

    axum::Router::new()
        // The cache policy wraps the whole route, not just the handler, so it also
        // covers the 400 the query extractor gives a width that isn't a number, and the
        // 405 for a method other than GET or HEAD.
        .route(
            "/images/compressed/{*image_path}",
            axum::routing::get(compressed).layer(MapResponseLayer::new(file_cache_control)),
        )
        .nest_service("/images", immutable_files(images_dir))
}

/// The `/readme` mount: screenshots and clips for the READMEs of the owner's repos,
/// served from `dir` as `/readme/<repo>/<file>`.
///
/// `no-cache`, not the year-long `immutable` of `/images` and `/content`: README files
/// are replaced in place under the same name, and `Last-Modified` turns each
/// revalidation into a 304 while nothing has changed.
#[cfg(feature = "ssr")]
fn readme_routes<S>(dir: &str) -> axum::Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    use axum::http::{HeaderValue, header};
    use tower::ServiceBuilder;
    use tower_http::{services::ServeDir, set_header::SetResponseHeaderLayer};

    axum::Router::new().nest_service(
        "/readme",
        ServiceBuilder::new()
            .layer(SetResponseHeaderLayer::overriding(
                header::CACHE_CONTROL,
                HeaderValue::from_static("no-cache"),
            ))
            .layer(SetResponseHeaderLayer::overriding(
                header::X_CONTENT_TYPE_OPTIONS,
                HeaderValue::from_static("nosniff"),
            ))
            .layer(axum::middleware::from_fn(readme_filter))
            .service(ServeDir::new(dir).append_index_html_on_directories(false)),
    )
}

/// Gate in front of the `/readme` file server: anything but an allowed media file is a
/// bare 404, decided before the file server looks at the disk.
#[cfg(feature = "ssr")]
async fn readme_filter(
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    use axum::response::IntoResponse;

    if portfolio::readme_assets::is_servable(req.uri().path()) {
        next.run(req).await
    } else {
        axum::http::StatusCode::NOT_FOUND.into_response()
    }
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
/// than tying up a tokio worker thread. Cache-Control is left to the
/// [`file_cache_control`] layer that [`image_routes`] puts around the route.
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
                [(header::CONTENT_TYPE, "image/webp")],
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
    use axum::{Router, http::StatusCode};
    use leptos::logging::log;
    use leptos::prelude::*;
    use leptos_axum::{LeptosRoutes, generate_route_list};
    use portfolio::app::{App, shell};
    use portfolio::image_cache::{cleanup_cache, prewarm_cache};
    use portfolio::paths::{images_root, resolve_dir};

    // Load `.env` (used locally to set LEPTOS_HASH_FILES=false so dev builds skip
    // content-hashed pkg filenames). Never present in the Docker image
    // (.dockerignore), so production keeps hashing on.
    dotenvy::dotenv().ok();

    let conf = get_configuration(None).unwrap();
    let addr = conf.leptos_options.site_addr;
    let leptos_options = conf.leptos_options;
    let routes = generate_route_list(App);
    let pages = portfolio::trailing_slash::PageRoutes::new(&routes);

    // The gallery code reads images_root() too, so the /images mount below and the
    // gallery listing agree on where the photos are, as long as the root does not
    // change while the server runs: this resolves it once, the gallery code per call.
    let images_dir = images_root().to_string_lossy().into_owned();
    let content_dir = resolve_dir("ABOUT_CONTENT_PATH", &["public/content", "./content"]);
    let cache_dir = resolve_dir("IMAGE_CACHE_DIR", &["public/cache", "./cache"]);
    let readme_dir = resolve_dir("README_ASSETS_DIR", &["public/readme", "./readme"]);

    log!("Serving images from: {}", images_dir);
    log!("Serving content from: {}", content_dir);
    log!("Serving README assets from: {}", readme_dir);

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
        // Short link to the open source page. A server-side redirect rather than a
        // second Leptos route: the page keeps one canonical URL, and the hydrate
        // bundle carries nothing for it.
        .route(
            "/os",
            axum::routing::get(|| async { axum::response::Redirect::permanent("/open-source") }),
        )
        .merge(image_routes(&images_dir, &cache_dir))
        .nest_service("/content", immutable_files(&content_dir))
        .merge(readme_routes(&readme_dir))
        .fallback(leptos_axum::file_and_error_handler(shell))
        .with_state(leptos_options)
        .layer(axum::middleware::from_fn_with_state(
            html_cache,
            cache_middleware::html_cache_middleware,
        ))
        .layer(compression_layer())
        // Outermost, so a redirect goes out before the HTML cache or compression
        // does any work.
        .layer(axum::middleware::from_fn_with_state(
            pages,
            portfolio::trailing_slash::redirect,
        ));

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

#[cfg(all(test, feature = "ssr"))]
mod tests {
    use super::compression_layer;
    use axum::body::Body;
    use axum::http::{Request, Response, header};
    use std::convert::Infallible;
    use tower::{Layer, ServiceExt, service_fn};

    /// Comfortably above the default predicate's 32-byte floor.
    const LARGE: &[u8] = &[b'a'; 1024];
    /// Under it.
    const SMALL: &[u8] = b"tiny";

    /// Send one response of `content_type` through the app's compression layer, to a
    /// client that accepts gzip, and say whether it came back compressed.
    async fn is_compressed(content_type: &'static str, body: &'static [u8]) -> bool {
        let inner = service_fn(move |_req: Request<Body>| async move {
            Ok::<_, Infallible>(
                Response::builder()
                    .header(header::CONTENT_TYPE, content_type)
                    .header(header::CONTENT_LENGTH, body.len())
                    .body(Body::from(body))
                    .expect("a valid response"),
            )
        });
        let request = Request::builder()
            .header(header::ACCEPT_ENCODING, "gzip")
            .body(Body::empty())
            .expect("a valid request");
        let response = compression_layer()
            .layer(inner)
            .oneshot(request)
            .await
            .expect("the inner service is infallible");
        response.headers().contains_key(header::CONTENT_ENCODING)
    }

    #[tokio::test]
    async fn leaves_video_uncompressed() {
        for content_type in ["video/mp4", "video/webm"] {
            assert!(!is_compressed(content_type, LARGE).await, "{content_type}");
        }
    }

    #[tokio::test]
    async fn compresses_html_wasm_and_svg() {
        for content_type in [
            "text/html; charset=utf-8",
            "application/wasm",
            "image/svg+xml",
        ] {
            assert!(is_compressed(content_type, LARGE).await, "{content_type}");
        }
    }

    #[tokio::test]
    async fn keeps_the_default_exclusions() {
        // Raster images, server-sent events and gRPC are tower-http's own exclusions. The
        // video rule has to add to them, not replace them.
        for content_type in ["image/png", "text/event-stream", "application/grpc"] {
            assert!(!is_compressed(content_type, LARGE).await, "{content_type}");
        }
    }

    #[tokio::test]
    async fn leaves_small_bodies_uncompressed() {
        assert!(!is_compressed("text/html; charset=utf-8", SMALL).await);
    }

    /// Cache-Control on the `/images` and `/content` file servers and on the
    /// compressed-image endpoint.
    mod cache_policy {
        use crate::{file_cache_control, image_routes, immutable_files};
        use axum::body::Body;
        use axum::http::{HeaderValue, Request, Response, StatusCode, header};
        use std::convert::Infallible;
        use tempfile::TempDir;
        use tower::{Layer, ServiceExt, service_fn, util::MapResponseLayer};

        const IMMUTABLE: &str = "public, max-age=31536000, immutable";
        const NO_STORE: &str = "no-store";
        /// Size of `clip.mp4` in [`files`].
        const CLIP_LEN: usize = 1024;

        /// A directory holding `clip.mp4`, [`CLIP_LEN`] bytes long.
        fn files() -> TempDir {
            let dir = TempDir::new().expect("a temp dir");
            std::fs::write(dir.path().join("clip.mp4"), [b'v'; CLIP_LEN]).expect("a file");
            dir
        }

        fn get(uri: &str) -> Request<Body> {
            Request::get(uri)
                .body(Body::empty())
                .expect("a valid request")
        }

        fn cache_control<B>(response: &Response<B>) -> Option<&str> {
            response
                .headers()
                .get(header::CACHE_CONTROL)
                .map(|value| value.to_str().expect("an ASCII header"))
        }

        /// Send `request` to the file server that `main` mounts at `/images`, over `dir`.
        async fn serve_file(dir: &TempDir, request: Request<Body>) -> crate::FileResponse {
            let dir = dir.path().to_str().expect("a UTF-8 temp path");
            immutable_files(dir)
                .oneshot(request)
                .await
                .expect("the file server is infallible")
        }

        /// The Cache-Control that a response with `status` gets, from a stand-in for the
        /// file server wrapped in the same policy.
        async fn policy_for(status: StatusCode) -> Option<String> {
            let inner = service_fn(move |_req: Request<Body>| async move {
                Ok::<_, Infallible>(
                    Response::builder()
                        .status(status)
                        .header(header::LOCATION, "/home/")
                        .body(Body::empty())
                        .expect("a valid response"),
                )
            });
            let response = MapResponseLayer::new(file_cache_control)
                .layer(inner)
                .oneshot(get("/home"))
                .await
                .expect("the stand-in is infallible");
            assert_eq!(response.status(), status);
            cache_control(&response).map(str::to_owned)
        }

        #[tokio::test]
        async fn a_file_is_cached_for_a_year() {
            let dir = files();
            let response = serve_file(&dir, get("/clip.mp4")).await;
            assert_eq!(response.status(), StatusCode::OK);
            assert_eq!(cache_control(&response), Some(IMMUTABLE));
        }

        #[tokio::test]
        async fn a_304_carries_the_header_of_its_200() {
            let dir = files();
            let first = serve_file(&dir, get("/clip.mp4")).await;
            let last_modified = first
                .headers()
                .get(header::LAST_MODIFIED)
                .expect("ServeDir sends Last-Modified")
                .clone();

            let mut request = get("/clip.mp4");
            request
                .headers_mut()
                .insert(header::IF_MODIFIED_SINCE, last_modified);
            let response = serve_file(&dir, request).await;
            assert_eq!(response.status(), StatusCode::NOT_MODIFIED);
            assert_eq!(cache_control(&response), Some(IMMUTABLE));
        }

        #[tokio::test]
        async fn a_range_request_answers_206_and_is_cached_for_a_year() {
            let dir = files();
            let mut request = get("/clip.mp4");
            request
                .headers_mut()
                .insert(header::RANGE, HeaderValue::from_static("bytes=0-99"));
            let response = serve_file(&dir, request).await;
            assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
            assert_eq!(
                response.headers()[header::CONTENT_RANGE],
                format!("bytes 0-99/{CLIP_LEN}").as_str()
            );
            assert_eq!(cache_control(&response), Some(IMMUTABLE));
        }

        #[tokio::test]
        async fn a_missing_file_is_not_stored() {
            let dir = files();
            let response = serve_file(&dir, get("/gone.jpg")).await;
            assert_eq!(response.status(), StatusCode::NOT_FOUND);
            assert_eq!(cache_control(&response), Some(NO_STORE));
        }

        #[tokio::test]
        async fn a_redirect_is_not_stored() {
            for status in [
                StatusCode::MOVED_PERMANENTLY,
                StatusCode::FOUND,
                StatusCode::TEMPORARY_REDIRECT,
                StatusCode::PERMANENT_REDIRECT,
            ] {
                assert_eq!(
                    policy_for(status).await.as_deref(),
                    Some(NO_STORE),
                    "{status}"
                );
            }
        }

        #[tokio::test]
        async fn an_error_is_not_stored() {
            for status in [
                StatusCode::BAD_REQUEST,
                StatusCode::NOT_FOUND,
                StatusCode::METHOD_NOT_ALLOWED,
                StatusCode::RANGE_NOT_SATISFIABLE,
                StatusCode::INTERNAL_SERVER_ERROR,
                StatusCode::SERVICE_UNAVAILABLE,
            ] {
                assert_eq!(
                    policy_for(status).await.as_deref(),
                    Some(NO_STORE),
                    "{status}"
                );
            }
        }

        #[tokio::test]
        async fn a_success_or_304_is_cached_for_a_year() {
            for status in [
                StatusCode::OK,
                StatusCode::NO_CONTENT,
                StatusCode::PARTIAL_CONTENT,
                StatusCode::NOT_MODIFIED,
            ] {
                assert_eq!(
                    policy_for(status).await.as_deref(),
                    Some(IMMUTABLE),
                    "{status}"
                );
            }
        }

        /// The status alone decides: a Cache-Control set further in is replaced, so a
        /// 404 can't keep a year-long header that something behind the policy added.
        #[tokio::test]
        async fn a_cache_control_from_further_in_is_replaced() {
            for (status, set, expected) in [
                (StatusCode::NOT_FOUND, IMMUTABLE, NO_STORE),
                (StatusCode::OK, NO_STORE, IMMUTABLE),
            ] {
                let inner = service_fn(move |_req: Request<Body>| async move {
                    Ok::<_, Infallible>(
                        Response::builder()
                            .status(status)
                            .header(header::CACHE_CONTROL, set)
                            .body(Body::empty())
                            .expect("a valid response"),
                    )
                });
                let response = MapResponseLayer::new(file_cache_control)
                    .layer(inner)
                    .oneshot(get("/photo.jpg"))
                    .await
                    .expect("the stand-in is infallible");
                assert_eq!(cache_control(&response), Some(expected), "{status}");
            }
        }

        /// The `/images` routes as `main` wires them, over a gallery holding one small
        /// PNG, `home/dot.png`, and an empty cache directory.
        struct Images {
            images: TempDir,
            cache: TempDir,
        }

        impl Images {
            fn new() -> Self {
                let images = TempDir::new().expect("a temp dir");
                std::fs::create_dir(images.path().join("home")).expect("a gallery dir");
                image::RgbImage::new(4, 4)
                    .save(images.path().join("home/dot.png"))
                    .expect("a PNG");
                let cache = TempDir::new().expect("a temp dir");
                Self { images, cache }
            }

            async fn send(&self, request: Request<Body>) -> Response<Body> {
                let images = self.images.path().to_str().expect("a UTF-8 temp path");
                let cache = self.cache.path().to_str().expect("a UTF-8 temp path");
                image_routes::<()>(images, cache)
                    .oneshot(request)
                    .await
                    .expect("the router is infallible")
            }
        }

        #[tokio::test]
        async fn a_compressed_image_is_cached_for_a_year() {
            let images = Images::new();
            let response = images.send(get("/images/compressed/home/dot.png")).await;
            assert_eq!(response.status(), StatusCode::OK);
            assert_eq!(response.headers()[header::CONTENT_TYPE], "image/webp");
            assert_eq!(cache_control(&response), Some(IMMUTABLE));
        }

        #[tokio::test]
        async fn a_missing_compressed_image_is_not_stored() {
            let images = Images::new();
            let response = images.send(get("/images/compressed/home/gone.jpg")).await;
            assert_eq!(response.status(), StatusCode::NOT_FOUND);
            assert_eq!(cache_control(&response), Some(NO_STORE));
        }

        #[tokio::test]
        async fn a_rejected_compressed_image_request_is_not_stored() {
            let images = Images::new();
            // A width that is no preset, which the handler rejects, and one that isn't a
            // number, which the query extractor rejects before the handler runs.
            for query in ["width=1", "width=abc"] {
                let uri = format!("/images/compressed/home/dot.png?{query}");
                let response = images.send(get(&uri)).await;
                assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{query}");
                assert_eq!(cache_control(&response), Some(NO_STORE), "{query}");
            }

            let post = Request::post("/images/compressed/home/dot.png")
                .body(Body::empty())
                .expect("a valid request");
            let response = images.send(post).await;
            assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
            assert_eq!(cache_control(&response), Some(NO_STORE));
        }

        #[tokio::test]
        async fn an_original_image_goes_through_the_file_server() {
            let images = Images::new();
            let response = images.send(get("/images/home/dot.png")).await;
            assert_eq!(response.status(), StatusCode::OK);
            assert_eq!(cache_control(&response), Some(IMMUTABLE));
            let response = images.send(get("/images/home/gone.png")).await;
            assert_eq!(response.status(), StatusCode::NOT_FOUND);
            assert_eq!(cache_control(&response), Some(NO_STORE));
        }
    }
}
