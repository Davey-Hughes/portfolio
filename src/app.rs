use crate::server::{
    get_about_content, get_all_gallery_photos, get_galleries, get_gallery_data_by_name,
    get_home_gallery_config, get_home_gallery_data, get_site_config,
};
use crate::types::PhotoInfo;
use leptos::prelude::*;
use leptos::wasm_bindgen::JsCast;
use leptos::web_sys;
use leptos_meta::{HashedStylesheet, MetaTags, Title, provide_meta_context};
use leptos_router::{
    ParamSegment, StaticSegment,
    components::{A, Route, Router, Routes},
    hooks::{use_location, use_params},
    params::Params,
};

#[must_use]
pub fn shell(options: LeptosOptions) -> impl IntoView {
    view! {
        <!DOCTYPE html>
        <html lang="en">
            <head>
                <meta charset="utf-8" />
                <meta name="viewport" content="width=device-width, initial-scale=1" />
                <link rel="icon" type="image/x-icon" href="/images/favicon.ico" />
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

// Helper component for page title
#[component]
fn PageTitle() -> impl IntoView {
    let config = Resource::new(|| (), |()| async { get_site_config().await });

    view! {
        <Suspense>
            {move || {
                let title = config
                    .get()
                    .and_then(std::result::Result::ok)
                    .map_or_else(|| "Photography Portfolio".to_string(), |cfg| cfg.title());
                view! { <Title text=title /> }
            }}
        </Suspense>
    }
}

#[component]
#[must_use]
pub fn App() -> impl IntoView {
    provide_meta_context();

    view! {
        <PageTitle />
        <Router>
            <Nav />
            <main class="main-content">
                <Routes fallback=|| "Page not found.".into_view()>
                    <Route path=StaticSegment("") view=HomePage />
                    <Route path=(StaticSegment("gallery"), ParamSegment("name")) view=GalleryPage />
                    <Route
                        path=(
                            StaticSegment("gallery"),
                            ParamSegment("gallery"),
                            ParamSegment("photo"),
                        )
                        view=PhotoDetailPage
                    />
                    <Route path=StaticSegment("about") view=AboutPage />
                    <Route path=StaticSegment("contact") view=ContactPage />
                </Routes>
            </main>
            <ConditionalFooter />
        </Router>
    }
}

#[component]
fn ConditionalFooter() -> impl IntoView {
    let location = use_location();

    view! {
        {move || {
            let path = location.pathname.get();
            let is_photo_detail = path.starts_with("/gallery/") && path.matches('/').count() >= 3;
            if is_photo_detail {
                // Hide footer on photo detail pages (format: /gallery/{gallery}/{photo})

                view! { <div></div> }
                    .into_any()
            } else {
                view! { <Footer /> }.into_any()
            }
        }}
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
    let config = Resource::new(|| (), |()| async { get_site_config().await });

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

#[component]
fn Footer() -> impl IntoView {
    let config = Resource::new(|| (), |()| async { get_site_config().await });

    view! {
        <footer class="footer">
            <Suspense fallback=|| {
                view! { <p>"© 2025 Your Photography. All rights reserved."</p> }
            }>
                {move || {
                    let copyright = config
                        .get()
                        .and_then(std::result::Result::ok)
                        .map_or_else(
                            || { "© 2025 Your Photography. All rights reserved.".to_string() },
                            |cfg| cfg.copyright(),
                        );
                    view! { <p>{copyright}</p> }
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
        .map(|fp| format!("object-position: {};", fp.to_css_position()))
        .unwrap_or_default();

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
    #[prop(optional)] mosaic_layout_tablet: Option<crate::types::MosaicLayout>,
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

            // Clone photos for tablet and mobile layouts
            let photos_tablet = photos.clone();
            let photos_mobile = photos.clone();

            view! {
                // Desktop layout (hidden on tablet/mobile)
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

                // Tablet layout (hidden on desktop/mobile)
                {mosaic_layout_tablet
                    .map(|layout_tablet| {
                        let grid_style_tablet = format!(
                            "grid-template-columns: repeat({}, 1fr); grid-template-rows: repeat({}, 1fr); height: {}px; gap: {}px;",
                            layout_tablet.grid_cols,
                            layout_tablet.grid_rows,
                            layout_tablet.container_height,
                            gap,
                        );
                        view! {
                            <div
                                class="photo-grid-mosaic photo-grid-mosaic-tablet"
                                style=grid_style_tablet
                            >
                                {layout_tablet
                                    .cells
                                    .into_iter()
                                    .zip(photos_tablet.into_iter())
                                    .enumerate()
                                    .map(|(idx, (cell, photo))| {
                                        view! {
                                            <MosaicPhotoGridItem photo=photo cell=cell index=idx />
                                        }
                                    })
                                    .collect_view()}
                            </div>
                        }
                    })}

                // Mobile fallback: single column (hidden on tablet/desktop)
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
    let config = Resource::new(|| (), |()| async { get_site_config().await });

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

// Helper component to render PhotoGrid with optional config and layouts
#[component]
fn PhotoGridRenderer(
    photos: Vec<PhotoInfo>,
    config: Option<crate::types::GalleryConfig>,
    mosaic_layout: Option<crate::types::MosaicLayout>,
    mosaic_layout_tablet: Option<crate::types::MosaicLayout>,
) -> impl IntoView {
    match (config, mosaic_layout, mosaic_layout_tablet) {
        (Some(cfg), Some(mosaic), Some(tablet)) => view! { <PhotoGrid photos=photos config=cfg mosaic_layout=mosaic mosaic_layout_tablet=tablet /> }
        .into_any(),
        (Some(cfg), Some(mosaic), None) => {
            view! { <PhotoGrid photos=photos config=cfg mosaic_layout=mosaic /> }.into_any()
        }
        (None, Some(mosaic), Some(tablet)) => view! { <PhotoGrid photos=photos mosaic_layout=mosaic mosaic_layout_tablet=tablet /> }
        .into_any(),
        (None, Some(mosaic), None) => {
            view! { <PhotoGrid photos=photos mosaic_layout=mosaic /> }.into_any()
        }
        (Some(cfg), None, _) => view! { <PhotoGrid photos=photos config=cfg /> }.into_any(),
        (None, None, _) => view! { <PhotoGrid photos=photos /> }.into_any(),
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
                match gallery_data.get().and_then(std::result::Result::ok) {
                    Some(data) => {
                        let data = std::sync::Arc::unwrap_or_clone(data);
                        view! {
                            <PhotoGridRenderer
                                photos=data.photos
                                config=gallery_config
                                mosaic_layout=data.mosaic_layout
                                mosaic_layout_tablet=data.mosaic_layout_tablet
                            />
                        }
                            .into_any()
                    }
                    None => view! { <div class="error">"Failed to load photos"</div> }.into_any(),
                }
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

// Helper component for gallery photos loader
#[component]
fn GalleryPhotosLoader(gallery_name: String) -> impl IntoView {
    let gallery_name_for_data = gallery_name.clone();
    let gallery_data = Resource::new(
        move || Some(gallery_name_for_data.clone()),
        |name| async move {
            if let Some(n) = name {
                get_gallery_data_by_name(n).await
            } else {
                Err(ServerFnError::new("No gallery name provided"))
            }
        },
    );

    let galleries = Resource::new(|| (), |()| async { get_galleries().await });
    let gallery_slug = gallery_name.to_lowercase().replace(' ', "-");

    view! {
        <Suspense fallback=|| {
            view! { <div class="loading">"Loading photos..."</div> }
        }>
            {move || {
                let gallery_config = galleries
                    .get()
                    .and_then(std::result::Result::ok)
                    .and_then(|gallery_list| {
                        gallery_list
                            .into_iter()
                            .find(|g| g.slug == gallery_slug)
                            .and_then(|g| g.config)
                    });
                match gallery_data.get().and_then(std::result::Result::ok) {
                    Some(data) if !data.photos.is_empty() => {
                        let data = std::sync::Arc::unwrap_or_clone(data);
                        view! {
                            <PhotoGridRenderer
                                photos=data.photos
                                config=gallery_config
                                mosaic_layout=data.mosaic_layout
                                mosaic_layout_tablet=data.mosaic_layout_tablet
                            />
                        }
                            .into_any()
                    }
                    Some(_) => {
                        view! {
                            <div class="empty-gallery">
                                <p>"No photos found in this gallery."</p>
                            </div>
                        }
                            .into_any()
                    }
                    None => view! { <div class="error">"Failed to load gallery"</div> }.into_any(),
                }
            }}
        </Suspense>
    }
}

#[component]
fn GalleryPage() -> impl IntoView {
    let params = use_params::<GalleryParams>();

    view! {
        <div class="home-page">
            <Suspense fallback=|| {
                view! {
                    <div class="hero-simple">
                        <div class="hero-text">
                            <h1>"GALLERY"</h1>
                            <p class="hero-tagline">"Loading..."</p>
                        </div>
                    </div>
                }
            }>
                {move || {
                    params
                        .get()
                        .ok()
                        .map(|p| {
                            view! {
                                <GalleryHero gallery_name=p.name.clone() />
                                <GalleryPhotosLoader gallery_name=p.name />
                            }
                        })
                }}
            </Suspense>
        </div>
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
    let has_letters = s.chars().any(|c| c.is_alphabetic());
    let all_uppercase = s
        .chars()
        .filter(|c| c.is_alphabetic())
        .all(|c| c.is_uppercase());

    if has_letters && all_uppercase {
        // Convert to smart title case
        s.split_whitespace()
            .map(|word| {
                let letter_count = word.chars().filter(|c| c.is_alphabetic()).count();
                let has_digit = word.chars().any(|c| c.is_numeric());

                // Keep acronyms (2-4 letters) and words with numbers in uppercase
                if ((2..=4).contains(&letter_count) && !has_digit) || has_digit {
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
    let config = Resource::new(|| (), |()| async { get_site_config().await });

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

// Helper component for photo detail error view
#[component]
fn PhotoNotFound() -> impl IntoView {
    view! {
        <div class="error">
            <p>"Photo not found"</p>
            <A href="/">"Return to Gallery"</A>
        </div>
    }
}

// Helper component for invalid photo ID error
#[component]
fn InvalidPhotoId() -> impl IntoView {
    view! {
        <div class="error">
            <p>"Invalid photo ID"</p>
            <A href="/">"Return to Gallery"</A>
        </div>
    }
}

/// Bundle of signals shared between the zoom/pan event handlers and the
/// fullscreen view. Held by value (RwSignal is Copy).
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

/// Fullscreen image viewer with zoom/pan/pinch controls. Owns all the
/// browser-event closures so the parent `PhotoDetailPage` doesn't have to
/// thread state through ~14 separate handlers.
#[component]
fn FullscreenViewer(
    photo: PhotoInfo,
    state: ZoomPanState,
    viewport_width: RwSignal<f64>,
) -> impl IntoView {
    // Calibrate max zoom for very-high-resolution photos.
    let calculated_max_zoom = match (photo.width, photo.height) {
        (Some(w), Some(h)) if w > 8000 || h > 8000 => 20.0,
        _ => 10.0,
    };
    state.max_zoom.set(calculated_max_zoom);

    let is_mobile = move || viewport_width.get() <= 768.0 && viewport_width.get() > 0.0;
    let photo_url_detail = StoredValue::new(photo.detail_url.clone());
    let photo_url_original = StoredValue::new(photo.original_url.clone());
    let photo_sources_original = StoredValue::new(photo.original_sources.clone());
    let photo_title_fs = photo.title.clone();
    let photo_width = photo.width;
    let photo_height = photo.height;

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
            if let Some(timeout_id) = _hide_controls_timeout.get_value() {
                if let Some(window) = web_sys::window() {
                    window.clear_timeout_with_handle(timeout_id);
                }
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

    // When the user enters fullscreen, kick the auto-hide timer so controls
    // briefly show then fade — mirroring the previous behaviour.
    #[cfg(feature = "hydrate")]
    {
        use leptos::prelude::Effect;
        Effect::new(move |_| {
            if is_fullscreen.get() {
                reset_hide_timer();
            }
        });
    }

    let exit_fullscreen = move || {
        is_fullscreen.set(false);
        state.reset_zoom();
        show_zoom_controls.set(true);
    };

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
                if let Some(target) = mouse_event.target() {
                    if let Some(element) = target.dyn_ref::<web_sys::Element>() {
                        let rect = element.get_bounding_client_rect();
                        let img_center_x = rect.left() + rect.width() / 2.0;
                        let img_center_y = rect.top() + rect.height() / 2.0;
                        let click_x = f64::from(mouse_event.client_x());
                        let click_y = f64::from(mouse_event.client_y());
                        pan_x.set((img_center_x - click_x) * 2.0);
                        pan_y.set((img_center_y - click_y) * 2.0);
                    }
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
                let center_x = (f64::from(touch0.client_x()) + f64::from(touch1.client_x())) / 2.0;
                let center_y = (f64::from(touch0.client_y()) + f64::from(touch1.client_y())) / 2.0;
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
                <div class="fullscreen-overlay" on:click=close_fullscreen on:wheel=on_wheel>
                    <div
                        class="fullscreen-close"
                        class:hidden=move || !show_zoom_controls.get()
                        on:click=on_close_button
                    >
                        "✕"
                    </div>
                    <div class="fullscreen-controls" class:hidden=move || !show_zoom_controls.get()>
                        <div class="zoom-slider-container">
                            <label class="zoom-label">"1×"</label>
                            <input
                                type="range"
                                class="zoom-slider"
                                min="1.0"
                                prop:max=move || max_zoom.get()
                                step="0.1"
                                prop:value=move || zoom_level.get()
                                on:input=on_zoom_change
                            />
                            <label class="zoom-label">
                                {move || {
                                    #[allow(clippy::cast_possible_truncation)]
                                    let zoom_int = max_zoom.get() as i32;
                                    format!("{zoom_int}×")
                                }}
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

    // Set up keyboard navigation
    #[cfg(feature = "hydrate")]
    {
        use leptos::prelude::Effect;
        use leptos_router::hooks::use_navigate;

        Effect::new(move |_| {
            let navigate = use_navigate();

            let handle_keydown = leptos::wasm_bindgen::closure::Closure::wrap(Box::new(
                move |event: web_sys::KeyboardEvent| {
                    let key = event.key();

                    // Handle Escape key to close fullscreen
                    if key == "Escape" && state.is_fullscreen.get() {
                        state.is_fullscreen.set(false);
                        state.reset_zoom();
                        return;
                    }

                    // Get current photo list and params
                    if let (Some(photo_list), Ok(current_params)) =
                        (photos.get().and_then(std::result::Result::ok), params.get())
                    {
                        if let Some((idx, _)) = photo_list
                            .iter()
                            .enumerate()
                            .find(|(_, p)| p.slug == current_params.photo)
                        {
                            match key.as_str() {
                                "ArrowLeft" => {
                                    if idx > 0 {
                                        if let Some(prev) = photo_list.get(idx - 1) {
                                            let url = format!(
                                                "/gallery/{}/{}",
                                                prev.gallery_name, prev.slug
                                            );
                                            navigate(&url, Default::default());
                                        }
                                    }
                                }
                                "ArrowRight" => {
                                    if idx < photo_list.len() - 1 {
                                        if let Some(next) = photo_list.get(idx + 1) {
                                            let url = format!(
                                                "/gallery/{}/{}",
                                                next.gallery_name, next.slug
                                            );
                                            navigate(&url, Default::default());
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                },
            )
                as Box<dyn FnMut(web_sys::KeyboardEvent)>);

            if let Some(document) = web_sys::window().and_then(|w| w.document()) {
                let _ = document.add_event_listener_with_callback(
                    "keydown",
                    handle_keydown.as_ref().unchecked_ref(),
                );
            }

            // Forget the closure to keep it alive for the lifetime of the
            // component (the listener stays registered).
            handle_keydown.forget();
        });
    }

    // Track viewport width for mobile detection. Initialized to a desktop
    // default so SSR matches the first hydration paint.
    let viewport_width = RwSignal::new(1920.0);

    #[cfg(feature = "hydrate")]
    {
        use leptos::prelude::Effect;
        Effect::new(move |_| {
            if let Some(window) = web_sys::window() {
                if let Ok(width) = window.inner_width() {
                    if let Some(width_f64) = width.as_f64() {
                        viewport_width.set(width_f64);
                    }
                }
            }
        });
    }

    let enter_fullscreen = move |_| {
        state.is_fullscreen.set(true);
        state.show_zoom_controls.set(true);
    };

    view! {
        <div class="photo-detail-page">
            <Suspense fallback=|| {
                view! { <div class="loading">"Loading photo..."</div> }
            }>
                {move || {
                    let Some(slug_val) = params.get().ok().map(|p| p.photo) else {
                        return // Get slug from params
                        view! { <InvalidPhotoId /> }
                            .into_any();
                    };
                    let Some(photo_list) = photos.get().and_then(std::result::Result::ok) else {
                        return // Get photo list
                        view! { <div class="error">"Failed to load photo"</div> }
                            .into_any();
                    };
                    let Some((idx, photo)) = photo_list
                        .iter()
                        .enumerate()
                        .find(|(_, p)| p.slug == slug_val)
                        .map(|(i, p)| (i, p.clone())) else {
                        return // Find the photo and its index
                        view! { <PhotoNotFound /> }
                            .into_any();
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
                    view! {
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
                                photo=photo
                                state=state
                                viewport_width=viewport_width
                            />
                        </div>
                    }
                        .into_any()
                }}
            </Suspense>
        </div>
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
                    <div class="about-image">
                        <img src="/images/profile.jpg" alt="Photographer" />
                    </div>
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

#[component]
fn AboutPage() -> impl IntoView {
    view! {
        <div class="about-page">
            <AboutContent />
        </div>
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
    let config = Resource::new(|| (), |()| async { get_site_config().await });

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
