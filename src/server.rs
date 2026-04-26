use crate::config::SiteConfig;
use crate::types::{AboutContent, GalleryData, GalleryInfo, PhotoInfo};
use leptos::prelude::*;
use std::sync::Arc;

#[cfg(feature = "ssr")]
use once_cell::sync::Lazy;
#[cfg(feature = "ssr")]
use std::sync::Mutex;
#[cfg(feature = "ssr")]
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(feature = "ssr")]
static MOSAIC_CACHE: Lazy<Arc<Mutex<MosaicCache>>> =
    Lazy::new(|| Arc::new(Mutex::new(MosaicCache::new())));

#[cfg(feature = "ssr")]
static ALL_PHOTOS_CACHE: Lazy<Arc<Mutex<AllPhotosCache>>> =
    Lazy::new(|| Arc::new(Mutex::new(AllPhotosCache::new())));

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
    photos: Arc<Vec<PhotoInfo>>,
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
        self.entries
            .insert(key, CachedMosaic { data, expires_at });
    }

    fn clear_expired(&mut self) {
        let now = now_unix_secs();
        self.entries.retain(|_, cached| cached.expires_at > now);
    }
}

#[cfg(feature = "ssr")]
impl AllPhotosCache {
    fn new() -> Self {
        Self { cached_data: None }
    }

    fn get(&mut self) -> Option<Arc<Vec<PhotoInfo>>> {
        let now = now_unix_secs();

        if let Some(cached) = &self.cached_data {
            if cached.expires_at > now {
                return Some(Arc::clone(&cached.photos));
            }

            // Expired, remove it
            self.cached_data = None;
        }
        None
    }

    fn set(&mut self, photos: Arc<Vec<PhotoInfo>>, duration_secs: u64) {
        let expires_at = now_unix_secs() + duration_secs;
        self.cached_data = Some(CachedAllPhotos {
            photos,
            expires_at,
        });
    }
}

#[cfg(feature = "ssr")]
fn generate_mosaic_layout_for_size(
    photos: &[crate::types::PhotoInfo],
    container_width: f64,
    base_height: f64,
) -> (crate::types::MosaicLayout, Vec<usize>) {
    use crate::mosaic::{calculate_orientation_bias, generate_mosaic_with_images, MosaicConfig};

    let num_images = photos.len();
    let image_aspects: Vec<(usize, f64)> = photos
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
        .collect();

    let photos_sqrt = (num_images as f64).sqrt();
    let container_height = base_height * photos_sqrt.max(2.0);

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

    generate_mosaic_with_images(num_images, &image_aspects, mosaic_config, 100)
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
        use std::path::Path;
        let home_path = Path::new("public/images/home");
        Ok(crate::gallery::load_gallery_config(home_path))
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

    if let Some(cfg) = config {
        if cfg.use_mosaic.unwrap_or(false) && !photos.is_empty() {
            let (layout_desktop, image_order) =
                generate_mosaic_layout_for_size(&photos, 1200.0, 600.0);

            let reordered_photos: Vec<PhotoInfo> =
                image_order.iter().map(|&idx| photos[idx].clone()).collect();

            // Tablet uses the same photo order so the mosaic stays consistent.
            let (layout_tablet, _) =
                generate_mosaic_layout_for_size(&reordered_photos, 768.0, 600.0);

            let data = GalleryData {
                photos: reordered_photos,
                mosaic_layout: Some(layout_desktop),
                mosaic_layout_tablet: Some(layout_tablet),
            };
            return (data, cfg.mosaic_cache_duration.unwrap_or(MOSAIC_DEFAULT_TTL));
        }
    }

    let data = GalleryData {
        photos,
        mosaic_layout: None,
        mosaic_layout_tablet: None,
    };
    (data, NON_MOSAIC_TTL)
}

