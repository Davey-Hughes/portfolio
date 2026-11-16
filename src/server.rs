use crate::config::SiteConfig;
use crate::open_source::OpenSourceContent;
use crate::types::{AboutContent, AllPhotos, GalleryData, GalleryInfo, PhotoInfo};
use leptos::prelude::*;
use std::sync::Arc;

#[cfg(feature = "ssr")]
use std::sync::Mutex;
#[cfg(feature = "ssr")]
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(feature = "ssr")]
static MOSAIC_CACHE: std::sync::LazyLock<Arc<Mutex<MosaicCache>>> =
    std::sync::LazyLock::new(|| Arc::new(Mutex::new(MosaicCache::new())));

/// A `std::sync::Mutex`, not an async lock, on purpose: `get_all_gallery_photos` must
/// not wait (see its doc comment).
#[cfg(feature = "ssr")]
static ALL_PHOTOS_CACHE: std::sync::LazyLock<Arc<Mutex<AllPhotosCache>>> =
    std::sync::LazyLock::new(|| Arc::new(Mutex::new(AllPhotosCache::new())));

#[cfg(feature = "ssr")]
struct CachedMosaic {
    data: Arc<GalleryData>,
    expires_at: u64, // Unix timestamp
}

#[cfg(feature = "ssr")]
struct MosaicCache {
    entries: std::collections::HashMap<String, CachedMosaic>,
}

#[cfg(feature = "ssr")]
struct CachedAllPhotos {
    all: AllPhotos,
    expires_at: u64, // Unix timestamp
}

#[cfg(feature = "ssr")]
struct AllPhotosCache {
    cached_data: Option<CachedAllPhotos>,
}

#[cfg(feature = "ssr")]
fn now_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

#[cfg(feature = "ssr")]
impl MosaicCache {
    fn new() -> Self {
        Self {
            entries: std::collections::HashMap::new(),
        }
    }

    fn get(&mut self, key: &str) -> Option<Arc<GalleryData>> {
        let now = now_unix_secs();

        if let Some(cached) = self.entries.get(key) {
            if cached.expires_at > now {
                return Some(Arc::clone(&cached.data));
            }

            // Expired, remove it
            self.entries.remove(key);
        }
        None
    }

    fn set(&mut self, key: String, data: Arc<GalleryData>, duration_secs: u64) {
        let expires_at = now_unix_secs() + duration_secs;
        self.entries.insert(key, CachedMosaic { data, expires_at });
    }

    fn clear_expired(&mut self) {
        let now = now_unix_secs();
        self.entries.retain(|_, cached| cached.expires_at > now);
    }

    fn clear_all(&mut self) {
        self.entries.clear();
    }
}

#[cfg(feature = "ssr")]
impl AllPhotosCache {
    fn new() -> Self {
        Self { cached_data: None }
    }

    fn get(&mut self) -> Option<AllPhotos> {
        let now = now_unix_secs();

        if let Some(cached) = &self.cached_data {
            if cached.expires_at > now {
                return Some(cached.all.clone());
            }

            // Expired, remove it
            self.cached_data = None;
        }
        None
    }

    fn set(&mut self, all: AllPhotos, duration_secs: u64) {
        let expires_at = now_unix_secs() + duration_secs;
        self.cached_data = Some(CachedAllPhotos { all, expires_at });
    }

    fn clear_all(&mut self) {
        self.cached_data = None;
    }
}

#[cfg(feature = "ssr")]
fn image_aspects(photos: &[crate::types::PhotoInfo]) -> Vec<(usize, f64)> {
    photos
        .iter()
        .enumerate()
        .map(|(idx, photo)| {
            let aspect = if let (Some(w), Some(h)) = (photo.width, photo.height) {
                f64::from(w) / f64::from(h)
            } else {
                1.0
            };
            (idx, aspect)
        })
        .collect()
}

#[cfg(feature = "ssr")]
fn generate_mosaic_layout_for_size(
    photos: &[crate::types::PhotoInfo],
    container_width: f64,
    base_height: f64,
) -> (crate::types::MosaicLayout, Vec<usize>) {
    use crate::mosaic::{
        MosaicConfig, calculate_orientation_bias, count_as_f64, generate_mosaic_with_images,
    };

    // How many photos we want to fit per `base_height` slice of the canvas. Used
    // below to scale container height linearly with photo count, so average cell
    // area stays roughly constant as the gallery grows.
    const PHOTOS_PER_BASE_HEIGHT: f64 = 3.0;

    let num_images = photos.len();
    let image_aspects = image_aspects(photos);

    // The floor of 2.0 keeps small galleries from looking cramped.
    let scale = (count_as_f64(num_images) / PHOTOS_PER_BASE_HEIGHT).max(2.0);
    let container_height = base_height * scale;

    // Calculate orientation bias from the actual images
    let orientation_bias = calculate_orientation_bias(&image_aspects);

    let mosaic_config = MosaicConfig {
        container_width,
        container_height,
        min_cell_dimension: 180.0,
        min_aspect_ratio: 0.4,
        max_aspect_ratio: 3.0,
        orientation_bias: Some(orientation_bias),
    };

    generate_mosaic_with_images(num_images, &image_aspects, &mosaic_config, 100)
}

/// Server function to get site configuration
#[server(GetSiteConfig, "/api")]
pub async fn get_site_config() -> Result<SiteConfig, ServerFnError> {
    Ok(crate::config::load_config())
}

/// Server function to get list of available galleries (excluding home)
#[server(GetGalleries, "/api")]
pub async fn get_galleries() -> Result<Vec<GalleryInfo>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        Ok(crate::gallery::load_galleries())
    }
    #[cfg(not(feature = "ssr"))]
    {
        Ok(Vec::new())
    }
}

/// Server function to read gallery photos from the home directory
#[server(GetGalleryPhotos, "/api")]
pub async fn get_gallery_photos() -> Result<Vec<PhotoInfo>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        Ok(crate::gallery::load_home_photos())
    }
    #[cfg(not(feature = "ssr"))]
    {
        Ok(Vec::new())
    }
}

/// Server function to get the home gallery configuration
#[server(GetHomeGalleryConfig, "/api")]
pub async fn get_home_gallery_config() -> Result<Option<crate::types::GalleryConfig>, ServerFnError>
{
    #[cfg(feature = "ssr")]
    {
        let home_path = crate::paths::home_gallery_dir();
        Ok(crate::gallery::load_gallery_config(&home_path))
    }
    #[cfg(not(feature = "ssr"))]
    {
        Ok(None)
    }
}

/// Build `GalleryData` for an already-loaded photo list, applying a mosaic
/// layout when the gallery config requests one. Returns the cache TTL the
/// caller should use along with the result.
#[cfg(feature = "ssr")]
fn build_gallery_data(
    photos: Vec<PhotoInfo>,
    config: Option<&crate::types::GalleryConfig>,
) -> (GalleryData, u64) {
    const MOSAIC_DEFAULT_TTL: u64 = 3600;
    const NON_MOSAIC_TTL: u64 = 300;

    if let Some(cfg) = config
        && cfg.use_mosaic.unwrap_or(false)
        && !photos.is_empty()
    {
        let (layout_desktop, image_order) = generate_mosaic_layout_for_size(&photos, 1200.0, 600.0);

        // The generator assigns exactly one photo per layout cell, so a
        // short `image_order` means some photos got no cell. Reordering by
        // it would silently drop those photos — and the truncated result
        // would be cached for the mosaic TTL (this is the bug that made the
        // film gallery intermittently show a single image). Only use the
        // mosaic when every photo was placed; otherwise fall through to the
        // plain responsive grid, which always renders them all.
        if image_order.len() == photos.len() {
            let reordered_photos: Vec<PhotoInfo> =
                image_order.iter().map(|&idx| photos[idx].clone()).collect();
            let data = GalleryData {
                photos: reordered_photos,
                mosaic_layout: Some(layout_desktop),
            };
            return (
                data,
                cfg.mosaic_cache_duration.unwrap_or(MOSAIC_DEFAULT_TTL),
            );
        }

        leptos::logging::log!(
            "Mosaic layout placed only {} of {} photos; falling back to grid layout",
            image_order.len(),
            photos.len()
        );
    }

    let data = GalleryData {
        photos,
        mosaic_layout: None,
    };
    (data, NON_MOSAIC_TTL)
}

