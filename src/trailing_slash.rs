//! One address per page: a page address with a trailing slash (`/about/`) is
//! permanently redirected to the same address without it (`/about`).
//!
//! Without this, the server and the router disagreed about such an address. The
//! server's routes are exact, so `/about/` fell through to the fallback, which
//! answers 404. But the router, rendering that same response and again in the
//! browser, takes one trailing slash as none, and showed the about page. A crawler
//! saw a missing page, and a visitor saw a working one. (With two slashes the router
//! matched nothing either, and the not-found page showed.)
//!
//! Only GET and HEAD requests for addresses shaped like one of the app's page
//! routes are redirected. Nothing else is touched: not `/` itself, not `/pkg`
//! (which the Leptos fallback serves from the site root), not the file mounts
//! (`/images`, `/content`, `/readme`) and not the server functions. Those keep their
//! own answers for their own paths; `/readme/<repo>/`, for one, must stay a 404.

use axum::{
    extract::{Request, State},
    http::{Method, StatusCode, Uri, header},
    middleware::Next,
    response::{IntoResponse, Response},
};
use leptos_axum::AxumRouteListing;
use std::sync::Arc;

/// The app's page routes, as axum paths such as `/gallery/{name}`: the state
/// [`redirect`] runs with.
#[derive(Clone)]
pub struct PageRoutes(Arc<[String]>);

impl PageRoutes {
    /// `routes` as `generate_route_list` gives them.
    pub fn new(routes: &[AxumRouteListing]) -> Self {
        Self(routes.iter().map(|route| route.path().to_owned()).collect())
    }

    /// Where a request for `uri` belongs instead, if it is a page address with a
    /// trailing slash: the same address without it, query kept.
    fn canonical_location(&self, uri: &Uri) -> Option<String> {
        let path = uri.path();
        let trimmed = path.trim_end_matches('/');
        if trimmed.len() == path.len() || !self.0.iter().any(|route| fits(trimmed, route)) {
            return None;
        }
        Some(match uri.query() {
            Some(query) => format!("{trimmed}?{query}"),
            None => trimmed.to_owned(),
        })
    }
}

/// Whether `path` fits `route` segment for segment, a `{param}` taking any
/// non-empty segment. The root route never fits a trimmed path, which is empty. A
/// `{*splat}` route would count as one segment here; the app has none.
fn fits(path: &str, route: &str) -> bool {
    path.split('/').count() == route.split('/').count()
        && path
            .split('/')
            .zip(route.split('/'))
            .all(|(segment, pattern)| {
                if pattern.starts_with('{') && pattern.ends_with('}') {
                    !segment.is_empty()
                } else {
                    segment == pattern
                }
            })
}

/// Middleware, for `axum::middleware::from_fn_with_state`: answer a GET or HEAD for
/// a page address with a trailing slash with 308 to the address without it, and
/// pass everything else on.
///
/// Other methods go on unchanged and get what they got before, the fallback's 404.
/// The page routes answer only GET and HEAD, so redirecting another method would
/// only trade one error for another.
pub async fn redirect(State(pages): State<PageRoutes>, req: Request, next: Next) -> Response {
    let location = match *req.method() {
        Method::GET | Method::HEAD => pages.canonical_location(req.uri()),
        _ => None,
    };
    match location {
        Some(location) => (
            StatusCode::PERMANENT_REDIRECT,
            [(header::LOCATION, location)],
        )
            .into_response(),
        None => next.run(req).await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::App;
    use serial_test::serial;
    use std::sync::LazyLock;

    /// The app's own page routes, generated once for all the tests here. Generating
    /// them builds `App`, whose resources need a tokio runtime to spawn on. This one
    /// is never driven, so they never run.
    ///
    /// Every test here is `#[serial]`: generating the routes sets `leptos_server`'s
    /// process-wide flag that stops resources loading, and a resource that one of
    /// `app::ssr_tests` polls meanwhile waits for ever. Run alongside those tests
    /// without it, they hung the test binary in 91 runs of 3000.
    static PAGES: LazyLock<PageRoutes> = LazyLock::new(|| {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let _entered = runtime.enter();
        PageRoutes::new(&leptos_axum::generate_route_list(App))
    });

    fn location(uri: &'static str) -> Option<String> {
        PAGES.canonical_location(&Uri::from_static(uri))
    }

    #[test]
    #[serial]
    fn page_addresses_lose_the_trailing_slash() {
        for (from, to) in [
            ("/about/", "/about"),
            ("/contact/", "/contact"),
            ("/open-source/", "/open-source"),
            ("/gallery/home/", "/gallery/home"),
            ("/gallery/home/laika/", "/gallery/home/laika"),
        ] {
            assert_eq!(location(from).as_deref(), Some(to), "{from}");
        }
    }

    // The redirect goes by the address's shape, not by what exists: a missing
    // gallery or photo is redirected too, and answers 404 at its one address.
    #[test]
    #[serial]
    fn missing_galleries_and_photos_are_redirected_too() {
        assert_eq!(location("/gallery/nope/").as_deref(), Some("/gallery/nope"));
        assert_eq!(
            location("/gallery/home/nope/").as_deref(),
            Some("/gallery/home/nope")
        );
    }

    #[test]
    #[serial]
    fn every_trailing_slash_goes_in_one_redirect() {
        assert_eq!(location("/about//").as_deref(), Some("/about"));
    }

    #[test]
    #[serial]
    fn the_query_is_kept() {
        assert_eq!(
            location("/gallery/home/?a=1&b=2").as_deref(),
            Some("/gallery/home?a=1&b=2")
        );
    }

    #[test]
    #[serial]
    fn canonical_addresses_and_the_root_are_left_alone() {
        for uri in [
            "/",
            "//",
            "/about",
            "/gallery/home",
            "/gallery/home/laika",
            "/about?a=1",
        ] {
            assert_eq!(location(uri), None, "{uri}");
        }
    }

    #[test]
    #[serial]
    fn mounts_and_other_addresses_are_left_alone() {
        for uri in [
            "/pkg/",
            "/images/",
            "/images/home/",
            "/content/",
            "/readme/",
            "/readme/fixture/",
            "/api/",
            "/healthz/",
            "/os/",
            "/no-such-page/",
            "/gallery/",
            "/gallery//",
            "/gallery//laika/",
            "/gallery/a/b/c/",
            "/gallery/home//laika",
            "//about/",
            "//evil.example/",
        ] {
            assert_eq!(location(uri), None, "{uri}");
        }
    }
}
