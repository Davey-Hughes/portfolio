use crate::config::SiteConfig;
use crate::open_source::OpenSourceEntry;
use crate::server::{
    get_about_content, get_all_gallery_photos, get_galleries, get_gallery_data_by_name,
    get_home_gallery_config, get_home_gallery_data, get_open_source, get_site_config,
    has_open_source, random_home_photo,
};
use crate::types::{PhotoInfo, ViewerImage};
use leptos::prelude::*;
use leptos::wasm_bindgen::JsCast;
use leptos::web_sys;
use leptos_meta::{HashedStylesheet, MetaTags, Title, provide_meta_context};
use leptos_router::{
    ParamSegment, SsrMode, StaticSegment,
    components::{A, Route, Router, Routes},
    hooks::{use_location, use_params},
    params::Params,
};

#[must_use]
pub fn shell(options: LeptosOptions) -> impl IntoView {
    // The shell is rendered on the server only, and the browser hydrates `<App />`
    // alone, so the favicon's versioned address doesn't have to reach the client.
    #[cfg(feature = "ssr")]
    let favicon = crate::gallery::favicon_url();
    #[cfg(not(feature = "ssr"))]
    let favicon = "/images/favicon.ico";

    view! {
        <!DOCTYPE html>
        <html lang="en">
            <head>
                <meta charset="utf-8" />
                <meta name="viewport" content="width=device-width, initial-scale=1" />
                <link rel="icon" type="image/x-icon" href=favicon />
                // Emits <link rel="stylesheet"> with the content-hashed CSS name
                // (reads hash.txt via LeptosOptions). Must live in the server shell
                // since LeptosOptions isn't available in the App body.
                <HashedStylesheet options=options.clone() id="leptos" />
                <AutoReload options=options.clone() />
                <HydrationScripts options />
                <MetaTags />
                <script>
                    r#"
                    document.addEventListener('DOMContentLoaded', function() {
                        document.addEventListener('contextmenu', function(e) {
                            if (e.target.tagName === 'IMG') {
                                e.preventDefault();
                                return false;
                            }
                        });
                    });
                    "#
                </script>
            </head>
            <body>
                <App />
            </body>
        </html>
    }
}

/// The site config, fetched once per visit and shared through context.
///
/// A component with its own resource fetches the config again every time it is
/// built, and shows its placeholder until that fetch comes back.
#[derive(Clone, Copy)]
struct SiteConfigResource(Resource<Result<SiteConfig, ServerFnError>>);

fn site_config() -> Resource<Result<SiteConfig, ServerFnError>> {
    expect_context::<SiteConfigResource>().0
}

/// Set by a page that loads new data without a change of route (one gallery to
/// another), so `<main>` is marked busy for it as it is for the router's own page
/// changes.
#[derive(Clone, Copy)]
struct PageLoading(WriteSignal<bool>);

/// How many photos the photo page has on screen, which the footer hides behind. A
/// count, not a flag, so it doesn't matter whether one photo's view goes before or
/// after the next one's comes.
///
/// A `signal`'s two halves rather than an `RwSignal`, as for `routing`'s count in
/// `App`: the bundle already has their code, and an `RwSignal` cost about 1 KB more
/// of the WASM budget.
#[derive(Clone, Copy)]
struct PhotosShown(ReadSignal<u32>, WriteSignal<u32>);

// Helper component for page title
#[component]
fn PageTitle() -> impl IntoView {
    let config = site_config();

    view! {
        <Suspense>
            {move || {
                // Fall back to SiteConfig::default() rather than a literal. There are two
                // ways to end up without a usable config — the load errored, or no
                // config.toml exists — and they used to produce different site names
                // ("Photography Portfolio" here vs. the default's "Your Name"), which is
                // confusing to debug and meant no single title held across both.
                let title = config
                    .get()
                    .and_then(std::result::Result::ok)
                    .unwrap_or_default()
                    .title();
                view! { <Title text=title /> }
            }}
        </Suspense>
    }
}

#[component]
#[must_use]
pub fn App() -> impl IntoView {
    provide_meta_context();
    provide_context(SiteConfigResource(Resource::new(
        || (),
        |()| async { get_site_config().await },
    )));
    // With a setter, the router runs a change to another route as a transition: the
    // old page stays until the resources the new page creates while it is being built
    // have resolved, so a page never shows up half-empty with the footer pulled up
    // into view. A param change within one route (one gallery to another) doesn't go
    // through this; GalleryPage handles that itself.
    //
    // The router reports the start and the end of each page change through the
    // setter. When a second click overtakes a page that is still loading, the router
    // reports the abandoned one's end straight away, while the second is still
    // loading. Counting starts against ends keeps `routing` true until the last one.
    let (unfinished, set_unfinished) = signal(0_u32);
    let routing = Memo::new(move |_| unfinished.get() > 0);
    let set_routing = SignalSetter::map(move |started: bool| {
        set_unfinished.update(|n| *n = if started { *n + 1 } else { n.saturating_sub(1) });
    });
    let (page_loading, set_page_loading) = signal(false);
    provide_context(PageLoading(set_page_loading));
    let (photos_shown, set_photos_shown) = signal(0);
    provide_context(PhotosShown(photos_shown, set_photos_shown));
    let loading = move || routing.get() || page_loading.get();

    view! {
        <PageTitle />
        <Router set_is_routing=set_routing>
            <Nav />
            // Nothing visible marks the wait: the old page simply stays until the new
            // one is ready. Screen readers are told the main area is busy meanwhile.
            <main class="main-content" aria-busy=move || loading().then_some("true")>
                <Routes fallback=|| view! { <NotFound /> }>
                    // Four pages can answer other than 200. The home, gallery and photo
                    // pages answer 503 while the images share is down; the gallery, photo
                    // and open-source pages answer 404 when there is no such page.
                    // leptos_axum sends the status with the first chunk of HTML, and
                    // streamed, that chunk can leave before the data has decided the
                    // status, so these render whole first (Async). PartiallyBlocked would
                    // not help: leptos_axum 0.8.10 streams it like OutOfOrder.
                    <Route path=StaticSegment("") view=HomePage ssr=SsrMode::Async />
                    <Route
                        path=(StaticSegment("gallery"), ParamSegment("name"))
                        view=GalleryPage
                        ssr=SsrMode::Async
                    />
                    <Route
                        path=(
                            StaticSegment("gallery"),
                            ParamSegment("gallery"),
                            ParamSegment("photo"),
                        )
                        view=PhotoDetailPage
                        ssr=SsrMode::Async
                    />
                    <Route path=StaticSegment("about") view=AboutPage />
                    <Route
                        path=StaticSegment("open-source")
                        view=OpenSourcePage
                        ssr=SsrMode::Async
                    />
                    <Route path=StaticSegment("contact") view=ContactPage />
                </Routes>
            </main>
            <Footer />
        </Router>
    }
}

// Helper component for gallery navigation links
#[component]
fn GalleryNavLinks() -> impl IntoView {
    let galleries = Resource::new(|| (), |()| async { get_galleries().await });

    view! {
        <Suspense>
            {move || {
                galleries
                    .get()
                    .and_then(std::result::Result::ok)
                    .map(|gallery_list| {
                        gallery_list
                            .into_iter()
                            .map(|gallery| {
                                view! {
                                    <li>
                                        <A href=format!(
                                            "/gallery/{}",
                                            gallery.slug,
                                        )>{gallery.name}</A>
                                    </li>
                                }
                            })
                            .collect_view()
                    })
            }}
        </Suspense>
    }
}

// Helper component for navigation links list
#[component]
fn NavLinksList() -> impl IntoView {
    view! {
        <ul class="nav-links">
            <GalleryNavLinks />
            <li>
                <A href="/about">"About"</A>
            </li>
            <li>
                <A href="/contact">"Contact"</A>
            </li>
        </ul>
    }
}

// Helper component for nav brand
#[component]
fn NavBrand() -> impl IntoView {
    let config = site_config();

    view! {
        <Suspense fallback=|| {
            view! { "Loading..." }
        }>
            {move || {
                let site_name = config
                    .get()
                    .and_then(std::result::Result::ok)
                    .map_or_else(|| "Your Name".to_string(), |cfg| cfg.site_name);

                view! {
                    <A href="/" attr:class="nav-brand">
                        {site_name}
                    </A>
                }
            }}
        </Suspense>
    }
}

#[component]
fn Nav() -> impl IntoView {
    view! {
        <nav class="navbar">
            <div class="nav-container">
                <NavBrand />
                <NavLinksList />
            </div>
        </nav>
    }
}

/// One footer for the whole visit, hidden rather than removed while the photo page
/// shows a photo, so a page change never rebuilds it.
///
/// It goes by what is on screen (`PhotosShown`), not by the address. A missing
/// photo's address looks like a photo page's but shows the not-found page, which
/// has the footer. And the address changes at the click, while the router keeps the
/// old page on screen until the new one has loaded; the count changes only when the
/// pages do.
///
/// The server writes the footer before the photo page's Suspense resolves, but the
/// count is in by then. The footer comes after `Routes` in `App`, that Suspense
/// builds its children once before it waits, and the photo list loads on its first
/// poll, because `get_all_gallery_photos` never waits (its doc comment says why it
/// mustn't). The browser keeps the server's `hidden` as it hydrates, without a
/// warning if it is wrong, so ssr_tests checks it there.
#[component]
fn Footer() -> impl IntoView {
    let config = site_config();
    let PhotosShown(shown, _) = expect_context();
    let hidden = move || shown.get() > 0;

    view! {
        <footer class="footer" hidden=hidden>
            <Suspense fallback=|| {
                view! { <p>"© 2025 Your Photography. All rights reserved."</p> }
            }>
                {move || {
                    let cfg = config.get().and_then(std::result::Result::ok);
                    let copyright = cfg
                        .as_ref()
                        .map_or_else(
                            || { "© 2025 Your Photography. All rights reserved.".to_string() },
                            SiteConfig::copyright,
                        );
                    let license_link = cfg
                        .as_ref()
                        .and_then(|c| c.license_footer_text().map(str::to_string));
                    view! {
                        <p>
                            {copyright}
                            {license_link
                                .map(|text| {
                                    view! {
                                        <span class="footer-license">
                                            // Decorative: the link's own text carries the
                                            // meaning, so a screen reader gains nothing
                                            // from announcing the separator.
                                            <span class="footer-separator" aria-hidden="true">
                                                " · "
                                            </span>
                                            // A plain anchor with rel="external", not <A>,
                                            // and the two are load-bearing together.
                                            //
                                            // leptos_router intercepts *every* same-origin
                                            // anchor click, then calls scroll_to_el, which
                                            // looks up the fragment's element and falls back
                                            // to scrolling to the top when it is missing. On
                                            // a cross-page hash link the target page's DOM
                                            // does not exist yet at that moment, so the
                                            // lookup fails and the reader lands at the top of
                                            // /about instead of at the license section.
                                            //
                                            // rel="external" is the router's documented
                                            // escape hatch (see location/mod.rs): it hands the
                                            // click back to the browser, whose native fragment
                                            // handling runs after the document has loaded and
                                            // therefore finds the element. Costs a full page
                                            // load for this one link.
                                            <a href="/about#license" rel="external">
                                                {text}
                                            </a>
                                        </span>
                                    }
                                })}
                        </p>
                    }
                }}
            </Suspense>
        </footer>
    }
}

// Helper function to determine orientation class from dimensions
fn orientation_class_from_dimensions(width: Option<u32>, height: Option<u32>) -> &'static str {
    if let (Some(w), Some(h)) = (width, height) {
        let ratio = f64::from(w) / f64::from(h);
        if ratio > 2.5 {
            "wide-landscape" // Ultra-wide: 3 columns x 1 row
        } else if ratio > 1.8 {
            "landscape" // Wide: 2 columns x 1 row
        } else if ratio > 1.3 {
            "landscape-square" // Standard landscape: 2 columns x 1 row
        } else if ratio > 1.05 {
            "square" // Square-ish: 1 column x 1 row
        } else if ratio > 0.75 {
            "portrait-square" // Near square portrait: 1 column x 1 row
        } else if ratio > 0.55 {
            "portrait" // Standard portrait: 1 column x 2 rows
        } else {
            "tall-portrait" // Tall portrait: 1 column x 3 rows
        }
    } else {
        "square"
    }
}

// Helper component for mosaic photo grid item
#[component]
fn MosaicPhotoGridItem(
    photo: PhotoInfo,
    cell: crate::types::MosaicCell,
    #[prop(optional)] index: Option<usize>,
) -> impl IntoView {
    let photo_slug = photo.slug.clone();
    let photo_gallery = photo.gallery_name.clone();
    let photo_url = photo.url.clone();
    let photo_srcset = photo.srcset.clone();
    let photo_title = photo.title.clone();
    let focal_point = photo.focal_point.clone();

    let grid_style = format!(
        "grid-row: {} / {}; grid-column: {} / {};",
        cell.row_start, cell.row_end, cell.col_start, cell.col_end
    );

    // Build image style with focal point if available
    let img_style = focal_point
        .as_ref()
        .map_or_default(|fp| format!("object-position: {};", fp.to_css_position()));

    // First 3 images get high priority
    let is_priority = index.is_none_or(|idx| idx < 3);

    view! {
        <a
            href=format!("/gallery/{}/{}", photo_gallery, photo_slug)
            class="photo-hero-link mosaic-cell"
            style=grid_style
        >
            <div class="photo-hero-section">
                <div class="photo-hero-image">
                    <img
                        src=photo_url
                        srcset=photo_srcset
                        sizes="(max-width: 767px) 100vw, (max-width: 1199px) 50vw, 33vw"
                        alt=photo_title.clone()
                        style=img_style
                        width=photo.width.unwrap_or(1200)
                        height=photo.height.unwrap_or(800)
                        loading=move || if is_priority { "eager" } else { "lazy" }
                        fetchpriority=move || if is_priority { "high" } else { "auto" }
                        decoding="async"
                    />
                </div>
                <div class="photo-hero-caption">
                    <h2>{photo_title}</h2>
                </div>
            </div>
        </a>
    }
}