/// Server function to get home gallery data with pre-computed mosaic layout.
///
/// `Err` while the home gallery is empty and the images root looks unmounted:
/// that is an outage, not an empty site.
#[server(GetHomeGalleryData, "/api")]
pub async fn get_home_gallery_data() -> Result<Arc<GalleryData>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::logging::log;

        let cache_key = "home_gallery".to_string();

        match MOSAIC_CACHE.lock() {
            Ok(mut cache) => {
                if let Some(cached) = cache.get(&cache_key) {
                    return Ok(cached);
                }
            }
            Err(_) => log!("MOSAIC_CACHE lock poisoned (read); regenerating"),
        }

        let photos = crate::gallery::load_home_photos();
        // While the root looks unmounted, an empty home gallery is the directory
        // the server made for itself: fail the load, and don't cache it, so the page
        // has its photos back as soon as the share does.
        if photos.is_empty() && images_root_looks_unmounted() {
            return Err(ServerFnError::new("the images root holds no galleries"));
        }
        let home_path = crate::paths::home_gallery_dir();
        let config = crate::gallery::load_gallery_config(&home_path);

        let (data, ttl) = build_gallery_data(photos, config.as_ref());
        let result = Arc::new(data);

        match MOSAIC_CACHE.lock() {
            Ok(mut cache) => cache.set(cache_key, Arc::clone(&result), ttl),
            Err(_) => log!("MOSAIC_CACHE lock poisoned (write); skipping cache"),
        }

        Ok(result)
    }
    #[cfg(not(feature = "ssr"))]
    {
        Ok(Arc::new(GalleryData {
            photos: Vec::new(),
            mosaic_layout: None,
        }))
    }
}

/// [`crate::gallery::images_root_looks_unmounted`] for the server's images root and
/// home gallery.
#[cfg(feature = "ssr")]
fn images_root_looks_unmounted() -> bool {
    crate::gallery::images_root_looks_unmounted(
        &crate::paths::images_root(),
        &crate::paths::home_gallery_dir(),
    )
}

/// How long the all-photos list is cached, in seconds, when it was loaded with the
/// images root mounted: ten minutes. The image watcher clears it sooner when a photo
/// changes.
#[cfg(feature = "ssr")]
const ALL_PHOTOS_TTL: u64 = 600;

/// How long the all-photos list is cached, in seconds, when the images root looked
/// unmounted as it was loaded (`AllPhotos::root_looks_unmounted`): 15 seconds, as
/// long as the HTML cache in `main.rs` keeps a page.
///
/// Short, because such a list may be missing photos that are only out of sight, and
/// the page of each of them answers 503 until the entry expires. Once the share is
/// back, that is no longer than a page served from the HTML cache can already be out
/// of date. Not nothing, because a site whose only gallery is home always looks
/// unmounted, share up or not, as does one whose only galleries are home and the
/// directories on the way to a nested `GALLERY_PATH`. Without a cache, such a site
/// would walk its galleries and read every photo's metadata again for each photo
/// page and each visit within the app; with one, about once per 15 seconds.
#[cfg(feature = "ssr")]
const UNMOUNTED_ALL_PHOTOS_TTL: u64 = 15;

/// How long to cache `all`: [`UNMOUNTED_ALL_PHOTOS_TTL`] when it is marked
/// `root_looks_unmounted`, else [`ALL_PHOTOS_TTL`].
#[cfg(feature = "ssr")]
fn all_photos_ttl(all: &AllPhotos) -> u64 {
    if all.root_looks_unmounted {
        UNMOUNTED_ALL_PHOTOS_TTL
    } else {
        ALL_PHOTOS_TTL
    }
}

/// Every gallery's photos (an expensive walk), marked when the images root looks
/// unmounted, or `None` when it does and there are none. That is an outage, not an
/// empty site.
#[cfg(feature = "ssr")]
fn load_all_photos() -> Option<AllPhotos> {
    let photos = crate::gallery::load_all_gallery_photos();
    let root_looks_unmounted = images_root_looks_unmounted();
    if photos.is_empty() && root_looks_unmounted {
        None
    } else {
        Some(AllPhotos {
            photos: Arc::new(photos),
            root_looks_unmounted,
        })
    }
}

/// Server function to get photos from ALL galleries (for photo detail page)
///
/// While the images root looks unmounted, it is `Err` when no photo is in sight, and
/// that isn't cached. Otherwise it is the photos that are in sight, such as one left
/// in the home gallery's directory under the mountpoint, marked
/// `root_looks_unmounted` and cached for only [`UNMOUNTED_ALL_PHOTOS_TTL`] seconds. A
/// photo page that can't find its photo in such a list answers 503, not 404.
///
/// A site whose only gallery is home always looks unmounted, with the share up too,
/// as does one whose only galleries are home and the directories on the way to a
/// nested `GALLERY_PATH`. Such a site always gets the marked list: there a missing
/// photo answers 503, not 404, and the list is cached only briefly. A missing
/// gallery answers 503 on such a site already (`get_gallery_data_by_name`).
///
/// On the server this must finish without waiting: no `.await` that can be pending,
/// no async lock, no `spawn_blocking`. The photo page counts a shown photo while its
/// Suspense makes its first pass, before the footer is written, and that only works
/// if this has already returned. A photo page whose request waits here goes out with
/// its footer showing, and hydration keeps it. See `Footer` in `app.rs`.
#[server(GetAllGalleryPhotos, "/api")]
pub async fn get_all_gallery_photos() -> Result<AllPhotos, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::logging::log;

        match ALL_PHOTOS_CACHE.lock() {
            Ok(mut cache) => {
                if let Some(cached) = cache.get() {
                    return Ok(cached);
                }
            }
            Err(_) => log!("ALL_PHOTOS_CACHE lock poisoned (read); regenerating"),
        }

        // During an outage with no photo in sight at all, the lookup fails.
        let Some(all) = load_all_photos() else {
            return Err(ServerFnError::new("the images root holds no galleries"));
        };

        match ALL_PHOTOS_CACHE.lock() {
            Ok(mut cache) => cache.set(all.clone(), all_photos_ttl(&all)),
            Err(_) => log!("ALL_PHOTOS_CACHE lock poisoned (write); skipping cache"),
        }

        Ok(all)
    }
    #[cfg(not(feature = "ssr"))]
    {
        Ok(AllPhotos {
            photos: Arc::new(Vec::new()),
            root_looks_unmounted: false,
        })
    }
}

/// Server function to get photos from a specific gallery
#[server(GetGalleryPhotosByName, "/api")]
pub async fn get_gallery_photos_by_name(
    gallery_name: String,
) -> Result<Vec<PhotoInfo>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        crate::gallery::load_gallery_photos(&gallery_name)
            .ok_or_else(|| ServerFnError::new("Gallery not found"))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = gallery_name;
        Ok(Vec::new())
    }
}

