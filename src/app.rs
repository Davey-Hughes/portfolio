use crate::server::*;
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

#[component]
pub fn App() -> impl IntoView {
    provide_meta_context();

    view! {
        <Stylesheet id="leptos" href="/pkg/portfolio.css" />
        <Title text="Photography Portfolio" />

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

#[component]
fn Nav() -> impl IntoView {
    let config = Resource::new(|| (), |()| async { get_site_config().await });
    let galleries = Resource::new(|| (), |()| async { get_galleries().await });

    view! {
        <nav class="navbar">
            <div class="nav-container">
                <Suspense fallback=move || {
                    view! { "Loading..." }
                }>
                    {move || {
                        config
                            .get()
                            .map(|config_result| match config_result {
                                Ok(cfg) => {
                                    view! {
                                        <A href="/" attr:class="nav-brand">
                                            {cfg.site_name.clone()}
                                        </A>
                                        <ul class="nav-links">
                                            <li>
                                                <A href="/">"Home"</A>
                                            </li>
                                            <Suspense fallback=|| ()>
                                                {move || {
                                                    galleries
                                                        .get()
                                                        .and_then(|galleries_result| { galleries_result.ok() })
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
                                            <li>
                                                <A href="/about">"About"</A>
                                            </li>
                                            <li>
                                                <A href="/contact">"Contact"</A>
                                            </li>
                                        </ul>
                                    }
                                        .into_any()
                                }
                                Err(_) => {
                                    view! {
                                        <A href="/" attr:class="nav-brand">
                                            "Your Name"
                                        </A>
                                        <ul class="nav-links">
                                            <li>
                                                <A href="/">"Home"</A>
                                            </li>
                                            <li>
                                                <A href="/about">"About"</A>
                                            </li>
                                            <li>
                                                <A href="/contact">"Contact"</A>
                                            </li>
                                        </ul>
                                    }
                                        .into_any()
                                }
                            })
                    }}
                </Suspense>
            </div>
        </nav>
    }
}

#[component]
fn Footer() -> impl IntoView {
    let config = Resource::new(|| (), |()| async { get_site_config().await });

    view! {
        <footer class="footer">
            <Suspense fallback=move || {
                view! { <p>"© 2025 Your Photography. All rights reserved."</p> }
            }>
                {move || {
                    config
                        .get()
                        .map(|config_result| match config_result {
                            Ok(cfg) => view! { <p>{cfg.site_copyright.clone()}</p> }.into_any(),
                            Err(_) => {
                                view! { <p>"© 2025 Your Photography. All rights reserved."</p> }
                                    .into_any()
                            }
                        })
                }}
            </Suspense>
        </footer>
    }
}

#[component]
fn HomePage() -> impl IntoView {
    let photos = Resource::new(|| (), |()| async { get_gallery_photos().await });
    let config = Resource::new(|| (), |()| async { get_site_config().await });

    view! {
        <div class="home-page">
            <div class="hero-simple">
                <div class="hero-text">
                    <Suspense fallback=move || {
                        view! {
                            <h1>"YOUR NAME"</h1>
                            <p class="hero-tagline">"Photography"</p>
                        }
                    }>
                        {move || {
                            config
                                .get()
                                .map(|config_result| match config_result {
                                    Ok(cfg) => {
                                        view! {
                                            <h1>{cfg.site_name.clone().to_uppercase()}</h1>
                                            <p class="hero-tagline">{cfg.site_tagline.clone()}</p>
                                        }
                                            .into_any()
                                    }
                                    Err(_) => {
                                        view! {
                                            <h1>"YOUR NAME"</h1>
                                            <p class="hero-tagline">"Photography"</p>
                                        }
                                            .into_any()
                                    }
                                })
                        }}
                    </Suspense>
                </div>
            </div>

            <div class="photo-grid-home">
                <Suspense fallback=move || {
                    view! { <div class="loading">"Loading photos..."</div> }
                }>
                    {move || {
                        photos
                            .get()
                            .map(|photos_result| match photos_result {
                                Ok(photo_list) => {
                                    if photo_list.is_empty() {
                                        view! {
                                            <div class="empty-gallery">
                                                <p>"No photos found."</p>
                                                <p class="hint">
                                                    "Add photos to " <code>"public/images/home/"</code>
                                                    " to see them here."
                                                </p>
                                            </div>
                                        }
                                            .into_any()
                                    } else {
                                        photo_list
                                            .into_iter()
                                            .map(|photo| {
                                                let photo_slug = photo.slug.clone();
                                                let photo_url = photo.url.clone();
                                                let photo_title = photo.title.clone();
                                                let orientation_class = if let (Some(w), Some(h)) = (
                                                    photo.width,
                                                    photo.height,
                                                ) {
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
                                                };

                                                // Determine orientation based on width/height with more granular sizing
                                                // Very wide: 2x1
                                                // Moderately wide: 2x1
                                                // Slightly wide: 1x1 (helps fill gaps)
                                                // Nearly square: 1x1
                                                // Slightly tall: 1x1 (helps fill gaps)
                                                // Moderately tall: 1x2
                                                // Very tall: 1x2
                                                // default fallback

                                                view! {
                                                    <a
                                                        href=format!("/photo/{}", photo_slug)
                                                        class=format!("photo-hero-link {}", orientation_class)
                                                    >
                                                        <div class="photo-hero-section">
                                                            <div class="photo-hero-image">
                                                                <img src=photo_url alt=photo_title.clone() />
                                                            </div>
                                                            <div class="photo-hero-caption">
                                                                <h2>{photo_title}</h2>
                                                            </div>
                                                        </div>
                                                    </a>
                                                }
                                            })
                                            .collect_view()
                                            .into_any()
                                    }
                                }
                                Err(_) => {
                                    view! { <div class="error">"Failed to load photos"</div> }
                                        .into_any()
                                }
                            })
                    }}
                </Suspense>
            </div>
        </div>
    }
}

#[derive(Params, PartialEq, Clone)]
struct GalleryParams {
    name: String,
}

#[component]
fn GalleryPage() -> impl IntoView {
    let params = use_params::<GalleryParams>();
    let photos = Resource::new(
        move || params.get().map(|p| p.name.clone()).ok(),
        |gallery_name| async move {
            if let Some(name) = gallery_name {
                get_gallery_photos_by_name(name).await
            } else {
                Err(ServerFnError::new("No gallery name provided"))
            }
        },
    );

    view! {
        <div class="home-page">
            <div class="hero-simple">
                <div class="hero-text">
                    <Suspense fallback=move || {
                        view! {
                            <h1>"GALLERY"</h1>
                            <p class="hero-tagline">"Loading..."</p>
                        }
                    }>
                        {move || {
                            params
                                .get()
                                .ok()
                                .map(|p| {
                                    let gallery_name = p
                                        .name
                                        .replace(['-', '_'], " ")
                                        .split_whitespace()
                                        .map(|word| {
                                            let mut chars = word.chars();
                                            match chars.next() {
                                                None => String::new(),
                                                Some(first) => {
                                                    first.to_uppercase().collect::<String>() + chars.as_str()
                                                }
                                            }
                                        })
                                        .collect::<Vec<_>>()
                                        .join(" ");
                                    view! {
                                        <h1>{gallery_name.to_uppercase()}</h1>
                                        <p class="hero-tagline">
                                            <A href="/">"← Back to Home"</A>
                                        </p>
                                    }
                                })
                        }}
                    </Suspense>
                </div>
            </div>

            <div class="photo-grid-home">
                <Suspense fallback=move || {
                    view! { <div class="loading">"Loading photos..."</div> }
                }>
                    {move || {
                        photos
                            .get()
                            .map(|photos_result| match photos_result {
                                Ok(photo_list) => {
                                    if photo_list.is_empty() {
                                        view! {
                                            <div class="empty-gallery">
                                                <p>"No photos found in this gallery."</p>
                                            </div>
                                        }
                                            .into_any()
                                    } else {
                                        photo_list
                                            .into_iter()
                                            .map(|photo| {
                                                let photo_slug = photo.slug.clone();
                                                let photo_url = photo.url.clone();
                                                let photo_title = photo.title.clone();
                                                let orientation_class = if let (Some(w), Some(h)) = (
                                                    photo.width,
                                                    photo.height,
                                                ) {
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
                                                };
                                                view! {
                                                    <a
                                                        href=format!("/photo/{}", photo_slug)
                                                        class=format!("photo-hero-link {}", orientation_class)
                                                    >
                                                        <div class="photo-hero-section">
                                                            <div class="photo-hero-image">
                                                                <img src=photo_url alt=photo_title.clone() />
                                                            </div>
                                                            <div class="photo-hero-caption">
                                                                <h2>{photo_title}</h2>
                                                            </div>
                                                        </div>
                                                    </a>
                                                }
                                            })
                                            .collect_view()
                                            .into_any()
                                    }
                                }
                                Err(_) => {
                                    view! { <div class="error">"Failed to load gallery"</div> }
                                        .into_any()
                                }
                            })
                    }}
                </Suspense>
            </div>
        </div>
    }
}

#[derive(Params, PartialEq, Clone)]
struct PhotoParams {
    id: String,
}

#[component]
fn PhotoDetailPage() -> impl IntoView {
    let params = use_params::<PhotoParams>();
    let photos = Resource::new(|| (), |()| async { get_all_gallery_photos().await });
    let config = Resource::new(|| (), |()| async { get_site_config().await });
    let is_fullscreen = RwSignal::new(false);
    let zoom_level = RwSignal::new(1.0);
    let pan_x = RwSignal::new(0.0);
    let pan_y = RwSignal::new(0.0);
    let is_panning = RwSignal::new(false);
    let start_x = RwSignal::new(0.0);
    let start_y = RwSignal::new(0.0);
    #[cfg(feature = "hydrate")]
    let initial_pinch_distance = RwSignal::new(0.0);
    #[cfg(feature = "hydrate")]
    let initial_zoom = RwSignal::new(1.0);

    let toggle_fullscreen = move |_| {
        is_fullscreen.update(|val| *val = !*val);
        // Reset zoom and pan when closing
        if !is_fullscreen.get() {
            zoom_level.set(1.0);
            pan_x.set(0.0);
            pan_y.set(0.0);
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
    };

    let on_mouse_up = move |_ev: leptos::ev::MouseEvent| {
        is_panning.set(false);
    };

    let on_wheel = move |ev: leptos::ev::WheelEvent| {
        ev.prevent_default();
        let delta = ev.delta_y();
        if delta < 0.0 {
            zoom_level.update(|z| *z = (*z * 1.1_f64).min(10.0));
        } else {
            zoom_level.update(|z| {
                *z = (*z / 1.1_f64).max(1.0);
                if (*z - 1.0).abs() < 0.01 {
                    pan_x.set(0.0);
                    pan_y.set(0.0);
                }
            });
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
                initial_pinch_distance.set(distance);
                initial_zoom.set(zoom_level.get());
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
                let new_zoom = (initial_zoom.get() * scale).clamp(1.0, 10.0);
                zoom_level.set(new_zoom);

                if (new_zoom - 1.0).abs() < 0.01 {
                    pan_x.set(0.0);
                    pan_y.set(0.0);
                }
            } else if touches.length() == 1 && is_panning.get() && zoom_level.get() > 1.0 {
                // Single finger panning
                touch_event.prevent_default();
                let touch = touches.get(0).unwrap();
                pan_x.set(f64::from(touch.client_x()) - start_x.get());
                pan_y.set(f64::from(touch.client_y()) - start_y.get());
            }
        }
    };

    let on_touch_end = move |_ev: leptos::ev::TouchEvent| {
        is_panning.set(false);
    };

    view! {
        <div class="photo-detail-page">
            <Suspense fallback=move || {
                view! { <div class="loading">"Loading photo..."</div> }
            }>
                {move || {
                    let slug = params.get().map(|p| p.id.clone()).ok();
                    photos
                        .get()
                        .map(move |photos_result| match photos_result {
                            Ok(photo_list) => {
                                if let Some(slug_val) = slug.clone() {
                                    if let Some((idx, photo)) = photo_list
                                        .iter()
                                        .enumerate()
                                        .find(|(_, p)| p.slug == slug_val)
                                    {
                                        let prev_photo = if idx > 0 {
                                            photo_list.get(idx - 1)
                                        } else {
                                            None
                                        };
                                        let next_photo = if idx < photo_list.len() - 1 {
                                            photo_list.get(idx + 1)
                                        } else {
                                            None
                                        };
                                        let photo_url = photo.url.clone();
                                        let photo_url_fs = photo.url.clone();
                                        let photo_title = photo.title.clone();
                                        let photo_title_fs = photo.title.clone();
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
                                                        <img src=photo_url alt=photo_title.clone() />
                                                    </div>
                                                    <div class="photo-detail-info">
                                                        <h1>{photo_title}</h1>
                                                        <div class="photo-exif">
                                                            {photo
                                                                .date_taken
                                                                .as_ref()
                                                                .map(|date| {
                                                                    view! {
                                                                        <div class="exif-section">
                                                                            <h3 class="exif-heading">"Date"</h3>
                                                                            <p class="exif-value">{date.clone()}</p>
                                                                        </div>
                                                                    }
                                                                })}
                                                            {photo
                                                                .camera_make
                                                                .as_ref()
                                                                .or(photo.camera_model.as_ref())
                                                                .map(|_| {
                                                                    view! {
                                                                        <div class="exif-section">
                                                                            <h3 class="exif-heading">"Camera"</h3>
                                                                            {photo
                                                                                .camera_make
                                                                                .as_ref()
                                                                                .zip(photo.camera_model.as_ref())
                                                                                .map(|(make, model)| {
                                                                                    view! {
                                                                                        <p class="exif-value">{format!("{} {}", make, model)}</p>
                                                                                    }
                                                                                })
                                                                                .or_else(|| {
                                                                                    photo
                                                                                        .camera_model
                                                                                        .as_ref()
                                                                                        .map(|model| {
                                                                                            view! { <p class="exif-value">{model.clone()}</p> }
                                                                                        })
                                                                                })}
                                                                        </div>
                                                                    }
                                                                })}
                                                            {photo
                                                                .lens_model
                                                                .as_ref()
                                                                .map(|lens| {
                                                                    view! {
                                                                        <div class="exif-section">
                                                                            <h3 class="exif-heading">"Lens"</h3>
                                                                            <p class="exif-value">{lens.clone()}</p>
                                                                        </div>
                                                                    }
                                                                })}
                                                            {if photo.focal_length.is_some() || photo.aperture.is_some()
                                                                || photo.shutter_speed.is_some() || photo.iso.is_some()
                                                            {
                                                                view! {
                                                                    <div class="exif-section">
                                                                        <h3 class="exif-heading">"Settings"</h3>
                                                                        <div class="exif-settings">
                                                                            {photo
                                                                                .focal_length
                                                                                .as_ref()
                                                                                .map(|fl| {
                                                                                    view! { <span class="exif-setting">{fl.clone()}</span> }
                                                                                })}
                                                                            {photo
                                                                                .aperture
                                                                                .as_ref()
                                                                                .map(|ap| {
                                                                                    view! { <span class="exif-setting">{ap.clone()}</span> }
                                                                                })}
                                                                            {photo
                                                                                .shutter_speed
                                                                                .as_ref()
                                                                                .map(|ss| {
                                                                                    view! { <span class="exif-setting">{ss.clone()}</span> }
                                                                                })}
                                                                            {photo
                                                                                .iso
                                                                                .as_ref()
                                                                                .map(|iso| {
                                                                                    view! { <span class="exif-setting">{iso.clone()}</span> }
                                                                                })}
                                                                        </div>
                                                                    </div>
                                                                }
                                                                    .into_any()
                                                            } else {
                                                                view! { <div></div> }.into_any()
                                                            }}
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
                                                        <Suspense fallback=move || {
                                                            view! { <p>"© 2025 All rights reserved."</p> }
                                                        }>
                                                            {move || {
                                                                config
                                                                    .get()
                                                                    .map(|config_result| match config_result {
                                                                        Ok(cfg) => {
                                                                            view! { <p>{cfg.site_copyright.clone()}</p> }.into_any()
                                                                        }
                                                                        Err(_) => {
                                                                            view! { <p>"© 2025 All rights reserved."</p> }.into_any()
                                                                        }
                                                                    })
                                                            }}
                                                        </Suspense>
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
                                                                <div class="fullscreen-close" on:click=toggle_fullscreen>
                                                                    "✕"
                                                                </div>
                                                                <div class="fullscreen-controls">
                                                                    <div class="zoom-slider-container">
                                                                        <label class="zoom-label">"1×"</label>
                                                                        <input
                                                                            type="range"
                                                                            class="zoom-slider"
                                                                            min="1.0"
                                                                            max="10.0"
                                                                            step="0.1"
                                                                            prop:value=move || zoom_level.get()
                                                                            on:input=on_zoom_change
                                                                        />
                                                                        <label class="zoom-label">"10×"</label>
                                                                    </div>
                                                                </div>
                                                                <img
                                                                    src=photo_url_fs.clone()
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
                                    } else {
                                        view! {
                                            <div class="error">
                                                <p>"Photo not found"</p>
                                                <A href="/">"Return to Gallery"</A>
                                            </div>
                                        }
                                            .into_any()
                                    }
                                } else {
                                    view! {
                                        <div class="error">
                                            <p>"Invalid photo ID"</p>
                                            <A href="/">"Return to Gallery"</A>
                                        </div>
                                    }
                                        .into_any()
                                }
                            }
                            Err(_) => {
                                view! { <div class="error">"Failed to load photo"</div> }.into_any()
                            }
                        })
                }}
            </Suspense>
        </div>
    }
}

#[component]
fn AboutPage() -> impl IntoView {
    let about_content = Resource::new(|| (), |()| async { get_about_content().await });

    view! {
        <div class="about-page">
            <Suspense fallback=move || {
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
                    about_content
                        .get()
                        .map(|content_result| match content_result {
                            Ok(about) => {
                                let paragraphs = about
                                    .content
                                    .split("\n\n")
                                    .map(|p| p.trim())
                                    .filter(|p| !p.is_empty())
                                    .collect::<Vec<_>>();
                                view! {
                                    <div class="about-container">
                                        {about
                                            .image_url
                                            .as_ref()
                                            .map(|url| {
                                                view! {
                                                    <div class="about-image">
                                                        <img src=url.clone() alt="Photographer" />
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
                            Err(_) => {
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
                        })
                }}
            </Suspense>
        </div>
    }
}

#[component]
fn ContactPage() -> impl IntoView {
    let name = RwSignal::new(String::new());
    let email = RwSignal::new(String::new());
    let message = RwSignal::new(String::new());
    let submitted = RwSignal::new(false);
    let config = Resource::new(|| (), |()| async { get_site_config().await });

    let on_submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        // Here you would normally handle form submission to a server
        submitted.set(true);
    };

    view! {
        <div class="contact-page">
            <h1>"Get In Touch"</h1>

            <div class="contact-container">
                <Suspense fallback=move || {
                    view! { <div class="contact-info"></div> }
                }>
                    {move || {
                        config
                            .get()
                            .map(|config_result| match config_result {
                                Ok(cfg) => {
                                    if !cfg.sections.is_empty() {
                                        let mut sections_vec: Vec<_> = cfg
                                            .sections
                                            .clone()
                                            .into_iter()
                                            .collect();
                                        sections_vec.sort_by(|a, b| a.0.cmp(&b.0));
                                        view! {
                                            <div class="contact-info">
                                                <h2>"Contact Information"</h2>
                                                {sections_vec
                                                    .into_iter()
                                                    .map(|(key, value)| {
                                                        let display_key = key
                                                            .replace('_', " ")
                                                            .split_whitespace()
                                                            .map(|word| {
                                                                let mut chars = word.chars();
                                                                match chars.next() {
                                                                    None => String::new(),
                                                                    Some(first) => {
                                                                        first.to_uppercase().collect::<String>() + chars.as_str()
                                                                    }
                                                                }
                                                            })
                                                            .collect::<Vec<_>>()
                                                            .join(" ");
                                                        let is_instagram = key.to_lowercase().contains("instagram");
                                                        let instagram_url = if is_instagram
                                                            && value.starts_with('@')
                                                        {
                                                            Some(format!("https://instagram.com/{}", &value[1..]))
                                                        } else if is_instagram && !value.starts_with("http") {
                                                            Some(
                                                                format!(
                                                                    "https://instagram.com/{}",
                                                                    value.trim_start_matches('@'),
                                                                ),
                                                            )
                                                        } else {
                                                            None
                                                        };

                                                        // Check if this is an Instagram handle

                                                        view! {
                                                            <div class="contact-item">
                                                                <strong>{display_key}":"</strong>
                                                                {if let Some(url) = instagram_url {
                                                                    view! {
                                                                        <p>
                                                                            <a href=url target="_blank" rel="noopener noreferrer">
                                                                                {value}
                                                                            </a>
                                                                        </p>
                                                                    }
                                                                        .into_any()
                                                                } else {
                                                                    view! { <p>{value}</p> }.into_any()
                                                                }}
                                                            </div>
                                                        }
                                                    })
                                                    .collect_view()}
                                            </div>
                                        }
                                            .into_any()
                                    } else {
                                        view! { <div class="contact-info"></div> }.into_any()
                                    }
                                }
                                Err(_) => view! { <div class="contact-info"></div> }.into_any(),
                            })
                    }}
                </Suspense>

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
            </div>
        </div>
    }
}
