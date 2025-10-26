use serde::{Deserialize, Serialize};

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

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct GalleryInfo {
    pub name: String,
    pub slug: String,
    pub photo_count: usize,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct AboutContent {
    pub image_url: Option<String>,
    pub content: String,
}