/// A gallery's photos with their pre-computed mosaic layout.
///
/// `Ok(None)` means there is no gallery of that name, so the page can show the
/// not-found page; an empty gallery directory is `Ok(Some(_))` with no photos.
/// `Err` means the lookup itself failed, including when the images root can't be
/// read or holds no gallery but the directories the server makes for itself (the
/// home gallery's, and those on the way to it), as it does while its mount is
/// down; then any of those, found empty, is an error too.
#[server(GetGalleryDataByName, "/api")]
pub async fn get_gallery_data_by_name(
    gallery_name: String,
) -> Result<Option<Arc<GalleryData>>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::logging::log;

        let cache_key = format!("gallery_{gallery_name}");

        match MOSAIC_CACHE.lock() {
            Ok(mut cache) => {
                if let Some(cached) = cache.get(&cache_key) {
                    return Ok(Some(cached));
                }
            }
            Err(_) => log!("MOSAIC_CACHE lock poisoned (read); regenerating"),
        }

        // Only galleries that exist are cached; a missing one is looked up each time.
        let Some(photos) = crate::gallery::load_gallery_photos(&gallery_name) else {
            // "No such gallery" becomes a 404, so be sure of it.
            if images_root_looks_unmounted() {
                return Err(ServerFnError::new("the images root holds no galleries"));
            }
            return Ok(None);
        };
        // Found but empty while the root looks unmounted: that can only be the home
        // gallery's directory or one on the way to it, which the server made for
        // itself. Fail, as above.
        if photos.is_empty() && images_root_looks_unmounted() {
            return Err(ServerFnError::new("the images root holds no galleries"));
        }

        let gallery_path = crate::paths::images_root().join(&gallery_name);
        let config = crate::gallery::load_gallery_config(&gallery_path);

        let (data, ttl) = build_gallery_data(photos, config.as_ref());
        let result = Arc::new(data);

        match MOSAIC_CACHE.lock() {
            Ok(mut cache) => cache.set(cache_key, Arc::clone(&result), ttl),
            Err(_) => log!("MOSAIC_CACHE lock poisoned (write); skipping cache"),
        }

        Ok(Some(result))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = gallery_name;
        Ok(None)
    }
}

/// One photo picked at random from the home gallery, for the not-found page.
///
/// `Ok(None)` when the home gallery has no photos, and an error during an outage
/// (see [`get_home_gallery_data`]). It reuses the home page's cached gallery data
/// instead of calling `load_home_photos` itself. 404s skip the HTML cache, so
/// calling it directly would reread every home photo's EXIF on each one; through
/// the cache, only the first lookup after a change does.
#[server(RandomHomePhoto, "/api")]
pub async fn random_home_photo() -> Result<Option<PhotoInfo>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let data = get_home_gallery_data().await?;
        Ok(pick_random_photo(&data.photos))
    }
    #[cfg(not(feature = "ssr"))]
    {
        Ok(None)
    }
}

/// A sync helper so the thread-local RNG, which is not `Send`, is never held
/// across an `.await`.
#[cfg(feature = "ssr")]
fn pick_random_photo(photos: &[PhotoInfo]) -> Option<PhotoInfo> {
    use rand::seq::IndexedRandom;
    photos.choose(&mut rand::rng()).cloned()
}

/// Server function to load about page content
#[server(GetAboutContent, "/api")]
pub async fn get_about_content() -> Result<AboutContent, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        Ok(crate::gallery::load_about_content())
    }
    #[cfg(not(feature = "ssr"))]
    {
        Ok(AboutContent {
            image_url: None,
            content: String::new(),
            is_html: false,
        })
    }
}

/// Server function to load the `/open-source` page's content. `None` when there is
/// no page to show (see [`crate::open_source::load_open_source_from`]).
#[server(GetOpenSource, "/api")]
pub async fn get_open_source() -> Result<Option<OpenSourceContent>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        Ok(crate::open_source::load_open_source())
    }
    #[cfg(not(feature = "ssr"))]
    {
        Ok(None)
    }
}

/// Server function backing the About page's "Open source" link.
///
/// A bool rather than the full content so `/about` does not serialize every card
/// into its hydration payload. Goes through the same loader as
/// [`get_open_source`], so a file that would 404 the page also hides the link.
#[server(HasOpenSource, "/api")]
pub async fn has_open_source() -> Result<bool, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        Ok(crate::open_source::load_open_source().is_some())
    }
    #[cfg(not(feature = "ssr"))]
    {
        Ok(false)
    }
}

/// Pre-warm the all-photos cache on server startup, by the same rule as
/// [`get_all_gallery_photos`]: a marked list only briefly, and no list at all during
/// an outage with no photo in sight.
#[cfg(feature = "ssr")]
pub fn prewarm_all_photos_cache() {
    use leptos::logging::log;

    log!("Pre-warming all-photos cache...");

    let Some(all) = load_all_photos() else {
        log!("All-photos cache not pre-warmed: the images root holds no galleries");
        return;
    };

    let ttl = all_photos_ttl(&all);
    // A marked list is no sign that all is well, so say why it is kept only briefly.
    let why = if all.root_looks_unmounted {
        " (the images root looks unmounted)"
    } else {
        ""
    };
    match ALL_PHOTOS_CACHE.lock() {
        Ok(mut cache) => {
            cache.set(all, ttl);
            log!("All-photos cache pre-warmed successfully, for {ttl} s{why}");
        }
        Err(_) => log!("Failed to acquire lock for all-photos cache pre-warming"),
    }
}

#[cfg(all(feature = "ssr", test))]
mod tests {
    use super::*;
    use crate::test_support::EnvVarGuard;
    use crate::types::{GalleryData, PhotoInfo};

    fn empty_gallery_data() -> Arc<GalleryData> {
        Arc::new(GalleryData {
            photos: Vec::new(),
            mosaic_layout: None,
        })
    }

    fn photo(filename: &str, width: u32, height: u32) -> PhotoInfo {
        PhotoInfo {
            url: String::new(),
            original_url: String::new(),
            detail_url: String::new(),
            srcset: String::new(),
            sources: Vec::new(),
            original_sources: Vec::new(),
            title: String::new(),
            filename: filename.to_string(),
            slug: String::new(),
            gallery_name: "film".to_string(),
            subfolder: None,
            width: Some(width),
            height: Some(height),
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
        }
    }

    // Regression: a `use_mosaic` gallery whose photo count makes the canvas very
    // tall used to have its first split fail intermittently, collapsing the
    // layout to one cell and dropping every other photo. `build_gallery_data`
    // must always return as many photos as it was given.
    #[test]
    fn build_gallery_data_mosaic_never_drops_photos() {
        use crate::types::GalleryConfig;

        // Mostly landscape, a few portrait — mirrors the film gallery.
        let photos: Vec<PhotoInfo> = (0..23)
            .map(|i| {
                let (w, h) = if i % 7 == 0 {
                    (5049, 8911)
                } else {
                    (8911, 5049)
                };
                photo(&format!("photo_{i}.jpg"), w, h)
            })
            .collect();

        let config = GalleryConfig {
            use_mosaic: Some(true),
            mosaic_cache_duration: Some(3600),
            ..Default::default()
        };

        for _ in 0..300 {
            let (data, _ttl) = build_gallery_data(photos.clone(), Some(&config));
            assert_eq!(
                data.photos.len(),
                photos.len(),
                "mosaic build must not drop photos"
            );
        }
    }

    #[test]
    fn mosaic_cache_set_then_get_returns_arc() {
        let mut cache = MosaicCache::new();
        let data = empty_gallery_data();
        cache.set("home".to_string(), Arc::clone(&data), 3600);
        let got = cache.get("home").expect("entry should be cached");
        assert!(Arc::ptr_eq(&got, &data));
    }

    #[test]
    fn mosaic_cache_get_misses_for_unknown_key() {
        let mut cache = MosaicCache::new();
        cache.set("a".to_string(), empty_gallery_data(), 3600);
        assert!(cache.get("b").is_none());
    }

    #[test]
    fn mosaic_cache_get_evicts_expired_entry() {
        let mut cache = MosaicCache::new();
        // Manually insert an entry that's already expired.
        cache.entries.insert(
            "stale".to_string(),
            CachedMosaic {
                data: empty_gallery_data(),
                expires_at: 0,
            },
        );
        assert!(cache.get("stale").is_none());
        // get() should also have removed it from the map.
        assert!(!cache.entries.contains_key("stale"));
    }

