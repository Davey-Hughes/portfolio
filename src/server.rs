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
        self.cached_data = Some(CachedAllPhotos { photos, expires_at });
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
    use crate::mosaic::{calculate_orientation_bias, generate_mosaic_with_images, MosaicConfig};

    let num_images = photos.len();
    let image_aspects = image_aspects(photos);

    // Scale container height linearly with photo count so average cell area
    // stays roughly constant as the gallery grows. `PHOTOS_PER_BASE_HEIGHT` is
    // how many photos we want to fit per `base_height` slice of the canvas.
    // The floor of 2.0 keeps small galleries from looking cramped.
    const PHOTOS_PER_BASE_HEIGHT: f64 = 3.0;
    let scale = (num_images as f64 / PHOTOS_PER_BASE_HEIGHT).max(2.0);
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

                // Tablet uses a CSS multi-column masonry on the
                // `.photo-grid-mobile` div — no server-computed layout needed.
                let data = GalleryData {
                    photos: reordered_photos,
                    mosaic_layout: Some(layout_desktop),
                    mosaic_layout_tablet: None,
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
        use notify::event::{CreateKind, RemoveKind};
        use notify::EventKind;
        assert!(watch_event_is_relevant(&EventKind::Create(
            CreateKind::File
        )));
        assert!(watch_event_is_relevant(&EventKind::Create(
            CreateKind::Folder
        )));
        assert!(watch_event_is_relevant(&EventKind::Create(CreateKind::Any)));
        assert!(watch_event_is_relevant(&EventKind::Remove(
            RemoveKind::File
        )));
        assert!(watch_event_is_relevant(&EventKind::Remove(
            RemoveKind::Folder
        )));
        assert!(watch_event_is_relevant(&EventKind::Remove(RemoveKind::Any)));
    }

    #[test]
    fn watch_event_is_relevant_includes_data_and_rename_modifies() {
        use notify::event::{DataChange, ModifyKind, RenameMode};
        use notify::EventKind;
        assert!(watch_event_is_relevant(&EventKind::Modify(
            ModifyKind::Data(DataChange::Content)
        )));
        assert!(watch_event_is_relevant(&EventKind::Modify(
            ModifyKind::Data(DataChange::Size)
        )));
        assert!(watch_event_is_relevant(&EventKind::Modify(
            ModifyKind::Data(DataChange::Any)
        )));
        assert!(watch_event_is_relevant(&EventKind::Modify(
            ModifyKind::Name(RenameMode::From)
        )));
        assert!(watch_event_is_relevant(&EventKind::Modify(
            ModifyKind::Name(RenameMode::To)
        )));
        assert!(watch_event_is_relevant(&EventKind::Modify(
            ModifyKind::Name(RenameMode::Both)
        )));
        assert!(watch_event_is_relevant(&EventKind::Modify(ModifyKind::Any)));
        assert!(watch_event_is_relevant(&EventKind::Modify(
            ModifyKind::Other
        )));
    }

    #[test]
    fn watch_event_is_relevant_excludes_metadata_modifies() {
        // The original bug surface: rsync/syncthing/touch churn fires
        // these and the watcher was treating them as content changes.
        use notify::event::{MetadataKind, ModifyKind};
        use notify::EventKind;
        assert!(!watch_event_is_relevant(&EventKind::Modify(
            ModifyKind::Metadata(MetadataKind::AccessTime)
        )));
        assert!(!watch_event_is_relevant(&EventKind::Modify(
            ModifyKind::Metadata(MetadataKind::WriteTime)
        )));
        assert!(!watch_event_is_relevant(&EventKind::Modify(
            ModifyKind::Metadata(MetadataKind::Permissions)
        )));
        assert!(!watch_event_is_relevant(&EventKind::Modify(
            ModifyKind::Metadata(MetadataKind::Ownership)
        )));
        assert!(!watch_event_is_relevant(&EventKind::Modify(
            ModifyKind::Metadata(MetadataKind::Extended)
        )));
        assert!(!watch_event_is_relevant(&EventKind::Modify(
            ModifyKind::Metadata(MetadataKind::Any)
        )));
    }

    #[test]
    fn watch_event_is_relevant_excludes_access_events() {
        // The headline cause of the constant invalidation: every read
        // (cache prewarm, every HTTP image serve) generates these.
        use notify::event::{AccessKind, AccessMode};
        use notify::EventKind;
        assert!(!watch_event_is_relevant(&EventKind::Access(
            AccessKind::Read
        )));
        assert!(!watch_event_is_relevant(&EventKind::Access(
            AccessKind::Open(AccessMode::Read)
        )));
        assert!(!watch_event_is_relevant(&EventKind::Access(
            AccessKind::Close(AccessMode::Read)
        )));
        assert!(!watch_event_is_relevant(&EventKind::Access(
            AccessKind::Any
        )));
    }

    #[test]
    fn watch_event_is_relevant_excludes_any_and_other_top_level() {
        // Be conservative on uncategorized events.
        use notify::EventKind;
        assert!(!watch_event_is_relevant(&EventKind::Any));
        assert!(!watch_event_is_relevant(&EventKind::Other));
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
fn watch_event_is_relevant(kind: &notify::EventKind) -> bool {
    use notify::event::ModifyKind;
    use notify::EventKind;
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
                Ok(_) => deleted += 1,
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
        if !Path::new(&images_dir).exists() {
            if let Err(err) = std::fs::create_dir_all(&images_dir) {
                log!("Image watcher: failed to create {images_dir}: {err}");
                return;
            }
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
            // Block until something happens.
            let first = match rx.recv() {
                Ok(ev) => ev,
                Err(_) => return, // sender dropped; watcher gone
            };

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
                if !watch_event_is_relevant(&event.kind) {
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