// Helper component for mobile single-column photo display
#[component]
fn MobilePhotoItem(photo: PhotoInfo) -> impl IntoView {
    let photo_slug = photo.slug.clone();
    let photo_gallery = photo.gallery_name.clone();
    let photo_url = photo.url.clone();
    let photo_srcset = photo.srcset.clone();
    let photo_title = photo.title.clone();

    view! {
        <a href=format!("/gallery/{}/{}", photo_gallery, photo_slug) class="photo-mobile-item">
            <img
                src=photo_url
                srcset=photo_srcset
                sizes="(max-width: 767px) 100vw, 50vw"
                alt=photo_title
                width=photo.width.unwrap_or(1200)
                height=photo.height.unwrap_or(800)
                loading="lazy"
                decoding="async"
            />
        </a>
    }
}

// Helper component for photo grid item
#[component]
fn PhotoGridItem(photo: PhotoInfo, #[prop(optional)] index: Option<usize>) -> impl IntoView {
    let photo_slug = photo.slug.clone();
    let photo_gallery = photo.gallery_name.clone();
    let photo_url = photo.url.clone();
    let photo_srcset = photo.srcset.clone();
    let photo_title = photo.title.clone();
    let base_orientation = orientation_class_from_dimensions(photo.width, photo.height);

    // For square photos, vary the size based on index to create visual interest
    // Using a more aggressive pattern to fill gaps better
    let orientation_class = if let Some(idx) = index {
        match base_orientation {
            "square" => {
                // More aggressive sizing:
                // - Every 4th becomes xlarge (25%)
                // - Every 2nd (not xlarge) becomes large (37.5%)
                // - Remaining stay small (37.5%)
                if idx % 4 == 0 {
                    "square-xlarge"
                } else if idx % 2 == 1 {
                    "square-large"
                } else {
                    base_orientation
                }
            }
            "portrait-square" => {
                // Every other portrait-square becomes large (50%)
                if idx % 2 == 0 {
                    "portrait-square-large"
                } else {
                    base_orientation
                }
            }
            _ => base_orientation,
        }
    } else {
        base_orientation
    };

    view! {
        <a
            href=format!("/gallery/{}/{}", photo_gallery, photo_slug)
            class=format!("photo-hero-link {}", orientation_class)
        >
            <div class="photo-hero-section">
                <div class="photo-hero-image">
                    <img
                        src=photo_url
                        srcset=photo_srcset
                        sizes="(max-width: 767px) 100vw, (max-width: 1199px) 50vw, 33vw"
                        alt=photo_title.clone()
                        width=photo.width.unwrap_or(1200)
                        height=photo.height.unwrap_or(800)
                        loading="lazy"
                        decoding="async"
                    />
                </div>
                <div class="photo-hero-caption">
                    <h2>{photo_title}</h2>
                </div>
            </div>
        </a>
    }
}

// Helper component for photo grid display
#[component]
fn PhotoGrid(
    photos: Vec<PhotoInfo>,
    #[prop(optional)] config: Option<crate::types::GalleryConfig>,
    #[prop(optional)] mosaic_layout: Option<crate::types::MosaicLayout>,
) -> impl IntoView {
    if photos.is_empty() {
        view! {
            <div class="empty-gallery">
                <p>"No photos found."</p>
                <p class="hint">
                    "Add photos to " <code>"public/images/home/"</code> " to see them here."
                </p>
            </div>
        }
        .into_any()
    } else {
        // Check if we have a pre-computed mosaic layout
        if let Some(layout) = mosaic_layout {
            // Use pre-computed mosaic layout (already has photos in correct order)
            let gap = config.as_ref().and_then(|cfg| cfg.gap).unwrap_or(8);

            // Desktop layout
            let grid_style_desktop = format!(
                "grid-template-columns: repeat({}, 1fr); grid-template-rows: repeat({}, 1fr); height: {}px; gap: {}px;",
                layout.grid_cols, layout.grid_rows, layout.container_height, gap
            );

            // The list below 1200px shows every photo too
            let photos_mobile = photos.clone();

            view! {
                // Desktop mosaic (1200px and wider)
                <div class="photo-grid-mosaic photo-grid-mosaic-desktop" style=grid_style_desktop>
                    {layout
                        .cells
                        .into_iter()
                        .zip(photos.into_iter())
                        .enumerate()
                        .map(|(idx, (cell, photo))| {
                            view! { <MosaicPhotoGridItem photo=photo cell=cell index=idx /> }
                        })
                        .collect_view()}
                </div>

                // Below 1200px: the uncropped list, two columns on a tablet and one on
                // a phone (style/main.scss)
                <div class="photo-grid-mobile">
                    {photos_mobile
                        .into_iter()
                        .map(|photo| view! { <MobilePhotoItem photo=photo /> })
                        .collect_view()}
                </div>
            }
            .into_any()
        } else {
            // Use traditional grid layout
            let grid_style = if let Some(cfg) = config {
                let columns = cfg.columns.unwrap_or(6);
                let row_height = cfg.row_height.unwrap_or(280);
                let gap = cfg.gap.unwrap_or(8);
                format!(
                    "--grid-columns: {columns}; --grid-row-height: {row_height}px; --grid-gap: {gap}px;"
                )
            } else {
                String::new()
            };

            let has_custom_style = !grid_style.is_empty();

            view! {
                <div class="photo-grid-home" class:custom-grid=has_custom_style style=grid_style>
                    {photos
                        .into_iter()
                        .enumerate()
                        .map(|(idx, photo)| view! { <PhotoGridItem photo=photo index=idx /> })
                        .collect_view()}
                </div>
            }
            .into_any()
        }
    }
}

// Helper component for hero section
#[component]
fn HeroSection() -> impl IntoView {
    let config = site_config();

    view! {
        <div class="hero-simple">
            <div class="hero-text">
                <Suspense fallback=|| {
                    view! {
                        <h1>"YOUR NAME"</h1>
                        <p class="hero-tagline">"Photography"</p>
                    }
                }>
                    {move || {
                        let (name, tagline) = config
                            .get()
                            .and_then(std::result::Result::ok)
                            .map_or_else(
                                || ("YOUR NAME".to_string(), "Photography".to_string()),
                                |cfg| (cfg.site_name.to_uppercase(), cfg.site_tagline),
                            );

                        view! {
                            <h1>{name}</h1>
                            <p class="hero-tagline">{tagline}</p>
                        }
                    }}
                </Suspense>
            </div>
        </div>
    }
}

// Helper component to render PhotoGrid with optional config and layout
#[component]
fn PhotoGridRenderer(
    photos: Vec<PhotoInfo>,
    config: Option<crate::types::GalleryConfig>,
    mosaic_layout: Option<crate::types::MosaicLayout>,
) -> impl IntoView {
    match (config, mosaic_layout) {
        (Some(cfg), Some(mosaic)) => {
            view! { <PhotoGrid photos=photos config=cfg mosaic_layout=mosaic /> }.into_any()
        }
        (None, Some(mosaic)) => {
            view! { <PhotoGrid photos=photos mosaic_layout=mosaic /> }.into_any()
        }
        (Some(cfg), None) => view! { <PhotoGrid photos=photos config=cfg /> }.into_any(),
        (None, None) => view! { <PhotoGrid photos=photos /> }.into_any(),
    }
}

// Helper component for photo grid loading
#[component]
fn PhotoGridLoader() -> impl IntoView {
    let gallery_data = Resource::new(|| (), |()| async { get_home_gallery_data().await });
    let home_config = Resource::new(|| (), |()| async { get_home_gallery_config().await });

    view! {
        <Suspense fallback=|| {
            view! { <div class="loading">"Loading photos..."</div> }
        }>
            {move || {
                let gallery_config = home_config.get().and_then(std::result::Result::ok).flatten();
                let Some(data) = gallery_data.get().and_then(std::result::Result::ok) else {
                    #[cfg(feature = "ssr")]
                    if load_failed(gallery_data.get_untracked().as_ref()) {
                        mark_unavailable();
                    }
                    return view! { <div class="error">"Failed to load photos"</div> }.into_any();
                };
                let data = std::sync::Arc::unwrap_or_clone(data);
                view! {
                    <PhotoGridRenderer
                        photos=data.photos
                        config=gallery_config
                        mosaic_layout=data.mosaic_layout
                    />
                }
                    .into_any()
            }}
        </Suspense>
    }
}

#[component]
fn HomePage() -> impl IntoView {
    view! {
        <div class="home-page">
            <HeroSection />
            <PhotoGridLoader />
        </div>
    }
}

#[derive(Params, PartialEq, Clone)]
struct GalleryParams {
    name: String,
}

// Helper component for gallery hero
#[component]
fn GalleryHero(gallery_name: String) -> impl IntoView {
    let display_name = normalize_display_key(&gallery_name.replace(['-', '_'], " "));

    view! {
        <div class="hero-simple">
            <div class="hero-text">
                <h1>{display_name.to_uppercase()}</h1>
                <p class="hero-tagline">
                    <A href="/">"← Back to Home"</A>
                </p>
            </div>
        </div>
    }
}

/// A `Transition` that reports through `set_pending` in the browser only.
///
/// Given `set_pending`, leptos calls it from an effect, and runs that effect on the
/// server too, on a worker thread of its own, where nothing it sets reaches the HTML.
/// There it races the render for the `Transition`'s locks. With `reactive_graph`
/// 0.2.14 the two can deadlock: the page never answers, and both threads stay
/// blocked for good. Even where that is fixed (0.2.15), the effect panics now and
/// then with "Tried to access a reactive value that has already been disposed" when
/// a read of its own finds a lock taken.
#[component]
fn PageTransition<Chil>(
    #[prop(into)] fallback: ViewFnOnce,
    #[prop(into)] set_pending: SignalSetter<bool>,
    children: TypedChildren<Chil>,
) -> impl IntoView
where
    Chil: IntoView + Send + 'static,
{
    let set_pending = (!cfg!(feature = "ssr")).then_some(set_pending);
    Transition(TransitionProps {
        fallback,
        set_pending,
        children,
    })
}

/// A gallery page. Its resources are created here, at the page's top level, keyed
/// on the route's `name`, for two reasons:
///
/// - The router's transition waits only for resources created while the page is
///   being built. One created inside a component that a closure builds later isn't
///   waited for, so the page would appear half-empty.
/// - Moving from one gallery to another changes only the param, and the page stays
///   mounted. Data keyed on the param is fetched again; a child component handed the
///   name once kept the old gallery's photos under the new title.
///
/// The resource hands back the name it fetched with the data, and the page shows
/// that name rather than the param's. While the next gallery loads, the old title
/// and the old photos stay together.
#[component]
fn GalleryPage() -> impl IntoView {
    let params = use_params::<GalleryParams>();
    let gallery = Resource::new(
        move || params.get().ok().map(|p| p.name),
        |name| async move {
            let data = match name.clone() {
                Some(n) => get_gallery_data_by_name(n).await,
                None => Err(ServerFnError::new("No gallery name provided")),
            };
            (name, data)
        },
    );
    let galleries = Resource::new(|| (), |()| async { get_galleries().await });
    // The Transition reports while it waits, which marks `<main>` busy. A page that
    // goes away mid-load never reports the end, so clear it on the way out.
    let PageLoading(set_loading) = expect_context();
    on_cleanup(move || set_loading.set(false));

    // A Transition, not a Suspense: moving between galleries refetches without
    // remounting, and the old gallery stays on screen until the new one is ready.
    // The `.home-page` wrapper sits inside the arms, so the not-found page is never
    // wrapped in a gallery's page.
    view! {
        <PageTransition set_pending=set_loading fallback=|| {
            view! {
                <div class="home-page">
                    <div class="hero-simple">
                        <div class="hero-text">
                            <h1>"GALLERY"</h1>
                            <p class="hero-tagline">"Loading..."</p>
                        </div>
                    </div>
                </div>
            }
        }>
            {move || {
                let (name, data) = gallery.get()?;
                // No name, or no gallery of that name: the not-found page, with 404.
                let (name, data) = match (name, data) {
                    (Some(name), Ok(Some(data))) => (name, Ok(data)),
                    (Some(name), Err(err)) => (name, Err(err)),
                    (None, _) | (_, Ok(None)) => {
                        return Some(view! { <NotFound /> }.into_any());
                    }
                };
                let slug = name.to_lowercase().replace(' ', "-");
                let gallery_config = galleries
                    .get()
                    .and_then(std::result::Result::ok)
                    .and_then(|gallery_list| {
                        gallery_list
                            .into_iter()
                            .find(|g| g.slug == slug)
                            .and_then(|g| g.config)
                    });
                let photos = match data {
                    Ok(data) if !data.photos.is_empty() => {
                        let data = std::sync::Arc::unwrap_or_clone(data);
                        view! {
                            <PhotoGridRenderer
                                photos=data.photos
                                config=gallery_config
                                mosaic_layout=data.mosaic_layout
                            />
                        }
                            .into_any()
                    }
                    Ok(_) => {
                        view! {
                            <div class="empty-gallery">
                                <p>"No photos found in this gallery."</p>
                            </div>
                        }
                            .into_any()
                    }
                    // A failed request, not a missing gallery.
                    Err(_) => {
                        #[cfg(feature = "ssr")]
                        mark_unavailable();
                        view! { <div class="error">"Failed to load gallery"</div> }.into_any()
                    }
                };
                Some(
                    view! {
                        <div class="home-page">
                            <GalleryHero gallery_name=name />
                            {photos}
                        </div>
                    }
                        .into_any(),
                )
            }}
        </PageTransition>
    }
}

#[derive(Params, PartialEq, Clone)]
struct PhotoParams {
    gallery: String,
    photo: String,
}

/// Strip surrounding quotes from a string
fn strip_quotes(s: &str) -> String {
    s.trim_matches('"').to_string()
}

/// Format aperture value by replacing 'f' with hooked f (ƒ)
fn format_aperture(aperture: &str) -> String {
    aperture.replace("f/", "ƒ/")
}