    #[test]
    fn mosaic_cache_clear_expired_only_removes_expired() {
        let mut cache = MosaicCache::new();
        let data = empty_gallery_data();
        cache.entries.insert(
            "fresh".to_string(),
            CachedMosaic {
                data: Arc::clone(&data),
                expires_at: now_unix_secs() + 3600,
            },
        );
        cache.entries.insert(
            "stale".to_string(),
            CachedMosaic {
                data: Arc::clone(&data),
                expires_at: 0,
            },
        );
        cache.clear_expired();
        assert!(cache.entries.contains_key("fresh"));
        assert!(!cache.entries.contains_key("stale"));
    }

    #[test]
    fn all_photos_cache_set_and_get_returns_arc() {
        let mut cache = AllPhotosCache::new();
        let photos = Arc::new(Vec::<PhotoInfo>::new());
        let all = AllPhotos {
            photos: Arc::clone(&photos),
            root_looks_unmounted: true,
        };
        cache.set(all, 600);
        let got = cache.get().expect("entry should be cached");
        assert!(Arc::ptr_eq(&got.photos, &photos));
        assert!(
            got.root_looks_unmounted,
            "the mark comes back as it was set"
        );
    }

    #[test]
    fn all_photos_cache_get_evicts_expired_entry() {
        let mut cache = AllPhotosCache::new();
        cache.cached_data = Some(CachedAllPhotos {
            all: AllPhotos {
                photos: Arc::new(Vec::new()),
                root_looks_unmounted: false,
            },
            expires_at: 0,
        });
        assert!(cache.get().is_none());
        assert!(cache.cached_data.is_none());
    }

    #[test]
    fn all_photos_cache_get_returns_none_when_empty() {
        let mut cache = AllPhotosCache::new();
        assert!(cache.get().is_none());
    }

    #[test]
    fn cache_filename_needle_uses_w_suffix_for_image_extensions() {
        let root = std::path::Path::new("/srv/images");
        let needle = cache_filename_needle(root, &root.join("home/sunset.jpg")).unwrap();
        assert_eq!(needle, "home_sunset_w");
    }

    #[test]
    fn cache_filename_needle_is_case_insensitive_on_extension() {
        let root = std::path::Path::new("/srv/images");
        let needle = cache_filename_needle(root, &root.join("home/Sunset.JPG")).unwrap();
        // Extension match is case-insensitive; the prefix preserves filename case.
        assert_eq!(needle, "home_Sunset_w");
    }

    #[test]
    fn cache_filename_needle_uses_broad_suffix_for_non_image() {
        let root = std::path::Path::new("/srv/images");
        // gallery.toml — config sibling: invalidate broadly.
        let needle = cache_filename_needle(root, &root.join("travel/gallery.toml")).unwrap();
        assert_eq!(needle, "travel_gallery_");
    }

    #[test]
    fn cache_filename_needle_uses_broad_suffix_for_directory() {
        let root = std::path::Path::new("/srv/images");
        // No extension — treat as directory-ish so we catch descendant caches.
        let needle = cache_filename_needle(root, &root.join("travel/iceland")).unwrap();
        assert_eq!(needle, "travel_iceland_");
    }

    #[test]
    fn cache_filename_needle_returns_none_outside_root() {
        let root = std::path::Path::new("/srv/images");
        assert!(cache_filename_needle(root, std::path::Path::new("/etc/passwd")).is_none());
    }

    #[test]
    fn cache_filename_needle_returns_none_for_root_itself() {
        let root = std::path::Path::new("/srv/images");
        assert!(cache_filename_needle(root, root).is_none());
    }

    #[test]
    fn delete_matching_cache_files_removes_only_matches() {
        use std::collections::HashSet;
        let dir = tempfile::TempDir::new().unwrap();
        let cache = dir.path();
        // Two cache entries for home/sunset, plus an unrelated entry.
        std::fs::write(cache.join("home_sunset_w800_q80_l1.webp"), b"a").unwrap();
        std::fs::write(cache.join("home_sunset_w2400_q80_l1.webp"), b"b").unwrap();
        std::fs::write(cache.join("home_other_w800_q80_l1.webp"), b"c").unwrap();

        let mut needles = HashSet::new();
        needles.insert("home_sunset_w".to_string());

        let deleted = delete_matching_cache_files(cache, &needles);
        assert_eq!(deleted, 2);
        assert!(!cache.join("home_sunset_w800_q80_l1.webp").exists());
        assert!(!cache.join("home_sunset_w2400_q80_l1.webp").exists());
        assert!(cache.join("home_other_w800_q80_l1.webp").exists());
    }

    #[test]
    fn delete_matching_cache_files_broad_needle_catches_descendants() {
        use std::collections::HashSet;
        let dir = tempfile::TempDir::new().unwrap();
        let cache = dir.path();
        // Cache files representing photos inside a renamed `travel/iceland/`
        // folder, plus an unrelated `travel/japan/...` entry.
        std::fs::write(cache.join("travel_iceland_glacier_w800_q80_l1.webp"), b"a").unwrap();
        std::fs::write(cache.join("travel_iceland_geyser_w2400_q80_l1.webp"), b"b").unwrap();
        std::fs::write(cache.join("travel_japan_tokyo_w800_q80_l1.webp"), b"c").unwrap();

        let mut needles = HashSet::new();
        needles.insert("travel_iceland_".to_string());

        let deleted = delete_matching_cache_files(cache, &needles);
        assert_eq!(deleted, 2);
        assert!(cache.join("travel_japan_tokyo_w800_q80_l1.webp").exists());
    }

    #[test]
    fn delete_matching_cache_files_handles_missing_cache_dir() {
        use std::collections::HashSet;
        let mut needles = HashSet::new();
        needles.insert("anything_w".to_string());
        let deleted =
            delete_matching_cache_files(std::path::Path::new("/no/such/dir/abc123"), &needles);
        assert_eq!(deleted, 0);
    }

    // ---- watch_event_is_relevant -------------------------------------
    //
    // The bug this guards against: reads (Access events) and metadata
    // churn (chmod/utime) were triggering full cache invalidation. These
    // tests pin the predicate's behavior for every EventKind variant we
    // care about. If `notify` adds new variants in a future version, the
    // `_` arm in the predicate falls through to `false` (conservative).

    #[test]
    fn watch_event_is_relevant_includes_create_and_remove() {
        use notify::EventKind;
        use notify::event::{CreateKind, RemoveKind};
        assert!(watch_event_is_relevant(EventKind::Create(CreateKind::File)));
        assert!(watch_event_is_relevant(EventKind::Create(
            CreateKind::Folder
        )));
        assert!(watch_event_is_relevant(EventKind::Create(CreateKind::Any)));
        assert!(watch_event_is_relevant(EventKind::Remove(RemoveKind::File)));
        assert!(watch_event_is_relevant(EventKind::Remove(
            RemoveKind::Folder
        )));
        assert!(watch_event_is_relevant(EventKind::Remove(RemoveKind::Any)));
    }

    #[test]
    fn watch_event_is_relevant_includes_data_and_rename_modifies() {
        use notify::EventKind;
        use notify::event::{DataChange, ModifyKind, RenameMode};
        assert!(watch_event_is_relevant(EventKind::Modify(
            ModifyKind::Data(DataChange::Content)
        )));
        assert!(watch_event_is_relevant(EventKind::Modify(
            ModifyKind::Data(DataChange::Size)
        )));
        assert!(watch_event_is_relevant(EventKind::Modify(
            ModifyKind::Data(DataChange::Any)
        )));
        assert!(watch_event_is_relevant(EventKind::Modify(
            ModifyKind::Name(RenameMode::From)
        )));
        assert!(watch_event_is_relevant(EventKind::Modify(
            ModifyKind::Name(RenameMode::To)
        )));
        assert!(watch_event_is_relevant(EventKind::Modify(
            ModifyKind::Name(RenameMode::Both)
        )));
        assert!(watch_event_is_relevant(EventKind::Modify(ModifyKind::Any)));
        assert!(watch_event_is_relevant(EventKind::Modify(
            ModifyKind::Other
        )));
    }

