use crate::config::SiteConfig;
use crate::types::{AboutContent, GalleryInfo, PhotoInfo};
use leptos::prelude::*;

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