/// Convert uppercase strings to Title Case, leave mixed case unchanged
/// Preserves acronyms (2-4 letter words) and words with numbers
fn to_title_case_if_uppercase(s: &str) -> String {
    // Check if the string is entirely uppercase (ignoring whitespace and punctuation)
    let has_letters = s.chars().any(char::is_alphabetic);
    let all_uppercase = s
        .chars()
        .filter(|c| c.is_alphabetic())
        .all(char::is_uppercase);

    if has_letters && all_uppercase {
        // Convert to smart title case
        s.split_whitespace()
            .map(|word| {
                let letter_count = word.chars().filter(|c| c.is_alphabetic()).count();
                let has_digit = word.chars().any(char::is_numeric);

                // Keep acronyms (2-4 letters) and words with numbers in uppercase
                if (2..=4).contains(&letter_count) || has_digit {
                    word.to_string()
                } else {
                    // Convert longer words to title case
                    let mut chars = word.chars();
                    match chars.next() {
                        None => String::new(),
                        Some(first) => {
                            first.to_uppercase().collect::<String>()
                                + &chars.as_str().to_lowercase()
                        }
                    }
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
    } else {
        s.to_string()
    }
}

// Helper component for EXIF field display
#[component]
fn ExifField(heading: &'static str, value: String) -> impl IntoView {
    let cleaned_value = strip_quotes(&value);
    view! {
        <div class="exif-section">
            <h3 class="exif-heading">{heading}</h3>
            <p class="exif-value">{cleaned_value}</p>
        </div>
    }
}

// Helper component for camera info
#[component]
fn CameraInfo(camera_make: Option<String>, camera_model: Option<String>) -> impl IntoView {
    if camera_make.is_none() && camera_model.is_none() {
        return view! { <div></div> }.into_any();
    }

    let make_cleaned = camera_make.map(|s| to_title_case_if_uppercase(&strip_quotes(&s)));
    let model_cleaned = camera_model.map(|s| to_title_case_if_uppercase(&strip_quotes(&s)));

    view! {
        <div class="exif-section">
            <h3 class="exif-heading">"Camera"</h3>
            {match (make_cleaned, model_cleaned) {
                (Some(make), Some(model)) => {
                    view! { <p class="exif-value">{format!("{make}, {model}")}</p> }.into_any()
                }
                (None, Some(model)) => view! { <p class="exif-value">{model}</p> }.into_any(),
                (Some(make), None) => view! { <p class="exif-value">{make}</p> }.into_any(),
                (None, None) => view! { <div></div> }.into_any(),
            }}
        </div>
    }
    .into_any()
}

// Helper component for photo settings
#[component]
fn PhotoSettings(
    focal_length: Option<String>,
    aperture: Option<String>,
    shutter_speed: Option<String>,
    iso: Option<String>,
) -> impl IntoView {
    if focal_length.is_none() && aperture.is_none() && shutter_speed.is_none() && iso.is_none() {
        return view! { <div></div> }.into_any();
    }

    let focal_cleaned = focal_length.map(|s| strip_quotes(&s));
    let aperture_formatted = aperture.map(|s| format_aperture(&strip_quotes(&s)));
    let shutter_cleaned = shutter_speed.map(|s| strip_quotes(&s));
    let iso_cleaned = iso.map(|s| strip_quotes(&s));

    view! {
        <div class="exif-section">
            <h3 class="exif-heading">"Settings"</h3>
            <div class="exif-settings">
                {focal_cleaned.map(|fl| view! { <span class="exif-setting">{fl}</span> })}
                {aperture_formatted.map(|ap| view! { <span class="exif-setting">{ap}</span> })}
                {shutter_cleaned.map(|ss| view! { <span class="exif-setting">{ss}</span> })}
                {iso_cleaned.map(|iso_val| view! { <span class="exif-setting">{iso_val}</span> })}
            </div>
        </div>
    }
    .into_any()
}

// Helper component for copyright footer
#[component]
fn CopyrightFooter() -> impl IntoView {
    let config = site_config();

    view! {
        <Suspense fallback=|| {
            view! { <p>"© 2025 All rights reserved."</p> }
        }>
            {move || {
                let copyright = config
                    .get()
                    .and_then(std::result::Result::ok)
                    .map_or_else(
                        || "© 2025 All rights reserved.".to_string(),
                        |cfg| cfg.copyright(),
                    );
                view! { <p>{copyright}</p> }
            }}
        </Suspense>
    }
}

/// True only when both dimensions are known and the photo is taller than wide.
fn is_portrait(width: Option<u32>, height: Option<u32>) -> bool {
    matches!((width, height), (Some(w), Some(h)) if h > w)
}

/// Marks the server's response 503 Service Unavailable, for a page whose data
/// could not be loaded, as when the images share is down. A 200 would let search
/// engines index the error message in the page's place; a 503 tells them the
/// page is still there and to come back later.
///
/// The status goes out with the first chunk of HTML, so like `NotFound`'s 404 it
/// relies on the route rendering whole first (`SsrMode::Async`). A visit within
/// the app has no status to set.
#[cfg(feature = "ssr")]
fn mark_unavailable() {
    use axum::http::{HeaderValue, StatusCode, header::RETRY_AFTER};

    if let Some(response) = use_context::<leptos_axum::ResponseOptions>() {
        response.set_status(StatusCode::SERVICE_UNAVAILABLE);
        // Five minutes, about a NAS reboot or remount; the failure isn't cached.
        response.insert_header(RETRY_AFTER, HeaderValue::from_static("300"));
    }
}

/// Whether a resource's value is a failed load, the only case that marks a page
/// unavailable. A Suspense also renders a page's error branch while the data is
/// still loading (`None`), and that is no outage.
#[cfg(feature = "ssr")]
fn load_failed<T, E>(value: Option<&Result<T, E>>) -> bool {
    matches!(value, Some(Err(_)))
}

/// The one page for every not-found case: an unknown address, a missing gallery
/// or photo, and `/open-source` without a usable file.
///
/// On the server it sets 404 while it renders. That status reaches the client
/// only because the routes that can end here render with `SsrMode::Async`, or,
/// for an unknown address, because the router fallback renders the whole page
/// before it answers.
///
/// It keeps the site's own `<title>`. A page-specific one was dropped: leptos_meta
/// lost it when the router rebuilt the fallback in place (one dead address to
/// another), and it cost about 1 KB of the WASM budget.
#[component]
fn NotFound() -> impl IntoView {
    #[cfg(feature = "ssr")]
    if let Some(response) = use_context::<leptos_axum::ResponseOptions>() {
        response.set_status(axum::http::StatusCode::NOT_FOUND);
    }
    let location = use_location();
    let photo = Resource::new(|| (), |()| async { random_home_photo().await });

    view! {
        <div class="not-found-page">
            // No fallback: until the photo arrives, or if there is none, the message
            // stands alone.
            <Suspense>
                {move || {
                    photo
                        .get()
                        .and_then(std::result::Result::ok)
                        .flatten()
                        .map(|p| view! { <NotFoundPhoto photo=p /> })
                }}
            </Suspense>
            <div class="not-found-text">
                <p class="not-found-label">"404"</p>
                <h1>"Page not found"</h1>
                <p class="not-found-message">
                    "There's nothing at " <code class="not-found-path">
                        {move || location.pathname.get()}
                    </code> ". The link may be old, or the address mistyped."
                </p>
                <p class="not-found-back">
                    <A href="/">"Back to the galleries"</A>
                </p>
            </div>
        </div>
    }
}

/// The not-found page's photo, linking to its own page, with its title as the
/// caption.
#[component]
fn NotFoundPhoto(photo: PhotoInfo) -> impl IntoView {
    let class = if is_portrait(photo.width, photo.height) {
        "not-found-photo portrait"
    } else {
        "not-found-photo"
    };
    // The link holds only the image, so the alt text is the link's name; a photo
    // whose filename leaves no title still needs one.
    let alt = if photo.title.is_empty() {
        "A photo from the home gallery".to_string()
    } else {
        photo.title.clone()
    };
    view! {
        <figure class=class>
            <a href=format!("/gallery/{}/{}", photo.gallery_name, photo.slug)>
                <img
                    src=photo.url
                    srcset=photo.srcset
                    sizes="(max-width: 768px) 100vw, 460px"
                    alt=alt
                    width=photo.width.unwrap_or(1200)
                    height=photo.height.unwrap_or(800)
                    decoding="async"
                />
            </a>
            <figcaption>{photo.title}</figcaption>
        </figure>
    }
}

/// Bundle of signals shared between the zoom/pan event handlers and the
/// fullscreen view. Held by value (`RwSignal` is Copy).
#[derive(Clone, Copy)]
struct ZoomPanState {
    zoom_level: RwSignal<f64>,
    max_zoom: RwSignal<f64>,
    pan_x: RwSignal<f64>,
    pan_y: RwSignal<f64>,
    is_panning: RwSignal<bool>,
    start_x: RwSignal<f64>,
    start_y: RwSignal<f64>,
    show_zoom_controls: RwSignal<bool>,
    is_fullscreen: RwSignal<bool>,
    // Pinch-zoom anchors (only used in hydrate).
    initial_pinch_distance: RwSignal<f64>,
    initial_zoom: RwSignal<f64>,
    initial_pinch_center_x: RwSignal<f64>,
    initial_pinch_center_y: RwSignal<f64>,
    initial_pan_x: RwSignal<f64>,
    initial_pan_y: RwSignal<f64>,
}

impl ZoomPanState {
    fn new() -> Self {
        Self {
            zoom_level: RwSignal::new(1.0),
            max_zoom: RwSignal::new(10.0),
            pan_x: RwSignal::new(0.0),
            pan_y: RwSignal::new(0.0),
            is_panning: RwSignal::new(false),
            start_x: RwSignal::new(0.0),
            start_y: RwSignal::new(0.0),
            show_zoom_controls: RwSignal::new(true),
            is_fullscreen: RwSignal::new(false),
            initial_pinch_distance: RwSignal::new(0.0),
            initial_zoom: RwSignal::new(1.0),
            initial_pinch_center_x: RwSignal::new(0.0),
            initial_pinch_center_y: RwSignal::new(0.0),
            initial_pan_x: RwSignal::new(0.0),
            initial_pan_y: RwSignal::new(0.0),
        }
    }

    fn reset_zoom(&self) {
        self.zoom_level.set(1.0);
        self.pan_x.set(0.0);
        self.pan_y.set(0.0);
    }
}

/// EXIF panel rendered next to the photo title. Pure presentation; the
/// outer page only needs to pass the photo and an expand/collapse signal.
#[component]
fn PhotoExifPanel(photo: PhotoInfo, is_expanded: RwSignal<bool>) -> impl IntoView {
    let dimensions_view = match (photo.width, photo.height) {
        (Some(w), Some(h)) => {
            view! { <ExifField heading="Dimensions" value=format!("{w} × {h} px") /> }.into_any()
        }
        _ => view! { <div></div> }.into_any(),
    };

    view! {
        <div class="photo-exif" class:expanded=move || is_expanded.get()>
            {photo
                .date_taken
                .as_ref()
                .map(|date| view! { <ExifField heading="Date" value=date.clone() /> })}
            {dimensions_view}
            <CameraInfo
                camera_make=photo.camera_make.clone()
                camera_model=photo.camera_model.clone()
            />
            {photo
                .lens_model
                .as_ref()
                .map(|lens| {
                    let formatted = format_aperture(&strip_quotes(lens));
                    view! { <ExifField heading="Lens" value=formatted /> }
                })}
            {photo
                .film_stock
                .as_ref()
                .map(|film| view! { <ExifField heading="Film Stock" value=film.clone() /> })}
            <PhotoSettings
                focal_length=photo.focal_length.clone()
                aperture=photo.aperture.clone()
                shutter_speed=photo.shutter_speed.clone()
                iso=photo.iso.clone()
            />
            {photo
                .copyright
                .as_ref()
                .map(|c| view! { <ExifField heading="Copyright" value=c.clone() /> })}
        </div>
    }
}

/// Prev/Next navigation row with the copyright sandwiched between, so that
/// links collapse cleanly when there is no neighbour in either direction.
#[component]
fn PhotoNavigationButtons(
    prev_photo: Option<PhotoInfo>,
    next_photo: Option<PhotoInfo>,
) -> impl IntoView {
    view! {
        <div class="photo-navigation">
            {prev_photo
                .map(|prev| {
                    view! {
                        <A
                            href=format!("/gallery/{}/{}", prev.gallery_name, prev.slug)
                            attr:class="nav-button nav-prev"
                        >
                            "← Previous"
                        </A>
                    }
                })} <div class="photo-nav-copyright">
                <CopyrightFooter />
            </div>
            {next_photo
                .map(|next| {
                    view! {
                        <A
                            href=format!("/gallery/{}/{}", next.gallery_name, next.slug)
                            attr:class="nav-button nav-next"
                        >
                            "Next →"
                        </A>
                    }
                })}
        </div>
    }
}

/// Focuses the open viewer's control matching `selector`. Only one viewer is open
/// at a time, so the first match is its own.
#[cfg(feature = "hydrate")]
fn focus_viewer_control(selector: &str) {
    if let Ok(Some(el)) = document().query_selector(selector) {
        let _ = el.unchecked_into::<web_sys::HtmlElement>().focus();
    }
}

/// Fullscreen image viewer with zoom/pan/pinch controls, used by the photo page
/// and by screenshots on `/open-source` cards. Owns all the browser-event
/// closures so its parent doesn't have to thread state through ~14 separate
/// handlers.
#[component]
fn FullscreenViewer(
    image: ViewerImage,
    state: ZoomPanState,
    viewport_width: RwSignal<f64>,
) -> impl IntoView {
    // Calibrate max zoom for very-high-resolution photos.
    let calculated_max_zoom = match (image.width, image.height) {
        (Some(w), Some(h)) if w > 8000 || h > 8000 => 20.0,
        _ => 10.0,
    };
    state.max_zoom.set(calculated_max_zoom);

    let is_mobile = move || viewport_width.get() <= 768.0 && viewport_width.get() > 0.0;
    let photo_url_detail = StoredValue::new(image.detail_url);
    let photo_url_original = StoredValue::new(image.original_url);
    let photo_sources_original = StoredValue::new(image.original_sources);
    let photo_title_fs = image.alt;
    let photo_width = image.width;
    let photo_height = image.height;

    let _hide_controls_timeout: StoredValue<Option<i32>> = StoredValue::new(None);

    let zoom_level = state.zoom_level;
    let max_zoom = state.max_zoom;
    let pan_x = state.pan_x;
    let pan_y = state.pan_y;
    let is_panning = state.is_panning;
    let start_x = state.start_x;
    let start_y = state.start_y;
    let show_zoom_controls = state.show_zoom_controls;
    let is_fullscreen = state.is_fullscreen;
    // Pinch-zoom anchors. Only read inside the `cfg(feature = "hydrate")`
    // blocks below; the underscore prefix mirrors the SSR-side suppression
    // already used in this module.
    let _initial_pinch_distance = state.initial_pinch_distance;
    let _initial_zoom = state.initial_zoom;
    let _initial_pinch_center_x = state.initial_pinch_center_x;
    let _initial_pinch_center_y = state.initial_pinch_center_y;
    let _initial_pan_x = state.initial_pan_x;
    let _initial_pan_y = state.initial_pan_y;

    let reset_hide_timer = move || {
        show_zoom_controls.set(true);
        #[cfg(feature = "hydrate")]
        {
            if let Some(timeout_id) = _hide_controls_timeout.get_value()
                && let Some(window) = web_sys::window()
            {
                window.clear_timeout_with_handle(timeout_id);
            }
            if let Some(window) = web_sys::window() {
                let timeout_id = window
                    .set_timeout_with_callback_and_timeout_and_arguments_0(
                        leptos::wasm_bindgen::closure::Closure::once(move || {
                            show_zoom_controls.set(false);
                        })
                        .into_js_value()
                        .unchecked_ref(),
                        1000,
                    )
                    .ok();
                _hide_controls_timeout.set_value(timeout_id);
            }
        }
    };

    // Where focus was when the viewer opened (a screenshot's button), to hand it
    // back on close. Browser only: local storage is tied to its thread, and the
    // server can drop a rendered page on another one.
    #[cfg(feature = "hydrate")]
    let opener = StoredValue::new_local(None::<web_sys::HtmlElement>);

    // When the user enters fullscreen, kick the auto-hide timer so controls
    // briefly show then fade — mirroring the previous behaviour — and move focus
    // to the ✕. Effects run straight after rendering, so the overlay is in the
    // page, and no key press can land between the two.
    #[cfg(feature = "hydrate")]
    {
        use leptos::prelude::Effect;
        Effect::new(move |_| {
            if is_fullscreen.get() {
                reset_hide_timer();
                opener.set_value(document().active_element().and_then(|e| e.dyn_into().ok()));
                focus_viewer_control(".fullscreen-close");
            }
        });
    }

    let exit_fullscreen = move || {
        is_fullscreen.set(false);
        state.reset_zoom();
        show_zoom_controls.set(true);
        #[cfg(feature = "hydrate")]
        opener.with_value(|el| el.as_ref().map(web_sys::HtmlElement::focus));
    };

    // A modal dialog's keys: Escape closes it, Tab stays on its two controls,
    // and nothing scrolls the page behind it.
    #[cfg(feature = "hydrate")]
    {
        let keys = window_event_listener(leptos::ev::keydown, move |ev| {
            if !is_fullscreen.get_untracked() {
                return;
            }
            let key = ev.key();
            let target = ev.target();
            let on_slider = target
                .as_ref()
                .is_some_and(JsCast::has_type::<web_sys::HtmlInputElement>);
            let on_close = target
                .as_ref()
                .is_some_and(JsCast::has_type::<web_sys::HtmlButtonElement>);
            match key.as_str() {
                "Escape" => exit_fullscreen(),
                "Tab" => {
                    ev.prevent_default();
                    // Either way round, the ✕ and the slider take turns. From anywhere
                    // else (a click on the image leaves focus on the page), Tab picks
                    // the ✕ and Shift+Tab the slider.
                    let to_slider = if ev.shift_key() { !on_slider } else { on_close };
                    focus_viewer_control(if to_slider {
                        ".zoom-slider"
                    } else {
                        ".fullscreen-close"
                    });
                }
                " " | "PageUp" | "PageDown" | "Home" | "End" | "ArrowUp" | "ArrowDown"
                | "ArrowLeft" | "ArrowRight" => {
                    // Alt or ⌘ with Left/Right is the browser's Back/Forward. The
                    // slider keeps its own keys, unless Ctrl or ⌘ makes them page
                    // scrolls, and Space presses the ✕.
                    let history = (ev.alt_key() || ev.meta_key())
                        && matches!(key.as_str(), "ArrowLeft" | "ArrowRight");
                    let own = history
                        || if key == " " {
                            on_close
                        } else {
                            on_slider && !ev.ctrl_key() && !ev.meta_key()
                        };
                    if !own {
                        ev.prevent_default();
                    }
                }
                _ => {}
            }
        });
        on_cleanup(move || keys.remove());
    }

    let close_fullscreen = move |ev: leptos::ev::MouseEvent| {
        // Only close when clicking the backdrop, not the image itself.
        let target = ev.target();
        if let Some(element) =
            target.and_then(|t: web_sys::EventTarget| t.dyn_into::<web_sys::Element>().ok())
            && element.class_name().contains("fullscreen-overlay")
        {
            exit_fullscreen();
        }
    };

    let on_close_button = move |_ev: leptos::ev::MouseEvent| {
        exit_fullscreen();
    };

    let on_zoom_change = move |ev: leptos::ev::Event| {
        ev.stop_propagation();
        let new_zoom = ev
            .target()
            .and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok())
            .and_then(|input| input.value().parse::<f64>().ok())
            .unwrap_or(1.0);
        zoom_level.set(new_zoom);
        if (new_zoom - 1.0).abs() < 0.01 {
            pan_x.set(0.0);
            pan_y.set(0.0);
        }
        reset_hide_timer();
    };

    let on_image_click = move |ev: leptos::ev::MouseEvent| {
        ev.stop_propagation();
    };

    let on_image_dblclick = move |ev: leptos::ev::MouseEvent| {
        ev.stop_propagation();
        ev.prevent_default();
        if (zoom_level.get() - 1.0).abs() < 0.1 {
            #[cfg(feature = "hydrate")]
            {
                let mouse_event = ev.unchecked_ref::<web_sys::MouseEvent>();
                if let Some(target) = mouse_event.target()
                    && let Some(element) = target.dyn_ref::<web_sys::Element>()
                {
                    let rect = element.get_bounding_client_rect();
                    let img_center_x = rect.left() + rect.width() / 2.0;
                    let img_center_y = rect.top() + rect.height() / 2.0;
                    let click_x = f64::from(mouse_event.client_x());
                    let click_y = f64::from(mouse_event.client_y());
                    pan_x.set((img_center_x - click_x) * 2.0);
                    pan_y.set((img_center_y - click_y) * 2.0);
                }
            }
            zoom_level.set(2.0);
        } else {
            zoom_level.set(1.0);
            pan_x.set(0.0);
            pan_y.set(0.0);
        }
    };

    let on_mouse_down = move |ev: leptos::ev::MouseEvent| {
        ev.stop_propagation();
        if zoom_level.get() > 1.0 {
            ev.prevent_default();
            is_panning.set(true);
            start_x.set(f64::from(ev.client_x()) - pan_x.get());
            start_y.set(f64::from(ev.client_y()) - pan_y.get());
        }
    };

    let on_mouse_move = move |ev: leptos::ev::MouseEvent| {
        if is_panning.get() && zoom_level.get() > 1.0 {
            ev.prevent_default();
            pan_x.set(f64::from(ev.client_x()) - start_x.get());
            pan_y.set(f64::from(ev.client_y()) - start_y.get());
        }
        reset_hide_timer();
    };

    let on_mouse_up = move |_ev: leptos::ev::MouseEvent| {
        is_panning.set(false);
    };

    let on_wheel = move |ev: leptos::ev::WheelEvent| {
        ev.prevent_default();
        #[cfg(feature = "hydrate")]
        {
            let delta = ev.delta_y();
            let old_zoom = zoom_level.get();
            let new_zoom = if delta < 0.0 {
                (old_zoom * 1.1_f64).min(max_zoom.get())
            } else {
                (old_zoom / 1.1_f64).max(1.0)
            };
            if (new_zoom - 1.0).abs() < 0.01 {
                zoom_level.set(1.0);
                pan_x.set(0.0);
                pan_y.set(0.0);
            } else {
                let mouse_x = f64::from(ev.client_x());
                let mouse_y = f64::from(ev.client_y());
                if let Some(target) = ev.target() {
                    if let Some(img) = target.dyn_ref::<web_sys::Element>() {
                        let rect = img.get_bounding_client_rect();
                        let img_center_x = rect.left() + (rect.width() / 2.0);
                        let img_center_y = rect.top() + (rect.height() / 2.0);
                        let offset_x = mouse_x - img_center_x;
                        let offset_y = mouse_y - img_center_y;
                        let zoom_ratio = new_zoom / old_zoom;
                        let new_pan_x = pan_x.get() + offset_x * (1.0 - zoom_ratio);
                        let new_pan_y = pan_y.get() + offset_y * (1.0 - zoom_ratio);
                        zoom_level.set(new_zoom);
                        pan_x.set(new_pan_x);
                        pan_y.set(new_pan_y);
                    } else {
                        zoom_level.set(new_zoom);
                    }
                } else {
                    zoom_level.set(new_zoom);
                }
            }
            reset_hide_timer();
        }
        #[cfg(not(feature = "hydrate"))]
        {
            let _ = ev;
        }
    };

    let on_touch_start = move |_ev: leptos::ev::TouchEvent| {
        #[cfg(feature = "hydrate")]
        {
            use leptos::wasm_bindgen::JsCast;
            let touch_event = _ev.unchecked_ref::<web_sys::TouchEvent>();
            let touches = touch_event.touches();
            if touches.length() == 2 {
                touch_event.prevent_default();
                let touch0 = touches.get(0).unwrap();
                let touch1 = touches.get(1).unwrap();
                let dx = f64::from(touch1.client_x() - touch0.client_x());
                let dy = f64::from(touch1.client_y() - touch0.client_y());
                let distance = (dx * dx + dy * dy).sqrt();
                let center_x =
                    f64::midpoint(f64::from(touch0.client_x()), f64::from(touch1.client_x()));
                let center_y =
                    f64::midpoint(f64::from(touch0.client_y()), f64::from(touch1.client_y()));
                _initial_pinch_distance.set(distance);
                _initial_zoom.set(zoom_level.get());
                _initial_pinch_center_x.set(center_x);
                _initial_pinch_center_y.set(center_y);
                _initial_pan_x.set(pan_x.get());
                _initial_pan_y.set(pan_y.get());
            } else if touches.length() == 1 && zoom_level.get() > 1.0 {
                touch_event.prevent_default();
                let touch = touches.get(0).unwrap();
                is_panning.set(true);
                start_x.set(f64::from(touch.client_x()) - pan_x.get());
                start_y.set(f64::from(touch.client_y()) - pan_y.get());
            }
        }
        #[cfg(not(feature = "hydrate"))]
        {
            let _ = _ev;
        }
    };

    let on_touch_move = move |_ev: leptos::ev::TouchEvent| {
        #[cfg(feature = "hydrate")]
        {
            use leptos::wasm_bindgen::JsCast;
            let touch_event = _ev.unchecked_ref::<web_sys::TouchEvent>();
            let touches = touch_event.touches();
            if touches.length() == 2 {
                touch_event.prevent_default();
                let touch0 = touches.get(0).unwrap();
                let touch1 = touches.get(1).unwrap();
                let dx = f64::from(touch1.client_x() - touch0.client_x());
                let dy = f64::from(touch1.client_y() - touch0.client_y());
                let distance = (dx * dx + dy * dy).sqrt();
                let scale = distance / _initial_pinch_distance.get();
                let new_zoom = (_initial_zoom.get() * scale).clamp(1.0, max_zoom.get());
                if (new_zoom - 1.0).abs() < 0.01 {
                    zoom_level.set(1.0);
                    pan_x.set(0.0);
                    pan_y.set(0.0);
                } else {
                    let viewport_center_x = web_sys::window()
                        .and_then(|w| w.inner_width().ok())
                        .and_then(|w| w.as_f64())
                        .unwrap_or(0.0)
                        / 2.0;
                    let viewport_center_y = web_sys::window()
                        .and_then(|w| w.inner_height().ok())
                        .and_then(|h| h.as_f64())
                        .unwrap_or(0.0)
                        / 2.0;
                    let offset_x = _initial_pinch_center_x.get() - viewport_center_x;
                    let offset_y = _initial_pinch_center_y.get() - viewport_center_y;
                    let zoom_ratio = new_zoom / _initial_zoom.get();
                    let new_pan_x =
                        _initial_pan_x.get() * zoom_ratio - offset_x * (zoom_ratio - 1.0);
                    let new_pan_y =
                        _initial_pan_y.get() * zoom_ratio - offset_y * (zoom_ratio - 1.0);
                    zoom_level.set(new_zoom);
                    pan_x.set(new_pan_x);
                    pan_y.set(new_pan_y);
                }
            } else if touches.length() == 1 && is_panning.get() && zoom_level.get() > 1.0 {
                touch_event.prevent_default();
                let touch = touches.get(0).unwrap();
                pan_x.set(f64::from(touch.client_x()) - start_x.get());
                pan_y.set(f64::from(touch.client_y()) - start_y.get());
            }
            reset_hide_timer();
        }
        #[cfg(not(feature = "hydrate"))]
        {
            let _ = _ev;
        }
    };

    let on_touch_end = move |_ev: leptos::ev::TouchEvent| {
        is_panning.set(false);
    };

    let transform_style = move || {
        format!(
            "transform: translate({}px, {}px) scale({}); cursor: {};",
            pan_x.get(),
            pan_y.get(),
            zoom_level.get(),
            if zoom_level.get() > 1.0 {
                if is_panning.get() { "grabbing" } else { "grab" }
            } else {
                "default"
            },
        )
    };

    view! {
        {move || {
            if !is_fullscreen.get() {
                return view! { <div></div> }.into_any();
            }
            view! {
                <div
                    class="fullscreen-overlay"
                    role="dialog"
                    aria-modal="true"
                    aria-label=photo_title_fs.clone()
                    on:click=close_fullscreen
                    on:wheel=on_wheel
                >
                    <button
                        type="button"
                        class="fullscreen-close"
                        class:hidden=move || !show_zoom_controls.get()
                        aria-label="Close"
                        on:click=on_close_button
                    >
                        "✕"
                    </button>
                    <div class="fullscreen-controls" class:hidden=move || !show_zoom_controls.get()>
                        <div class="zoom-slider-container">
                            <label class="zoom-label">"1×"</label>
                            <input
                                type="range"
                                class="zoom-slider"
                                aria-label="Zoom"
                                min="1.0"
                                prop:max=move || max_zoom.get()
                                step="0.1"
                                prop:value=move || zoom_level.get()
                                on:input=on_zoom_change
                            />
                            <label class="zoom-label">
                                {move || format!("{:.0}×", max_zoom.get())}
                            </label>
                        </div>
                    </div>
                    <picture>
                        {move || {
                            if is_mobile() {
                                Vec::new()
                            } else {
                                photo_sources_original
                                    .get_value()
                                    .into_iter()
                                    .map(|source| {
                                        view! { <source srcset=source.url type=source.mime_type /> }
                                    })
                                    .collect()
                            }
                        }}
                        <img
                            src=move || {
                                if is_mobile() {
                                    photo_url_detail.get_value()
                                } else {
                                    photo_url_original.get_value()
                                }
                            }
                            alt=photo_title_fs.clone()
                            width=photo_width.unwrap_or(3600)
                            height=photo_height.unwrap_or(2400)
                            class="fullscreen-image"
                            style=transform_style
                            on:click=on_image_click
                            on:dblclick=on_image_dblclick
                            on:mousedown=on_mouse_down
                            on:mousemove=on_mouse_move
                            on:mouseup=on_mouse_up
                            on:mouseleave=on_mouse_up
                            on:touchstart=on_touch_start
                            on:touchmove=on_touch_move
                            on:touchend=on_touch_end
                        />
                    </picture>
                </div>
            }
                .into_any()
        }}
    }
}

#[component]
fn PhotoDetailPage() -> impl IntoView {
    let params = use_params::<PhotoParams>();
    let photos = Resource::new(|| (), |()| async { get_all_gallery_photos().await });
    let state = ZoomPanState::new();
    let is_details_expanded = RwSignal::new(false);
    let PhotosShown(_, photos_shown) = expect_context();

    // Arrow keys step through the photos. Removed on cleanup: left behind after a
    // client-side visit, the listener would read this page's disposed signals on
    // every key press and panic.
    #[cfg(feature = "hydrate")]
    {
        let navigate = leptos_router::hooks::use_navigate();
        let arrows = window_event_listener(leptos::ev::keydown, move |event| {
            // The full-screen viewer's zoom slider takes its own arrow keys, and
            // with Alt, Ctrl or ⌘ they are the browser's (Alt+Left is Back).
            if event.alt_key()
                || event.ctrl_key()
                || event.meta_key()
                || event
                    .target()
                    .is_some_and(|t| t.has_type::<web_sys::HtmlInputElement>())
            {
                return;
            }
            let key = event.key();

            if let (Some(photo_list), Ok(current_params)) = (
                photos.get_untracked().and_then(std::result::Result::ok),
                params.get_untracked(),
            ) && let Some((idx, _)) = photo_list
                .iter()
                .enumerate()
                .find(|(_, p)| p.slug == current_params.photo)
            {
                match key.as_str() {
                    "ArrowLeft" => {
                        if idx > 0
                            && let Some(prev) = photo_list.get(idx - 1)
                        {
                            let url = format!("/gallery/{}/{}", prev.gallery_name, prev.slug);
                            navigate(&url, leptos_router::NavigateOptions::default());
                        }
                    }
                    "ArrowRight" => {
                        if idx < photo_list.len() - 1
                            && let Some(next) = photo_list.get(idx + 1)
                        {
                            let url = format!("/gallery/{}/{}", next.gallery_name, next.slug);
                            navigate(&url, leptos_router::NavigateOptions::default());
                        }
                    }
                    _ => {}
                }
            }
        });
        on_cleanup(move || arrows.remove());
    }

    // Track viewport width for mobile detection. Initialized to a desktop
    // default so SSR matches the first hydration paint.
    let viewport_width = RwSignal::new(1920.0);

    #[cfg(feature = "hydrate")]
    {
        use leptos::prelude::Effect;
        Effect::new(move |_| {
            if let Some(window) = web_sys::window()
                && let Ok(width) = window.inner_width()
                && let Some(width_f64) = width.as_f64()
            {
                viewport_width.set(width_f64);
            }
        });
    }

    let enter_fullscreen = move |_| {
        state.is_fullscreen.set(true);
        state.show_zoom_controls.set(true);
    };

    // `.photo-detail-page` has a fixed height and hides overflow, so it wraps each
    // branch except the not-found page, which it would clip.
    view! {
        <Suspense fallback=|| {
            view! {
                <div class="photo-detail-page">
                    <div class="loading">"Loading photo..."</div>
                </div>
            }
        }>
            {move || {
                let Some(slug_val) = params.get().ok().map(|p| p.photo) else {
                    return view! { <NotFound /> }.into_any();
                };
                let Some(photo_list) = photos.get().and_then(std::result::Result::ok) else {
                    #[cfg(feature = "ssr")]
                    if load_failed(photos.get_untracked().as_ref()) {
                        mark_unavailable();
                    }
                    return view! {
                        <div class="photo-detail-page">
                            <div class="error">"Failed to load photo"</div>
                        </div>
                    }
                        .into_any();
                };
                let Some((idx, photo)) = photo_list
                    .iter()
                    .enumerate()
                    .find(|(_, p)| p.slug == slug_val)
                    .map(|(i, p)| (i, p.clone())) else {
                    return view! { <NotFound /> }.into_any();
                };
                let prev_photo = if idx > 0 { photo_list.get(idx - 1).cloned() } else { None };
                let next_photo = if idx < photo_list.len() - 1 {
                    photo_list.get(idx + 1).cloned()
                } else {
                    None
                };
                let photo_url_detail = StoredValue::new(photo.detail_url.clone());
                let photo_url_original = StoredValue::new(photo.original_url.clone());
                let photo_sources_original = StoredValue::new(photo.original_sources.clone());
                let photo_title = photo.title.clone();
                let back_link = if photo.gallery_name == "home" {
                    "/".to_string()
                } else {
                    format!("/gallery/{}", photo.gallery_name)
                };
                // The footer hides while this photo is on screen.
                photos_shown.update(|n| *n += 1);
                on_cleanup(move || photos_shown.update(|n| *n = n.saturating_sub(1)));
                view! {
                    <div class="photo-detail-page">
                        <div class="photo-detail-container">
                            <div class="photo-detail-header">
                                <A href=back_link attr:class="back-link">
                                    "← Back to Gallery"
                                </A>
                            </div>
                            <div class="photo-detail-content">
                                <div
                                    class="photo-detail-image"
                                    on:click=enter_fullscreen
                                    style="cursor: pointer;"
                                >
                                    <picture>
                                        // Desktop: alternative original-file formats. Restricted
                                        // to >=768px so mobile never tries to fetch the (often
                                        // multi-MB) original — it falls through to the high-quality
                                        // compressed <img> below.
                                        {photo_sources_original
                                            .get_value()
                                            .into_iter()
                                            .map(|source| {
                                                view! {
                                                    <source
                                                        media="(min-width: 768px)"
                                                        srcset=source.url
                                                        type=source.mime_type
                                                    />
                                                }
                                            })
                                            .collect_view()}
                                        // Desktop primary original (no type → matched if reached).
                                        <source
                                            media="(min-width: 768px)"
                                            srcset=photo_url_original.get_value()
                                        />
                                        // Mobile + final fallback: 4000w/90q WebP — significantly
                                        // higher quality than the grid preset, well below the
                                        // multi-MB originals that would OOM phones.
                                        <img
                                            src=photo_url_detail.get_value()
                                            alt=photo_title.clone()
                                            width=photo.width.unwrap_or(3600)
                                            height=photo.height.unwrap_or(2400)
                                            decoding="async"
                                        />
                                    </picture>
                                </div>
                                <div class="photo-detail-info">
                                    <h1
                                        class="photo-title-toggle"
                                        class:expanded=move || is_details_expanded.get()
                                        on:click=move |_| {
                                            is_details_expanded
                                                .update(|expanded| *expanded = !*expanded);
                                        }
                                    >
                                        {photo_title}
                                    </h1>
                                    <PhotoExifPanel
                                        photo=photo.clone()
                                        is_expanded=is_details_expanded
                                    />
                                </div>
                            </div>
                            <PhotoNavigationButtons prev_photo=prev_photo next_photo=next_photo />
                            <FullscreenViewer
                                image=ViewerImage::from(photo)
                                state=state
                                viewport_width=viewport_width
                            />
                        </div>
                    </div>
                }
                    .into_any()
            }}
        </Suspense>
    }
}

// Helper component for about content display
#[component]
fn AboutContent() -> impl IntoView {
    let about_content = Resource::new(|| (), |()| async { get_about_content().await });

    view! {
        <Suspense fallback=|| {
            view! {
                <div class="about-container">
                    <div class="about-content">
                        <h1>"About Me"</h1>
                        <p>"Loading..."</p>
                    </div>
                </div>
            }
        }>
            {move || {
                match about_content.get().and_then(std::result::Result::ok) {
                    Some(about) => {
                        view! {
                            <div class="about-container">
                                {about
                                    .image_url
                                    .map(|url| {
                                        view! {
                                            <div class="about-image">
                                                <img
                                                    src=url
                                                    alt="Photographer"
                                                    width="800"
                                                    height="800"
                                                    loading="lazy"
                                                    decoding="async"
                                                />
                                            </div>
                                        }
                                    })} <div class="about-content">
                                    <h1>"About Me"</h1>
                                    {if about.is_html {
                                        let html_content = about.content.clone();
                                        view! {
                                            <div
                                                class="about-html-content"
                                                inner_html=html_content
                                            ></div>
                                        }
                                            .into_any()
                                    } else {
                                        let paragraphs: Vec<_> = about
                                            .content
                                            .split("\n\n")
                                            .map(str::trim)
                                            .filter(|p| !p.is_empty())
                                            .collect();
                                        paragraphs
                                            .into_iter()
                                            .map(|p| view! { <p>{p}</p> })
                                            .collect_view()
                                            .into_any()
                                    }}
                                </div>
                            </div>
                        }
                            .into_any()
                    }
                    None => {
                        view! {
                            <div class="about-container">
                                <div class="about-content">
                                    <h1>"About Me"</h1>
                                    <p>"Failed to load about content"</p>
                                </div>
                            </div>
                        }
                            .into_any()
                    }
                }
            }}
        </Suspense>
    }
}

/// Licensing terms for the two works this site is made of: the photographs, which
/// belong to whoever runs it, and the code, which is GPL-3.0-or-later. They are
/// stated separately and adjacently on purpose — a lone "licensed under the GPL" on
/// a photography site invites the reading that the photographs are GPL too.
#[component]
fn LicenseSection() -> impl IntoView {
    let config = site_config();

    view! {
        <section class="about-license" id="license">
            <h2>"License"</h2>
            <Suspense fallback=|| {
                view! { <p class="license-loading">"Loading..."</p> }
            }>
                {move || {
                    config
                        .get()
                        .and_then(std::result::Result::ok)
                        .map(|cfg| {
                            let statement = cfg.image_license();
                            let note = cfg.image_license_note();
                            let contact = cfg.license_contact().map(str::to_string);
                            let source = cfg.source_url().to_string();
                            view! {
                                <div class="license-block">
                                    <h3>"Photographs"</h3>
                                    <p>{statement}</p>
                                    {note.map(|n| view! { <p>{n}</p> })}
                                    {contact
                                        .map(|c| {
                                            view! {
                                                <p>
                                                    "For print, editorial, or commercial use, email "
                                                    <span class="license-contact">{c}</span>
                                                    "."
                                                </p>
                                            }
                                        })}
                                </div>
                                <div class="license-block">
                                    <h3>"Site code"</h3>
                                    <p>
                                        "This site runs on open source software licensed under the "
                                        <a
                                            href=crate::config::CODE_LICENSE_URL
                                            target="_blank"
                                            rel="noopener noreferrer"
                                        >
                                            {crate::config::CODE_LICENSE_NAME}
                                        </a>
                                        " or later. "
                                        <a href=source target="_blank" rel="noopener noreferrer">
                                            "Source"
                                        </a>
                                        "."
                                    </p>
                                </div>
                            }
                        })
                }}
            </Suspense>
        </section>
    }
}

#[component]
fn AboutPage() -> impl IntoView {
    view! {
        <div class="about-page">
            <AboutContent />
            <OpenSourceLink />
            <LicenseSection />
        </div>
    }
}

/// The About page's link to `/open-source`, rendered only while that page has
/// content. No Suspense fallback: a placeholder would flash a link-shaped gap on
/// deployments that never add the file.
#[component]
fn OpenSourceLink() -> impl IntoView {
    let has_page = Resource::new(|| (), |()| async { has_open_source().await });

    view! {
        <Suspense>
            {move || {
                matches!(has_page.get(), Some(Ok(true)))
                    .then(|| {
                        view! {
                            <p class="about-open-source">
                                <A href="/open-source">"Open source"</A>
                            </p>
                        }
                    })
            }}
        </Suspense>
    }
}

/// `url` made safe as a single `srcset` candidate. Unlike `src`, `srcset` splits on
/// whitespace and commas, so a path such as `/readme/my project/card.webp` would
/// otherwise be read as a URL followed by a bogus descriptor.
fn srcset_url(url: &str) -> String {
    url.chars()
        .fold(String::with_capacity(url.len()), |mut out, c| {
            // ',' and the five characters `char::is_ascii_whitespace` matches.
            match c {
                ',' => out.push_str("%2C"),
                ' ' => out.push_str("%20"),
                '\t' => out.push_str("%09"),
                '\n' => out.push_str("%0A"),
                '\x0C' => out.push_str("%0C"),
                '\r' => out.push_str("%0D"),
                c => out.push(c),
            }
            out
        })
}

/// One card on the `/open-source` page. The name links out only when the entry has
/// a non-blank `url`; highlights likewise fall back to plain text. A card with an
/// `image` shows it beside the text, and a click opens it in the full-screen viewer.
#[component]
fn OssCard(entry: OpenSourceEntry) -> impl IntoView {
    let name = match entry.link() {
        Some(url) => view! {
            <a href=url.to_string() target="_blank" rel="noopener noreferrer">
                {entry.name.clone()}
            </a>
        }
        .into_any(),
        None => entry.name.clone().into_any(),
    };
    // Beside the heading rather than inside it, so the heading's accessible name
    // stays the project name.
    let website = entry.website().map(|(href, label)| {
        let (href, label) = (href.to_string(), label.to_string());
        view! {
            <span class="oss-website">
                <span class="oss-separator" aria-hidden="true">
                    "·"
                </span>
                <a href=href target="_blank" rel="noopener noreferrer">
                    {label}
                </a>
            </span>
        }
    });
    let tags = (!entry.tags.is_empty()).then(|| {
        view! {
            <ul class="oss-tags">
                {entry.tags.iter().map(|t| view! { <li>{t.clone()}</li> }).collect_view()}
            </ul>
        }
    });
    let highlights = (!entry.highlights.is_empty()).then(|| {
        view! {
            <ul class="oss-highlights">
                {entry
                    .highlights
                    .iter()
                    .map(|h| {
                        let item = match h.link() {
                            Some(url) => {
                                view! {
                                    <a
                                        href=url.to_string()
                                        target="_blank"
                                        rel="noopener noreferrer"
                                    >
                                        {h.title.clone()}
                                    </a>
                                }
                                    .into_any()
                            }
                            None => h.title.clone().into_any(),
                        };
                        view! { <li>{item}</li> }
                    })
                    .collect_view()}
            </ul>
        }
    });

    let head = view! {
        <div class="oss-card-head">
            <h3>{name}</h3>
            {website}
        </div>
    };
    let summary = view! { <p class="oss-summary">{entry.summary.clone()}</p> };

    match entry.image() {
        None => view! {
            <li class="oss-card">
                {head}
                {summary}
                {tags}
                {highlights}
            </li>
        }
        .into_any(),
        Some((src, alt)) => {
            let image = ViewerImage::screenshot(src, &alt);
            let invert = entry.image_invert;
            // The desktop slot shows `image_card` when it differs. Phones show the
            // screenshot full width, at 2x or 3x density, which needs more pixels
            // than that file has, so they keep `image`. The media query is the exact
            // complement of the card's phone layout (max-width: 768px in main.scss),
            // so a fractional viewport width can't fall between the two.
            let card = entry.card_src(src);
            let desktop = (card != src).then(|| {
                let card = srcset_url(card);
                view! { <source media="not all and (max-width: 768px)" srcset=card /> }
            });
            let src = src.to_string();
            let label = format!("View the {} screenshot full screen", entry.name.trim());
            let state = ZoomPanState::new();
            let open = move |_| {
                state.is_fullscreen.set(true);
                state.show_zoom_controls.set(true);
            };
            view! {
                <li class="oss-card oss-card-with-image">
                    <div class="oss-card-text">
                        {head}
                        {summary}
                        {tags}
                        {highlights}
                        // In the text column because, closed, the viewer still renders an
                        // empty <div>, which as a grid item would add a row on phones.
                        // Its width is never measured: a screenshot's two URLs are the
                        // same and it has no other formats, so the viewer's phone/desktop
                        // switch has nothing to choose between.
                        <FullscreenViewer
                            image=image
                            state=state
                            viewport_width=RwSignal::new(1920.0)
                        />
                    </div>
                    <button type="button" class="oss-image" aria-label=label on:click=open>
                        <picture>
                            {desktop}
                            // Only the card inverts: full screen, white art already
                            // shows light on the viewer's dark overlay.
                            <img
                                src=src
                                alt=alt
                                class:image-invert=invert
                                loading="lazy"
                                decoding="async"
                            />
                        </picture>
                    </button>
                </li>
            }
            .into_any()
        }
    }
}

/// A headed list of cards, or nothing at all when `entries` is empty, so a file
/// with only contributions shows no empty "Projects" heading.
#[component]
fn OssSection(heading: &'static str, entries: Vec<OpenSourceEntry>) -> impl IntoView {
    (!entries.is_empty()).then(|| {
        view! {
            <section class="oss-section">
                <h2>{heading}</h2>
                <ul class="oss-cards">
                    {entries
                        .into_iter()
                        .map(|entry| view! { <OssCard entry=entry /> })
                        .collect_view()}
                </ul>
            </section>
        }
    })
}

#[component]
fn OpenSourcePage() -> impl IntoView {
    let content = Resource::new(|| (), |()| async { get_open_source().await });

    view! {
        <Suspense fallback=|| {
            view! {
                <div class="open-source-page">
                    <p class="oss-loading">"Loading..."</p>
                </div>
            }
        }>
            {move || {
                match content.get() {
                    Some(Ok(Some(page))) => {
                        let intro = page.intro_text().map(str::to_string);
                        view! {
                            <div class="open-source-page">
                                <h1>"Open Source"</h1>
                                {intro.map(|text| view! { <p class="oss-intro">{text}</p> })}
                                <OssSection heading="Projects" entries=page.projects />
                                <OssSection heading="Contributions" entries=page.contributions />
                            </div>
                        }
                            .into_any()
                    }
                    // Still loading on the client: keep the Suspense fallback's shape
                    // rather than flashing the not-found page before the data lands.
                    None => ().into_any(),
                    // A missing, malformed or empty file, or a failed request: the
                    // site's not-found page, and on the server a 404.
                    Some(_) => view! { <NotFound /> }.into_any(),
                }
            }}
        </Suspense>
    }
}

// Helper function to capitalize display keys
fn normalize_display_key(key: &str) -> String {
    key.replace('_', " ")
        .split_whitespace()
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

// Helper function to get social media URL and display text from a simple string value
fn get_social_media_link_from_string(key: &str, value: &str) -> Option<(String, String)> {
    let key_lower = key.to_lowercase();

    if key_lower.contains("instagram") {
        if let Some(handle) = value.strip_prefix('@') {
            Some((format!("https://instagram.com/{handle}"), value.to_string()))
        } else if !value.starts_with("http") {
            Some((
                format!("https://instagram.com/{}", value.trim_start_matches('@')),
                value.to_string(),
            ))
        } else {
            None
        }
    } else if key_lower.contains("github") {
        // Handle GitHub URLs
        if value.starts_with("https://github.com/") || value.starts_with("http://github.com/") {
            // Extract username from full URL
            if let Some(username) = value.split('/').nth(3)
                && !username.is_empty()
            {
                return Some((
                    format!("https://github.com/{username}"),
                    username.to_string(),
                ));
            }
            None
        } else {
            // Treat as username
            let username = value.trim_start_matches('@');
            Some((
                format!("https://github.com/{username}"),
                username.to_string(),
            ))
        }
    } else {
        None
    }
}

// Helper component for contact information item
#[component]
fn ContactInfoItem(key: String, value: crate::config::SectionValue) -> impl IntoView {
    use crate::config::SectionValue;

    let display_key = normalize_display_key(&key);

    // Determine the link information based on the section value type
    let link_info: Option<(String, String)> = match &value {
        SectionValue::Link { display, url } => {
            // Explicit URL and display provided - use them directly
            Some((url.clone(), display.clone()))
        }
        SectionValue::Simple(s) => {
            // Simple string - try automatic formatting for known social media
            get_social_media_link_from_string(&key, s)
        }
    };

    view! {
        <div class="contact-item">
            <strong>{display_key}":"</strong>
            {if let Some((url, display_text)) = link_info {
                view! {
                    <p>
                        <a href=url target="_blank" rel="noopener noreferrer">
                            {display_text}
                        </a>
                    </p>
                }
                    .into_any()
            } else {
                view! { <p>{value.display()}</p> }.into_any()
            }}
        </div>
    }
}

// Helper component for contact information section
#[component]
fn ContactInfoSection() -> impl IntoView {
    let config = site_config();

    view! {
        <Suspense fallback=|| {
            view! { <div class="contact-info"></div> }
        }>
            {move || {
                match config.get().and_then(std::result::Result::ok) {
                    Some(cfg) if !cfg.sections.is_empty() => {
                        let mut sections_vec: Vec<_> = cfg.sections.into_iter().collect();
                        sections_vec.sort_by(|a, b| a.0.cmp(&b.0));
                        view! {
                            <div class="contact-info">
                                <h2>"Contact Information"</h2>
                                {sections_vec
                                    .into_iter()
                                    .map(|(key, value)| {
                                        view! { <ContactInfoItem key=key value=value /> }
                                    })
                                    .collect_view()}
                            </div>
                        }
                            .into_any()
                    }
                    _ => view! { <div class="contact-info"></div> }.into_any(),
                }
            }}
        </Suspense>
    }
}

// Helper component for contact form
#[component]
fn ContactForm(submitted: RwSignal<bool>) -> impl IntoView {
    let name = RwSignal::new(String::new());
    let email = RwSignal::new(String::new());
    let message = RwSignal::new(String::new());

    let on_submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        // Here you would normally handle form submission to a server
        submitted.set(true);
    };

    view! {
        <div class="contact-form">
            {move || {
                if submitted.get() {
                    view! {
                        <div class="success-message">
                            <p>"Thank you for your message! I'll get back to you soon."</p>
                        </div>
                    }
                        .into_any()
                } else {
                    view! {
                        <form on:submit=on_submit>
                            <div class="form-group">
                                <label for="name">"Name"</label>
                                <input
                                    type="text"
                                    id="name"
                                    required
                                    prop:value=name
                                    on:input=move |ev| name.set(event_target_value(&ev))
                                />
                            </div>
                            <div class="form-group">
                                <label for="email">"Email"</label>
                                <input
                                    type="email"
                                    id="email"
                                    required
                                    prop:value=email
                                    on:input=move |ev| email.set(event_target_value(&ev))
                                />
                            </div>
                            <div class="form-group">
                                <label for="message">"Message"</label>
                                <textarea
                                    id="message"
                                    rows="5"
                                    required
                                    prop:value=message
                                    on:input=move |ev| message.set(event_target_value(&ev))
                                />
                            </div>
                            <button type="submit" class="submit-button">
                                "Send Message"
                            </button>
                        </form>
                    }
                        .into_any()
                }
            }}
        </div>
    }
}

#[component]
fn ContactPage() -> impl IntoView {
    let submitted = RwSignal::new(false);

    view! {
        <div class="contact-page">
            <h1>"Get In Touch"</h1>
            <div class="contact-container">
                <ContactInfoSection />
                <ContactForm submitted=submitted />
            </div>
        </div>
    }
}

#[cfg(all(test, feature = "ssr"))]
mod tests {
    use super::*;

    #[test]
    fn srcset_url_escapes_what_srcset_would_split_on() {
        assert_eq!(srcset_url("/readme/x/card.webp"), "/readme/x/card.webp");
        assert_eq!(
            srcset_url("/readme/my project/a,b.webp"),
            "/readme/my%20project/a%2Cb.webp"
        );
        assert_eq!(srcset_url("/a\tb"), "/a%09b");
    }

    #[test]
    fn only_a_failed_load_marks_a_page_unavailable() {
        assert!(load_failed(Some(&Err::<(), _>("the share is down"))));
        assert!(!load_failed(Some(&Ok::<_, &str>(()))));
        assert!(!load_failed(None::<&Result<(), &str>>), "still loading");
    }

    // The server renders a page on one tokio worker and can drop it on another, so
    // nothing the viewer creates while rendering may be tied to its thread. A
    // thread-bound value aborted the server mid-request.
    #[test]
    fn a_rendered_viewer_can_be_dropped_on_another_thread() {
        let owner = Owner::new();
        owner.with(|| {
            let _ = view! {
                <FullscreenViewer
                    image=ViewerImage::screenshot("/shot.webp", "A screenshot")
                    state=ZoomPanState::new()
                    viewport_width=RwSignal::new(1920.0)
                />
            }
            .to_html();
        });

        let dropped = std::thread::spawn(move || drop(owner)).join();
        assert!(
            dropped.is_ok(),
            "dropping the viewer on another thread panicked"
        );
    }

    // The photo link wraps only the image, so the image's alt is the link's name.
    #[test]
    fn a_not_found_photo_without_a_title_still_names_its_link() {
        let photo = PhotoInfo {
            url: "/images/compressed/home/01.jpg".to_string(),
            original_url: String::new(),
            detail_url: String::new(),
            srcset: String::new(),
            sources: Vec::new(),
            original_sources: Vec::new(),
            title: String::new(),
            filename: "01 - .jpg".to_string(),
            slug: "01".to_string(),
            gallery_name: "home".to_string(),
            subfolder: None,
            width: Some(1200),
            height: Some(800),
            date_taken: None,
            camera_make: None,
            camera_model: None,
            lens_model: None,
            focal_length: None,
            aperture: None,
            shutter_speed: None,
            iso: None,
            film_stock: None,
            copyright: None,
            focal_point: None,
        };
        let html = Owner::new().with(|| view! { <NotFoundPhoto photo=photo /> }.to_html());
        assert!(
            html.contains(r#"alt="A photo from the home gallery""#),
            "{html}"
        );
    }
}

// Runs under both feature sets, unlike `tests` above.
#[cfg(test)]
mod not_found_tests {
    use super::is_portrait;

    #[test]
    fn is_portrait_only_when_taller_than_wide() {
        assert!(is_portrait(Some(800), Some(1200)));
        assert!(!is_portrait(Some(1200), Some(800)));
        assert!(!is_portrait(Some(1000), Some(1000)));
        assert!(!is_portrait(None, Some(5)));
        assert!(!is_portrait(Some(5), None));
    }
}

/// Whole pages rendered through the app's routes, the way the server serves them.
#[cfg(all(test, feature = "ssr"))]
mod ssr_tests {
    use super::{App, shell};
    use crate::test_support::EnvVarGuard;
    use crate::trailing_slash::{self, PageRoutes};
    use axum::{
        body::Body,
        http::{
            HeaderMap, Method, Request, StatusCode,
            header::{self, RETRY_AFTER},
        },
        response::Response,
    };
    use leptos::prelude::LeptosOptions;
    use leptos_axum::{LeptosRoutes, generate_route_list};
    use leptos_router::SsrMode;
    use serial_test::serial;
    use tower::ServiceExt;

    /// One test's site: a temp dir that is the content dir (no `config.toml`, no
    /// `opensource.toml`) and holds `images/` as the images root, with an empty
    /// `images/home`. Sets `ABOUT_CONTENT_PATH`, `IMAGES_DIR` and `CONFIG_PATH` (to
    /// the absent `dir/config.toml`), removes `GALLERY_PATH`, and clears the
    /// process-wide caches on the way in and out.
    struct Site {
        dir: tempfile::TempDir,
        _env: [EnvVarGuard; 3],
    }

    impl Site {
        fn new() -> Self {
            let dir = tempfile::TempDir::new().unwrap();
            std::fs::create_dir_all(dir.path().join("images/home")).unwrap();
            unsafe { std::env::remove_var("GALLERY_PATH") };
            let env = [
                EnvVarGuard::set("ABOUT_CONTENT_PATH", dir.path()),
                EnvVarGuard::set("IMAGES_DIR", dir.path().join("images")),
                EnvVarGuard::set("CONFIG_PATH", dir.path().join("config.toml")),
            ];
            crate::server::invalidate_caches();
            Self { dir, _env: env }
        }

        /// As `new`, plus one small JPEG at `images/home/photo.jpg` and a `nature`
        /// gallery holding another. A gallery other than `home` has to exist before
        /// a missing one counts as missing (`get_gallery_data_by_name`).
        fn with_home_photo() -> Self {
            let site = Self::new();
            site.share_comes_back();
            site
        }

        /// As `new`, plus a `nature` gallery with one photo: the share is mounted,
        /// and the home gallery really is empty.
        fn with_empty_home() -> Self {
            let site = Self::new();
            site.add_photo("nature");
            site
        }

        /// The photos `with_home_photo` has, added to a running site, as when the
        /// share comes back after `during_outage`.
        fn share_comes_back(&self) {
            self.add_photo("home");
            self.add_photo("nature");
        }

        /// One small JPEG at `images/<gallery>/photo.jpg`.
        fn add_photo(&self, gallery: &str) {
            let dir = self.dir.path().join("images").join(gallery);
            std::fs::create_dir_all(&dir).unwrap();
            image::RgbImage::new(4, 3)
                .save(dir.join("photo.jpg"))
                .unwrap();
        }

        /// The site while its images share is down: the mountpoint is there but
        /// empty, apart from the `home` the server makes for itself. That is `new`'s
        /// layout, and the server reads it as an outage rather than as a site with
        /// no galleries (`images_root_looks_unmounted`).
        fn during_outage() -> Self {
            Self::new()
        }

        fn write_open_source(&self, toml: &str) {
            std::fs::write(self.dir.path().join("opensource.toml"), toml).unwrap();
        }
    }

    impl Drop for Site {
        fn drop(&mut self) {
            crate::server::invalidate_caches();
        }
    }

    /// `html` without tachys's hydration markers, the `<!>` and `<!--…-->` comments
    /// a browser doesn't show. Match rendered HTML against this, because where the
    /// markers go depends on the build. `cargo leptos test` compiles with
    /// `--cfg erase_components`, which puts a `<!>` before the closing tag of most
    /// elements that aren't plain static HTML (`<code class="not-found-path">/x<!></code>`).
    /// `cargo ci` and the release build that serves the site don't.
    fn without_markers(html: &str) -> String {
        regex::Regex::new(r"(?s)<!(?:>|--.*?-->)")
            .unwrap()
            .replace_all(html, "")
            .into_owned()
    }

    /// The app's Leptos routes, fallback and trailing-slash redirect, wired the way
    /// src/main.rs wires them (minus the file mounts, the HTML cache and compression).
    fn app() -> axum::Router {
        let options = LeptosOptions::builder().output_name("portfolio").build();
        // A resource first polled anywhere in the process while the route list is
        // being built never loads, so any test that builds it or renders resources
        // must be #[serial].
        let routes = generate_route_list(App);
        let pages = PageRoutes::new(&routes);
        axum::Router::new()
            .leptos_routes(&options, routes, {
                let options = options.clone();
                move || shell(options.clone())
            })
            .fallback(leptos_axum::file_and_error_handler(shell))
            .with_state(options)
            .layer(axum::middleware::from_fn_with_state(
                pages,
                trailing_slash::redirect,
            ))
    }

    /// Send `method` for `path` through [`app`].
    async fn request(method: Method, path: &str) -> Response {
        let request = Request::builder().method(method).uri(path);
        app()
            .oneshot(request.body(Body::empty()).unwrap())
            .await
            .unwrap()
    }

    /// The status and the `Location` header of the answer to `method` for `path`.
    async fn answer(method: Method, path: &str) -> (StatusCode, Option<String>) {
        let res = request(method, path).await;
        let location = res
            .headers()
            .get(header::LOCATION)
            .map(|value| value.to_str().unwrap().to_owned());
        (res.status(), location)
    }

    /// The status and the `Location` header of the answer to a GET for `path`.
    async fn redirect_of(path: &str) -> (StatusCode, Option<String>) {
        answer(Method::GET, path).await
    }

    /// The status, the headers and the body of the answer to a GET for `path`.
    async fn get_with_headers(path: &str) -> (StatusCode, HeaderMap, String) {
        let res = request(Method::GET, path).await;
        let status = res.status();
        let headers = res.headers().clone();
        let body = axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap();
        (status, headers, String::from_utf8(body.to_vec()).unwrap())
    }

    /// The status and the body of the answer to a GET for `path`.
    async fn get(path: &str) -> (StatusCode, String) {
        let (status, _, body) = get_with_headers(path).await;
        (status, body)
    }

    #[tokio::test]
    #[serial]
    async fn unknown_address_answers_404_with_the_not_found_page() {
        let _site = Site::with_home_photo();
        let (status, body) = get("/no-such-page").await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert!(body.contains(r#"class="not-found-page""#), "{body}");
        assert!(
            without_markers(&body).contains(r#"<code class="not-found-path">/no-such-page</code>"#),
            "{body}"
        );
        assert!(body.contains(r#"href="/gallery/home/photo""#), "{body}");
        // The site's own title, as on every page: the owner chose no special one.
        assert!(body.contains("<title>Your Name</title>"), "{body}");
        assert!(!body.contains("Page not found ·"), "{body}");
    }

    #[tokio::test]
    #[serial]
    async fn unknown_address_without_home_photos_shows_the_message_alone() {
        let _site = Site::new();
        let (status, body) = get("/no-such-page").await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert!(body.contains("Back to the galleries"), "{body}");
        assert!(!body.contains("not-found-photo"), "{body}");
    }

    #[tokio::test]
    #[serial]
    async fn the_path_is_shown_as_text() {
        let _site = Site::new();
        let (status, body) = get("/%3Cscript%3Ealert(1)%3C/script%3E").await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert!(!body.contains("<script>alert(1)"), "{body}");
        assert!(
            without_markers(&body).contains(
                r#"<code class="not-found-path">/%3Cscript%3Ealert(1)%3C/script%3E</code>"#
            ),
            "{body}"
        );
        let (_, body) = get("/a&b").await;
        assert!(
            without_markers(&body).contains(r#"<code class="not-found-path">/a&amp;b</code>"#),
            "{body}"
        );
    }

    type PanicHook = Box<dyn Fn(&std::panic::PanicHookInfo<'_>) + Send + Sync>;

    /// While it lives, counts the panics on threads named `thread`, and hands every
    /// panic on to the hook that was there before. Dropped during a panic, it leaves
    /// itself in place, since the hook can't be changed then; it still passes
    /// everything on.
    struct PanicCount {
        count: std::sync::Arc<std::sync::atomic::AtomicUsize>,
        previous: Option<std::sync::Arc<PanicHook>>,
    }

    impl PanicCount {
        fn on_threads_named(thread: &'static str) -> Self {
            let count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
            let previous = std::sync::Arc::new(std::panic::take_hook());
            std::panic::set_hook(Box::new({
                let (count, previous) = (count.clone(), previous.clone());
                move |info| {
                    if std::thread::current().name() == Some(thread) {
                        count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    }
                    previous(info);
                }
            }));
            Self {
                count,
                previous: Some(previous),
            }
        }

        fn get(&self) -> usize {
            self.count.load(std::sync::atomic::Ordering::SeqCst)
        }
    }

    impl Drop for PanicCount {
        fn drop(&mut self) {
            if std::thread::panicking() {
                return;
            }
            drop(std::panic::take_hook());
            if let Some(previous) = self.previous.take() {
                match std::sync::Arc::try_unwrap(previous) {
                    Ok(previous) => std::panic::set_hook(previous),
                    Err(shared) => std::panic::set_hook(Box::new(move |info| shared(info))),
                }
            }
        }
    }

    /// Gallery pages, rendered 3000 times one after another on a runtime with two
    /// worker threads, all answer, and nothing on those threads panics.
    ///
    /// While the server still ran the effect behind the gallery page's `Transition`
    /// (see `PageTransition`), the effect could deadlock against the render. The
    /// render, writing to the `Transition`'s task list as it read a resource, held
    /// a memo's lock and waited for the effect's, while the effect, re-running on
    /// the other worker, held its own lock and waited for the memo's. The page
    /// never answered, and both workers stayed blocked for good.
    ///
    /// The two have to run at the same time, so this takes two CPUs or more to catch
    /// it. Against that version it failed in all 16 runs on 2, 4 and 32 CPUs, after
    /// 27 to 2217 pages, and passed in all 3 on a single CPU.
    ///
    /// Newer Leptos releases fix the deadlock, but that effect still panics now and
    /// then. The panic only ends the effect's own task, which would leave the test
    /// green, so the test counts panics on the runtime's threads too.
    ///
    /// The runtime gets a thread of its own, so a page that never answers fails the
    /// test after 10 seconds instead of hanging it.
    #[test]
    #[serial]
    fn gallery_pages_all_answer_under_load() {
        const PAGES: usize = 3000;
        const THREADS: &str = "gallery-pages-under-load";
        let _site = Site::with_home_photo();
        let panics = PanicCount::on_threads_named(THREADS);
        let (answered, answers) = std::sync::mpsc::channel();
        let server = std::thread::Builder::new()
            .name(THREADS.into())
            .spawn(move || {
                let runtime = tokio::runtime::Builder::new_multi_thread()
                    .worker_threads(2)
                    .thread_name(THREADS)
                    .enable_all()
                    .build()
                    .unwrap();
                runtime.block_on(async move {
                    let app = app();
                    for n in 0..PAGES {
                        let path = if n % 2 == 0 {
                            "/gallery/home"
                        } else {
                            "/gallery/nature"
                        };
                        let request = Request::get(path).body(Body::empty()).unwrap();
                        // On a worker, as the server runs each request.
                        let page = tokio::spawn({
                            let app = app.clone();
                            async move {
                                let res = app.oneshot(request).await.unwrap();
                                let status = res.status();
                                axum::body::to_bytes(res.into_body(), usize::MAX)
                                    .await
                                    .unwrap();
                                status
                            }
                        });
                        // Once the test has failed, it has dropped `Site` and let the
                        // next serial test start, so stop rather than render against
                        // that test's settings.
                        if answered.send(page.await.unwrap()).is_err() {
                            break;
                        }
                    }
                });
                // Dropping a runtime waits for its workers with no limit, so a worker
                // stuck after the last page would hang the test. Wait 10 s at most and
                // report whether that was enough.
                let start = std::time::Instant::now();
                runtime.shutdown_timeout(std::time::Duration::from_secs(10));
                start.elapsed() < std::time::Duration::from_secs(10)
            })
            .unwrap();
        for n in 0..PAGES {
            let status = answers
                .recv_timeout(std::time::Duration::from_secs(10))
                .unwrap_or_else(|err| panic!("{n} of {PAGES} pages answered ({err})"));
            assert_eq!(status, StatusCode::OK);
        }
        // The runtime is gone before `Site` is, and before the next serial test.
        assert!(
            server.join().unwrap(),
            "the server's runtime didn't shut down within 10 s"
        );
        let count = panics.get();
        drop(panics);
        assert_eq!(
            count, 0,
            "panics on the server's threads; see the output above"
        );
    }

    #[tokio::test]
    #[serial]
    async fn open_source_without_a_file_answers_404_with_the_not_found_page() {
        let _site = Site::new();
        let (status, body) = get("/open-source").await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert!(
            without_markers(&body).contains(r#"<code class="not-found-path">/open-source</code>"#),
            "{body}"
        );
    }

    #[tokio::test]
    #[serial]
    async fn open_source_with_a_file_answers_200() {
        let site = Site::new();
        site.write_open_source("[[projects]]\nname = \"P\"\nsummary = \"S\"\n");
        let (status, body) = get("/open-source").await;
        assert_eq!(status, StatusCode::OK);
        assert!(body.contains("<h1>Open Source</h1>"), "{body}");
        assert!(!body.contains("not-found-page"), "{body}");
    }

    /// The `<footer>` start tag in a page's HTML.
    fn footer_tag(body: &str) -> &str {
        let start = body.find("<footer").expect("no <footer>");
        let end = start + body[start..].find('>').expect("unclosed <footer");
        &body[start..=end]
    }

    // The browser keeps the server's `hidden` while it hydrates, so the server's
    // HTML has to hide the footer only where the photo page shows a photo.
    #[tokio::test]
    #[serial]
    async fn the_footer_shows_on_a_not_found_page_at_a_photo_address() {
        let _site = Site::with_home_photo();
        for path in ["/gallery/home/nope", "/gallery/nope/x"] {
            let (status, body) = get(path).await;
            assert_eq!(status, StatusCode::NOT_FOUND, "{path}");
            assert!(body.contains(r#"class="not-found-page""#), "{path}: {body}");
            let footer = footer_tag(&body);
            assert!(!footer.contains("hidden"), "{path}: {footer}");
        }
    }

    #[tokio::test]
    #[serial]
    async fn the_footer_is_hidden_on_a_photo_page() {
        let _site = Site::with_home_photo();
        // Twice: the first request walks the galleries, the second reads the cache.
        // Either path waiting would send the footer out showing.
        for pass in ["cold", "warm"] {
            let (status, body) = get("/gallery/home/photo").await;
            assert_eq!(status, StatusCode::OK, "{pass}");
            let footer = footer_tag(&body);
            assert!(footer.contains(" hidden"), "{pass}: {footer}");
        }
    }

    // Only a photo hides the footer. When the photo list fails to load, the photo
    // page's error message has the footer under it.
    #[tokio::test]
    #[serial]
    async fn the_footer_shows_when_the_photo_list_fails_to_load() {
        let _site = Site::new();
        let (_, body) = get("/gallery/home/photo").await;
        assert!(body.contains("Failed to load photo"), "{body}");
        let footer = footer_tag(&body);
        assert!(!footer.contains("hidden"), "{footer}");
    }

    #[tokio::test]
    #[serial]
    async fn missing_gallery_and_photo_answer_404() {
        let _site = Site::with_home_photo();
        for path in ["/gallery/nope", "/gallery/home/nope"] {
            let (status, body) = get(path).await;
            assert_eq!(status, StatusCode::NOT_FOUND, "{path}");
            assert!(
                without_markers(&body)
                    .contains(&format!(r#"<code class="not-found-path">{path}</code>"#)),
                "{path}: {body}"
            );
        }
    }

    #[tokio::test]
    #[serial]
    async fn existing_gallery_and_photo_answer_200() {
        let _site = Site::with_home_photo();
        for path in ["/gallery/home", "/gallery/home/photo"] {
            let (status, headers, body) = get_with_headers(path).await;
            assert_eq!(status, StatusCode::OK, "{path}");
            assert!(headers.get(RETRY_AFTER).is_none(), "{path}");
            assert!(!body.contains("not-found-page"), "{path}: {body}");
        }
    }

    /// A status set while a page renders goes out with the first chunk of HTML.
    /// The status tests in this module can't tell whether that chunk is the whole
    /// page: the server functions behind these routes never yield, so even
    /// streamed, the page is one chunk today. Once one really waits, a streamed
    /// page sends its fallback first and its 404 or 503 is lost, so these routes
    /// render whole first. (A tokio test only because the route list needs an
    /// executor. Serial because building it suppresses resource loads process-wide
    /// while it runs, which stalls a page another test is rendering.)
    #[tokio::test]
    #[serial]
    async fn routes_that_set_a_status_render_whole_first() {
        let routes = generate_route_list(App);
        for path in [
            "/",
            "/gallery/{name}",
            "/gallery/{gallery}/{photo}",
            "/open-source",
        ] {
            let route = routes
                .iter()
                .find(|r| r.path() == path)
                .unwrap_or_else(|| panic!("no route {path} in {routes:?}"));
            assert_eq!(*route.mode(), SsrMode::Async, "{path}");
        }
    }

    #[tokio::test]
    #[serial]
    async fn gallery_and_photo_during_an_outage_answer_503_with_retry_after() {
        let _site = Site::during_outage();
        for (path, message) in [
            ("/gallery/nature", "Failed to load gallery"),
            ("/gallery/nature/photo", "Failed to load photo"),
        ] {
            let (status, headers, body) = get_with_headers(path).await;
            assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{path}");
            assert_eq!(
                headers.get(RETRY_AFTER).and_then(|v| v.to_str().ok()),
                Some("300"),
                "{path}"
            );
            assert!(body.contains(message), "{path}: {body}");
            assert!(!body.contains("not-found-page"), "{path}: {body}");
        }
    }

    /// Guards the photo page's `load_failed` check: only a failed load marks the
    /// page unavailable, not the pass its Suspense makes while the photo list is
    /// still loading. The page tests never see that pass, since the list resolves on
    /// its first poll. Here `SuppressResourceLoad` holds the resources pending, and
    /// the Suspense walks its children once on the server before they resolve, so
    /// the error branch is built while the data is still loading. An unconditional
    /// `mark_unavailable()` there makes the status `Some(503)`. The router is there
    /// only for the page's params.
    #[tokio::test]
    #[serial]
    async fn a_loading_photo_page_marks_nothing() {
        use leptos::prelude::{Owner, RenderHtml, provide_context, signal, view};
        use leptos_router::{
            ParamSegment, StaticSegment,
            components::{Route, Router, Routes},
            location::RequestUrl,
        };

        let _site = Site::with_home_photo();
        // Sets up leptos's executor.
        let _ = generate_route_list(App);
        let response = leptos_axum::ResponseOptions::default();
        Owner::new().with(|| {
            provide_context(response.clone());
            provide_context(RequestUrl::new("http://leptos.dev/gallery/home/photo"));
            // The photo page counts itself for the footer, which `App` provides; this
            // test renders the page without `App`.
            let (shown, set_shown) = signal(0_u32);
            provide_context(super::PhotosShown(shown, set_shown));
            let _pending = leptos::server::SuppressResourceLoad::new();
            let _stream = view! {
                <Router>
                    <Routes fallback=|| ()>
                        <Route
                            path=(
                                StaticSegment("gallery"),
                                ParamSegment("gallery"),
                                ParamSegment("photo"),
                            )
                            view=super::PhotoDetailPage
                        />
                    </Routes>
                </Router>
            }
            .to_html_stream_in_order();
        });
        assert_eq!(response.0.read().unwrap().status, None);
    }

    // Before the redirect, each of these answered 404. With one trailing slash the
    // page itself showed, since the server's routes are exact and the router takes
    // one trailing slash as none. With two (`/about//`) the router matched nothing
    // either, and the not-found page showed.
    #[tokio::test]
    #[serial]
    async fn page_addresses_with_a_trailing_slash_redirect_to_the_page() {
        let site = Site::with_home_photo();
        site.write_open_source("[[projects]]\nname = \"P\"\nsummary = \"S\"\n");
        for (from, to) in [
            ("/gallery/home/", "/gallery/home"),
            ("/gallery/home/photo/", "/gallery/home/photo"),
            ("/about/", "/about"),
            ("/open-source/", "/open-source"),
            ("/contact/", "/contact"),
            ("/about//", "/about"),
        ] {
            let (status, location) = redirect_of(from).await;
            assert_eq!(status, StatusCode::PERMANENT_REDIRECT, "{from}");
            assert_eq!(location.as_deref(), Some(to), "{from}");
            let (status, body) = get(to).await;
            assert_eq!(status, StatusCode::OK, "{to}");
            assert!(!body.contains("not-found-page"), "{to}: {body}");
        }
    }

    #[tokio::test]
    #[serial]
    async fn missing_gallery_and_photo_with_a_trailing_slash_redirect_then_answer_404() {
        let _site = Site::with_home_photo();
        for (from, to) in [
            ("/gallery/nope/", "/gallery/nope"),
            ("/gallery/home/nope/", "/gallery/home/nope"),
        ] {
            let (status, location) = redirect_of(from).await;
            assert_eq!(status, StatusCode::PERMANENT_REDIRECT, "{from}");
            assert_eq!(location.as_deref(), Some(to), "{from}");
            let (status, body) = get(to).await;
            assert_eq!(status, StatusCode::NOT_FOUND, "{to}");
            assert!(
                without_markers(&body)
                    .contains(&format!(r#"<code class="not-found-path">{to}</code>"#)),
                "{to}: {body}"
            );
        }
    }

    // Any other method goes on to what it got before the redirect existed: the
    // fallback's 404.
    #[tokio::test]
    #[serial]
    async fn only_get_and_head_are_redirected() {
        let _site = Site::with_home_photo();
        for method in [Method::GET, Method::HEAD] {
            assert_eq!(
                answer(method.clone(), "/about/").await,
                (StatusCode::PERMANENT_REDIRECT, Some("/about".to_owned())),
                "{method}"
            );
        }
        for method in [
            Method::POST,
            Method::PUT,
            Method::PATCH,
            Method::DELETE,
            Method::OPTIONS,
        ] {
            assert_eq!(
                answer(method.clone(), "/about/").await,
                (StatusCode::NOT_FOUND, None),
                "{method}"
            );
        }
    }

    #[tokio::test]
    #[serial]
    async fn the_redirect_keeps_the_query() {
        let _site = Site::with_home_photo();
        let (status, location) = redirect_of("/gallery/home/?a=1&b=2").await;
        assert_eq!(status, StatusCode::PERMANENT_REDIRECT);
        assert_eq!(location.as_deref(), Some("/gallery/home?a=1&b=2"));
    }

    #[tokio::test]
    #[serial]
    async fn the_home_page_is_not_redirected() {
        let _site = Site::with_home_photo();
        let (status, location) = redirect_of("/").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(location, None);
    }

    // Not page addresses, so they stay as they are and answer 404 with the
    // not-found page. `/pkg/` is served by the fallback, which this app has; the
    // file mounts are not here, so trailing-slash.spec.ts covers them.
    #[tokio::test]
    #[serial]
    async fn other_addresses_with_a_slash_too_many_answer_404_where_they_are() {
        let _site = Site::with_home_photo();
        for path in [
            "/no-such-page/",
            "/gallery/a/b/c/",
            "/gallery/home//photo",
            "/pkg/",
        ] {
            let (status, body) = get(path).await;
            assert_eq!(status, StatusCode::NOT_FOUND, "{path}");
            assert!(
                without_markers(&body)
                    .contains(&format!(r#"<code class="not-found-path">{path}</code>"#)),
                "{path}: {body}"
            );
        }
    }

    #[tokio::test]
    #[serial]
    async fn home_answers_200_with_its_photos() {
        let _site = Site::with_home_photo();
        let (status, headers, body) = get_with_headers("/").await;
        assert_eq!(status, StatusCode::OK);
        assert!(headers.get(RETRY_AFTER).is_none());
        assert!(body.contains(r#"href="/gallery/home/photo""#), "{body}");
    }

    /// During an outage the home gallery is the empty directory the server made for
    /// itself. Shown as an empty gallery, both pages would answer 200 with "No
    /// photos found".
    #[tokio::test]
    #[serial]
    async fn home_and_home_gallery_during_an_outage_answer_503_with_retry_after() {
        let _site = Site::during_outage();
        for (path, message) in [
            ("/", "Failed to load photos"),
            ("/gallery/home", "Failed to load gallery"),
        ] {
            let (status, headers, body) = get_with_headers(path).await;
            assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{path}");
            assert_eq!(
                headers.get(RETRY_AFTER).and_then(|v| v.to_str().ok()),
                Some("300"),
                "{path}"
            );
            assert!(body.contains(message), "{path}: {body}");
            assert!(!body.contains("No photos found"), "{path}: {body}");
        }
    }

    /// Nothing keeps the outage: the first visit after the share is back gets the
    /// photos, with no wait for a cache to expire.
    #[tokio::test]
    #[serial]
    async fn home_and_home_gallery_recover_as_soon_as_the_share_is_back() {
        let site = Site::during_outage();
        for path in ["/", "/gallery/home"] {
            let (status, _) = get(path).await;
            assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{path}");
        }
        site.share_comes_back();
        for path in ["/", "/gallery/home"] {
            let (status, headers, body) = get_with_headers(path).await;
            assert_eq!(status, StatusCode::OK, "{path}");
            assert!(headers.get(RETRY_AFTER).is_none(), "{path}");
            assert!(
                body.contains(r#"href="/gallery/home/photo""#),
                "{path}: {body}"
            );
        }
    }

    #[tokio::test]
    #[serial]
    async fn an_empty_home_gallery_on_a_mounted_share_answers_200() {
        let _site = Site::with_empty_home();
        for (path, message) in [
            ("/", "No photos found."),
            ("/gallery/home", "No photos found in this gallery."),
        ] {
            let (status, headers, body) = get_with_headers(path).await;
            assert_eq!(status, StatusCode::OK, "{path}");
            assert!(headers.get(RETRY_AFTER).is_none(), "{path}");
            assert!(body.contains(message), "{path}: {body}");
        }
    }

    /// Guards the home grid's `load_failed` check, as `a_loading_photo_page_marks_nothing`
    /// does the photo page's. With `SuppressResourceLoad` holding the photos pending,
    /// the Suspense's first walk over its children on the server builds the grid's
    /// error branch, and that pass must not mark the page 503. The page tests never
    /// see it, since the home loader resolves on its first poll.
    #[tokio::test]
    #[serial]
    async fn a_loading_home_grid_marks_nothing() {
        use leptos::prelude::{Owner, RenderHtml, provide_context, view};

        let _site = Site::with_home_photo();
        // Sets up leptos's executor.
        let _ = generate_route_list(App);
        let response = leptos_axum::ResponseOptions::default();
        Owner::new().with(|| {
            provide_context(response.clone());
            let _pending = leptos::server::SuppressResourceLoad::new();
            let _stream = view! { <super::PhotoGridLoader /> }.to_html_stream_in_order();
        });
        assert_eq!(response.0.read().unwrap().status, None);
    }

    /// The not-found page's photo comes from the home gallery, which fails to load
    /// during an outage. The page is still a 404, with no photo, and the missing
    /// photos aren't remembered once the share is back.
    #[tokio::test]
    #[serial]
    async fn not_found_during_an_outage_answers_404_without_a_photo() {
        let site = Site::during_outage();
        let (status, body) = get("/no-such-page").await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert!(body.contains(r#"class="not-found-page""#), "{body}");
        assert!(!body.contains("not-found-photo"), "{body}");

        site.share_comes_back();
        let (status, body) = get("/no-such-page").await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert!(body.contains(r#"href="/gallery/home/photo""#), "{body}");
    }
}