    #[test]
    fn watch_event_is_relevant_excludes_metadata_modifies() {
        // The original bug surface: rsync/syncthing/touch churn fires
        // these and the watcher was treating them as content changes.
        use notify::EventKind;
        use notify::event::{MetadataKind, ModifyKind};
        assert!(!watch_event_is_relevant(EventKind::Modify(
            ModifyKind::Metadata(MetadataKind::AccessTime)
        )));
        assert!(!watch_event_is_relevant(EventKind::Modify(
            ModifyKind::Metadata(MetadataKind::WriteTime)
        )));
        assert!(!watch_event_is_relevant(EventKind::Modify(
            ModifyKind::Metadata(MetadataKind::Permissions)
        )));
        assert!(!watch_event_is_relevant(EventKind::Modify(
            ModifyKind::Metadata(MetadataKind::Ownership)
        )));
        assert!(!watch_event_is_relevant(EventKind::Modify(
            ModifyKind::Metadata(MetadataKind::Extended)
        )));
        assert!(!watch_event_is_relevant(EventKind::Modify(
            ModifyKind::Metadata(MetadataKind::Any)
        )));
    }

    #[test]
    fn watch_event_is_relevant_excludes_access_events() {
        // The headline cause of the constant invalidation: every read
        // (cache prewarm, every HTTP image serve) generates these.
        use notify::EventKind;
        use notify::event::{AccessKind, AccessMode};
        assert!(!watch_event_is_relevant(EventKind::Access(
            AccessKind::Read
        )));
        assert!(!watch_event_is_relevant(EventKind::Access(
            AccessKind::Open(AccessMode::Read)
        )));
        assert!(!watch_event_is_relevant(EventKind::Access(
            AccessKind::Close(AccessMode::Read)
        )));
        assert!(!watch_event_is_relevant(EventKind::Access(AccessKind::Any)));
    }

    #[test]
    fn watch_event_is_relevant_excludes_any_and_other_top_level() {
        // Be conservative on uncategorized events.
        use notify::EventKind;
        assert!(!watch_event_is_relevant(EventKind::Any));
        assert!(!watch_event_is_relevant(EventKind::Other));
    }

    fn block_on<F: std::future::Future>(future: F) -> F::Output {
        tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap()
            .block_on(future)
    }

    /// A one-gallery `IMAGES_DIR` in a temp dir: `<gallery>/` holding one small JPEG
    /// and a `gallery.toml` with the given contents.
    fn images_dir_with_gallery(gallery: &str, gallery_toml: &str) -> tempfile::TempDir {
        let root = tempfile::TempDir::new().unwrap();
        let dir = root.path().join(gallery);
        std::fs::create_dir_all(&dir).unwrap();
        image::RgbImage::new(4, 3)
            .save(dir.join("photo.jpg"))
            .unwrap();
        std::fs::write(dir.join("gallery.toml"), gallery_toml).unwrap();
        root
    }

    #[test]
    #[serial_test::serial]
    fn home_gallery_config_is_read_from_images_dir() {
        // The checked-in public/images/home/gallery.toml sets columns = 4.
        let root = images_dir_with_gallery("home", "columns = 7\n");
        unsafe { std::env::remove_var("GALLERY_PATH") };
        let _images_dir = EnvVarGuard::set("IMAGES_DIR", root.path());

        let config = block_on(get_home_gallery_config());

        assert_eq!(config.unwrap().and_then(|c| c.columns), Some(7));
    }

    #[test]
    #[serial_test::serial]
    fn home_gallery_data_uses_the_home_config_under_images_dir() {
        // The checked-in public/images/home/gallery.toml turns the mosaic on, so a
        // config lookup that still went there would lay this photo out as a mosaic.
        let root = images_dir_with_gallery("home", "use_mosaic = false\n");
        unsafe { std::env::remove_var("GALLERY_PATH") };
        let _images_dir = EnvVarGuard::set("IMAGES_DIR", root.path());

        // MOSAIC_CACHE is process-wide: clear it first so a leftover "home_gallery"
        // entry cannot stand in for this load, and again before asserting so no entry
        // is left pointing at a temp dir that is about to be deleted.
        invalidate_caches();
        let data = block_on(get_home_gallery_data());
        invalidate_caches();

        let data = data.unwrap();
        assert_eq!(data.photos.len(), 1);
        assert!(
            data.mosaic_layout.is_none(),
            "its gallery.toml says no mosaic"
        );
    }

    #[test]
    #[serial_test::serial]
    fn gallery_data_uses_the_gallery_config_under_images_dir() {
        let root = images_dir_with_gallery("images-dir-mosaic", "use_mosaic = true\n");
        let _images_dir = EnvVarGuard::set("IMAGES_DIR", root.path());

        // Cleared on both sides for the same reason as the home gallery test above.
        invalidate_caches();
        let data = block_on(get_gallery_data_by_name("images-dir-mosaic".to_string()));
        invalidate_caches();

        let data = data.unwrap().expect("the gallery exists");
        assert_eq!(data.photos.len(), 1);
        assert!(
            data.mosaic_layout.is_some(),
            "its gallery.toml asks for a mosaic"
        );
    }

    #[test]
    #[serial_test::serial]
    fn gallery_data_is_none_for_a_missing_gallery() {
        let root = images_dir_with_gallery("nature", "");
        unsafe { std::env::remove_var("GALLERY_PATH") };
        let _images_dir = EnvVarGuard::set("IMAGES_DIR", root.path());

        invalidate_caches();
        let missing = block_on(get_gallery_data_by_name("nope".to_string()));
        let found = block_on(get_gallery_data_by_name("nature".to_string()));
        invalidate_caches();

        assert!(
            missing.unwrap().is_none(),
            "there is no gallery called nope"
        );
        let found = found.unwrap().expect("the nature gallery exists");
        assert_eq!(found.photos.len(), 1);
    }

    #[test]
    #[serial_test::serial]
    fn a_missing_gallery_is_not_cached() {
        let root = images_dir_with_gallery("nature", "");
        unsafe { std::env::remove_var("GALLERY_PATH") };
        let _images_dir = EnvVarGuard::set("IMAGES_DIR", root.path());

        invalidate_caches();
        let before = block_on(get_gallery_data_by_name("later".to_string()));
        let dir = root.path().join("later");
        std::fs::create_dir_all(&dir).unwrap();
        image::RgbImage::new(4, 3)
            .save(dir.join("photo.jpg"))
            .unwrap();
        let after = block_on(get_gallery_data_by_name("later".to_string()));
        invalidate_caches();

        assert!(before.unwrap().is_none(), "not there yet");
        assert!(
            after.unwrap().is_some(),
            "a gallery added after a miss is found at once"
        );
    }

