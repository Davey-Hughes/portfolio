use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[cfg(feature = "ssr")]
use chrono::Datelike;

/// Get the current year for copyright notice
/// Uses JavaScript Date API on client, chrono on server
fn get_current_year() -> i32 {
    // Gated on the target, not just the feature: js-sys' imports only exist on wasm, so
    // calling Date::new_0() from a host binary panics with "cannot call wasm-bindgen
    // imported functions on non-wasm targets". The hydrate test job does exactly that —
    // it compiles the lib under `hydrate` but runs the tests natively — so without the
    // target_arch check this arm is selected there and every test touching SiteConfig
    // panics.
    #[cfg(all(feature = "hydrate", target_arch = "wasm32"))]
    {
        js_sys::Date::new_0().get_full_year() as i32
    }

    #[cfg(feature = "ssr")]
    {
        chrono::Local::now().year()
    }

    // Reached by host-target builds with neither feature — in practice the `hydrate`
    // test run (see above). Derived from the clock rather than hardcoded: this used to
    // return a literal `2025`, which was silently wrong the moment the year turned.
    // chrono is an ssr-only dependency and js-sys needs wasm, so neither is available
    // here; walking the epoch by hand is exact and costs nothing at this call rate.
    #[cfg(not(any(all(feature = "hydrate", target_arch = "wasm32"), feature = "ssr")))]
    {
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        let mut days = i64::try_from(secs / 86_400).unwrap_or(0);
        let mut year: i32 = 1970;
        loop {
            let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
            let year_len = if leap { 366 } else { 365 };
            if days < year_len {
                break;
            }
            days -= year_len;
            year += 1;
        }
        year
    }
}

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
    #[must_use]
    pub fn display(&self) -> &str {
        match self {
            SectionValue::Simple(s) => s,
            SectionValue::Link { display, .. } => display,
        }
    }

    /// Get the URL if this is a Link variant
    #[must_use]
    pub fn url(&self) -> Option<&str> {
        match self {
            SectionValue::Simple(_) => None,
            SectionValue::Link { url, .. } => Some(url),
        }
    }

    /// Check if this is a simple string value
    #[must_use]
    pub fn is_simple(&self) -> bool {
        matches!(self, SectionValue::Simple(_))
    }
}

/// Prose name of the code license, shown on the About page.
///
/// Deliberately not configurable. The GPL's copyleft means a fork of this
/// repository cannot relicense it, so this string is true of every deployment —
/// unlike the photograph terms, which belong to whoever runs the site.
pub const CODE_LICENSE_NAME: &str = "GNU General Public License v3.0";

/// Canonical URL for the full text of the code license.
pub const CODE_LICENSE_URL: &str = "https://www.gnu.org/licenses/gpl-3.0.html";

/// Upstream source, used when `[license].source` is unset.
///
/// A fork that modifies the code should point `[license].source` at its own
/// repository: this default names *upstream*, which is not the source of a modified
/// deployment.
pub const DEFAULT_SOURCE_URL: &str = "https://github.com/Davey-Hughes/portfolio";

/// Elaboration rendered under the copyright line while the deployer has supplied no
/// terms of their own.
pub const DEFAULT_IMAGE_LICENSE_NOTE: &str = "These photographs are not licensed for reuse.";

/// Label for the footer link pointing at the About page's license section.
pub const DEFAULT_LICENSE_FOOTER_TEXT: &str = "License";

/// Licensing details for the About page's license section.
///
/// Every field is optional. Omitting the whole `[license]` table yields an
/// all-rights-reserved statement derived from `site_name` and the current year, so a
/// fresh deployment is correct with no configuration.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct LicenseConfig {
    /// Terms for the photographs. Replaces the generated copyright line *and*
    /// suppresses [`DEFAULT_IMAGE_LICENSE_NOTE`] — see
    /// [`SiteConfig::image_license_note`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub images: Option<String>,
    /// Address for licensing enquiries. No contact sentence renders when unset, or
    /// when set to an empty string.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contact: Option<String>,
    /// Public URL for this site's source. Defaults to [`DEFAULT_SOURCE_URL`]; an
    /// empty string falls back to that default rather than emitting `href=""`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// Label for the footer link to the license section. Defaults to
    /// [`DEFAULT_LICENSE_FOOTER_TEXT`]; set it to an empty string to suppress the
    /// footer link entirely. The empty string means "hide" here rather than "fall
    /// back" because this is the one field with a hidden state to express.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub footer_text: Option<String>,
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
/// # [license] is optional. Omit it and the About page states all rights reserved,
/// # derived from site_name and the current year.
/// # [license]
/// # images = "All rights reserved."
/// # contact = "you@example.com"
/// # source = "https://github.com/you/portfolio"
/// # footer_text = "License"
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
    /// Licensing terms shown on the About page. Optional; see [`LicenseConfig`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub license: Option<LicenseConfig>,
    /// Ordered list of gallery slugs controlling nav-bar order.
    /// Galleries listed here appear first in the given order; any galleries
    /// not listed fall through to alphabetical order afterwards.
    #[serde(default)]
    pub gallery_order: Vec<String>,
    #[serde(default)]
    pub sections: HashMap<String, SectionValue>,
}

