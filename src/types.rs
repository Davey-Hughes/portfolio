use serde::{Deserialize, Serialize};

/// Represents an image source with its URL and MIME type for use in <picture> elements
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct ImageSource {
    pub url: String,
    pub mime_type: String,
}

/// Represents metadata and information for a photo in the portfolio.
///
/// # Examples
///
/// ```
/// use portfolio::types::PhotoInfo;
///
/// let photo = PhotoInfo {
///     url: "/images/photo.jpg".to_string(),
///     original_url: "/images/photo.jpg".to_string(),
///     sources: vec![],
///     original_sources: vec![],
///     title: "Sunset".to_string(),
///     filename: "photo.jpg".to_string(),
///     slug: "sunset".to_string(),
///     gallery_name: "landscapes".to_string(),
///     subfolder: None,
///     width: Some(1920),
///     height: Some(1080),
///     date_taken: Some("2024-01-15".to_string()),
///     camera_make: Some("Canon".to_string()),
///     camera_model: Some("EOS R5".to_string()),
///     lens_model: Some("RF 24-70mm".to_string()),
///     focal_length: Some("50 mm".to_string()),
///     aperture: Some("f/2.8".to_string()),
///     shutter_speed: Some("1/200 s".to_string()),
///     iso: Some("ISO 100".to_string()),
/// };
///
/// assert_eq!(photo.title, "Sunset");
/// assert_eq!(photo.width, Some(1920));
/// ```
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct PhotoInfo {
    pub url: String,                        // Primary/fallback image URL
    pub original_url: String,               // Full resolution primary image
    pub sources: Vec<ImageSource>,          // Alternative compressed formats
    pub original_sources: Vec<ImageSource>, // Alternative original formats
    pub title: String,
    pub filename: String,
    pub slug: String,
    pub gallery_name: String, // Name of the gallery this photo belongs to
    pub subfolder: Option<String>, // Subfolder path relative to gallery root
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

/// Represents information about a photo gallery.
///
/// # Examples
///
/// ```
/// use portfolio::types::GalleryInfo;
///
/// let gallery = GalleryInfo {
///     name: "Landscapes".to_string(),
///     slug: "landscapes".to_string(),
///     photo_count: 42,
///     config: None,
/// };
///
/// assert_eq!(gallery.name, "Landscapes");
/// assert_eq!(gallery.photo_count, 42);
/// ```
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct GalleryInfo {
    pub name: String,
    pub slug: String,
    pub photo_count: usize,
    pub config: Option<GalleryConfig>,
}

/// Configuration for gallery grid layout
///
/// # Examples
///
/// ```
/// use portfolio::types::GalleryConfig;
///
/// let config = GalleryConfig {
///     columns: Some(6),
///     row_height: Some(280),
///     gap: Some(8),
///     use_mosaic: None,
///     mosaic_cache_duration: None,
/// };
///
/// assert_eq!(config.columns, Some(6));
/// ```
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct GalleryConfig {
    pub columns: Option<u32>,
    pub row_height: Option<u32>,
    pub gap: Option<u32>,
    pub use_mosaic: Option<bool>,
    pub mosaic_cache_duration: Option<u64>, // Cache duration in seconds
}

impl Default for GalleryConfig {
    fn default() -> Self {
        Self {
            columns: Some(6),
            row_height: Some(280),
            gap: Some(8), // 0.5rem = 8px
            use_mosaic: None,
            mosaic_cache_duration: Some(3600), // Default 1 hour cache
        }
    }
}

/// Represents a cell in the mosaic grid layout
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct MosaicCell {
    pub row_start: u32,
    pub row_end: u32,
    pub col_start: u32,
    pub col_end: u32,
}

/// Layout data for mosaic-style galleries
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct MosaicLayout {
    pub cells: Vec<MosaicCell>,
    pub grid_rows: u32,
    pub grid_cols: u32,
    pub container_height: f64, // Actual height in pixels
}

/// Gallery data with photos and optional pre-computed mosaic layout
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct GalleryData {
    pub photos: Vec<PhotoInfo>,
    pub mosaic_layout: Option<MosaicLayout>, // Desktop layout
    pub mosaic_layout_tablet: Option<MosaicLayout>, // Tablet layout
}

/// Content for the About page including optional profile image and text.
///
/// # Examples
///
/// ```
/// use portfolio::types::AboutContent;
///
/// let about = AboutContent {
///     image_url: Some("/content/profile.jpg".to_string()),
///     content: "Welcome to my portfolio!".to_string(),
///     is_html: false,
/// };
///
/// assert_eq!(about.content, "Welcome to my portfolio!");
/// ```
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct AboutContent {
    pub image_url: Option<String>,
    pub content: String,
    pub is_html: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_photo_info_serialization() {
        let photo = PhotoInfo {
            url: "/images/test.jpg".to_string(),
            original_url: "/images/test.jpg".to_string(),
            sources: vec![],
            original_sources: vec![],
            title: "Test Photo".to_string(),
            filename: "test.jpg".to_string(),
            slug: "test-photo".to_string(),
            gallery_name: "home".to_string(),
            subfolder: None,
            width: Some(1920),
            height: Some(1080),
            date_taken: Some("2024-01-15 14:30".to_string()),
            camera_make: Some("Canon".to_string()),
            camera_model: Some("EOS R5".to_string()),
            lens_model: Some("RF 24-70mm F2.8".to_string()),
            focal_length: Some("50 mm".to_string()),
            aperture: Some("f/2.8".to_string()),
            shutter_speed: Some("1/200 s".to_string()),
            iso: Some("ISO 100".to_string()),
        };

        let json = leptos::serde_json::to_string(&photo).unwrap();
        let deserialized: PhotoInfo = leptos::serde_json::from_str(&json).unwrap();

        assert_eq!(photo.url, deserialized.url);
        assert_eq!(photo.title, deserialized.title);
        assert_eq!(photo.width, deserialized.width);
        assert_eq!(photo.camera_make, deserialized.camera_make);
    }

    #[test]
    fn test_photo_info_with_missing_exif() {
        let photo = PhotoInfo {
            url: "/images/test.jpg".to_string(),
            original_url: "/images/test.jpg".to_string(),
            sources: vec![],
            original_sources: vec![],
            title: "Test Photo".to_string(),
            filename: "test.jpg".to_string(),
            slug: "test-photo".to_string(),
            gallery_name: "home".to_string(),
            subfolder: None,
            width: None,
            height: None,
            date_taken: None,
            camera_make: None,
            camera_model: None,
            lens_model: None,
            focal_length: None,
            aperture: None,
            shutter_speed: None,
            iso: None,
        };

        let json = leptos::serde_json::to_string(&photo).unwrap();
        let deserialized: PhotoInfo = leptos::serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.width, None);
        assert_eq!(deserialized.camera_make, None);
    }

    #[test]
    fn test_gallery_info_serialization() {
        let gallery = GalleryInfo {
            name: "Landscapes".to_string(),
            slug: "landscapes".to_string(),
            photo_count: 42,
            config: None,
        };

        let json = leptos::serde_json::to_string(&gallery).unwrap();
        let deserialized: GalleryInfo = leptos::serde_json::from_str(&json).unwrap();

        assert_eq!(gallery.name, deserialized.name);
        assert_eq!(gallery.slug, deserialized.slug);
        assert_eq!(gallery.photo_count, deserialized.photo_count);
    }

    #[test]
    fn test_about_content_serialization() {
        let about = AboutContent {
            image_url: Some("/content/profile.jpg".to_string()),
            content: "This is my portfolio website.".to_string(),
            is_html: false,
        };

        let json = leptos::serde_json::to_string(&about).unwrap();
        let deserialized: AboutContent = leptos::serde_json::from_str(&json).unwrap();

        assert_eq!(about.image_url, deserialized.image_url);
        assert_eq!(about.content, deserialized.content);
        assert_eq!(about.is_html, deserialized.is_html);
    }

    #[test]
    fn test_about_content_without_image() {
        let about = AboutContent {
            image_url: None,
            content: "This is my portfolio website.".to_string(),
            is_html: false,
        };

        let json = leptos::serde_json::to_string(&about).unwrap();
        let deserialized: AboutContent = leptos::serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.image_url, None);
        assert_eq!(deserialized.content, about.content);
        assert_eq!(deserialized.is_html, false);
    }

    #[test]
    fn test_photo_info_clone() {
        let photo = PhotoInfo {
            url: "/images/test.jpg".to_string(),
            original_url: "/images/test.jpg".to_string(),
            sources: vec![],
            original_sources: vec![],
            title: "Test Photo".to_string(),
            filename: "test.jpg".to_string(),
            slug: "test-photo".to_string(),
            gallery_name: "home".to_string(),
            subfolder: None,
            width: Some(1920),
            height: Some(1080),
            date_taken: None,
            camera_make: None,
            camera_model: None,
            lens_model: None,
            focal_length: None,
            aperture: None,
            shutter_speed: None,
            iso: None,
        };

        let cloned = photo.clone();
        assert_eq!(photo.url, cloned.url);
        assert_eq!(photo.width, cloned.width);
    }
}
