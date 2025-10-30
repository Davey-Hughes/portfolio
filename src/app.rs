use crate::server::*;
use crate::types::PhotoInfo;
use leptos::prelude::*;
use leptos::wasm_bindgen::JsCast;
use leptos::web_sys;
use leptos_meta::{MetaTags, Stylesheet, Title, provide_meta_context};
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
        <Suspense fallback=|| {
            view! { <Title text="Loading..." /> }
        }>
            {move || {
                let title = config
                    .get()
                    .and_then(|result| result.ok())
                    .map(|cfg| cfg.title())
                    .unwrap_or_else(|| "Photography Portfolio".to_string());
                view! { <Title text=title /> }
            }}
        </Suspense>
    }
}

#[component]
pub fn App() -> impl IntoView {
    provide_meta_context();

    view! {
        <Stylesheet id="leptos" href="/pkg/portfolio.css" />
        <PageTitle />

        <Router>
            <Nav />
            <main class="main-content">
                <Routes fallback=|| "Page not found.".into_view()>
                    <Route path=StaticSegment("") view=HomePage />
                    <Route path=(StaticSegment("gallery"), ParamSegment("name")) view=GalleryPage />
                    <Route path=(StaticSegment("photo"), ParamSegment("id")) view=PhotoDetailPage />
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
            if location.pathname.get().starts_with("/photo/") {
                view! { <div></div> }.into_any()
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
        <Suspense fallback=|| ()>
            {move || {
                galleries
                    .get()
                    .and_then(|galleries_result| galleries_result.ok())
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
            <li>
                <A href="/">"Home"</A>
            </li>
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
                    .and_then(|result| result.ok())
                    .map(|cfg| cfg.site_name)
                    .unwrap_or_else(|| "Your Name".to_string());

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
                        .and_then(|result| result.ok())
                        .map(|cfg| cfg.copyright())
                        .unwrap_or_else(|| {
                            "© 2025 Your Photography. All rights reserved.".to_string()
                        });
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
        if ratio > 1.8 {
            "wide-landscape"
        } else if ratio > 1.3 {
            "landscape"
        } else if ratio > 1.05 {
            "landscape-square"
        } else if ratio > 0.95 {
            "square"
        } else if ratio > 0.75 {
            "portrait-square"
        } else if ratio > 0.55 {
            "portrait"
        } else {
            "tall-portrait"
        }
    } else {
        "square"
    }
}

// Helper component for photo grid item
#[component]
fn PhotoGridItem(photo: PhotoInfo) -> impl IntoView {
    let photo_slug = photo.slug.clone();
    let photo_url = photo.url.clone();
    let photo_sources = photo.sources.clone();
    let photo_title = photo.title.clone();
    let orientation_class = orientation_class_from_dimensions(photo.width, photo.height);

    view! {
        <a
            href=format!("/photo/{}", photo_slug)
            class=format!("photo-hero-link {}", orientation_class)
        >
            <div class="photo-hero-section">
                <div class="photo-hero-image">
                    <picture>
                        {photo_sources
                            .into_iter()
                            .map(|source| {
                                view! { <source srcset=source.url type=source.mime_type /> }
                            })
                            .collect_view()} <img src=photo_url alt=photo_title.clone() />
                    </picture>
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
fn PhotoGrid(photos: Vec<PhotoInfo>) -> impl IntoView {
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
        photos
            .into_iter()
            .map(|photo| view! { <PhotoGridItem photo=photo /> })
            .collect_view()
            .into_any()
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
                            .and_then(|result| result.ok())
                            .map(|cfg| (cfg.site_name.to_uppercase(), cfg.site_tagline))
                            .unwrap_or_else(|| (
                                "YOUR NAME".to_string(),
                                "Photography".to_string(),
                            ));

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

// Helper component for photo grid loading
#[component]
fn PhotoGridLoader() -> impl IntoView {
    let photos = Resource::new(|| (), |()| async { get_gallery_photos().await });

    view! {
        <div class="photo-grid-home">
            <Suspense fallback=|| {
                view! { <div class="loading">"Loading photos..."</div> }
            }>
                {move || {
                    match photos.get().and_then(|result| result.ok()) {
                        Some(photo_list) => view! { <PhotoGrid photos=photo_list /> }.into_any(),
                        None => {
                            view! { <div class="error">"Failed to load photos"</div> }.into_any()
                        }
                    }
                }}
            </Suspense>
        </div>
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
    let photos = Resource::new(
        move || Some(gallery_name.clone()),
        |name| async move {
            if let Some(n) = name {
                get_gallery_photos_by_name(n).await
            } else {
                Err(ServerFnError::new("No gallery name provided"))
            }
        },
    );

    view! {
        <div class="photo-grid-home">
            <Suspense fallback=|| {
                view! { <div class="loading">"Loading photos..."</div> }
            }>
                {move || {
                    match photos.get().and_then(|result| result.ok()) {
                        Some(photo_list) if !photo_list.is_empty() => {
                            view! { <PhotoGrid photos=photo_list /> }.into_any()
                        }
                        Some(_) => {
                            view! {
                                <div class="empty-gallery">
                                    <p>"No photos found in this gallery."</p>
                                </div>
                            }
                                .into_any()
                        }
                        None => {
                            view! { <div class="error">"Failed to load gallery"</div> }.into_any()
                        }
                    }
                }}
            </Suspense>
        </div>
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
    id: String,
}

// Helper component for EXIF field display
#[component]
fn ExifField(heading: &'static str, value: String) -> impl IntoView {
    view! {
        <div class="exif-section">
            <h3 class="exif-heading">{heading}</h3>
            <p class="exif-value">{value}</p>
        </div>
    }
}

// Helper component for camera info
#[component]
fn CameraInfo(camera_make: Option<String>, camera_model: Option<String>) -> impl IntoView {
    if camera_make.is_none() && camera_model.is_none() {
        return view! { <div></div> }.into_any();
    }

    view! {
        <div class="exif-section">
            <h3 class="exif-heading">"Camera"</h3>
            {match (camera_make, camera_model) {
                (Some(make), Some(model)) => {
                    view! { <p class="exif-value">{format!("{} {}", make, model)}</p> }.into_any()
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

    view! {
        <div class="exif-section">
            <h3 class="exif-heading">"Settings"</h3>
            <div class="exif-settings">
                {focal_length.map(|fl| view! { <span class="exif-setting">{fl}</span> })}
                {aperture.map(|ap| view! { <span class="exif-setting">{ap}</span> })}
                {shutter_speed.map(|ss| view! { <span class="exif-setting">{ss}</span> })}
                {iso.map(|iso_val| view! { <span class="exif-setting">{iso_val}</span> })}
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
                    .and_then(|result| result.ok())
                    .map(|cfg| cfg.copyright())
                    .unwrap_or_else(|| "© 2025 All rights reserved.".to_string());
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

#[component]
fn PhotoDetailPage() -> impl IntoView {
    let params = use_params::<PhotoParams>();
    let photos = Resource::new(|| (), |()| async { get_all_gallery_photos().await });
    let is_fullscreen = RwSignal::new(false);
    let zoom_level = RwSignal::new(1.0);
    let max_zoom = RwSignal::new(10.0); // Default 10x, will be updated based on image resolution
    let pan_x = RwSignal::new(0.0);
    let pan_y = RwSignal::new(0.0);
    let is_panning = RwSignal::new(false);
    let start_x = RwSignal::new(0.0);
    let start_y = RwSignal::new(0.0);
    let is_details_expanded = RwSignal::new(false);
    let show_zoom_controls = RwSignal::new(true);

    // Create a signal to track viewport width for mobile detection
    let viewport_width = RwSignal::new(0.0);

    #[cfg(feature = "hydrate")]
    {
        use leptos::prelude::Effect;
        // Set initial viewport width on mount
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

    #[cfg(feature = "hydrate")]
    let initial_pinch_distance = RwSignal::new(0.0);
    #[cfg(feature = "hydrate")]
    let initial_zoom = RwSignal::new(1.0);
    #[cfg(feature = "hydrate")]
    let initial_pinch_center_x = RwSignal::new(0.0);
    #[cfg(feature = "hydrate")]
    let initial_pinch_center_y = RwSignal::new(0.0);
    #[cfg(feature = "hydrate")]
    let initial_pan_x = RwSignal::new(0.0);
    #[cfg(feature = "hydrate")]
    let initial_pan_y = RwSignal::new(0.0);
    #[cfg(feature = "hydrate")]
    let hide_controls_timeout: StoredValue<Option<i32>> = StoredValue::new(None);

    #[cfg(feature = "hydrate")]
    let reset_hide_timer = move || {
        show_zoom_controls.set(true);

        // Clear existing timeout
        if let Some(timeout_id) = hide_controls_timeout.get_value() {
            web_sys::window()
                .unwrap()
                .clear_timeout_with_handle(timeout_id);
        }

        // Set new timeout
        let timeout_id = web_sys::window()
            .unwrap()
            .set_timeout_with_callback_and_timeout_and_arguments_0(
                leptos::wasm_bindgen::closure::Closure::once(move || {
                    show_zoom_controls.set(false);
                })
                .into_js_value()
                .unchecked_ref(),
                1000,
            )
            .ok();

        hide_controls_timeout.set_value(timeout_id);
    };

    let toggle_fullscreen = move |_| {
        is_fullscreen.update(|val| *val = !*val);
        // Reset zoom and pan when closing
        if !is_fullscreen.get() {
            zoom_level.set(1.0);
            pan_x.set(0.0);
            pan_y.set(0.0);
        }
        #[cfg(feature = "hydrate")]
        {
            show_zoom_controls.set(true);
            reset_hide_timer();
        }
    };

    let close_fullscreen = move |ev: leptos::ev::MouseEvent| {
        // Only close if clicking the background, not the image
        let target = ev.target();
        if let Some(element) =
            target.and_then(|t: web_sys::EventTarget| t.dyn_into::<web_sys::Element>().ok())
        {
            if element.class_name().contains("fullscreen-overlay") {
                toggle_fullscreen(ev);
            }
        }
    };

    let on_zoom_change = move |ev: leptos::ev::Event| {
        ev.stop_propagation();
        let target = ev.target().unwrap();
        let input = target.dyn_into::<web_sys::HtmlInputElement>().unwrap();
        let new_zoom = input.value().parse::<f64>().unwrap_or(1.0);
        zoom_level.set(new_zoom);
        if (new_zoom - 1.0).abs() < 0.01 {
            pan_x.set(0.0);
            pan_y.set(0.0);
        }
        #[cfg(feature = "hydrate")]
        reset_hide_timer();
    };

    let on_image_click = move |ev: leptos::ev::MouseEvent| {
        ev.stop_propagation();
        // Clicking doesn't zoom when already zoomed (for panning)
        // Use the zoom slider to zoom in/out
    };

    let on_image_dblclick = move |ev: leptos::ev::MouseEvent| {
        ev.stop_propagation();
        ev.prevent_default();
        // Toggle between 1x and 2x zoom
        if (zoom_level.get() - 1.0).abs() < 0.1 {
            #[cfg(feature = "hydrate")]
            {
                // Get the click position relative to the viewport center
                let mouse_event = ev.unchecked_ref::<web_sys::MouseEvent>();
                if let Some(target) = mouse_event.target() {
                    if let Some(element) = target.dyn_ref::<web_sys::Element>() {
                        let rect = element.get_bounding_client_rect();
                        let img_center_x = rect.left() + rect.width() / 2.0;
                        let img_center_y = rect.top() + rect.height() / 2.0;

                        // Calculate offset from center to click point
                        let click_x = f64::from(mouse_event.client_x());
                        let click_y = f64::from(mouse_event.client_y());

                        // Pan to center the clicked point (negated because we're moving the image)
                        let offset_x = (img_center_x - click_x) * 2.0;
                        let offset_y = (img_center_y - click_y) * 2.0;

                        pan_x.set(offset_x);
                        pan_y.set(offset_y);
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
        #[cfg(feature = "hydrate")]
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
                // Get mouse position relative to viewport
                let mouse_x = f64::from(ev.client_x());
                let mouse_y = f64::from(ev.client_y());

                // Get the image element to calculate position relative to it
                if let Some(target) = ev.target() {
                    if let Some(img) = target.dyn_ref::<web_sys::Element>() {
                        let rect = img.get_bounding_client_rect();

                        // Calculate mouse position relative to the image center
                        let img_center_x = rect.left() + (rect.width() / 2.0);
                        let img_center_y = rect.top() + (rect.height() / 2.0);

                        // Offset from mouse to current image center
                        let offset_x = mouse_x - img_center_x;
                        let offset_y = mouse_y - img_center_y;

                        // Calculate the zoom ratio
                        let zoom_ratio = new_zoom / old_zoom;

                        // Adjust pan to zoom towards cursor
                        let current_pan_x = pan_x.get();
                        let current_pan_y = pan_y.get();

                        let new_pan_x = current_pan_x + offset_x * (1.0 - zoom_ratio);
                        let new_pan_y = current_pan_y + offset_y * (1.0 - zoom_ratio);

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
    };

    let on_touch_start = move |_ev: leptos::ev::TouchEvent| {
        #[cfg(feature = "hydrate")]
        {
            use leptos::wasm_bindgen::JsCast;
            let touch_event = _ev.unchecked_ref::<web_sys::TouchEvent>();
            let touches = touch_event.touches();
            if touches.length() == 2 {
                // Pinch zoom starting
                touch_event.prevent_default();
                let touch0 = touches.get(0).unwrap();
                let touch1 = touches.get(1).unwrap();
                let dx = f64::from(touch1.client_x() - touch0.client_x());
                let dy = f64::from(touch1.client_y() - touch0.client_y());
                let distance = (dx * dx + dy * dy).sqrt();

                // Calculate the center point between the two touches
                let center_x = (f64::from(touch0.client_x()) + f64::from(touch1.client_x())) / 2.0;
                let center_y = (f64::from(touch0.client_y()) + f64::from(touch1.client_y())) / 2.0;

                initial_pinch_distance.set(distance);
                initial_zoom.set(zoom_level.get());
                initial_pinch_center_x.set(center_x);
                initial_pinch_center_y.set(center_y);
                initial_pan_x.set(pan_x.get());
                initial_pan_y.set(pan_y.get());
            } else if touches.length() == 1 && zoom_level.get() > 1.0 {
                // Single finger panning
                touch_event.prevent_default();
                let touch = touches.get(0).unwrap();
                is_panning.set(true);
                start_x.set(f64::from(touch.client_x()) - pan_x.get());
                start_y.set(f64::from(touch.client_y()) - pan_y.get());
            }
        }
    };

    let on_touch_move = move |_ev: leptos::ev::TouchEvent| {
        #[cfg(feature = "hydrate")]
        {
            use leptos::wasm_bindgen::JsCast;
            let touch_event = _ev.unchecked_ref::<web_sys::TouchEvent>();
            let touches = touch_event.touches();
            if touches.length() == 2 {
                // Pinch zoom
                touch_event.prevent_default();
                let touch0 = touches.get(0).unwrap();
                let touch1 = touches.get(1).unwrap();
                let dx = f64::from(touch1.client_x() - touch0.client_x());
                let dy = f64::from(touch1.client_y() - touch0.client_y());
                let distance = (dx * dx + dy * dy).sqrt();

                let scale = distance / initial_pinch_distance.get();
                let new_zoom = (initial_zoom.get() * scale).clamp(1.0, max_zoom.get());

                if (new_zoom - 1.0).abs() < 0.01 {
                    zoom_level.set(1.0);
                    pan_x.set(0.0);
                    pan_y.set(0.0);
                } else {
                    // Get viewport center
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

                    // Calculate offset from viewport center to pinch center
                    let offset_x = initial_pinch_center_x.get() - viewport_center_x;
                    let offset_y = initial_pinch_center_y.get() - viewport_center_y;

                    // Calculate zoom ratio
                    let zoom_ratio = new_zoom / initial_zoom.get();

                    // Adjust pan to keep the pinch point stationary
                    let new_pan_x =
                        initial_pan_x.get() * zoom_ratio - offset_x * (zoom_ratio - 1.0);
                    let new_pan_y =
                        initial_pan_y.get() * zoom_ratio - offset_y * (zoom_ratio - 1.0);

                    zoom_level.set(new_zoom);
                    pan_x.set(new_pan_x);
                    pan_y.set(new_pan_y);
                }
            } else if touches.length() == 1 && is_panning.get() && zoom_level.get() > 1.0 {
                // Single finger panning
                touch_event.prevent_default();
                let touch = touches.get(0).unwrap();
                pan_x.set(f64::from(touch.client_x()) - start_x.get());
                pan_y.set(f64::from(touch.client_y()) - start_y.get());
            }
            reset_hide_timer();
        }
    };

    let on_touch_end = move |_ev: leptos::ev::TouchEvent| {
        is_panning.set(false);
    };

    view! {
        <div class="photo-detail-page">
            <Suspense fallback=|| {
                view! { <div class="loading">"Loading photo..."</div> }
            }>
                {move || {
                    let Some(slug_val) = params.get().ok().map(|p| p.id) else {
                        return // Get slug from params
                        view! { <InvalidPhotoId /> }
                            .into_any();
                    };
                    let Some(photo_list) = photos.get().and_then(|result| result.ok()) else {
                        return // Get photo list
                        view! { <div class="error">"Failed to load photo"</div> }
                            .into_any();
                    };
                    let Some((idx, photo)) = photo_list
                        .iter()
                        .enumerate()
                        .find(|(_, p)| p.slug == slug_val) else {
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
                    let is_mobile = move || {
                        viewport_width.get() <= 768.0 && viewport_width.get() > 0.0
                    };
                    let photo_url_cached = StoredValue::new(photo.url.clone());
                    let photo_url_original = StoredValue::new(photo.original_url.clone());
                    let photo_sources_cached = StoredValue::new(photo.sources.clone());
                    let photo_sources_original = StoredValue::new(photo.original_sources.clone());
                    let photo_title = photo.title.clone();
                    let photo_title_fs = photo.title.clone();
                    let calculated_max_zoom = match (photo.width, photo.height) {
                        (Some(w), Some(h)) if w > 8000 || h > 8000 => 20.0,
                        (Some(_), Some(_)) => 10.0,
                        _ => 10.0,
                    };
                    max_zoom.set(calculated_max_zoom);
                    view! {
                        <div class="photo-detail-container">
                            <div class="photo-detail-header">
                                <A href="/" attr:class="back-link">
                                    "← Back to Gallery"
                                </A>
                            </div>
                            <div class="photo-detail-content">
                                <div
                                    class="photo-detail-image"
                                    on:click=toggle_fullscreen
                                    style="cursor: pointer;"
                                >
                                    <picture>
                                        {move || {
                                            let sources = if is_mobile() {
                                                photo_sources_cached.get_value()
                                            } else {
                                                photo_sources_original.get_value()
                                            };
                                            sources
                                                .into_iter()
                                                .map(|source| {
                                                    view! { <source srcset=source.url type=source.mime_type /> }
                                                })
                                                .collect_view()
                                        }}
                                        <img
                                            src=move || {
                                                if is_mobile() {
                                                    photo_url_cached.get_value()
                                                } else {
                                                    photo_url_original.get_value()
                                                }
                                            }
                                            alt=photo_title.clone()
                                        />
                                    </picture>
                                </div>
                                <div class="photo-detail-info">
                                    <h1
                                        class="photo-title-toggle"
                                        class:expanded=move || is_details_expanded.get()
                                        on:click=move |_| {
                                            is_details_expanded
                                                .update(|expanded| *expanded = !*expanded)
                                        }
                                    >
                                        {photo_title}
                                    </h1>
                                    <div
                                        class="photo-exif"
                                        class:expanded=move || is_details_expanded.get()
                                    >
                                        {photo
                                            .date_taken
                                            .as_ref()
                                            .map(|date| {
                                                view! { <ExifField heading="Date" value=date.clone() /> }
                                            })}
                                        {if let (Some(width), Some(height)) = (
                                            photo.width,
                                            photo.height,
                                        ) {
                                            let dimensions = format!("{} × {} px", width, height);
                                            view! {
                                                <ExifField heading="Dimensions" value=dimensions />
                                            }
                                                .into_any()
                                        } else {
                                            view! { <div></div> }.into_any()
                                        }}
                                        <CameraInfo
                                            camera_make=photo.camera_make.clone()
                                            camera_model=photo.camera_model.clone()
                                        />
                                        {photo
                                            .lens_model
                                            .as_ref()
                                            .map(|lens| {
                                                view! { <ExifField heading="Lens" value=lens.clone() /> }
                                            })}
                                        <PhotoSettings
                                            focal_length=photo.focal_length.clone()
                                            aperture=photo.aperture.clone()
                                            shutter_speed=photo.shutter_speed.clone()
                                            iso=photo.iso.clone()
                                        />
                                    </div>
                                </div>
                            </div>
                            <div class="photo-navigation">
                                {prev_photo
                                    .map(|prev| {
                                        view! {
                                            <A
                                                href=format!("/photo/{}", prev.slug)
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
                                                href=format!("/photo/{}", next.slug)
                                                attr:class="nav-button nav-next"
                                            >
                                                "Next →"
                                            </A>
                                        }
                                    })}
                            </div>

                            {move || {
                                if is_fullscreen.get() {
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
                                        <div
                                            class="fullscreen-overlay"
                                            on:click=close_fullscreen
                                            on:wheel=on_wheel
                                        >
                                            <div
                                                class="fullscreen-close"
                                                class:hidden=move || !show_zoom_controls.get()
                                                on:click=toggle_fullscreen
                                            >
                                                "✕"
                                            </div>
                                            <div
                                                class="fullscreen-controls"
                                                class:hidden=move || !show_zoom_controls.get()
                                            >
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
                                                        {move || format!("{}×", max_zoom.get() as i32)}
                                                    </label>
                                                </div>
                                            </div>
                                            <picture>
                                                {move || {
                                                    let sources = if is_mobile() {
                                                        photo_sources_cached.get_value()
                                                    } else {
                                                        photo_sources_original.get_value()
                                                    };
                                                    sources
                                                        .into_iter()
                                                        .map(|source| {
                                                            view! { <source srcset=source.url type=source.mime_type /> }
                                                        })
                                                        .collect_view()
                                                }}
                                                <img
                                                    src=move || {
                                                        if is_mobile() {
                                                            photo_url_cached.get_value()
                                                        } else {
                                                            photo_url_original.get_value()
                                                        }
                                                    }
                                                    alt=photo_title_fs.clone()
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
                                } else {
                                    view! { <div></div> }.into_any()
                                }
                            }}
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
                match about_content.get().and_then(|result| result.ok()) {
                    Some(about) => {
                        let paragraphs: Vec<_> = about
                            .content
                            .split("\n\n")
                            .map(|p| p.trim())
                            .filter(|p| !p.is_empty())
                            .collect();

                        view! {
                            <div class="about-container">
                                {about
                                    .image_url
                                    .map(|url| {
                                        view! {
                                            <div class="about-image">
                                                <img src=url alt="Photographer" />
                                            </div>
                                        }
                                    })} <div class="about-content">
                                    <h1>"About Me"</h1>
                                    {paragraphs
                                        .into_iter()
                                        .map(|p| view! { <p>{p}</p> })
                                        .collect_view()}
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
            Some((
                format!("https://instagram.com/{}", handle),
                value.to_string(),
            ))
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
            if let Some(username) = value.split('/').nth(3) {
                if !username.is_empty() {
                    return Some((
                        format!("https://github.com/{}", username),
                        username.to_string(),
                    ));
                }
            }
            None
        } else {
            // Treat as username
            let username = value.trim_start_matches('@');
            Some((
                format!("https://github.com/{}", username),
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
                match config.get().and_then(|result| result.ok()) {
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