impl SiteConfig {
    /// Get the page title, defaulting to `site_name` if not explicitly set
    #[must_use]
    pub fn title(&self) -> String {
        self.site_title
            .clone()
            .unwrap_or_else(|| self.site_name.clone())
    }

    /// Get the copyright text, generating it if not explicitly set
    #[must_use]
    pub fn copyright(&self) -> String {
        self.site_copyright.clone().unwrap_or_else(|| {
            let current_year = get_current_year();
            format!(
                "© {} {}. All rights reserved.",
                current_year, self.site_name
            )
        })
    }

    /// Photograph licensing terms, defaulting to the site copyright line so the
    /// name and the year are computed in exactly one place.
    #[must_use]
    pub fn image_license(&self) -> String {
        self.license
            .as_ref()
            .and_then(|l| l.images.clone())
            .unwrap_or_else(|| self.copyright())
    }

    /// The all-rights-reserved elaboration, rendered only while `[license].images`
    /// is unset. Returning `None` once it is set is what keeps a deployer's own
    /// license from being contradicted by this crate's boilerplate.
    #[must_use]
    pub fn image_license_note(&self) -> Option<&'static str> {
        self.license
            .as_ref()
            .and_then(|l| l.images.as_ref())
            .is_none()
            .then_some(DEFAULT_IMAGE_LICENSE_NOTE)
    }

    /// Address for licensing enquiries, if one is configured.
    ///
    /// An empty or whitespace-only value is treated as unset, so a stray
    /// `contact = ""` cannot render a sentence that trails off into nothing.
    #[must_use]
    pub fn license_contact(&self) -> Option<&str> {
        self.license
            .as_ref()
            .and_then(|l| l.contact.as_deref())
            .map(str::trim)
            .filter(|c| !c.is_empty())
    }

    /// Public URL for this site's source.
    ///
    /// An empty value falls back to the default: `href=""` would render a "Source"
    /// link that silently reloads the current page, which is worse than one pointing
    /// at upstream.
    #[must_use]
    pub fn source_url(&self) -> &str {
        self.license
            .as_ref()
            .and_then(|l| l.source.as_deref())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or(DEFAULT_SOURCE_URL)
    }

    /// Label for the footer link to the license section, or `None` when the link
    /// should not render at all.
    ///
    /// Unset yields [`DEFAULT_LICENSE_FOOTER_TEXT`]; an empty string suppresses the
    /// link. That asymmetry with [`Self::source_url`] is deliberate — hiding a
    /// navigational link is a reasonable thing to want, whereas hiding the source
    /// link would leave the GPL notice pointing nowhere.
    #[must_use]
    pub fn license_footer_text(&self) -> Option<&str> {
        match self.license.as_ref().and_then(|l| l.footer_text.as_deref()) {
            None => Some(DEFAULT_LICENSE_FOOTER_TEXT),
            Some(t) if t.trim().is_empty() => None,
            Some(t) => Some(t.trim()),
        }
    }
}

