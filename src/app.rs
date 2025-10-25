use crate::config::SiteConfig;
use leptos::prelude::*;
use leptos_meta::{provide_meta_context, MetaTags, Stylesheet, Title};
use leptos_router::{
    components::{Route, Router, Routes, A},
    hooks::use_params,
    params::Params,
    ParamSegment, StaticSegment,
};
use serde::{Deserialize, Serialize};

#[cfg(feature = "ssr")]
use std::fs;
#[cfg(feature = "ssr")]
use std::path::Path;

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
            <Footer />
        </Router>
    }
}

#[component]
fn Nav() -> impl IntoView {
    let config = Resource::new(|| (), |_| async { get_site_config().await });

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
    let config = Resource::new(|| (), |_| async { get_site_config().await });

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
    let photos = Resource::new(|| (), |_| async { get_gallery_photos().await });
    let config = Resource::new(|| (), |_| async { get_site_config().await });

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
                                            .enumerate()
                                            .map(|(idx, photo)| {
                                                let photo_id = idx.to_string();
                                                let photo_url = photo.url.clone();
                                                let photo_title = photo.title.clone();
                                                view! {
                                                    <A
                                                        href=format!("/photo/{}", photo_id)
                                                        attr:class="photo-hero-link"
                                                    >
                                                        <div class="photo-hero-section">
                                                            <div class="photo-hero-image">
                                                                <img src=photo_url alt=photo_title.clone() />
                                                            </div>
                                                            <div class="photo-hero-caption">
                                                                <h2>{photo_title}</h2>
                                                            </div>
                                                        </div>
                                                    </A>
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
    let photos = Resource::new(|| (), |_| async { get_gallery_photos().await });

    view! {
        <div class="photo-detail-page">
            <Suspense fallback=move || {
                view! { <div class="loading">"Loading photo..."</div> }
            }>
                {move || {
                    let id_result = params.get().map(|p| p.id.parse::<usize>().ok()).ok().flatten();
                    photos
                        .get()
                        .map(move |photos_result| match photos_result {
                            Ok(photo_list) => {
                                if let Some(id) = id_result {
                                    if let Some(photo) = photo_list.get(id) {
                                        let prev_id = if id > 0 { Some(id - 1) } else { None };
                                        let next_id = if id < photo_list.len() - 1 {
                                            Some(id + 1)
                                        } else {
                                            None
                                        };
                                        view! {
                                            <div class="photo-detail-container">
                                                <div class="photo-detail-header">
                                                    <A href="/" attr:class="back-link">
                                                        "← Back to Gallery"
                                                    </A>
                                                </div>
                                                <div class="photo-detail-content">
                                                    <div class="photo-detail-image">
                                                        <img src=photo.url.clone() alt=photo.title.clone() />
                                                    </div>
                                                    <div class="photo-detail-info">
                                                        <h1>{photo.title.clone()}</h1>
                                                    </div>
                                                </div>
                                                <div class="photo-navigation">
                                                    {prev_id
                                                        .map(|prev| {
                                                            view! {
                                                                <A
                                                                    href=format!("/photo/{}", prev)
                                                                    attr:class="nav-button nav-prev"
                                                                >
                                                                    "← Previous"
                                                                </A>
                                                            }
                                                        })}
                                                    {next_id
                                                        .map(|next| {
                                                            view! {
                                                                <A
                                                                    href=format!("/photo/{}", next)
                                                                    attr:class="nav-button nav-next"
                                                                >
                                                                    "Next →"
                                                                </A>
                                                            }
                                                        })}
                                                </div>
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
    view! {
        <div class="about-page">
            <div class="about-container">
                <div class="about-image">
                    <img src="/images/profile.jpg" alt="Photographer" />
                </div>
                <div class="about-content">
                    <h1>"About Me"</h1>
                    <p>
                        "Hello! I'm a passionate photographer specializing in capturing the beauty of everyday moments.
                        With over 10 years of experience, I've worked on various projects ranging from landscapes to portraits."
                    </p>
                    <p>
                        "My photography style focuses on natural lighting and authentic emotions.
                        I believe every photograph tells a unique story, and I'm here to help you tell yours."
                    </p>
                </div>
            </div>
        </div>
    }
}

#[component]
fn ContactPage() -> impl IntoView {
    let name = RwSignal::new(String::new());
    let email = RwSignal::new(String::new());
    let message = RwSignal::new(String::new());
    let submitted = RwSignal::new(false);
    let config = Resource::new(|| (), |_| async { get_site_config().await });

    let on_submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        // Here you would normally handle form submission to a server
        submitted.set(true);
    };

    view! {
        <div class="contact-page">
            <h1>"Get In Touch"</h1>

            <div class="contact-container">
                <Suspense fallback=move || view! { <div class="contact-info"></div> }>
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
}

// Server function to read gallery photos from the gallery directory
#[server(GetGalleryPhotos, "/api")]
pub async fn get_gallery_photos() -> Result<Vec<PhotoInfo>, ServerFnError> {
    let gallery_path = "public/images/gallery";

    // Create directory if it doesn't exist
    if !Path::new(gallery_path).exists() {
        fs::create_dir_all(gallery_path).ok();
    }

    let mut photos = Vec::new();

    if let Ok(entries) = fs::read_dir(gallery_path) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(extension) = path.extension() {
                let ext = extension.to_string_lossy().to_lowercase();
                if matches!(ext.as_ref(), "jpg" | "jpeg" | "png" | "webp" | "gif") {
                    if let Some(filename) = path.file_name() {
                        let filename_str = filename.to_string_lossy().to_string();
                        let title = filename_str
                            .trim_end_matches(&format!(".{}", ext))
                            .replace('-', " ")
                            .replace('_', " ");

                        photos.push(PhotoInfo {
                            url: format!("/images/gallery/{}", filename_str),
                            title,
                            filename: filename_str,
                        });
                    }
                }
            }
        }
    }

    // Sort by filename for consistent ordering
    photos.sort_by(|a, b| a.filename.cmp(&b.filename));

    Ok(photos)
}

// Server function to read category hero images
#[server(GetCategoryImages, "/api")]
pub async fn get_category_images() -> Result<Vec<String>, ServerFnError> {
    let categories_path = "public/images/categories";

    // Create directory if it doesn't exist
    if !Path::new(categories_path).exists() {
        fs::create_dir_all(categories_path).ok();
    }

    let expected_categories = vec![
        "portraits-hero.jpg",
        "landscapes-hero.jpg",
        "wildlife-hero.jpg",
    ];
    let mut found_images = Vec::new();

    for category in expected_categories {
        let path = format!("{}/{}", categories_path, category);
        if Path::new(&path).exists() {
            found_images.push(format!("/images/categories/{}", category));
        } else {
            // Use placeholder if not found
            found_images.push(format!("/images/{}", category));
        }
    }

    Ok(found_images)
}

// Server function to get site configuration from environment variables
#[server(GetSiteConfig, "/api")]
pub async fn get_site_config() -> Result<SiteConfig, ServerFnError> {
    Ok(crate::config::load_config())
}
