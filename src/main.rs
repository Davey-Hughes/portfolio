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
/// A file at one of these addresses is treated as never changing under the same address,
/// so a success is cached for a year: any 2xx, including the 206 a video player gets for
/// a range. So is a 304, because a cache copies a 304's headers onto the copy it already
/// holds, so the 304 has to carry the same Cache-Control as the 200.
///
/// The addresses the site builds for these files end in `?v=` and the file's
/// modification time (see `portfolio::gallery`), and nothing here reads `v`. So a file
/// replaced under its old name gets a new address that no browser or CDN holds yet: a
/// photo once the server's cached photo lists see the change, the favicon and the
/// profile photo on the next render. An address without the version, such as an image
/// path written into `opensource.toml`, still keeps the old file for up to a year.
///
/// Everything else gets `no-store`: a 404, a redirect, a 405 or 416, the endpoint's 400
/// for bad parameters, a 5xx. A missing file may appear later, when a photo is added or
/// the images share comes back after an outage, so nobody should remember its absence.
/// Nor should anyone keep a redirect or an error, which may answer differently next time.
///
/// So does a success that may hold part of a file still being written: one from a file
/// whose `Last-Modified` is within `image_cache::SETTLE_TIME` of now, or one the
/// compressed endpoint marked [`Unsettled`]. Its address may stay the same once the
/// file is finished.
#[cfg(feature = "ssr")]
fn file_cache_control<B>(mut response: axum::http::Response<B>) -> axum::http::Response<B> {
    use axum::http::{HeaderValue, StatusCode, header};

    let status = response.status();
    let kept = status.is_success() || status == StatusCode::NOT_MODIFIED;
    let value = if kept && is_settled(&response) {
        "public, max-age=31536000, immutable"
    } else {
        "no-store"
    };
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static(value));
    response
}

/// Whether `response` comes from a file the server trusts to be whole: not marked
/// [`Unsettled`], and from a file left alone long enough, going by its `Last-Modified`.
#[cfg(feature = "ssr")]
fn is_settled<B>(response: &axum::http::Response<B>) -> bool {
    use axum::http::header;
    use portfolio::image_cache::has_settled;

    if response.extensions().get::<Unsettled>().is_some() {
        return false;
    }
    response
        .headers()
        .get(header::LAST_MODIFIED)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| httpdate::parse_http_date(value).ok())
        .is_none_or(|modified| has_settled(modified, std::time::SystemTime::now()))
}

/// Put on a response whose body may be part of a file that is still being written.
/// [`file_cache_control`] then sends `no-store`, whatever the status.
#[cfg(feature = "ssr")]
#[derive(Clone, Copy, Debug)]
struct Unsettled;

/// What the `/images` and `/content` file servers answer with.
#[cfg(feature = "ssr")]
type FileResponse = axum::http::Response<tower_http::services::fs::ServeFileSystemResponseBody>;

/// The file server for the `/images` and `/content` mounts: the files in `dir`, with
/// Cache-Control set by [`file_cache_control`].
///
/// A directory is a 404, with or without a trailing slash; neither mount has index
/// pages. By default `ServeDir` redirects `/home` to `/home/`, but it builds that
/// `Location` from the path it is given, and `nest_service` has already taken the
/// mount's prefix off it, so `/images/home` was sent to `/home/`. `ServeDir` can put
/// the prefix back (`redirect_path_prefix`), but with no index pages the redirect
/// would only lead to another 404.
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
        .service(ServeDir::new(dir).append_index_html_on_directories(false))
}

/// The `/images` routes: the original files in `images_dir`, and the transcoding
/// endpoint at `/images/compressed/<path>`, which keeps the WebP files it makes in
/// `cache_dir`. A request that has to make a size first takes one of `transcodes`'
/// permits.
#[cfg(feature = "ssr")]
fn image_routes<S>(
    images_dir: &str,
    cache_dir: &str,
    transcodes: std::sync::Arc<tokio::sync::Semaphore>,
) -> axum::Router<S>
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
        serve_compressed_image(
            Arc::clone(&images),
            Arc::clone(&cache),
            Arc::clone(&transcodes),
            image_path,
            params,
        )
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

