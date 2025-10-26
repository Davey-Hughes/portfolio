use crate::config::SiteConfig;
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
use serde::{Deserialize, Serialize};

#[cfg(feature = "ssr")]
use std::fs;
#[cfg(feature = "ssr")]
use std::path::Path;

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
            let pathname = location.pathname.get();
            if pathname.starts_with("/photo/") {
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
                                                    "Add photos to " <code>"public/images/gallery/"</code>
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
struct PhotoParams {
    id: String,
}

#[component]
fn PhotoDetailPage() -> impl IntoView {
    let params = use_params::<PhotoParams>();
    let photos = Resource::new(|| (), |()| async { get_gallery_photos().await });
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

    let on_touch_start = move |ev: leptos::ev::TouchEvent| {
        #[cfg(feature = "hydrate")]
        {
            use leptos::wasm_bindgen::JsCast;
            let touch_event = ev.unchecked_ref::<web_sys::TouchEvent>();
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

    let on_touch_move = move |ev: leptos::ev::TouchEvent| {
        #[cfg(feature = "hydrate")]
        {
            use leptos::wasm_bindgen::JsCast;
            let touch_event = ev.unchecked_ref::<web_sys::TouchEvent>();
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
                                    let has_any_contact = cfg.contact_email.is_some()
                                        || cfg.contact_phone.is_some()
                                        || cfg.contact_location.is_some();
                                    if has_any_contact {

                                        view! {
                                            <div class="contact-info">
                                                <h2>"Contact Information"</h2>
                                                {cfg
                                                    .contact_email
                                                    .as_ref()
                                                    .map(|email| {
                                                        view! {
                                                            <div class="contact-item">
                                                                <strong>"Email:"</strong>
                                                                <p>{email.clone()}</p>
                                                            </div>
                                                        }
                                                    })}
                                                {cfg
                                                    .contact_phone
                                                    .as_ref()
                                                    .map(|phone| {
                                                        view! {
                                                            <div class="contact-item">
                                                                <strong>"Phone:"</strong>
                                                                <p>{phone.clone()}</p>
                                                            </div>
                                                        }
                                                    })}
                                                {cfg
                                                    .contact_location
                                                    .as_ref()
                                                    .map(|location| {
                                                        view! {
                                                            <div class="contact-item">
                                                                <strong>"Location:"</strong>
                                                                <p>{location.clone()}</p>
                                                            </div>
                                                        }
                                                    })}
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

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct PhotoInfo {
    pub url: String,
    pub title: String,
    pub filename: String,
    pub slug: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub date_taken: Option<String>,
    pub camera_make: Option<String>,
    pub camera_model: Option<String>,
    pub lens_model: Option<String>,
    pub focal_length: Option<String>,
    pub aperture: Option<String>,
    pub shutter_speed: Option<String>,
    pub iso: Option<String>,
}

// Server function to read gallery photos from the gallery directory
#[server(GetGalleryPhotos, "/api")]
pub async fn get_gallery_photos() -> Result<Vec<PhotoInfo>, ServerFnError> {
    // Read from a dedicated images directory that can be mounted/configured at runtime
    // Default to ./images/gallery for production, public/images/gallery for development
    let gallery_path = std::env::var("GALLERY_PATH").unwrap_or_else(|_| {
        // Check if we're in development (public directory exists) or production
        if Path::new("public/images/gallery").exists() {
            "public/images/gallery".to_string()
        } else {
            "./images/gallery".to_string()
        }
    });

    // Create directory if it doesn't exist
    if !Path::new(&gallery_path).exists() {
        fs::create_dir_all(&gallery_path).ok();
    }

    let mut photos = Vec::new();
    let gallery_path_buf = Path::new(&gallery_path).to_path_buf();

    // Recursively find all images
    find_images_recursive(&gallery_path_buf, &gallery_path_buf, &mut photos);

    // Sort by filename for consistent ordering
    photos.sort_by(|a, b| a.filename.cmp(&b.filename));

    Ok(photos)
}

#[cfg(feature = "ssr")]
fn find_images_recursive(dir: &Path, gallery_root: &Path, photos: &mut Vec<PhotoInfo>) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();

            if path.is_dir() {
                // Recursively search subdirectories
                find_images_recursive(&path, gallery_root, photos);
            } else if let Some(extension) = path.extension() {
                let ext = extension.to_string_lossy().to_lowercase();
                if matches!(ext.as_ref(), "jpg" | "jpeg" | "png" | "webp" | "gif") {
                    if let Some(filename) = path.file_name() {
                        let filename_str = filename.to_string_lossy().to_string();

                        // Calculate the relative path from gallery root
                        let relative_path = path
                            .strip_prefix(gallery_root)
                            .unwrap_or(&path)
                            .to_string_lossy()
                            .to_string();

                        // Create slug from relative path (unique identifier)
                        let slug = relative_path
                            .trim_end_matches(&format!(".{}", ext))
                            .to_lowercase()
                            .replace(['/', '\\', ' '], "-");

                        let title = filename_str
                            .trim_end_matches(&format!(".{}", ext))
                            .replace(['-', '_'], " ");

                        // Extract EXIF data
                        let (
                            width,
                            height,
                            date_taken,
                            camera_make,
                            camera_model,
                            lens_model,
                            focal_length,
                            aperture,
                            shutter_speed,
                            iso,
                        ) = extract_exif_data(&path);

                        photos.push(PhotoInfo {
                            url: format!("/images/gallery/{}", relative_path),
                            title,
                            filename: filename_str,
                            slug,
                            width,
                            height,
                            date_taken,
                            camera_make,
                            camera_model,
                            lens_model,
                            focal_length,
                            aperture,
                            shutter_speed,
                            iso,
                        });
                    }
                }
            }
        }
    }
}

#[cfg(feature = "ssr")]
type ExifData = (
    Option<u32>,    // width
    Option<u32>,    // height
    Option<String>, // date_taken
    Option<String>, // camera_make
    Option<String>, // camera_model
    Option<String>, // lens_model
    Option<String>, // focal_length
    Option<String>, // aperture
    Option<String>, // shutter_speed
    Option<String>, // iso
);

#[cfg(feature = "ssr")]
fn extract_exif_data(path: &Path) -> ExifData {
    use std::fs::File;
    use std::io::BufReader;

    let Ok(file) = File::open(path) else {
        return (None, None, None, None, None, None, None, None, None, None);
    };

    let mut reader = BufReader::new(file);
    let Ok(exif_reader) = exif::Reader::new().read_from_container(&mut reader) else {
        return (None, None, None, None, None, None, None, None, None, None);
    };

    let mut width = exif_reader
        .get_field(exif::Tag::PixelXDimension, exif::In::PRIMARY)
        .or_else(|| exif_reader.get_field(exif::Tag::ImageWidth, exif::In::PRIMARY))
        .and_then(|f| f.value.get_uint(0));

    let mut height = exif_reader
        .get_field(exif::Tag::PixelYDimension, exif::In::PRIMARY)
        .or_else(|| exif_reader.get_field(exif::Tag::ImageLength, exif::In::PRIMARY))
        .and_then(|f| f.value.get_uint(0));

    // If EXIF didn't have dimensions, try reading image dimensions from file header
    if width.is_none() || height.is_none() {
        if let Ok(reader) = image::ImageReader::open(path) {
            if let Ok(dimensions) = reader.into_dimensions() {
                width = Some(dimensions.0);
                height = Some(dimensions.1);
            }
        }
    }

    let date_taken = exif_reader
        .get_field(exif::Tag::DateTime, exif::In::PRIMARY)
        .or_else(|| exif_reader.get_field(exif::Tag::DateTimeOriginal, exif::In::PRIMARY))
        .map(|f| {
            let datetime_str = f.display_value().to_string();
            // Remove seconds from format "YYYY-MM-DD HH:MM:SS" -> "YYYY-MM-DD HH:MM"
            // or from "YYYY:MM:DD HH:MM:SS" -> "YYYY:MM:DD HH:MM"
            if let Some(last_colon_idx) = datetime_str.rfind(':') {
                datetime_str[..last_colon_idx].to_string()
            } else {
                datetime_str
            }
        });

    let camera_make = exif_reader
        .get_field(exif::Tag::Make, exif::In::PRIMARY)
        .and_then(|f| f.display_value().to_string().into());

    let camera_model = exif_reader
        .get_field(exif::Tag::Model, exif::In::PRIMARY)
        .and_then(|f| f.display_value().to_string().into());

    let lens_model = exif_reader
        .get_field(exif::Tag::LensModel, exif::In::PRIMARY)
        .and_then(|f| f.display_value().to_string().into());

    let focal_length = exif_reader
        .get_field(exif::Tag::FocalLength, exif::In::PRIMARY)
        .map(|f| {
            let val = f.display_value().to_string();
            if val.contains("mm") {
                val
            } else {
                format!("{} mm", val)
            }
        });

    let aperture = exif_reader
        .get_field(exif::Tag::FNumber, exif::In::PRIMARY)
        .map(|f| format!("f/{}", f.display_value()));

    let shutter_speed = exif_reader
        .get_field(exif::Tag::ExposureTime, exif::In::PRIMARY)
        .map(|f| {
            let val = f.display_value().to_string();
            if val.contains("s") {
                val
            } else {
                format!("{} s", val)
            }
        });

    let iso = exif_reader
        .get_field(exif::Tag::PhotographicSensitivity, exif::In::PRIMARY)
        .map(|f| format!("ISO {}", f.display_value().to_string()));

    (
        width,
        height,
        date_taken,
        camera_make,
        camera_model,
        lens_model,
        focal_length,
        aperture,
        shutter_speed,
        iso,
    )
}

// Server function to get site configuration from environment variables
#[server(GetSiteConfig, "/api")]
pub async fn get_site_config() -> Result<SiteConfig, ServerFnError> {
    Ok(crate::config::load_config())
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct AboutContent {
    pub image_url: Option<String>,
    pub content: String,
}

// Server function to load about page content
#[server(GetAboutContent, "/api")]
pub async fn get_about_content() -> Result<AboutContent, ServerFnError> {
    let content_path = std::env::var("ABOUT_CONTENT_PATH").unwrap_or_else(|_| {
        if Path::new("public/content").exists() {
            "public/content".to_string()
        } else {
            "./content".to_string()
        }
    });

    // Create directory if it doesn't exist
    if !Path::new(&content_path).exists() {
        fs::create_dir_all(&content_path).ok();
    }

    // Try to load the about text file
    let text_path = Path::new(&content_path).join("about.txt");
    let content = if text_path.exists() {
        fs::read_to_string(&text_path).unwrap_or_else(|_| {
            "Hello! I'm a passionate photographer specializing in capturing the beauty of everyday moments.\n\n\
            With over 10 years of experience, I've worked on various projects ranging from landscapes to portraits.\n\n\
            My photography style focuses on natural lighting and authentic emotions. \
            I believe every photograph tells a unique story, and I'm here to help you tell yours.".to_string()
        })
    } else {
        "Hello! I'm a passionate photographer specializing in capturing the beauty of everyday moments.\n\n\
        With over 10 years of experience, I've worked on various projects ranging from landscapes to portraits.\n\n\
        My photography style focuses on natural lighting and authentic emotions. \
        I believe every photograph tells a unique story, and I'm here to help you tell yours.".to_string()
    };

    // Try to find an about image (profile.jpg, profile.png, etc.)
    let image_extensions = ["jpg", "jpeg", "png", "webp"];
    let mut image_url = None;

    for ext in &image_extensions {
        let img_path = Path::new(&content_path).join(format!("profile.{}", ext));
        if img_path.exists() {
            // Calculate relative path for the URL
            image_url = Some(format!("/content/profile.{}", ext));
            break;
        }
    }

    // Fallback to /images/profile.jpg if it exists
    if image_url.is_none() && Path::new("public/images/profile.jpg").exists() {
        image_url = Some("/images/profile.jpg".to_string());
    }

    Ok(AboutContent { image_url, content })
}
