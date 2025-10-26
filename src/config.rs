use chrono::Datelike;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SiteConfig {
    pub site_name: String,
    pub site_tagline: String,
    pub site_copyright: String,
    pub sections: HashMap<String, String>,
}

impl Default for SiteConfig {
    fn default() -> Self {
        let site_name = "Your Name".to_string();
        let current_year = chrono::Local::now().year();
        Self {
            site_name: site_name.clone(),
            site_tagline: "Photography".to_string(),
            site_copyright: format!("© {} {}. All rights reserved.", current_year, site_name),
            sections: HashMap::new(),
        }
    }
}

#[cfg(feature = "ssr")]
pub fn load_config() -> SiteConfig {
    use std::fs;
    use std::path::Path;

    let config_path = std::env::var("CONFIG_PATH").unwrap_or_else(|_| {
        if Path::new("public/content/config.txt").exists() {
            "public/content/config.txt".to_string()
        } else {
            "./content/config.txt".to_string()
        }
    });

    let site_name;
    let site_tagline;
    let site_copyright;
    let mut sections = HashMap::new();

    if let Ok(content) = fs::read_to_string(&config_path) {
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            if let Some((key, value)) = line.split_once('=') {
                let key = key.trim();
                let value = value.trim();
                sections.insert(key.to_string(), value.to_string());
            }
        }

        let current_year = chrono::Local::now().year();
        site_name = sections.remove("site_name")
            .unwrap_or_else(|| "Your Name".to_string());
        site_tagline = sections.remove("site_tagline")
            .unwrap_or_else(|| "Photography".to_string());
        site_copyright = sections.remove("site_copyright")
            .unwrap_or_else(|| format!("© {} {}. All rights reserved.", current_year, site_name));
    } else {
        // Fallback to default if file doesn't exist
        let default = SiteConfig::default();
        return default;
    }

    SiteConfig {
        site_name,
        site_tagline,
        site_copyright,
        sections,
    }
}

#[cfg(not(feature = "ssr"))]
pub fn load_config() -> SiteConfig {
    SiteConfig::default()
}