/// Server function to get home gallery data with pre-computed mosaic layout
#[server(GetHomeGalleryData, "/api")]
pub async fn get_home_gallery_data() -> Result<Arc<GalleryData>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::logging::log;
        use std::path::Path;

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
        let home_path = Path::new("public/images/home");
        let config = crate::gallery::load_gallery_config(home_path);

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
            mosaic_layout_tablet: None,
        }))
    }
}

/// Server function to get photos from ALL galleries (for photo detail page)
#[server(GetAllGalleryPhotos, "/api")]
pub async fn get_all_gallery_photos() -> Result<Arc<Vec<PhotoInfo>>, ServerFnError> {
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

        // Load all photos (expensive operation)
        let photos = Arc::new(crate::gallery::load_all_gallery_photos());

        // Cache the result for 10 minutes (600 seconds)
        match ALL_PHOTOS_CACHE.lock() {
            Ok(mut cache) => cache.set(Arc::clone(&photos), 600),
            Err(_) => log!("ALL_PHOTOS_CACHE lock poisoned (write); skipping cache"),
        }

        Ok(photos)
    }
    #[cfg(not(feature = "ssr"))]
    {
        Ok(Arc::new(Vec::new()))
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

/// Server function to get gallery data with pre-computed mosaic layout for any gallery
#[server(GetGalleryDataByName, "/api")]
pub async fn get_gallery_data_by_name(
    gallery_name: String,
) -> Result<Arc<GalleryData>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::logging::log;
        use std::path::Path;

        let cache_key = format!("gallery_{}", gallery_name);

        match MOSAIC_CACHE.lock() {
            Ok(mut cache) => {
                if let Some(cached) = cache.get(&cache_key) {
                    return Ok(cached);
                }
            }
            Err(_) => log!("MOSAIC_CACHE lock poisoned (read); regenerating"),
        }

        let photos = crate::gallery::load_gallery_photos(&gallery_name)
            .ok_or_else(|| ServerFnError::new("Gallery not found"))?;

        let gallery_path = Path::new("public/images").join(&gallery_name);
        let config = crate::gallery::load_gallery_config(&gallery_path);

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
        let _ = gallery_name;
        Ok(Arc::new(GalleryData {
            photos: Vec::new(),
            mosaic_layout: None,
            mosaic_layout_tablet: None,
        }))
    }
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

/// Pre-warm the all-photos cache on server startup
#[cfg(feature = "ssr")]
pub fn prewarm_all_photos_cache() {
    use leptos::logging::log;

    log!("Pre-warming all-photos cache...");

    let photos = Arc::new(crate::gallery::load_all_gallery_photos());

    match ALL_PHOTOS_CACHE.lock() {
        Ok(mut cache) => {
            cache.set(photos, 600);
            log!("All-photos cache pre-warmed successfully");
        }
        Err(_) => log!("Failed to acquire lock for all-photos cache pre-warming"),
    }
}

#[cfg(all(feature = "ssr", test))]
mod tests {
    use super::*;
    use crate::types::{GalleryData, PhotoInfo};

    fn empty_gallery_data() -> Arc<GalleryData> {
        Arc::new(GalleryData {
            photos: Vec::new(),
            mosaic_layout: None,
            mosaic_layout_tablet: None,
        })
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
        cache.set(Arc::clone(&photos), 600);
        let got = cache.get().expect("entry should be cached");
        assert!(Arc::ptr_eq(&got, &photos));
    }

    #[test]
    fn all_photos_cache_get_evicts_expired_entry() {
        let mut cache = AllPhotosCache::new();
        cache.cached_data = Some(CachedAllPhotos {
            photos: Arc::new(Vec::new()),
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
}

/// Spawn a background task that periodically evicts expired entries from
/// `MOSAIC_CACHE`. The work is O(num_galleries) per tick, so we keep it off
/// the request path. Tick interval is intentionally coarser than the
/// shortest TTL (5 min) — a stale entry waiting an extra minute to be
/// evicted is harmless; the per-key get() also evicts on access.
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