    /// A photo's addresses carry its file's modification time. The cached photo lists
    /// keep the old one until they are cleared, which the image watcher does when the
    /// file is written, or until they expire.
    #[test]
    #[serial_test::serial]
    fn a_changed_photo_gets_its_new_address_once_the_caches_are_cleared() {
        let root = images_dir_with_gallery("nature", "");
        let _images_dir = EnvVarGuard::set("IMAGES_DIR", root.path());
        let photo = root.path().join("nature/photo.jpg");
        let set_mtime = |secs| {
            std::fs::File::options()
                .write(true)
                .open(&photo)
                .unwrap()
                .set_modified(UNIX_EPOCH + std::time::Duration::from_secs(secs))
                .unwrap();
        };
        let addresses = || {
            let all = block_on(get_all_gallery_photos()).unwrap().photos[0]
                .original_url
                .clone();
            let gallery = block_on(get_gallery_data_by_name("nature".to_string()))
                .unwrap()
                .expect("the nature gallery exists")
                .photos[0]
                .original_url
                .clone();
            [all, gallery]
        };

        invalidate_caches();
        set_mtime(1_731_652_400);
        let before = addresses();
        set_mtime(1_731_652_460);
        let cached = addresses();
        invalidate_caches();
        let after = addresses();
        invalidate_caches();

        let old = "/images/nature/photo.jpg?v=1731652400";
        let new = "/images/nature/photo.jpg?v=1731652460";
        assert_eq!(before, [old, old]);
        assert_eq!(cached, [old, old], "still cached");
        assert_eq!(after, [new, new]);
    }

    #[test]
    #[serial_test::serial]
    fn gallery_data_is_an_error_when_the_images_root_is_unreadable() {
        // A NAS mount that is down: the root doesn't exist. That is a failed lookup,
        // not proof the gallery doesn't exist, so it must not become a 404.
        let root = tempfile::TempDir::new().unwrap();
        unsafe { std::env::remove_var("GALLERY_PATH") };
        let _images_dir = EnvVarGuard::set("IMAGES_DIR", root.path().join("unmounted"));

        invalidate_caches();
        let data = block_on(get_gallery_data_by_name("nature".to_string()));
        invalidate_caches();

        assert!(data.is_err(), "{data:?}");
    }

    #[test]
    #[serial_test::serial]
    fn gallery_data_is_an_error_when_the_images_root_holds_no_galleries() {
        // What a NAS outage usually looks like: the mountpoint is there but empty,
        // and the server has made its own empty `home` (load_home_photos and the
        // image watcher create them). Every gallery is "missing" then, so none of
        // them may answer 404.
        let root = tempfile::TempDir::new().unwrap();
        std::fs::create_dir_all(root.path().join("home")).unwrap();
        unsafe { std::env::remove_var("GALLERY_PATH") };
        let _images_dir = EnvVarGuard::set("IMAGES_DIR", root.path());

        invalidate_caches();
        let data = block_on(get_gallery_data_by_name("nature".to_string()));
        invalidate_caches();

        assert!(data.is_err(), "{data:?}");
    }

    #[test]
    #[serial_test::serial]
    fn gallery_data_is_an_error_when_only_a_gallery_path_home_exists() {
        // GALLERY_PATH can put the home gallery inside the images root under its
        // own name; the server creates that directory too, so it doesn't count as
        // another gallery either.
        let root = tempfile::TempDir::new().unwrap();
        let landing = root.path().join("landing");
        std::fs::create_dir_all(&landing).unwrap();
        let _images_dir = EnvVarGuard::set("IMAGES_DIR", root.path());
        let _gallery_path = EnvVarGuard::set("GALLERY_PATH", &landing);

        invalidate_caches();
        let data = block_on(get_gallery_data_by_name("nature".to_string()));
        invalidate_caches();

        assert!(data.is_err(), "{data:?}");
    }

    #[test]
    #[serial_test::serial]
    fn lookups_are_errors_when_only_the_way_to_a_nested_gallery_path_home_exists() {
        // GALLERY_PATH can put the home gallery inside another gallery too. Then the
        // server makes `nature` on its way to `nature/featured`, so with nothing else
        // in it, `nature` is no sign of the share either: a missing gallery and an
        // empty photo list are failed lookups, not 404s.
        let root = tempfile::TempDir::new().unwrap();
        let featured = root.path().join("nature").join("featured");
        std::fs::create_dir_all(&featured).unwrap();
        let _images_dir = EnvVarGuard::set("IMAGES_DIR", root.path());
        let _gallery_path = EnvVarGuard::set("GALLERY_PATH", &featured);

        invalidate_caches();
        let data = block_on(get_gallery_data_by_name("city".to_string()));
        let photos = block_on(get_all_gallery_photos());
        invalidate_caches();

        assert!(data.is_err(), "{data:?}");
        assert!(photos.is_err(), "{photos:?}");
    }

    #[test]
    #[serial_test::serial]
    fn all_gallery_photos_is_an_error_while_the_images_root_holds_no_galleries() {
        // Photo pages look themselves up in this list, and an empty list makes every
        // one of them a 404. During an outage that must be a failed lookup instead,
        // and it must not be cached past the outage.
        let root = tempfile::TempDir::new().unwrap();
        std::fs::create_dir_all(root.path().join("home")).unwrap();
        unsafe { std::env::remove_var("GALLERY_PATH") };
        let _images_dir = EnvVarGuard::set("IMAGES_DIR", root.path());

        invalidate_caches();
        let during = block_on(get_all_gallery_photos());
        let dir = root.path().join("nature");
        std::fs::create_dir_all(&dir).unwrap();
        image::RgbImage::new(4, 3)
            .save(dir.join("photo.jpg"))
            .unwrap();
        let after = block_on(get_all_gallery_photos());
        invalidate_caches();

        assert!(during.is_err(), "{during:?}");
        assert_eq!(
            after.unwrap().photos.len(),
            1,
            "the outage result wasn't cached"
        );
    }

    #[test]
    #[serial_test::serial]
    fn prewarming_during_an_outage_caches_nothing() {
        // A server that starts while the share is down, as after a power cut when
        // the server is up before the NAS, prewarms an empty list. Cached, it would
        // make every photo page a 404 instead of a failed lookup until it expired.
        let root = tempfile::TempDir::new().unwrap();
        std::fs::create_dir_all(root.path().join("home")).unwrap();
        unsafe { std::env::remove_var("GALLERY_PATH") };
        let _images_dir = EnvVarGuard::set("IMAGES_DIR", root.path());

        invalidate_caches();
        prewarm_all_photos_cache();
        let after = block_on(get_all_gallery_photos());
        invalidate_caches();

        assert!(after.is_err(), "{after:?}");
    }

    /// Runs `cache_it`, which caches the all-photos list, and returns its result and
    /// how long the list is cached for, in seconds. That is a range, since the clock
    /// can tick between its reading before `cache_it` and the one after.
    fn cached_for<T>(cache_it: impl FnOnce() -> T) -> (T, std::ops::RangeInclusive<u64>) {
        let before = now_unix_secs();
        let result = cache_it();
        let after = now_unix_secs();
        // The lock is let go before the `expect`: a test that panics while holding it
        // would poison the cache for every test after it.
        let expires_at = ALL_PHOTOS_CACHE
            .lock()
            .unwrap()
            .cached_data
            .as_ref()
            .map(|cached| cached.expires_at);
        let expires_at = expires_at.expect("a list is cached");
        (result, expires_at - after..=expires_at - before)
    }

    #[test]
    #[serial_test::serial]
    fn all_gallery_photos_left_in_home_during_an_outage_are_cached_only_briefly() {
        // A dead mountpoint whose `home` still holds a photo, such as one left there
        // under the mountpoint: the walk finds that photo and nothing else. Cached for
        // the usual ten minutes, that list would make every other photo page fail for
        // ten minutes after the share came back.
        let root = images_dir_with_gallery("home", "");
        unsafe { std::env::remove_var("GALLERY_PATH") };
        let _images_dir = EnvVarGuard::set("IMAGES_DIR", root.path());

        invalidate_caches();
        let (during, during_ttl) = cached_for(|| block_on(get_all_gallery_photos()));
        let dir = root.path().join("nature");
        std::fs::create_dir_all(&dir).unwrap();
        image::RgbImage::new(4, 3)
            .save(dir.join("photo.jpg"))
            .unwrap();
        let back = block_on(get_all_gallery_photos());
        expire_all_photos_cache();
        let (after, after_ttl) = cached_for(|| block_on(get_all_gallery_photos()));
        invalidate_caches();

        let during = during.unwrap();
        assert_eq!(during.photos.len(), 1, "the photo in sight is listed");
        assert!(during.root_looks_unmounted);
        assert!(during_ttl.contains(&15), "{during_ttl:?}");
        let back = back.unwrap();
        assert_eq!(back.photos.len(), 1, "the outage's list, still cached");
        assert!(back.root_looks_unmounted, "as it was loaded");
        let after = after.unwrap();
        assert_eq!(after.photos.len(), 2, "reloaded once the entry expired");
        assert!(!after.root_looks_unmounted);
        assert!(after_ttl.contains(&600), "{after_ttl:?}");
    }