impl Default for SiteConfig {
    fn default() -> Self {
        Self {
            site_name: "Your Name".to_string(),
            site_tagline: "Photography".to_string(),
            site_title: None,
            site_copyright: None,
            license: None,
            gallery_order: Vec::new(),
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
#[must_use]
pub fn load_config() -> SiteConfig {
    SiteConfig::default()
}

#[cfg(test)]
mod tests {
    use super::*;
    // Every `#[serial]` below sits on an `#[cfg(feature = "ssr")]` test, so under
    // `hydrate` they all compile out and this import goes unused. `config` is one of
    // the handful of modules that builds under both feature sets (lib.rs gates
    // gallery/image_cache/image_params/mosaic on `ssr`), which is why it is the only
    // import that needs the gate.
    #[cfg(feature = "ssr")]
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
        let current_year = get_current_year();

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
        unsafe { std::env::set_var("CONFIG_PATH", "/tmp/nonexistent_config_file_12345.txt") };

        let config = load_config();

        assert_eq!(config.site_name, "Your Name");
        assert_eq!(config.site_tagline, "Photography");

        unsafe { std::env::remove_var("CONFIG_PATH") };
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
        unsafe { std::env::set_var("CONFIG_PATH", config_file.to_str().unwrap()) };

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
        unsafe { std::env::remove_var("CONFIG_PATH") };
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
        unsafe { std::env::set_var("CONFIG_PATH", config_file.to_str().unwrap()) };

        let config = load_config();

        assert_eq!(config.site_name, "Test Site");
        assert_eq!(config.site_tagline, "Test Tagline");
        assert_eq!(
            config.sections.get("custom_field"),
            Some(&SectionValue::Simple("value".to_string()))
        );

        // Cleanup
        unsafe { std::env::remove_var("CONFIG_PATH") };
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
        unsafe { std::env::set_var("CONFIG_PATH", config_file.to_str().unwrap()) };

        let config = load_config();

        assert_eq!(config.site_name, "Whitespace Test");
        assert_eq!(
            config.sections.get("custom_key"),
            Some(&SectionValue::Simple("value with spaces".to_string()))
        );

        // Cleanup
        unsafe { std::env::remove_var("CONFIG_PATH") };
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
        unsafe { std::env::set_var("CONFIG_PATH", config_file.to_str().unwrap()) };

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
        unsafe { std::env::remove_var("CONFIG_PATH") };
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
        unsafe { std::env::set_var("CONFIG_PATH", config_file.to_str().unwrap()) };

        let config = load_config();

        assert_eq!(config.site_name, "Test User");
        assert_eq!(config.site_tagline, "Test Tagline");

        // Copyright should be auto-generated
        let copyright = config.copyright();
        assert!(copyright.contains("Test User"));
        assert!(copyright.contains("©"));
        assert!(copyright.contains("All rights reserved"));

        let current_year = get_current_year();
        assert!(copyright.contains(&current_year.to_string()));

        // Cleanup
        unsafe { std::env::remove_var("CONFIG_PATH") };
        fs::remove_file(&config_file).ok();
    }

    #[cfg(feature = "ssr")]
    #[test]
    #[serial]
    fn test_load_config_gallery_order() {
        use std::fs;

        let temp_dir = std::env::temp_dir();
        let config_file = temp_dir.join(format!(
            "test_config_gallery_order_{}_{}.toml",
            std::process::id(),
            line!()
        ));

        let content = r#"
site_name = "Test"
site_tagline = "Test"
gallery_order = ["landscapes", "portraits", "film"]
"#;

        fs::write(&config_file, content).unwrap();
        unsafe { std::env::set_var("CONFIG_PATH", config_file.to_str().unwrap()) };

        let config = load_config();
        assert_eq!(
            config.gallery_order,
            vec![
                "landscapes".to_string(),
                "portraits".to_string(),
                "film".to_string()
            ]
        );

        unsafe { std::env::remove_var("CONFIG_PATH") };
        fs::remove_file(&config_file).ok();
    }

    #[cfg(feature = "ssr")]
    #[test]
    #[serial]
    fn test_load_config_gallery_order_defaults_empty() {
        use std::fs;

        let temp_dir = std::env::temp_dir();
        let config_file = temp_dir.join(format!(
            "test_config_gallery_order_default_{}_{}.toml",
            std::process::id(),
            line!()
        ));

        let content = r#"
site_name = "Test"
site_tagline = "Test"
"#;

        fs::write(&config_file, content).unwrap();
        unsafe { std::env::set_var("CONFIG_PATH", config_file.to_str().unwrap()) };

        let config = load_config();
        assert!(
            config.gallery_order.is_empty(),
            "expected no gallery_order, got {:?}",
            config.gallery_order
        );

        unsafe { std::env::remove_var("CONFIG_PATH") };
        fs::remove_file(&config_file).ok();
    }

    #[test]
    fn test_image_license_defaults_to_copyright() {
        let config = SiteConfig::default();

        // The photograph terms and the footer line are the same claim, so they
        // share one source of truth for the name and the year.
        assert_eq!(config.image_license(), config.copyright());
        assert_eq!(
            config.image_license_note(),
            Some(DEFAULT_IMAGE_LICENSE_NOTE)
        );
    }

    #[test]
    fn test_explicit_image_license_suppresses_default_note() {
        let config = SiteConfig {
            license: Some(LicenseConfig {
                images: Some("CC BY-SA 4.0".to_string()),
                ..LicenseConfig::default()
            }),
            ..SiteConfig::default()
        };

        // A deployer who grants a license must not be contradicted by the
        // boilerplate "not licensed for reuse" sentence.
        assert_eq!(config.image_license(), "CC BY-SA 4.0");
        assert_eq!(config.image_license_note(), None);
    }

    #[test]
    fn test_source_url_defaults_and_overrides() {
        let config = SiteConfig::default();
        assert_eq!(config.source_url(), DEFAULT_SOURCE_URL);

        let forked = SiteConfig {
            license: Some(LicenseConfig {
                source: Some("https://example.com/fork".to_string()),
                ..LicenseConfig::default()
            }),
            ..SiteConfig::default()
        };
        assert_eq!(forked.source_url(), "https://example.com/fork");
    }

    #[test]
    fn test_license_contact_absent_by_default() {
        assert_eq!(SiteConfig::default().license_contact(), None);

        let config = SiteConfig {
            license: Some(LicenseConfig {
                contact: Some("me@example.com".to_string()),
                ..LicenseConfig::default()
            }),
            ..SiteConfig::default()
        };
        assert_eq!(config.license_contact(), Some("me@example.com"));
    }

    #[test]
    fn test_empty_strings_are_treated_as_unset() {
        let config = SiteConfig {
            license: Some(LicenseConfig {
                contact: Some("   ".to_string()),
                source: Some(String::new()),
                ..LicenseConfig::default()
            }),
            ..SiteConfig::default()
        };

        // A stray empty value must not render a sentence trailing off into nothing,
        // nor an href="" that silently reloads the current page.
        assert_eq!(config.license_contact(), None);
        assert_eq!(config.source_url(), DEFAULT_SOURCE_URL);
    }

    #[test]
    fn test_license_footer_text_defaults_and_suppression() {
        // Unset: the link renders with the default label.
        assert_eq!(
            SiteConfig::default().license_footer_text(),
            Some(DEFAULT_LICENSE_FOOTER_TEXT)
        );

        let renamed = SiteConfig {
            license: Some(LicenseConfig {
                footer_text: Some("Rights".to_string()),
                ..LicenseConfig::default()
            }),
            ..SiteConfig::default()
        };
        assert_eq!(renamed.license_footer_text(), Some("Rights"));

        // Empty string means "hide the link", not "fall back to the default" — the
        // one field where an empty value is a deliberate instruction.
        let hidden = SiteConfig {
            license: Some(LicenseConfig {
                footer_text: Some(String::new()),
                ..LicenseConfig::default()
            }),
            ..SiteConfig::default()
        };
        assert_eq!(hidden.license_footer_text(), None);
    }

    #[test]
    fn test_default_source_url_points_at_the_public_repo() {
        // The footer and About page both surface this; a wrong value here ships a
        // dead link on every deployment that does not override it.
        assert_eq!(
            DEFAULT_SOURCE_URL,
            "https://github.com/Davey-Hughes/portfolio"
        );
    }

    #[cfg(feature = "ssr")]
    #[test]
    #[serial]
    fn test_load_config_with_license_table() {
        use std::fs;

        let temp_dir = std::env::temp_dir();
        let config_file = temp_dir.join(format!(
            "test_config_license_{}_{}.toml",
            std::process::id(),
            line!()
        ));

        let content = r#"
site_name = "John Doe Photography"
site_tagline = "Capturing Moments"

[license]
images = "All rights reserved."
contact = "john@example.com"
source = "https://example.com/src"
"#;

        fs::write(&config_file, content).unwrap();
        unsafe { std::env::set_var("CONFIG_PATH", config_file.to_str().unwrap()) };

        let config = load_config();

        assert_eq!(config.image_license(), "All rights reserved.");
        assert_eq!(config.image_license_note(), None);
        assert_eq!(config.license_contact(), Some("john@example.com"));
        assert_eq!(config.source_url(), "https://example.com/src");

        unsafe { std::env::remove_var("CONFIG_PATH") };
        fs::remove_file(&config_file).ok();
    }

    #[cfg(feature = "ssr")]
    #[test]
    #[serial]
    fn test_load_config_without_license_table() {
        use std::fs;

        let temp_dir = std::env::temp_dir();
        let config_file = temp_dir.join(format!(
            "test_config_no_license_{}_{}.toml",
            std::process::id(),
            line!()
        ));

        let content = r#"
site_name = "John Doe Photography"
site_tagline = "Capturing Moments"
"#;

        fs::write(&config_file, content).unwrap();
        unsafe { std::env::set_var("CONFIG_PATH", config_file.to_str().unwrap()) };

        let config = load_config();

        // Omitting the table entirely is the common case and must still produce a
        // complete statement.
        assert!(config.license.is_none());
        assert!(config.image_license().contains("John Doe Photography"));
        assert!(config.image_license().contains("All rights reserved"));
        assert_eq!(
            config.image_license_note(),
            Some(DEFAULT_IMAGE_LICENSE_NOTE)
        );
        assert_eq!(config.source_url(), DEFAULT_SOURCE_URL);

        unsafe { std::env::remove_var("CONFIG_PATH") };
        fs::remove_file(&config_file).ok();
    }
}
