use chrono::Datelike;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Configuration for the portfolio site loaded from a config file.
///
/// # Examples
///
/// ```
/// use portfolio::config::SiteConfig;
/// use std::collections::HashMap;
///
/// let config = SiteConfig::default();
/// assert_eq!(config.site_name, "Your Name");
/// assert_eq!(config.site_tagline, "Photography");
/// ```
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
        site_name = sections
            .remove("site_name")
            .unwrap_or_else(|| "Your Name".to_string());
        site_tagline = sections
            .remove("site_tagline")
            .unwrap_or_else(|| "Photography".to_string());
        site_copyright = sections
            .remove("site_copyright")
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_site_config_default() {
        let config = SiteConfig::default();

        assert_eq!(config.site_name, "Your Name");
        assert_eq!(config.site_tagline, "Photography");
        assert!(config.site_copyright.contains("Your Name"));
        assert!(config.site_copyright.contains("©"));
        assert!(config.sections.is_empty());
    }

    #[test]
    fn test_site_config_default_copyright_has_current_year() {
        let config = SiteConfig::default();
        let current_year = chrono::Local::now().year();

        assert!(config.site_copyright.contains(&current_year.to_string()));
    }

    #[test]
    fn test_site_config_clone() {
        let config = SiteConfig::default();
        let cloned = config.clone();

        assert_eq!(config.site_name, cloned.site_name);
        assert_eq!(config.site_tagline, cloned.site_tagline);
        assert_eq!(config.site_copyright, cloned.site_copyright);
    }

    #[cfg(feature = "ssr")]
    #[test]
    fn test_load_config_fallback_to_default() {
        // Temporarily set CONFIG_PATH to a non-existent file
        std::env::set_var("CONFIG_PATH", "/tmp/nonexistent_config_file_12345.txt");

        let config = load_config();

        assert_eq!(config.site_name, "Your Name");
        assert_eq!(config.site_tagline, "Photography");

        std::env::remove_var("CONFIG_PATH");
    }

    #[cfg(feature = "ssr")]
    #[test]
    fn test_load_config_from_content() {
        use std::fs;

        let temp_dir = std::env::temp_dir();
        let config_file = temp_dir.join(format!(
            "test_config_{}_{}.txt",
            std::process::id(),
            line!()
        ));

        let content = r#"
# This is a comment
site_name=John Doe Photography
site_tagline=Capturing Moments
site_copyright=© 2024 John Doe

# Custom sections
about_title=About Me
contact_email=john@example.com
"#;

        fs::write(&config_file, content).unwrap();
        std::env::set_var("CONFIG_PATH", config_file.to_str().unwrap());

        let config = load_config();

        assert_eq!(config.site_name, "John Doe Photography");
        assert_eq!(config.site_tagline, "Capturing Moments");
        assert_eq!(config.site_copyright, "© 2024 John Doe");
        assert_eq!(
            config.sections.get("about_title"),
            Some(&"About Me".to_string())
        );
        assert_eq!(
            config.sections.get("contact_email"),
            Some(&"john@example.com".to_string())
        );

        // Cleanup
        std::env::remove_var("CONFIG_PATH");
        fs::remove_file(&config_file).ok();
    }

    #[cfg(feature = "ssr")]
    #[test]
    fn test_load_config_ignores_comments_and_empty_lines() {
        use std::fs;

        let temp_dir = std::env::temp_dir();
        let config_file = temp_dir.join(format!(
            "test_config_comments_{}_{}.txt",
            std::process::id(),
            line!()
        ));

        let content = r#"
# Comment at start

site_name=Test Site

# Another comment
site_tagline=Test Tagline

custom_field=value
"#;

        fs::write(&config_file, content).unwrap();
        std::env::set_var("CONFIG_PATH", config_file.to_str().unwrap());

        let config = load_config();

        assert_eq!(config.site_name, "Test Site");
        assert_eq!(config.site_tagline, "Test Tagline");
        assert_eq!(
            config.sections.get("custom_field"),
            Some(&"value".to_string())
        );

        // Cleanup
        std::env::remove_var("CONFIG_PATH");
        fs::remove_file(&config_file).ok();
    }

    #[cfg(feature = "ssr")]
    #[test]
    fn test_load_config_handles_whitespace() {
        use std::fs;

        let temp_dir = std::env::temp_dir();
        let config_file = temp_dir.join(format!(
            "test_config_whitespace_{}_{}.txt",
            std::process::id(),
            line!()
        ));

        let content = "  site_name  =  Whitespace Test  \n  custom_key  =  value with spaces  ";

        fs::write(&config_file, content).unwrap();
        std::env::set_var("CONFIG_PATH", config_file.to_str().unwrap());

        let config = load_config();

        assert_eq!(config.site_name, "Whitespace Test");
        assert_eq!(
            config.sections.get("custom_key"),
            Some(&"value with spaces".to_string())
        );

        // Cleanup
        std::env::remove_var("CONFIG_PATH");
        fs::remove_file(&config_file).ok();
    }

    #[cfg(feature = "ssr")]
    #[test]
    fn test_load_config_custom_sections() {
        use std::fs;

        let temp_dir = std::env::temp_dir();
        let config_file = temp_dir.join(format!(
            "test_config_sections_{}_{}.txt",
            std::process::id(),
            line!()
        ));

        let content = r#"
site_name=My Portfolio
custom_section_1=Value 1
custom_section_2=Value 2
another_field=Another Value
"#;

        fs::write(&config_file, content).unwrap();
        std::env::set_var("CONFIG_PATH", config_file.to_str().unwrap());

        let config = load_config();

        assert_eq!(config.sections.len(), 3);
        assert_eq!(
            config.sections.get("custom_section_1"),
            Some(&"Value 1".to_string())
        );
        assert_eq!(
            config.sections.get("custom_section_2"),
            Some(&"Value 2".to_string())
        );
        assert_eq!(
            config.sections.get("another_field"),
            Some(&"Another Value".to_string())
        );

        // Special keys should not be in sections
        assert_eq!(config.sections.get("site_name"), None);
        assert_eq!(config.sections.get("site_tagline"), None);
        assert_eq!(config.sections.get("site_copyright"), None);

        // Cleanup
        std::env::remove_var("CONFIG_PATH");
        fs::remove_file(&config_file).ok();
    }
}