    #[test]
    #[serial_test::serial]
    fn a_home_only_site_serves_its_cached_photos_until_they_expire() {
        // With the share up, a site whose only gallery is home looks unmounted too, so
        // its list is marked. It is still cached, if briefly: without that, every photo
        // page and every visit within the app would walk the gallery again.
        let root = images_dir_with_gallery("home", "");
        unsafe { std::env::remove_var("GALLERY_PATH") };
        let _images_dir = EnvVarGuard::set("IMAGES_DIR", root.path());

        invalidate_caches();
        let first = block_on(get_all_gallery_photos());
        image::RgbImage::new(4, 3)
            .save(root.path().join("home/second.jpg"))
            .unwrap();
        let second = block_on(get_all_gallery_photos());
        expire_all_photos_cache();
        let expired = block_on(get_all_gallery_photos());
        invalidate_caches();

        assert_eq!(first.unwrap().photos.len(), 1);
        let second = second.unwrap();
        assert_eq!(second.photos.len(), 1, "served from the cache, not walked");
        assert!(second.root_looks_unmounted);
        let expired = expired.unwrap();
        assert_eq!(
            expired.photos.len(),
            2,
            "walked again once the entry expired"
        );
        assert!(expired.root_looks_unmounted);
    }

    #[test]
    #[serial_test::serial]
    fn prewarming_caches_the_list_for_as_long_as_a_lookup_does() {
        // A server that starts during an outage with a photo left in `home`, or on a
        // site whose only gallery is home: 15 seconds, as above. With another
        // gallery there, ten minutes.
        let root = images_dir_with_gallery("home", "");
        unsafe { std::env::remove_var("GALLERY_PATH") };
        let _images_dir = EnvVarGuard::set("IMAGES_DIR", root.path());

        invalidate_caches();
        let ((), marked_ttl) = cached_for(prewarm_all_photos_cache);
        let marked = block_on(get_all_gallery_photos());
        invalidate_caches();
        let dir = root.path().join("nature");
        std::fs::create_dir_all(&dir).unwrap();
        image::RgbImage::new(4, 3)
            .save(dir.join("photo.jpg"))
            .unwrap();
        let ((), mounted_ttl) = cached_for(prewarm_all_photos_cache);
        invalidate_caches();

        assert!(marked_ttl.contains(&15), "{marked_ttl:?}");
        let marked = marked.unwrap();
        assert_eq!(marked.photos.len(), 1);
        assert!(
            marked.root_looks_unmounted,
            "the prewarmed list, as it was loaded"
        );
        assert!(mounted_ttl.contains(&600), "{mounted_ttl:?}");
    }

    #[test]
    #[serial_test::serial]
    fn random_home_photo_picks_a_home_photo() {
        let root = images_dir_with_gallery("home", "use_mosaic = false\n");
        unsafe { std::env::remove_var("GALLERY_PATH") };
        let _images_dir = EnvVarGuard::set("IMAGES_DIR", root.path());

        invalidate_caches();
        let picked = block_on(random_home_photo());
        invalidate_caches();

        let picked = picked.unwrap().expect("the home gallery has a photo");
        assert_eq!(picked.gallery_name, "home");
        assert_eq!(picked.slug, "photo");
        assert!(
            picked.url.starts_with("/images/compressed/home/"),
            "{}",
            picked.url
        );
    }

    #[test]
    #[serial_test::serial]
    fn home_gallery_data_is_an_error_while_the_images_root_holds_no_galleries() {
        // During an outage the home gallery is the empty directory the server made
        // for itself. Cached as an empty gallery, it would outlast the outage by the
        // cache's five minutes, on the home page and on /gallery/home alike.
        let root = tempfile::TempDir::new().unwrap();
        std::fs::create_dir_all(root.path().join("home")).unwrap();
        unsafe { std::env::remove_var("GALLERY_PATH") };
        let _images_dir = EnvVarGuard::set("IMAGES_DIR", root.path());

        invalidate_caches();
        let home = block_on(get_home_gallery_data());
        let gallery = block_on(get_gallery_data_by_name("home".to_string()));
        for dir in ["home", "nature"] {
            let dir = root.path().join(dir);
            std::fs::create_dir_all(&dir).unwrap();
            image::RgbImage::new(4, 3)
                .save(dir.join("photo.jpg"))
                .unwrap();
        }
        let home_after = block_on(get_home_gallery_data());
        let gallery_after = block_on(get_gallery_data_by_name("home".to_string()));
        invalidate_caches();

        assert!(home.is_err(), "{home:?}");
        assert!(gallery.is_err(), "{gallery:?}");
        assert_eq!(home_after.unwrap().photos.len(), 1, "not cached");
        let gallery_after = gallery_after.unwrap().expect("the home gallery exists");
        assert_eq!(gallery_after.photos.len(), 1, "not cached");
    }

    #[test]
    #[serial_test::serial]
    fn random_home_photo_is_none_without_home_photos() {
        // Another gallery, so the share counts as mounted and the empty home gallery
        // as really empty.
        let root = tempfile::TempDir::new().unwrap();
        std::fs::create_dir_all(root.path().join("nature")).unwrap();
        unsafe { std::env::remove_var("GALLERY_PATH") };
        let _images_dir = EnvVarGuard::set("IMAGES_DIR", root.path());

        invalidate_caches();
        let picked = block_on(random_home_photo());
        invalidate_caches();

        assert!(picked.unwrap().is_none());
    }

    #[test]
    fn pick_random_photo_handles_empty_and_single() {
        assert_eq!(pick_random_photo(&[]), None);
        let only = photo("only.jpg", 4, 3);
        assert_eq!(pick_random_photo(std::slice::from_ref(&only)), Some(only));
    }
}

/// Return true when a notify `EventKind` represents an actual content
/// change worth invalidating cached galleries for.
///
/// Filters out:
/// - `Access(_)` — reads. Serving an image generates these constantly and
///   they don't change on-disk state.
/// - `Modify(Metadata(_))` — chmod / utime / xattr churn. Tools like rsync
///   or syncthing stamp mtimes; that shouldn't blow away the layout cache.
/// - `Any` / `Other` at the top level — we'd rather miss an exotic event
///   than invalidate on something we can't classify.
#[cfg(feature = "ssr")]
fn watch_event_is_relevant(kind: notify::EventKind) -> bool {
    use notify::EventKind;
    use notify::event::ModifyKind;
    matches!(
        kind,
        EventKind::Create(_)
            | EventKind::Remove(_)
            | EventKind::Modify(
                ModifyKind::Data(_) | ModifyKind::Name(_) | ModifyKind::Any | ModifyKind::Other
            )
    )
}

/// Drop every entry from both in-memory galleries caches. Called by the
/// image directory watcher when the underlying files change, so the next
/// request re-scans the filesystem.
#[cfg(feature = "ssr")]
pub fn invalidate_caches() {
    use leptos::logging::log;

    match MOSAIC_CACHE.lock() {
        Ok(mut cache) => cache.clear_all(),
        Err(_) => log!("MOSAIC_CACHE lock poisoned during invalidation; skipping"),
    }
    match ALL_PHOTOS_CACHE.lock() {
        Ok(mut cache) => cache.clear_all(),
        Err(_) => log!("ALL_PHOTOS_CACHE lock poisoned during invalidation; skipping"),
    }
}

