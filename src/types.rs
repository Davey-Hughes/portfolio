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
/// };
///
/// assert_eq!(about.content, "Welcome to my portfolio!");
/// ```
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct AboutContent {
    pub image_url: Option<String>,
    pub content: String,
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
        };

        let json = leptos::serde_json::to_string(&about).unwrap();
        let deserialized: AboutContent = leptos::serde_json::from_str(&json).unwrap();

        assert_eq!(about.image_url, deserialized.image_url);
        assert_eq!(about.content, deserialized.content);
    }

    #[test]
    fn test_about_content_without_image() {
        let about = AboutContent {
            image_url: None,
            content: "This is my portfolio website.".to_string(),
        };

        let json = leptos::serde_json::to_string(&about).unwrap();
        let deserialized: AboutContent = leptos::serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.image_url, None);
        assert_eq!(deserialized.content, about.content);
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
