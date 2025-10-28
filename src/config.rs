use chrono::Datelike;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A section value can be either a simple string or a structured link with display text and URL
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SectionValue {
    /// A simple string value
    Simple(String),
    /// A structured link with display text and URL
    Link { display: String, url: String },
}

impl SectionValue {
    /// Get the display text for this value
    pub fn display(&self) -> &str {
        match self {
            SectionValue::Simple(s) => s,
            SectionValue::Link { display, .. } => display,
        }
    }

    /// Get the URL if this is a Link variant
    pub fn url(&self) -> Option<&str> {
        match self {
            SectionValue::Simple(_) => None,
            SectionValue::Link { url, .. } => Some(url),
        }
    }

    /// Check if this is a simple string value
    pub fn is_simple(&self) -> bool {
        matches!(self, SectionValue::Simple(_))
    }
}

/// Configuration for the portfolio site loaded from a TOML config file.
///
/// # TOML Format
///
/// ```toml
/// site_name = "John Doe"
/// site_tagline = "Photography Portfolio"
/// # site_title is optional - if not specified, it will default to site_name
/// # site_title = "John Doe Photography"
/// # site_copyright is optional - if not specified, it will be auto-generated as:
/// # "© {current_year} {site_name}. All rights reserved."
/// # site_copyright = "© 2024 John Doe. All rights reserved."
///
/// [sections]
/// about_title = "About Me"
/// contact_email = "john@example.com"
/// ```
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub site_title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub site_copyright: Option<String>,
    #[serde(default)]
    pub sections: HashMap<String, SectionValue>,
}

impl SiteConfig {
    /// Get the page title, defaulting to site_name if not explicitly set
    pub fn title(&self) -> String {
        self.site_title
            .clone()
            .unwrap_or_else(|| self.site_name.clone())
    }

    /// Get the copyright text, generating it if not explicitly set
    pub fn copyright(&self) -> String {
        self.site_copyright.clone().unwrap_or_else(|| {
            let current_year = chrono::Local::now().year();
            format!(
                "© {} {}. All rights reserved.",
                current_year, self.site_name
            )
        })
    }
}

impl Default for SiteConfig {
    fn default() -> Self {
        Self {
            site_name: "Your Name".to_string(),
            site_tagline: "Photography".to_string(),
            site_title: None,
            site_copyright: None,
            sections: HashMap::new(),
        }
    }
}

#[cfg(feature = "ssr")]
pub fn load_config() -> SiteConfig {
    use std::fs;
    use std::path::Path;

    let config_path = std::env::var("CONFIG_PATH").unwrap_or_else(|_| {
        if Path::new("public/content/config.toml").exists() {
            "public/content/config.toml".to_string()
        } else {
            "./content/config.toml".to_string()
        }
    });

    if let Ok(content) = fs::read_to_string(&config_path) {
        // Try to parse as TOML
        if let Ok(config) = toml::from_str::<SiteConfig>(&content) {
            return config;
        }
    }

    // Fallback to default if file doesn't exist or parsing fails
    SiteConfig::default()
}

