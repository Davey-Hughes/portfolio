use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SiteConfig {
    pub site_name: String,
    pub site_tagline: String,
    pub site_copyright: String,
    pub contact_email: Option<String>,
    pub contact_phone: Option<String>,
    pub contact_location: Option<String>,
}

impl Default for SiteConfig {
    fn default() -> Self {
        Self {
            site_name: "Your Name".to_string(),
            site_tagline: "Photography".to_string(),
            site_copyright: "© 2025 Your Photography. All rights reserved.".to_string(),
            contact_email: None,
            contact_phone: None,
            contact_location: None,
        }
    }
}

#[cfg(feature = "ssr")]
pub fn load_config() -> SiteConfig {
    use std::env;
    
    // Try to load .env file, but don't panic if it doesn't exist
    dotenvy::dotenv().ok();
    
    SiteConfig {
        site_name: env::var("SITE_NAME").unwrap_or_else(|_| "Your Name".to_string()),
        site_tagline: env::var("SITE_TAGLINE").unwrap_or_else(|_| "Photography".to_string()),
        site_copyright: env::var("SITE_COPYRIGHT")
            .unwrap_or_else(|_| "© 2025 Your Photography. All rights reserved.".to_string()),
        contact_email: env::var("CONTACT_EMAIL").ok(),
        contact_phone: env::var("CONTACT_PHONE").ok(),
        contact_location: env::var("CONTACT_LOCATION").ok(),
    }
}

#[cfg(not(feature = "ssr"))]
pub fn load_config() -> SiteConfig {
    SiteConfig::default()
}
