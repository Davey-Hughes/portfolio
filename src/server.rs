use crate::config::SiteConfig;
use crate::types::{AboutContent, GalleryData, GalleryInfo, PhotoInfo};
use leptos::prelude::*;

#[cfg(feature = "ssr")]
use once_cell::sync::Lazy;
#[cfg(feature = "ssr")]
use std::sync::{Arc, Mutex};
#[cfg(feature = "ssr")]
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(feature = "ssr")]
static MOSAIC_CACHE: Lazy<Arc<Mutex<MosaicCache>>> =
    Lazy::new(|| Arc::new(Mutex::new(MosaicCache::new())));

#[cfg(feature = "ssr")]
struct CachedMosaic {
    data: GalleryData,
    expires_at: u64, // Unix timestamp
}

#[cfg(feature = "ssr")]
struct MosaicCache {
    entries: std::collections::HashMap<String, CachedMosaic>,
}

#[cfg(feature = "ssr")]
impl MosaicCache {
    fn new() -> Self {
        Self {
            entries: std::collections::HashMap::new(),
        }
    }

    fn get(&mut self, key: &str) -> Option<GalleryData> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        if let Some(cached) = self.entries.get(key) {
            if cached.expires_at > now {
                return Some(cached.data.clone());
            } else {
                // Expired, remove it
                self.entries.remove(key);
            }
        }
        None
    }

    fn set(&mut self, key: String, data: GalleryData, duration_secs: u64) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        self.entries.insert(
            key,
            CachedMosaic {
                data,
                expires_at: now + duration_secs,
            },
        );
    }

    fn clear_expired(&mut self) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        self.entries.retain(|_, cached| cached.expires_at > now);
    }
}

#[cfg(feature = "ssr")]
fn generate_mosaic_layout_for_size(
    photos: &[crate::types::PhotoInfo],
    container_width: f64,
    base_height: f64,
) -> (crate::types::MosaicLayout, Vec<usize>) {
    use crate::mosaic::{generate_mosaic_with_images, MosaicConfig};

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

    let mosaic_config = MosaicConfig {
        container_width,
        container_height,
        min_cell_dimension: 180.0,
        min_aspect_ratio: 0.4,
        max_aspect_ratio: 3.0,
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

/// Server function to get home gallery data with pre-computed mosaic layout
#[server(GetHomeGalleryData, "/api")]
pub async fn get_home_gallery_data() -> Result<GalleryData, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use std::path::Path;

        let cache_key = "home_gallery".to_string();

        // Check cache first
        if let Ok(mut cache) = MOSAIC_CACHE.lock() {
            cache.clear_expired(); // Clean up expired entries
            if let Some(cached_data) = cache.get(&cache_key) {
                return Ok(cached_data);
            }
        }

        let photos = crate::gallery::load_home_photos();
        let home_path = Path::new("public/images/home");
        let config = crate::gallery::load_gallery_config(home_path);

        // Check if we should generate mosaic layout
        if let Some(cfg) = config.clone() {
            if cfg.use_mosaic.unwrap_or(false) && !photos.is_empty() {
                // Generate desktop layout (1200px)
                let (layout_desktop, image_order) =
                    generate_mosaic_layout_for_size(&photos, 1200.0, 600.0);

                // Generate tablet layout (768px)
                let (layout_tablet, _) = generate_mosaic_layout_for_size(&photos, 768.0, 500.0);

                // Reorder photos to match the desktop layout
                let reordered_photos: Vec<PhotoInfo> =
                    image_order.iter().map(|&idx| photos[idx].clone()).collect();

                let result = GalleryData {
                    photos: reordered_photos,
                    mosaic_layout: Some(layout_desktop),
                    mosaic_layout_tablet: Some(layout_tablet),
                };

                // Cache the result
                let cache_duration = cfg.mosaic_cache_duration.unwrap_or(3600);
                if let Ok(mut cache) = MOSAIC_CACHE.lock() {
                    cache.set(cache_key, result.clone(), cache_duration);
                }

                return Ok(result);
            }
        }

        let result = GalleryData {
            photos,
            mosaic_layout: None,
            mosaic_layout_tablet: None,
        };

        // Cache non-mosaic galleries too (shorter duration)
        if let Ok(mut cache) = MOSAIC_CACHE.lock() {
            cache.set(cache_key, result.clone(), 300); // 5 minutes for non-mosaic
        }

        Ok(result)
    }
    #[cfg(not(feature = "ssr"))]
    {
        Ok(GalleryData {
            photos: Vec::new(),
            mosaic_layout: None,
            mosaic_layout_tablet: None,
        })
    }
}

/// Server function to get photos from ALL galleries (for photo detail page)
#[server(GetAllGalleryPhotos, "/api")]
pub async fn get_all_gallery_photos() -> Result<Vec<PhotoInfo>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        Ok(crate::gallery::load_all_gallery_photos())
    }
    #[cfg(not(feature = "ssr"))]
    {
        Ok(Vec::new())
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
pub async fn get_gallery_data_by_name(gallery_name: String) -> Result<GalleryData, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use std::path::Path;

        let cache_key = format!("gallery_{}", gallery_name);

        // Check cache first
        if let Ok(mut cache) = MOSAIC_CACHE.lock() {
            cache.clear_expired();
            if let Some(cached_data) = cache.get(&cache_key) {
                return Ok(cached_data);
            }
        }

        let photos = crate::gallery::load_gallery_photos(&gallery_name)
            .ok_or_else(|| ServerFnError::new("Gallery not found"))?;

        let gallery_path = Path::new("public/images").join(&gallery_name);
        let config = crate::gallery::load_gallery_config(&gallery_path);

        // Check if we should generate mosaic layout
        if let Some(cfg) = config.clone() {
            if cfg.use_mosaic.unwrap_or(false) && !photos.is_empty() {
                // Generate desktop layout (1200px)
                let (layout_desktop, image_order) =
                    generate_mosaic_layout_for_size(&photos, 1200.0, 600.0);

                // Generate tablet layout (768px)
                let (layout_tablet, _) = generate_mosaic_layout_for_size(&photos, 768.0, 500.0);

                // Reorder photos to match the desktop layout
                let reordered_photos: Vec<PhotoInfo> =
                    image_order.iter().map(|&idx| photos[idx].clone()).collect();

                let result = GalleryData {
                    photos: reordered_photos,
                    mosaic_layout: Some(layout_desktop),
                    mosaic_layout_tablet: Some(layout_tablet),
                };

                // Cache the result
                let cache_duration = cfg.mosaic_cache_duration.unwrap_or(3600);
                if let Ok(mut cache) = MOSAIC_CACHE.lock() {
                    cache.set(cache_key, result.clone(), cache_duration);
                }

                return Ok(result);
            }
        }

        let result = GalleryData {
            photos,
            mosaic_layout: None,
            mosaic_layout_tablet: None,
        };

        // Cache non-mosaic galleries too (shorter duration)
        if let Ok(mut cache) = MOSAIC_CACHE.lock() {
            cache.set(cache_key, result.clone(), 300); // 5 minutes for non-mosaic
        }

        Ok(result)
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = gallery_name;
        Ok(GalleryData {
            photos: Vec::new(),
            mosaic_layout: None,
            mosaic_layout_tablet: None,
        })
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