#[cfg(not(feature = "ssr"))]
pub fn load_config() -> SiteConfig {
    SiteConfig::default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[test]
    fn test_site_config_default() {
        let config = SiteConfig::default();

        assert_eq!(config.site_name, "Your Name");
        assert_eq!(config.site_tagline, "Photography");
        assert!(config.copyright().contains("Your Name"));
        assert!(config.copyright().contains("©"));
        assert!(config.sections.is_empty());
    }

    #[test]
    fn test_site_config_default_copyright_has_current_year() {
        let config = SiteConfig::default();
        let current_year = chrono::Local::now().year();

        assert!(config.copyright().contains(&current_year.to_string()));
    }

    #[test]
    fn test_site_config_clone() {
        let config = SiteConfig::default();
        let cloned = config.clone();

        assert_eq!(config.site_name, cloned.site_name);
        assert_eq!(config.site_tagline, cloned.site_tagline);
        assert_eq!(config.copyright(), cloned.copyright());
    }

    #[cfg(feature = "ssr")]
    #[test]
    #[serial]
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
    #[serial]
    fn test_load_config_from_toml() {
        use std::fs;

        let temp_dir = std::env::temp_dir();
        let config_file = temp_dir.join(format!(
            "test_config_{}_{}.toml",
            std::process::id(),
            line!()
        ));

        let content = r#"
# This is a comment
site_name = "John Doe Photography"
site_tagline = "Capturing Moments"
site_copyright = "© 2024 John Doe"

# Custom sections
[sections]
about_title = "About Me"
contact_email = "john@example.com"
"#;

        fs::write(&config_file, content).unwrap();
        std::env::set_var("CONFIG_PATH", config_file.to_str().unwrap());

        let config = load_config();

        assert_eq!(config.site_name, "John Doe Photography");
        assert_eq!(config.site_tagline, "Capturing Moments");
        assert_eq!(config.copyright(), "© 2024 John Doe");
        assert_eq!(
            config.sections.get("about_title"),
            Some(&SectionValue::Simple("About Me".to_string()))
        );
        assert_eq!(
            config.sections.get("contact_email"),
            Some(&SectionValue::Simple("john@example.com".to_string()))
        );

        // Cleanup
        std::env::remove_var("CONFIG_PATH");
        fs::remove_file(&config_file).ok();
    }

    #[cfg(feature = "ssr")]
    #[test]
    #[serial]
    fn test_load_config_toml_comments_and_empty_lines() {
        use std::fs;

        let temp_dir = std::env::temp_dir();
        let config_file = temp_dir.join(format!(
            "test_config_comments_{}_{}.toml",
            std::process::id(),
            line!()
        ));

        let content = r#"
# Comment at start

site_name = "Test Site"

# Another comment
site_tagline = "Test Tagline"
site_copyright = "© 2024 Test"

[sections]
custom_field = "value"
"#;

        fs::write(&config_file, content).unwrap();
        std::env::set_var("CONFIG_PATH", config_file.to_str().unwrap());

        let config = load_config();

        assert_eq!(config.site_name, "Test Site");
        assert_eq!(config.site_tagline, "Test Tagline");
        assert_eq!(
            config.sections.get("custom_field"),
            Some(&SectionValue::Simple("value".to_string()))
        );

        // Cleanup
        std::env::remove_var("CONFIG_PATH");
        fs::remove_file(&config_file).ok();
    }

    #[cfg(feature = "ssr")]
    #[test]
    #[serial]
    fn test_load_config_toml_whitespace() {
        use std::fs;

        let temp_dir = std::env::temp_dir();
        let config_file = temp_dir.join(format!(
            "test_config_whitespace_{}_{}.toml",
            std::process::id(),
            line!()
        ));

        let content = r#"
  site_name  =  "Whitespace Test"
  site_tagline = "Test"
  site_copyright = "Test"

  [sections]
  custom_key  =  "value with spaces"
"#;

        fs::write(&config_file, content).unwrap();
        std::env::set_var("CONFIG_PATH", config_file.to_str().unwrap());

        let config = load_config();

        assert_eq!(config.site_name, "Whitespace Test");
        assert_eq!(
            config.sections.get("custom_key"),
            Some(&SectionValue::Simple("value with spaces".to_string()))
        );

        // Cleanup
        std::env::remove_var("CONFIG_PATH");
        fs::remove_file(&config_file).ok();
    }

    #[cfg(feature = "ssr")]
    #[test]
    #[serial]
    fn test_load_config_toml_custom_sections() {
        use std::fs;

        let temp_dir = std::env::temp_dir();
        let config_file = temp_dir.join(format!(
            "test_config_sections_{}_{}.toml",
            std::process::id(),
            line!()
        ));

        let content = r#"
site_name = "My Portfolio"
site_tagline = "Photography"
site_copyright = "© 2024"

[sections]
custom_section_1 = "Value 1"
custom_section_2 = "Value 2"
another_field = "Another Value"
"#;

        fs::write(&config_file, content).unwrap();
        std::env::set_var("CONFIG_PATH", config_file.to_str().unwrap());

        let config = load_config();

        assert_eq!(config.sections.len(), 3);
        assert_eq!(
            config.sections.get("custom_section_1"),
            Some(&SectionValue::Simple("Value 1".to_string()))
        );
        assert_eq!(
            config.sections.get("custom_section_2"),
            Some(&SectionValue::Simple("Value 2".to_string()))
        );
        assert_eq!(
            config.sections.get("another_field"),
            Some(&SectionValue::Simple("Another Value".to_string()))
        );

        // Special keys should not be in sections
        assert_eq!(config.sections.get("site_name"), None);
        assert_eq!(config.sections.get("site_tagline"), None);
        assert_eq!(config.sections.get("site_copyright"), None);

        // Cleanup
        std::env::remove_var("CONFIG_PATH");
        fs::remove_file(&config_file).ok();
    }

    #[cfg(feature = "ssr")]
    #[test]
    #[serial]
    fn test_load_config_auto_copyright() {
        use std::fs;

        let temp_dir = std::env::temp_dir();
        let config_file = temp_dir.join(format!(
            "test_config_auto_copyright_{}_{}.toml",
            std::process::id(),
            line!()
        ));

        // Config without site_copyright field
        let content = r#"
site_name = "Test User"
site_tagline = "Test Tagline"

[sections]
test_field = "test value"
"#;

        fs::write(&config_file, content).unwrap();
        std::env::set_var("CONFIG_PATH", config_file.to_str().unwrap());

        let config = load_config();

        assert_eq!(config.site_name, "Test User");
        assert_eq!(config.site_tagline, "Test Tagline");

        // Copyright should be auto-generated
        let copyright = config.copyright();
        assert!(copyright.contains("Test User"));
        assert!(copyright.contains("©"));
        assert!(copyright.contains("All rights reserved"));

        let current_year = chrono::Local::now().year();
        assert!(copyright.contains(&current_year.to_string()));

        // Cleanup
        std::env::remove_var("CONFIG_PATH");
        fs::remove_file(&config_file).ok();
    }
}