/// Ends the cached all-photos list's time now, as if its TTL had run out, for the
/// tests that can't wait that long. The entry stays until the next lookup finds it
/// expired, as it would.
#[cfg(all(test, feature = "ssr"))]
pub(crate) fn expire_all_photos_cache() {
    if let Some(cached) = &mut ALL_PHOTOS_CACHE.lock().unwrap().cached_data {
        cached.expires_at = 0;
    }
}

/// Translate a source path under `images_root` into a filename needle that
/// matches the compressed-cache entries produced for it by
/// `image_cache::process_and_cache_image`. Cache files are named
/// `{prefix}_w{w}_q{q}_l1.webp`, where `prefix` is the path relative to
/// `images_root` with `/` (and `\`) replaced by `_` and the extension
/// stripped.
///
/// For image-extension paths we return `{prefix}_w` (matches that one
/// file's variants). For anything else — directories, `.toml` siblings,
/// unknown extensions — we return `{prefix}_`, which is broader: it also
/// catches cache files for descendants of a renamed/deleted folder, at
/// the cost of occasional over-invalidation when a sibling source's name
/// happens to share a prefix.
#[cfg(feature = "ssr")]
fn cache_filename_needle(
    images_root: &std::path::Path,
    source: &std::path::Path,
) -> Option<String> {
    let relative = source.strip_prefix(images_root).ok()?;
    let no_ext = relative.with_extension("");
    let s = no_ext.to_string_lossy();
    if s.is_empty() {
        return None;
    }
    let prefix = s.replace(['/', '\\'], "_");

    let is_image = source
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
        .is_some_and(|ext| {
            matches!(
                ext,
                "jpg" | "jpeg" | "png" | "webp" | "gif" | "jxl" | "avif"
            )
        });

    Some(if is_image {
        format!("{prefix}_w")
    } else {
        format!("{prefix}_")
    })
}

/// Remove every file in `cache_dir` whose name starts with any of `needles`.
/// Errors are logged but otherwise ignored — a stale cache entry is harmless
/// (the next request will simply regenerate it), so we don't want a cleanup
/// failure to mask the in-memory invalidation that follows.
#[cfg(feature = "ssr")]
fn delete_matching_cache_files(
    cache_dir: &std::path::Path,
    needles: &std::collections::HashSet<String>,
) -> usize {
    use leptos::logging::log;

    let Ok(entries) = std::fs::read_dir(cache_dir) else {
        // Cache dir may not exist yet — that's not an error.
        return 0;
    };

    let mut deleted = 0;
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if needles.iter().any(|n| name.starts_with(n)) {
            match std::fs::remove_file(&path) {
                Ok(()) => deleted += 1,
                Err(err) => log!("Image watcher: failed to remove {}: {err}", path.display()),
            }
        }
    }
    deleted
}

/// Watch `images_dir` recursively and, when files change, (a) delete the
/// matching compressed-cache entries under `cache_dir` and (b) invalidate
/// the in-memory galleries caches. Events are debounced — a burst of moves
/// (e.g. dragging a folder of photos in) collapses to one cleanup after
/// activity settles.
///
/// Runs on a dedicated `std::thread` because `notify` delivers events via a
/// sync `mpsc` channel and the loop blocks indefinitely; the tokio blocking
/// pool isn't the right place for that.
#[cfg(feature = "ssr")]
pub fn spawn_image_watcher(images_dir: String, cache_dir: String) {
    use leptos::logging::log;
    use notify::{RecommendedWatcher, RecursiveMode, Watcher};
    use std::collections::HashSet;
    use std::path::{Path, PathBuf};
    use std::sync::mpsc;
    use std::time::Duration;

    std::thread::spawn(move || {
        // The watcher errors if the path doesn't exist yet; create it so the
        // server can be started before any photos have been added.
        if !Path::new(&images_dir).exists()
            && let Err(err) = std::fs::create_dir_all(&images_dir)
        {
            log!("Image watcher: failed to create {images_dir}: {err}");
            return;
        }

        // Canonicalize so event paths (which arrive joined to whatever we
        // pass to watch()) reliably share a prefix with `images_root`.
        let images_root = match Path::new(&images_dir).canonicalize() {
            Ok(p) => p,
            Err(err) => {
                log!("Image watcher: canonicalize({images_dir}) failed: {err}");
                return;
            }
        };
        let cache_root = PathBuf::from(&cache_dir);

        let (tx, rx) = mpsc::channel();
        let mut watcher = match RecommendedWatcher::new(
            move |res| {
                let _ = tx.send(res);
            },
            notify::Config::default(),
        ) {
            Ok(w) => w,
            Err(err) => {
                log!("Image watcher: failed to initialize: {err}");
                return;
            }
        };

        if let Err(err) = watcher.watch(&images_root, RecursiveMode::Recursive) {
            log!(
                "Image watcher: failed to watch {}: {err}",
                images_root.display()
            );
            return;
        }
        log!(
            "Watching {} for changes; caches invalidate on file events",
            images_root.display()
        );

        let quiet = Duration::from_millis(500);
        loop {
            // Block until something happens. A recv error means the sender was
            // dropped, i.e. the watcher is gone — nothing left to do.
            let Ok(first) = rx.recv() else { return };

            let mut events = Vec::new();
            match first {
                Ok(ev) => events.push(ev),
                Err(err) => log!("Image watcher: event error: {err}"),
            }

            // Drain follow-up events within the quiet window so a burst of
            // filesystem activity collapses to a single invalidation.
            while let Ok(res) = rx.recv_timeout(quiet) {
                match res {
                    Ok(ev) => events.push(ev),
                    Err(err) => log!("Image watcher: event error: {err}"),
                }
            }

            // Collect unique cache-filename needles from relevant events.
            // Irrelevant events (reads, metadata churn) are filtered by
            // `watch_event_is_relevant` — see its doc comment for details.
            let mut needles: HashSet<String> = HashSet::new();
            let mut had_relevant_event = false;
            for event in &events {
                if !watch_event_is_relevant(event.kind) {
                    continue;
                }
                had_relevant_event = true;
                for path in &event.paths {
                    if let Some(needle) = cache_filename_needle(&images_root, path) {
                        needles.insert(needle);
                    }
                }
            }

            if !had_relevant_event {
                continue;
            }

            if !needles.is_empty() {
                let deleted = delete_matching_cache_files(&cache_root, &needles);
                if deleted > 0 {
                    log!("Image watcher: removed {deleted} stale compressed cache file(s)");
                }
            }

            log!("Image directory changed; invalidating in-memory caches");
            invalidate_caches();
        }
    });
}

/// Spawn a background task that periodically evicts expired entries from
/// `MOSAIC_CACHE`. The work is `O(num_galleries)` per tick, so we keep it off
/// the request path. Tick interval is intentionally coarser than the
/// shortest TTL (5 min) — a stale entry waiting an extra minute to be
/// evicted is harmless; the per-key `get()` also evicts on access.
#[cfg(feature = "ssr")]
pub fn spawn_cache_sweeper() {
    use leptos::logging::log;

    tokio::spawn(async {
        let mut ticker = tokio::time::interval(std::time::Duration::from_secs(60));
        // Skip the immediate first tick so we don't run before the prewarm completes.
        ticker.tick().await;
        loop {
            ticker.tick().await;
            match MOSAIC_CACHE.lock() {
                Ok(mut cache) => cache.clear_expired(),
                Err(_) => log!("MOSAIC_CACHE lock poisoned during sweep; skipping tick"),
            }
        }
    });
}
