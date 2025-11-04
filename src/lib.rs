pub mod app;
pub mod config;
pub mod server;
pub mod types;

#[cfg(feature = "ssr")]
pub mod mosaic;

#[cfg(feature = "ssr")]
pub mod gallery;

#[cfg(feature = "ssr")]
pub mod image_params;

#[cfg(feature = "ssr")]
pub mod image_cache;

#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() {
    use crate::app::*;
    console_error_panic_hook::set_once();
    leptos::mount::hydrate_body(App);
}