/// The startup work, in the background: clear out cache files without a source, make
/// the default size of every photo, then fill the all-photos list.
///
/// It runs on a thread of its own, not as a task on the async runtime. The cache
/// pre-warm transcodes and blocks for as long as it runs, which after a deploy can be
/// minutes, and as a runtime task it held a worker that whole time. On a one-CPU host,
/// where the runtime has one worker, the site answered nothing, not even `/healthz`.
#[cfg(feature = "ssr")]
fn spawn_startup_work(images_dir: String, cache_dir: String) -> std::thread::JoinHandle<()> {
    use portfolio::image_cache::{cleanup_cache, prewarm_cache};

    std::thread::spawn(move || {
        // Run cleanup first, then prewarming
        cleanup_cache(&images_dir, &cache_dir);
        prewarm_cache(&images_dir, &cache_dir);

        // Pre-warm the all-photos cache for faster photo detail page loads
        portfolio::server::prewarm_all_photos_cache();
    })
}

/// Transcode-on-demand image endpoint.
///
/// Image processing is CPU/IO heavy, so the work goes to the blocking pool rather
/// than tying up a tokio worker thread. Cache-Control is left to the
/// [`file_cache_control`] layer that [`image_routes`] puts around the route.
///
/// A size already in the cache is only a file read. A size that has to be made waits
/// for one of `transcodes`' permits and keeps it until the work is done, even if the
/// client goes away, so a burst of requests (a gallery opened just after a deploy)
/// can't decode every photo at once.
#[cfg(feature = "ssr")]
async fn serve_compressed_image(
    images_dir: std::sync::Arc<str>,
    cache_dir: std::sync::Arc<str>,
    transcodes: std::sync::Arc<tokio::sync::Semaphore>,
    image_path: String,
    params: portfolio::image_params::ImageParams,
) -> axum::response::Response {
    use axum::http::{StatusCode, header};
    use axum::response::IntoResponse;
    use leptos::logging::log;
    use portfolio::image_cache::{Lookup, Webp, cached_image, process_and_cache_image};
    use std::sync::Arc;

    let (width, quality) = match params.validate() {
        Ok(values) => values,
        Err(err_msg) => return (StatusCode::BAD_REQUEST, err_msg).into_response(),
    };

    // The whole path, extension included, for both lookups: that is how a request names
    // a file whose extension is spelled `Jpg`, which the name alone doesn't find, and
    // the check after the permit has to find the same file as the one before it.
    let requested_path: Arc<str> = Arc::from(image_path.as_str());

    let cached = {
        let (images_dir, cache_dir, path) = (
            Arc::clone(&images_dir),
            Arc::clone(&cache_dir),
            Arc::clone(&requested_path),
        );
        tokio::task::spawn_blocking(move || {
            cached_image(&images_dir, &cache_dir, &path, width, quality)
        })
        .await
    };
    let result = match cached {
        // A source that failed to decode before, as it is now, needs no permit to fail.
        Ok(Lookup::NoSource | Lookup::Undecodable) => Ok(None),
        Ok(Lookup::Cached(data)) => Ok(Some(Webp {
            data,
            settled: true,
        })),
        Ok(Lookup::Missing) => {
            // Nothing closes the semaphore, so this only ever waits.
            let Ok(permit) = transcodes.acquire_owned().await else {
                return (StatusCode::INTERNAL_SERVER_ERROR, "Image processing failed")
                    .into_response();
            };
            // Checks the cache again: another request may have made the size while
            // this one waited.
            tokio::task::spawn_blocking(move || {
                let _permit = permit;
                process_and_cache_image(&images_dir, &cache_dir, &requested_path, width, quality)
            })
            .await
        }
        Err(err) => Err(err),
    };

    match result {
        Ok(Some(webp)) => {
            log!("Serving image: {}", image_path);
            let mut response = (
                StatusCode::OK,
                [(header::CONTENT_TYPE, "image/webp")],
                webp.data,
            )
                .into_response();
            if !webp.settled {
                response.extensions_mut().insert(Unsettled);
            }
            response
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

    spawn_startup_work(images_dir.clone(), cache_dir.clone());

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
        .merge(image_routes(
            &images_dir,
            &cache_dir,
            std::sync::Arc::new(tokio::sync::Semaphore::new(
                portfolio::image_cache::transcode_limit(),
            )),
        ))
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
    use super::{compression_layer, image_routes, immutable_files};
    use axum::body::Body;
    use axum::http::{HeaderValue, Method, Request, Response, StatusCode, header};
    use std::convert::Infallible;
    use tower::{Layer, Service, ServiceExt, service_fn};

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

    /// A folder for a file mount: `home/` holding one photo, and `album/` holding an
    /// `index.html`, which the mounts don't serve either.
    fn mount_dir() -> tempfile::TempDir {
        let root = tempfile::TempDir::new().unwrap();
        std::fs::create_dir(root.path().join("home")).unwrap();
        std::fs::write(root.path().join("home/photo.jpg"), b"jpeg").unwrap();
        std::fs::create_dir(root.path().join("album")).unwrap();
        std::fs::write(root.path().join("album/index.html"), b"<p>album</p>").unwrap();
        root
    }

    /// Send `method` `uri` to `service`: the status, and the `Location` header if there
    /// is one.
    async fn fetch<S, B>(
        service: S,
        method: &Method,
        uri: &str,
    ) -> (StatusCode, Option<HeaderValue>)
    where
        S: Service<Request<Body>, Response = Response<B>>,
        S::Error: std::fmt::Debug,
    {
        let request = Request::builder()
            .method(method)
            .uri(uri)
            .body(Body::empty())
            .unwrap();
        let response = service.oneshot(request).await.unwrap();
        (
            response.status(),
            response.headers().get(header::LOCATION).cloned(),
        )
    }

    #[tokio::test]
    async fn file_mount_answers_404_for_a_directory_with_or_without_its_slash() {
        let root = mount_dir();
        let dir = root.path().to_str().unwrap();

        for uri in ["/", "/home", "/home/", "/album", "/album/"] {
            assert_eq!(
                fetch(immutable_files(dir), &Method::GET, uri).await,
                (StatusCode::NOT_FOUND, None),
                "{uri}"
            );
        }
        assert_eq!(
            fetch(immutable_files(dir), &Method::GET, "/home/photo.jpg").await,
            (StatusCode::OK, None)
        );
    }

    /// Through `image_routes`, as `main` mounts `/images`. `nest_service` hands the file
    /// server the path without the mount's prefix, so a redirect built from that path
    /// would leave the mount: it sent `/images/home` to `/home/`.
    #[tokio::test]
    async fn nested_file_mount_answers_404_for_a_directory_with_or_without_its_slash() {
        let root = mount_dir();
        let cache = tempfile::TempDir::new().unwrap();
        let app = image_routes::<()>(
            root.path().to_str().unwrap(),
            cache.path().to_str().unwrap(),
            std::sync::Arc::new(tokio::sync::Semaphore::new(1)),
        );

        for method in [Method::GET, Method::HEAD] {
            for uri in [
                "/images",
                "/images/",
                "/images/home",
                "/images/home/",
                "/images/home?width=2400",
                "/images/album",
                "/images/album/",
            ] {
                assert_eq!(
                    fetch(app.clone(), &method, uri).await,
                    (StatusCode::NOT_FOUND, None),
                    "{method} {uri}"
                );
            }
            assert_eq!(
                fetch(app.clone(), &method, "/images/home/photo.jpg").await,
                (StatusCode::OK, None),
                "{method}"
            );
        }
    }

    /// Cache-Control on the `/images` and `/content` file servers and on the
    /// compressed-image endpoint.
    mod cache_policy {
        use crate::{file_cache_control, image_routes, immutable_files};
        use axum::body::Body;
        use axum::http::{HeaderValue, Request, Response, StatusCode, header};
        use std::convert::Infallible;
        use std::sync::Arc;
        use std::time::Duration;
        use tempfile::TempDir;
        use tokio::sync::Semaphore;
        use tower::{Layer, ServiceExt, service_fn, util::MapResponseLayer};

        const IMMUTABLE: &str = "public, max-age=31536000, immutable";
        const NO_STORE: &str = "no-store";
        /// Size of `clip.mp4` in [`files`].
        const CLIP_LEN: usize = 1024;

        /// A directory holding `clip.mp4`, [`CLIP_LEN`] bytes long.
        fn files() -> TempDir {
            let dir = TempDir::new().expect("a temp dir");
            std::fs::write(dir.path().join("clip.mp4"), [b'v'; CLIP_LEN]).expect("a file");
            backdate(&dir.path().join("clip.mp4"));
            dir
        }

        /// Give the file at `path` a modification time an hour ago, long enough for the
        /// server to trust it whole.
        fn backdate(path: &std::path::Path) {
            set_modified(path, std::time::SystemTime::now() - Duration::from_hours(1));
        }

        fn set_modified(path: &std::path::Path, time: std::time::SystemTime) {
            std::fs::File::options()
                .write(true)
                .open(path)
                .expect("a file to date")
                .set_modified(time)
                .expect("a time set");
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
        /// PNG, `home/dot.png`, and an empty cache directory, with two permits to make
        /// sizes.
        struct Images {
            photos: TempDir,
            cache: TempDir,
            transcodes: Arc<Semaphore>,
        }

        impl Images {
            const PERMITS: u32 = 2;

            fn new() -> Self {
                let photos = TempDir::new().expect("a temp dir");
                std::fs::create_dir(photos.path().join("home")).expect("a gallery dir");
                image::RgbImage::new(4, 4)
                    .save(photos.path().join("home/dot.png"))
                    .expect("a PNG");
                backdate(&photos.path().join("home/dot.png"));
                let cache = TempDir::new().expect("a temp dir");
                let transcodes = Arc::new(Semaphore::new(Self::PERMITS as usize));
                Self {
                    photos,
                    cache,
                    transcodes,
                }
            }

            async fn send(&self, request: Request<Body>) -> Response<Body> {
                let images = self.photos.path().to_str().expect("a UTF-8 temp path");
                let cache = self.cache.path().to_str().expect("a UTF-8 temp path");
                image_routes::<()>(images, cache, Arc::clone(&self.transcodes))
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

        /// The site adds `v=<file version>` to the image addresses it builds, so that
        /// they change when the file does. Both routes answer as they would without it.
        #[tokio::test]
        async fn a_version_in_the_query_is_ignored() {
            let images = Images::new();
            for (plain, versioned) in [
                (
                    "/images/compressed/home/dot.png?width=2400&quality=90",
                    "/images/compressed/home/dot.png?width=2400&quality=90&v=1731652400",
                ),
                (
                    "/images/compressed/home/dot.png",
                    "/images/compressed/home/dot.png?v=1731652400",
                ),
                ("/images/home/dot.png", "/images/home/dot.png?v=1731652400"),
            ] {
                let expected = images.send(get(plain)).await;
                let expected = axum::body::to_bytes(expected.into_body(), usize::MAX)
                    .await
                    .expect("a body");
                let response = images.send(get(versioned)).await;
                assert_eq!(response.status(), StatusCode::OK, "{versioned}");
                assert_eq!(cache_control(&response), Some(IMMUTABLE), "{versioned}");
                let body = axum::body::to_bytes(response.into_body(), usize::MAX)
                    .await
                    .expect("a body");
                assert_eq!(body, expected, "{versioned}");
            }
        }

        /// A size that has to be made waits for a permit. One already in the cache is
        /// only a file read, and is served even while every permit is taken.
        #[tokio::test]
        async fn only_a_size_to_make_waits_for_a_permit() {
            use tokio::time::timeout;

            const CACHED: &str = "/images/compressed/home/dot.png?width=2400&quality=90";
            const TO_MAKE: &str = "/images/compressed/home/dot.png?width=4000&quality=90";

            let images = Images::new();
            assert_eq!(images.send(get(CACHED)).await.status(), StatusCode::OK);
            let every_permit = Arc::clone(&images.transcodes)
                .acquire_many_owned(Images::PERMITS)
                .await
                .expect("an open semaphore");

            let cached = timeout(Duration::from_secs(10), images.send(get(CACHED)))
                .await
                .expect("a cached size needs no permit");
            assert_eq!(cached.status(), StatusCode::OK);

            let to_make = images.send(get(TO_MAKE));
            tokio::pin!(to_make);
            assert!(
                timeout(Duration::from_millis(300), &mut to_make)
                    .await
                    .is_err(),
                "a size to make waits while every permit is taken"
            );
            drop(every_permit);
            let made = timeout(Duration::from_secs(10), to_make)
                .await
                .expect("made once a permit is free");
            assert_eq!(made.status(), StatusCode::OK);
        }

        /// A size of a photo that isn't there is a 404 at once, without waiting for a
        /// permit to make it.
        #[tokio::test]
        async fn a_missing_source_is_a_404_without_a_permit() {
            let images = Images::new();
            let _every_permit = Arc::clone(&images.transcodes)
                .acquire_many_owned(Images::PERMITS)
                .await
                .expect("an open semaphore");

            let response = tokio::time::timeout(
                Duration::from_secs(2),
                images.send(get(
                    "/images/compressed/home/gone.jpg?width=2400&quality=90",
                )),
            )
            .await
            .expect("a missing source needs no permit");
            assert_eq!(response.status(), StatusCode::NOT_FOUND);
            assert_eq!(cache_control(&response), Some(NO_STORE));
        }

        /// A response marked [`crate::Unsettled`] isn't kept, whatever its status.
        #[tokio::test]
        async fn a_response_marked_unsettled_is_not_stored() {
            let inner = service_fn(|_req: Request<Body>| async {
                let mut response = Response::new(Body::empty());
                response.extensions_mut().insert(crate::Unsettled);
                Ok::<_, Infallible>(response)
            });
            let response = MapResponseLayer::new(file_cache_control)
                .layer(inner)
                .oneshot(get("/photo.jpg"))
                .await
                .expect("the stand-in is infallible");
            assert_eq!(response.status(), StatusCode::OK);
            assert_eq!(cache_control(&response), Some(NO_STORE));
        }

        // A file written moments ago may still be being written, so what is read may be
        // part of it. Once it has been left alone long enough, it is kept for a year.
        #[tokio::test]
        async fn an_original_changed_moments_ago_is_not_stored() {
            let dir = files();
            let clip = dir.path().join("clip.mp4");

            set_modified(&clip, std::time::SystemTime::now());
            let response = serve_file(&dir, get("/clip.mp4")).await;
            assert_eq!(response.status(), StatusCode::OK);
            assert_eq!(cache_control(&response), Some(NO_STORE));

            backdate(&clip);
            let response = serve_file(&dir, get("/clip.mp4")).await;
            assert_eq!(response.status(), StatusCode::OK);
            assert_eq!(cache_control(&response), Some(IMMUTABLE));
        }

        /// A file dated a day ahead, as a copy from a camera card with its clock ahead
        /// would be, is kept for a year like any settled file.
        #[tokio::test]
        async fn an_original_dated_a_day_ahead_is_cached_for_a_year() {
            let dir = files();
            let clip = dir.path().join("clip.mp4");

            set_modified(
                &clip,
                std::time::SystemTime::now() + Duration::from_hours(24),
            );
            let response = serve_file(&dir, get("/clip.mp4")).await;
            assert_eq!(response.status(), StatusCode::OK);
            assert_eq!(cache_control(&response), Some(IMMUTABLE));
        }

        /// A size made from a source written moments ago is sent, but neither kept nor
        /// cached. Once the source has been left alone long enough, it is both.
        #[tokio::test]
        async fn a_size_of_a_source_changed_moments_ago_is_neither_kept_nor_cached() {
            const SIZE: &str = "/images/compressed/home/dot.png?width=2400&quality=90";
            let images = Images::new();
            let dot = images.photos.path().join("home/dot.png");
            let cached =
                || std::fs::read_dir(images.cache.path()).map_or(0, std::iter::Iterator::count);

            set_modified(&dot, std::time::SystemTime::now());
            let response = images.send(get(SIZE)).await;
            assert_eq!(response.status(), StatusCode::OK);
            assert_eq!(cache_control(&response), Some(NO_STORE));
            assert_eq!(cached(), 0, "nothing cached");

            backdate(&dot);
            let response = images.send(get(SIZE)).await;
            assert_eq!(response.status(), StatusCode::OK);
            assert_eq!(cache_control(&response), Some(IMMUTABLE));
            assert_eq!(cached(), 1, "the size is cached");
        }
    }

    /// The startup work leaves the async runtime free while it runs.
    mod startup {
        use crate::spawn_startup_work;
        use std::time::{Duration, Instant};
        use tempfile::TempDir;

        /// `IMAGES_DIR` for the life of one `#[serial]` test, removed again on drop.
        struct ImagesDir;

        impl ImagesDir {
            fn set(dir: &str) -> Self {
                unsafe { std::env::set_var("IMAGES_DIR", dir) };
                Self
            }
        }

        impl Drop for ImagesDir {
            fn drop(&mut self) {
                unsafe { std::env::remove_var("IMAGES_DIR") };
            }
        }

        /// On a runtime with one worker, as on a one-CPU host, anything else the
        /// runtime runs, such as a `/healthz` request, gets an answer while the cache is
        /// still being pre-warmed.
        #[tokio::test(flavor = "current_thread")]
        #[serial_test::serial] // sets IMAGES_DIR
        async fn the_runtime_answers_while_the_cache_is_prewarmed() {
            // Enough to keep the pre-warm busy for a good while: noise transcodes slowly.
            let dir = TempDir::new().expect("a temp dir");
            let gallery = dir.path().join("images/home");
            std::fs::create_dir_all(&gallery).expect("a gallery dir");
            let mut state = 0x2545_f491_u32;
            for n in 0..2 {
                image::RgbImage::from_fn(640, 480, |_, _| {
                    state ^= state << 13;
                    state ^= state >> 17;
                    state ^= state << 5;
                    let [r, g, b, _] = state.to_le_bytes();
                    image::Rgb([r, g, b])
                })
                .save(gallery.join(format!("{n}.png")))
                .expect("a PNG");
            }
            let path = |name: &str| dir.path().join(name).to_string_lossy().into_owned();
            // The all-photos list the startup work fills reads IMAGES_DIR, which would
            // otherwise be the checkout's own `public/images`.
            let _images_dir = ImagesDir::set(&path("images"));

            let started = Instant::now();
            let work = spawn_startup_work(path("images"), path("cache"));
            // Gives the one worker to anything that is waiting for it, as a request
            // would.
            tokio::spawn(async {})
                .await
                .expect("a task that can't fail");
            let answered = started.elapsed();
            let still_working = !work.is_finished();
            work.join().expect("the startup work doesn't panic");
            let finished = started.elapsed();

            assert!(
                still_working,
                "answered after {answered:?}, but the pre-warm was done by then; it took {finished:?}"
            );
            assert!(
                answered < Duration::from_millis(200),
                "took {answered:?} to answer; the pre-warm took {finished:?}"
            );
        }
    }
}

/// Photos whose extensions are in upper or mixed case, as cameras and other tools name
/// them, through the compressed-image route and the `/images` mount wired as `main`
/// wires them.
#[cfg(all(test, feature = "ssr"))]
mod extension_case_tests {
    use super::image_routes;
    use axum::body::Body;
    use axum::http::{Request, StatusCode, header};
    use portfolio::types::PhotoInfo;
    use std::sync::Arc;
    use std::time::Duration;
    use tempfile::TempDir;
    use tokio::sync::Semaphore;
    use tower::ServiceExt;

    /// The home gallery's files, each as wide as it says, so that a compressed answer
    /// shows which file it was made from: one of each case of extension, a name the
    /// addresses have to encode, and photos with more than one file.
    const PHOTOS: [(&str, u32); 14] = [
        ("Upper.JPG", 16),
        ("Mixed.Jpg", 17),
        ("Shot.PNG", 18),
        ("Pic.WEBP", 19),
        ("Loop.GIF", 28),
        ("1 - Harbour.JPG", 20),
        // Names that differ only in the case of the extension: lower case wins.
        ("Twin.jpg", 21),
        ("Twin.JPG", 22),
        // A mixed spelling yields to any other.
        ("Pair.Jpg", 23),
        ("Pair.webp", 24),
        // jpg before jpeg, as in the endpoint's order.
        ("Late.jpeg", 25),
        ("Late.JPG", 26),
        // An AVIF that this build can't decode is the main file, but the PNG serves.
        ("Moon.AVIF", 0),
        ("Moon.png", 27),
    ];

    /// For each photo in [`PHOTOS`]: its slug, its main file, and the file its
    /// compressed sizes are made from.
    const EXPECTED: [(&str, &str, &str); 10] = [
        ("upper", "Upper.JPG", "Upper.JPG"),
        ("mixed", "Mixed.Jpg", "Mixed.Jpg"),
        ("shot", "Shot.PNG", "Shot.PNG"),
        ("pic", "Pic.WEBP", "Pic.WEBP"),
        ("loop", "Loop.GIF", "Loop.GIF"),
        ("harbour", "1 - Harbour.JPG", "1 - Harbour.JPG"),
        ("twin", "Twin.jpg", "Twin.jpg"),
        ("pair", "Pair.webp", "Pair.webp"),
        ("late", "Late.JPG", "Late.JPG"),
        ("moon", "Moon.AVIF", "Moon.png"),
    ];

    /// An images dir with [`PHOTOS`] in `home/`, and an empty cache dir. Every file is
    /// dated an hour ago, long enough for the server to trust it whole and cache what
    /// it makes from it.
    fn dirs() -> (TempDir, TempDir) {
        let images = TempDir::new().expect("a temp dir");
        let home = images.path().join("home");
        std::fs::create_dir_all(&home).expect("a gallery dir");
        for (photo, width) in PHOTOS {
            if width == 0 {
                std::fs::write(home.join(photo), b"not an image").expect("a file");
            } else {
                image::RgbImage::new(width, 12)
                    .save(home.join(photo))
                    .expect("an image");
            }
            backdate(&home.join(photo));
        }
        (images, TempDir::new().expect("a temp dir"))
    }

    fn backdate(path: &std::path::Path) {
        std::fs::File::options()
            .write(true)
            .open(path)
            .expect("a file to date")
            .set_modified(std::time::SystemTime::now() - Duration::from_hours(1))
            .expect("a time set");
    }

    /// The image routes over `images` and `cache`, as `main` sets them up, with
    /// `transcodes` for the permits to make a size.
    fn router(images: &TempDir, cache: &TempDir, transcodes: Arc<Semaphore>) -> axum::Router {
        image_routes(
            images.path().to_str().expect("a UTF-8 temp path"),
            cache.path().to_str().expect("a UTF-8 temp path"),
            transcodes,
        )
    }

    /// [`router`] with permits enough not to matter.
    fn app(images: &TempDir, cache: &TempDir) -> axum::Router {
        router(images, cache, Arc::new(Semaphore::new(4)))
    }

    /// The status, Content-Type and body of a GET for `uri`.
    async fn fetch(app: &axum::Router, uri: &str) -> (StatusCode, String, axum::body::Bytes) {
        let request = Request::get(uri)
            .body(Body::empty())
            .expect("a valid request");
        let response = app
            .clone()
            .oneshot(request)
            .await
            .expect("the router is infallible");
        let status = response.status();
        let content_type = response
            .headers()
            .get(header::CONTENT_TYPE)
            .map_or_default(|value| value.to_str().expect("an ASCII header").to_owned());
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("a body");
        (status, content_type, body)
    }

    /// The status and Content-Type of a GET for `uri`.
    async fn get(app: &axum::Router, uri: &str) -> (StatusCode, String) {
        let (status, content_type, _) = fetch(app, uri).await;
        (status, content_type)
    }

    /// The home gallery's photos as the site lists them.
    fn home_photos(images: &TempDir) -> Vec<PhotoInfo> {
        let mut photos = Vec::new();
        portfolio::gallery::find_images_for_gallery(
            &images.path().join("home"),
            images.path(),
            &mut photos,
        );
        photos
    }

    #[tokio::test]
    async fn every_address_the_gallery_builds_loads_the_file_it_should() {
        let (images, cache) = dirs();
        let app = app(&images, &cache);
        let mut photos = home_photos(&images);
        photos.sort_by(|a, b| a.slug.cmp(&b.slug));
        let mut expected = EXPECTED;
        expected.sort_unstable();
        let found: Vec<(&str, &str)> = photos
            .iter()
            .map(|p| (p.slug.as_str(), p.filename.as_str()))
            .collect();
        let wanted: Vec<(&str, &str)> = expected.iter().map(|e| (e.0, e.1)).collect();
        assert_eq!(found, wanted);

        for (photo, (_, main, source)) in photos.iter().zip(expected) {
            let width = PHOTOS
                .iter()
                .find(|(name, _)| *name == source)
                .map(|(_, width)| *width);
            let srcset = photo
                .srcset
                .split(", ")
                .map(|candidate| candidate.split_once(' ').expect("a width").0);
            for compressed in [photo.url.as_str(), photo.detail_url.as_str()]
                .into_iter()
                .chain(srcset)
            {
                let (status, content_type, body) = fetch(&app, compressed).await;
                assert_eq!(status, StatusCode::OK, "{compressed}");
                assert_eq!(content_type, "image/webp", "{compressed}");
                let picture = image::load_from_memory(&body).expect("a WebP");
                assert_eq!(Some(picture.width()), width, "{compressed} from {source}");
            }
            let (status, _, body) = fetch(&app, &photo.original_url).await;
            assert_eq!(status, StatusCode::OK, "{}", photo.original_url);
            let file = std::fs::read(images.path().join("home").join(main)).expect("a file");
            assert_eq!(body, file, "{}", photo.original_url);
        }
    }

    #[tokio::test]
    async fn an_original_answers_at_its_real_name_only() {
        let (images, cache) = dirs();
        let app = app(&images, &cache);

        for (uri, expected) in [
            ("/images/home/Upper.JPG", "image/jpeg"),
            ("/images/home/Mixed.Jpg", "image/jpeg"),
            ("/images/home/Shot.PNG", "image/png"),
            ("/images/home/Pic.WEBP", "image/webp"),
            ("/images/home/Loop.GIF", "image/gif"),
        ] {
            assert_eq!(
                get(&app, uri).await,
                (StatusCode::OK, expected.to_owned()),
                "{uri}"
            );
        }
        // The file server matches names exactly, so the gallery's addresses keep them.
        let (status, _) = get(&app, "/images/home/Upper.jpg").await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn the_compressed_endpoint_finds_any_case_of_extension_and_nothing_else() {
        let (images, cache) = dirs();
        let app = app(&images, &cache);

        for photo in [
            "Upper.JPG",
            "Upper.jpg",
            "Mixed.Jpg",
            "Shot.PNG",
            "Pic.WEBP",
            "Loop.GIF",
        ] {
            let uri = format!("/images/compressed/home/{photo}?width=2400&quality=90");
            assert_eq!(
                get(&app, &uri).await,
                (StatusCode::OK, "image/webp".to_owned()),
                "{uri}"
            );
        }
        // `Mixed.jpg`: a mixed spelling is only looked for through a file the request
        // names that is there.
        for missing in ["Gone.JPG", "Gone.Jpg", "Mixed.jpg", "Mixed.JPG"] {
            let uri = format!("/images/compressed/home/{missing}?width=2400&quality=90");
            let (status, _) = get(&app, &uri).await;
            assert_eq!(status, StatusCode::NOT_FOUND, "{uri}");
        }
    }

    /// The handler looks a size up twice, before and after taking a permit, and both
    /// lookups have to find a mixed spelling the same way. Once made, `Mixed.Jpg`'s size
    /// comes from the cache with every permit taken, so the first lookup finds it.
    #[tokio::test]
    async fn a_mixed_case_photo_comes_from_the_cache_without_a_permit() {
        const URI: &str = "/images/compressed/home/Mixed.Jpg?width=2400&quality=90";
        let (images, cache) = dirs();
        let transcodes = Arc::new(Semaphore::new(1));
        let app = router(&images, &cache, Arc::clone(&transcodes));

        assert_eq!(get(&app, URI).await.0, StatusCode::OK, "made");
        assert!(cache.path().join("home_Mixed_w2400_q90_l1.webp").is_file());
        let _every_permit = transcodes.acquire_owned().await.expect("an open semaphore");

        let again = tokio::time::timeout(Duration::from_secs(10), get(&app, URI))
            .await
            .expect("a cached size needs no permit");
        assert_eq!(again, (StatusCode::OK, "image/webp".to_owned()));
    }

    /// A source the decode limits refused answers 404 without a permit, once it is
    /// remembered: a GIF whose header declares a canvas 32,769 pixels wide.
    #[tokio::test]
    async fn a_refused_source_is_a_404_without_a_permit() {
        const URI: &str = "/images/compressed/home/Wide.gif?width=2400&quality=90";
        let (images, cache) = dirs();
        let mut gif = Vec::new();
        image::RgbaImage::new(1, 1)
            .write_to(&mut std::io::Cursor::new(&mut gif), image::ImageFormat::Gif)
            .expect("a GIF");
        gif[6..8].copy_from_slice(&32_769_u16.to_le_bytes());
        let wide = images.path().join("home/Wide.gif");
        std::fs::write(&wide, gif).expect("a file");
        backdate(&wide);
        let transcodes = Arc::new(Semaphore::new(1));
        let app = router(&images, &cache, Arc::clone(&transcodes));

        assert_eq!(get(&app, URI).await.0, StatusCode::NOT_FOUND, "refused");
        let _every_permit = transcodes.acquire_owned().await.expect("an open semaphore");

        let again = tokio::time::timeout(Duration::from_secs(10), get(&app, URI))
            .await
            .expect("a refused source needs no permit");
        assert_eq!(again.0, StatusCode::NOT_FOUND);
    }
}
