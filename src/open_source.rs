//! The `/open-source` page's content: a hand-curated `opensource.toml` in the
//! content directory, listing the deployer's own projects and their upstream
//! contributions.
//!
//! The file is optional. When it is missing, malformed, or lists no entries, the
//! page is the site's not-found page (HTTP 404) and the About page omits its link,
//! so a deployment that never adds the file never shows an empty page.

use serde::{Deserialize, Serialize};

/// File name inside the content directory (see `gallery::content_dir`).
pub const OPEN_SOURCE_FILE: &str = "opensource.toml";

/// The whole page, parsed from [`OPEN_SOURCE_FILE`].
///
/// Unknown keys are rejected rather than ignored: in a hand-edited file a typo like
/// `highlight` for `highlights` would otherwise drop data without a trace.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OpenSourceContent {
    /// Optional paragraph under the page heading.
    #[serde(default)]
    pub intro: Option<String>,
    /// The deployer's own projects, in file order.
    #[serde(default)]
    pub projects: Vec<OpenSourceEntry>,
    /// Upstream projects the deployer has contributed to, in file order.
    #[serde(default)]
    pub contributions: Vec<OpenSourceEntry>,
}

/// One card: a project or an upstream the deployer contributed to.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OpenSourceEntry {
    pub name: String,
    /// Where the name links to. Rendered as plain text when unset or blank.
    #[serde(default)]
    pub url: Option<String>,
    /// The project's own site, shown beside the name as its bare domain, so a
    /// card can link both its source (`url`) and where it runs.
    #[serde(default)]
    pub website: Option<String>,
    pub summary: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub highlights: Vec<Highlight>,
    /// A screenshot shown on the card and opened full screen, as any URL or site path
    /// (such as `/readme/<repo>/card.webp`). No image when unset or blank.
    #[serde(default)]
    pub image: Option<String>,
    /// The screenshot's alt text. Defaults to "<name> screenshot" when unset or blank.
    #[serde(default)]
    pub image_alt: Option<String>,
    /// Draw the card's screenshot inverted (`filter: invert(1) hue-rotate(180deg)`).
    /// For white line art on a transparent background, which then reads black on the
    /// white page; the full-screen viewer shows it as it is, white on its dark
    /// overlay. A normal screenshot would come out as its own negative.
    #[serde(default)]
    pub image_invert: bool,
    /// What the card shows on desktop instead of `image`, as any URL or site path;
    /// full screen, and the full-width card on phones, still use `image`. Lets the
    /// card's file be sized for the desktop slot (so the browser doesn't have to
    /// shrink a large image much, which can alias fine line art) while full screen
    /// gets every pixel. Falls back to `image` when unset or blank. Set alone, it is
    /// the image: shown on the card and opened full screen.
    #[serde(default)]
    pub image_card: Option<String>,
}

/// A link to one notable piece of work, such as a merged PR, under a card.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Highlight {
    pub title: String,
    pub url: String,
}

/// Trimmed `s`, or `None` when nothing is left. A blank `url` must not become
/// `href=""`, which renders a link that silently reloads the current page.
fn non_blank(s: &str) -> Option<&str> {
    let trimmed = s.trim();
    (!trimmed.is_empty()).then_some(trimmed)
}

impl OpenSourceContent {
    /// True when there is nothing to put on the page. An intro alone does not count.
    pub fn is_empty(&self) -> bool {
        self.projects.is_empty() && self.contributions.is_empty()
    }

    pub fn intro_text(&self) -> Option<&str> {
        self.intro.as_deref().and_then(non_blank)
    }

    /// The content with every screenshot address under `/readme/` given its file's
    /// version (`?v=<modification time>`), the file looked up in `readme_dir`.
    ///
    /// The `/readme` mount has browsers check a bare address on every load, so that a
    /// file replaced in place reaches the READMEs that embed it. On this page that
    /// meant a reload drew the text a round trip before the screenshots. A versioned
    /// address is kept for a year instead (`readme_routes` in main.rs), and changes
    /// with the file.
    ///
    /// An address outside `/readme/`, one that leads out of the folder, one the mount
    /// wouldn't serve, and one that names no file in it, such as one with a query of
    /// its own, stay as written.
    #[cfg(feature = "ssr")]
    #[must_use]
    pub fn with_readme_versions(mut self, readme_dir: &std::path::Path) -> Self {
        for entry in self.projects.iter_mut().chain(&mut self.contributions) {
            for address in [&mut entry.image, &mut entry.image_card]
                .into_iter()
                .flatten()
            {
                if let Some(versioned) = readme_versioned(address, readme_dir) {
                    *address = versioned;
                }
            }
        }
        self
    }
}

/// `address` with its file's version, when it is the `/readme/` address of a file in
/// `readme_dir`. See [`OpenSourceContent::with_readme_versions`].
#[cfg(feature = "ssr")]
fn readme_versioned(address: &str, readme_dir: &std::path::Path) -> Option<String> {
    let address = address.trim();
    let rel = address.strip_prefix("/readme/")?;
    if !crate::readme_assets::is_servable(rel) {
        return None;
    }
    // Decoded and joined the way the file server does it, which refuses `..`.
    let rel = urlencoding::decode(rel).ok()?;
    let mut path = readme_dir.to_path_buf();
    for segment in rel.split('/') {
        match segment {
            "" | "." => {}
            ".." => return None,
            segment => path.push(segment),
        }
    }
    let metadata = std::fs::metadata(path)
        .ok()
        .filter(std::fs::Metadata::is_file)?;
    let version = crate::gallery::file_version(&metadata)?;
    Some(crate::gallery::with_version(
        address.to_string(),
        Some(version),
    ))
}

impl OpenSourceEntry {
    pub fn link(&self) -> Option<&str> {
        self.url.as_deref().and_then(non_blank)
    }

    /// `(href, label)` for the website link, or `None` when unset or blank. The
    /// label is the URL without its scheme, a leading `www.` or a trailing slash:
    /// `https://www.redextape.com/` reads as `redextape.com`.
    pub fn website(&self) -> Option<(&str, &str)> {
        let href = self.website.as_deref().and_then(non_blank)?;
        let label = href
            .strip_prefix("https://")
            .or_else(|| href.strip_prefix("http://"))
            .unwrap_or(href);
        let label = label.strip_prefix("www.").unwrap_or(label);
        let label = label.strip_suffix('/').unwrap_or(label);
        Some((href, label))
    }

    /// `(src, alt)` for the screenshot, or `None` when neither `image` nor
    /// `image_card` is set. `src` is what full screen opens: `image`, or
    /// `image_card` when that is all there is, so a card file set alone isn't
    /// dropped without a trace. The card shows [`Self::card_src`]. The alt is
    /// `image_alt` when that is set and not blank, else "<name> screenshot".
    pub fn image(&self) -> Option<(&str, String)> {
        let src = self
            .image
            .as_deref()
            .and_then(non_blank)
            .or_else(|| self.image_card.as_deref().and_then(non_blank))?;
        let alt = self.image_alt.as_deref().and_then(non_blank).map_or_else(
            || format!("{} screenshot", self.name.trim()),
            str::to_string,
        );
        Some((src, alt))
    }

    /// What the card shows on desktop, given `src` from [`Self::image`]:
    /// `image_card` when that is set and not blank, else `src` itself.
    pub fn card_src<'a>(&'a self, src: &'a str) -> &'a str {
        self.image_card
            .as_deref()
            .and_then(non_blank)
            .unwrap_or(src)
    }
}

impl Highlight {
    pub fn link(&self) -> Option<&str> {
        non_blank(&self.url)
    }
}

/// Parse the contents of an `opensource.toml`.
///
/// # Errors
///
/// Returns the TOML error when the text is not valid TOML, a required field
/// (`name`, `summary`, a highlight's `title` or `url`) is missing, or a key is
/// not part of the format.
#[cfg(feature = "ssr")]
pub fn parse_open_source(text: &str) -> Result<OpenSourceContent, toml::de::Error> {
    toml::from_str(text)
}

/// Load [`OPEN_SOURCE_FILE`] from `dir`.
///
/// `None` when the file is missing, unreadable, malformed, or lists no entries.
/// Read and parse failures are logged, so a broken file is diagnosable from the
/// server log instead of silently hiding the page.
#[cfg(feature = "ssr")]
pub fn load_open_source_from(dir: &std::path::Path) -> Option<OpenSourceContent> {
    let path = dir.join(OPEN_SOURCE_FILE);
    if !path.exists() {
        return None;
    }
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(err) => {
            leptos::logging::log!(
                "open source page hidden: cannot read {}: {err}",
                path.display()
            );
            return None;
        }
    };
    match parse_open_source(&text) {
        Ok(content) if content.is_empty() => None,
        Ok(content) => Some(content),
        Err(err) => {
            leptos::logging::log!(
                "open source page hidden: cannot parse {}: {err}",
                path.display()
            );
            None
        }
    }
}

/// Load [`OPEN_SOURCE_FILE`] from the content directory.
#[cfg(feature = "ssr")]
pub fn load_open_source() -> Option<OpenSourceContent> {
    load_open_source_from(std::path::Path::new(&crate::gallery::content_dir()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(url: Option<&str>) -> OpenSourceEntry {
        OpenSourceEntry {
            name: "Name".to_string(),
            url: url.map(str::to_string),
            website: None,
            summary: "Summary".to_string(),
            tags: Vec::new(),
            highlights: Vec::new(),
            image: None,
            image_alt: None,
            image_invert: false,
            image_card: None,
        }
    }

    fn with_image(image: Option<&str>, image_alt: Option<&str>) -> OpenSourceEntry {
        OpenSourceEntry {
            image: image.map(str::to_string),
            image_alt: image_alt.map(str::to_string),
            ..entry(None)
        }
    }

    #[test]
    fn image_and_card_src_fall_back_to_each_other() {
        let entry = |image: Option<&str>, card: Option<&str>| OpenSourceEntry {
            image_card: card.map(str::to_string),
            ..with_image(image, None)
        };
        // (what full screen opens, what the card shows)
        let srcs = |e: &OpenSourceEntry| {
            e.image()
                .map(|(src, _)| (src.to_string(), e.card_src(src).to_string()))
        };
        let pair = |full: &str, card: &str| Some((full.to_string(), card.to_string()));

        assert_eq!(
            srcs(&entry(Some("/i.webp"), None)),
            pair("/i.webp", "/i.webp")
        );
        assert_eq!(
            srcs(&entry(Some("/i.webp"), Some("  "))),
            pair("/i.webp", "/i.webp")
        );
        assert_eq!(
            srcs(&entry(Some("/i.webp"), Some(" /card.webp "))),
            pair("/i.webp", "/card.webp")
        );
        // A card file set alone is the image, not dropped without a trace.
        assert_eq!(
            srcs(&entry(None, Some("/card.webp"))),
            pair("/card.webp", "/card.webp")
        );
        assert_eq!(
            srcs(&entry(Some(" "), Some("/card.webp"))),
            pair("/card.webp", "/card.webp")
        );
        assert_eq!(srcs(&entry(None, Some(" "))), None);
    }

    #[test]
    fn image_is_none_when_unset_or_blank() {
        assert_eq!(with_image(None, None).image(), None);
        assert_eq!(with_image(Some(""), None).image(), None);
        assert_eq!(with_image(Some("   "), None).image(), None);
        // An alt without an image has nothing to describe.
        assert_eq!(with_image(None, Some("Alt")).image(), None);
    }

    #[test]
    fn image_trims_and_defaults_its_alt() {
        assert_eq!(
            with_image(Some(" /readme/x/card.webp "), None).image(),
            Some(("/readme/x/card.webp", "Name screenshot".to_string()))
        );
        assert_eq!(
            with_image(Some(" /readme/x/card.webp "), Some("  ")).image(),
            Some(("/readme/x/card.webp", "Name screenshot".to_string()))
        );
        assert_eq!(
            with_image(Some(" /readme/x/card.webp "), Some(" The home page ")).image(),
            Some(("/readme/x/card.webp", "The home page".to_string()))
        );
    }

    #[test]
    fn website_shows_as_its_bare_domain() {
        let site = |w: Option<&str>| OpenSourceEntry {
            website: w.map(str::to_string),
            ..entry(None)
        };

        assert_eq!(
            site(Some("https://redextape.com")).website(),
            Some(("https://redextape.com", "redextape.com"))
        );
        assert_eq!(
            site(Some("https://www.redextape.com/")).website(),
            Some(("https://www.redextape.com/", "redextape.com"))
        );
        assert_eq!(
            site(Some("http://example.com/app/")).website(),
            Some(("http://example.com/app/", "example.com/app"))
        );
        assert_eq!(
            site(Some(" https://a.example ")).website(),
            Some(("https://a.example", "a.example"))
        );
        // Blank must not render an empty link, same rule as `url`.
        assert_eq!(site(Some("   ")).website(), None);
        assert_eq!(site(None).website(), None);
    }

    #[test]
    fn blank_urls_are_not_links() {
        // A blank url must render the name as text, never as href="" — which
        // would be a link that silently reloads the current page.
        assert_eq!(entry(None).link(), None);
        assert_eq!(entry(Some("")).link(), None);
        assert_eq!(entry(Some("   ")).link(), None);
        assert_eq!(
            entry(Some(" https://a.example ")).link(),
            Some("https://a.example")
        );

        let blank = Highlight {
            title: "#1".to_string(),
            url: String::new(),
        };
        assert_eq!(blank.link(), None);
        let padded = Highlight {
            title: "#1".to_string(),
            url: " https://a.example/pull/1 ".to_string(),
        };
        assert_eq!(padded.link(), Some("https://a.example/pull/1"));
    }

    #[test]
    fn intro_text_ignores_blank() {
        let blank = OpenSourceContent {
            intro: Some("   ".to_string()),
            ..OpenSourceContent::default()
        };
        assert_eq!(blank.intro_text(), None);

        let set = OpenSourceContent {
            intro: Some(" Hi ".to_string()),
            ..OpenSourceContent::default()
        };
        assert_eq!(set.intro_text(), Some("Hi"));
    }

    #[test]
    fn is_empty_checks_both_sections() {
        assert!(OpenSourceContent::default().is_empty());

        let projects_only = OpenSourceContent {
            projects: vec![entry(None)],
            ..OpenSourceContent::default()
        };
        assert!(!projects_only.is_empty());

        let contributions_only = OpenSourceContent {
            contributions: vec![entry(None)],
            ..OpenSourceContent::default()
        };
        assert!(!contributions_only.is_empty());
    }

    #[cfg(feature = "ssr")]
    #[test]
    fn parses_full_file() {
        let content = parse_open_source(
            r##"
intro = "Things I build and fix."

[[projects]]
name = "portfolio"
url = "https://example.com/portfolio"
website = "https://portfolio.example"
summary = "This site."
tags = ["Rust", "Leptos"]

[[contributions]]
name = "Upstream"
url = "https://example.com/upstream"
summary = "Fixes."

[[contributions.highlights]]
title = "#7 A fix"
url = "https://example.com/upstream/pull/7"
"##,
        )
        .unwrap();

        assert_eq!(content.intro.as_deref(), Some("Things I build and fix."));

        assert_eq!(content.projects.len(), 1);
        let project = &content.projects[0];
        assert_eq!(project.name, "portfolio");
        assert_eq!(
            project.url.as_deref(),
            Some("https://example.com/portfolio")
        );
        assert_eq!(
            project.website.as_deref(),
            Some("https://portfolio.example")
        );
        assert_eq!(project.summary, "This site.");
        assert_eq!(project.tags, vec!["Rust".to_string(), "Leptos".to_string()]);
        assert_eq!(project.highlights, Vec::<Highlight>::new());

        assert_eq!(content.contributions.len(), 1);
        let contribution = &content.contributions[0];
        assert_eq!(contribution.name, "Upstream");
        assert_eq!(
            contribution.highlights,
            vec![Highlight {
                title: "#7 A fix".to_string(),
                url: "https://example.com/upstream/pull/7".to_string(),
            }]
        );
    }

    #[cfg(feature = "ssr")]
    #[test]
    fn optional_fields_default() {
        let content = parse_open_source(
            r#"
[[projects]]
name = "bare"
summary = "Only the required fields."
"#,
        )
        .unwrap();

        assert_eq!(content.intro, None);
        assert_eq!(content.contributions, Vec::<OpenSourceEntry>::new());
        let project = &content.projects[0];
        assert_eq!(project.url, None);
        assert_eq!(project.website, None);
        assert_eq!(project.tags, Vec::<String>::new());
        assert_eq!(project.highlights, Vec::<Highlight>::new());
        assert_eq!(project.image, None);
        assert_eq!(project.image_alt, None);
        assert!(!project.image_invert);
        assert_eq!(project.image_card, None);
    }

    #[cfg(feature = "ssr")]
    #[test]
    fn parses_image_fields() {
        let content = parse_open_source(
            r#"
[[projects]]
name = "redextape"
summary = "A site."
image = "/readme/redextape/card.webp"
image_alt = "The redextape home page"
image_invert = true
image_card = "/readme/redextape/card-small.webp"

[[contributions]]
name = "Upstream"
summary = "Fixes."
image = "/readme/upstream/card.webp"
"#,
        )
        .unwrap();

        let project = &content.projects[0];
        assert_eq!(
            project.image.as_deref(),
            Some("/readme/redextape/card.webp")
        );
        assert_eq!(
            project.image_alt.as_deref(),
            Some("The redextape home page")
        );
        assert_eq!(
            project.image(),
            Some((
                "/readme/redextape/card.webp",
                "The redextape home page".to_string()
            ))
        );

        assert!(project.image_invert);
        // The chain the card uses: image() for full screen, then card_src for the card.
        let (full, _) = project.image().unwrap();
        assert_eq!(project.card_src(full), "/readme/redextape/card-small.webp");

        // Contributions share the shape, alt default included.
        assert_eq!(
            content.contributions[0].image(),
            Some((
                "/readme/upstream/card.webp",
                "Upstream screenshot".to_string()
            ))
        );
        assert!(!content.contributions[0].image_invert);
        let upstream = &content.contributions[0];
        let (full, _) = upstream.image().unwrap();
        assert_eq!(upstream.card_src(full), "/readme/upstream/card.webp");
    }

    #[cfg(feature = "ssr")]
    #[test]
    fn image_invert_must_be_a_bool() {
        let quoted = "[[projects]]\nname = \"x\"\nsummary = \"y\"\nimage_invert = \"true\"\n";
        assert!(parse_open_source(quoted).is_err());
    }

    #[cfg(feature = "ssr")]
    #[test]
    fn unknown_image_key_is_rejected() {
        let typo = "[[projects]]\nname = \"x\"\nsummary = \"y\"\nimage_url = \"/a.webp\"\n";
        assert!(parse_open_source(typo).is_err());
    }

    #[cfg(feature = "ssr")]
    #[test]
    fn missing_required_field_is_rejected() {
        assert!(parse_open_source("[[projects]]\nname = \"x\"\n").is_err());
    }

    #[cfg(feature = "ssr")]
    #[test]
    fn unknown_field_is_rejected() {
        // `highlight` for `highlights`: a typo must surface as an error rather than
        // silently dropping the data the deployer meant to publish.
        let typo = "[[projects]]\nname = \"x\"\nsummary = \"y\"\nhighlight = []\n";
        assert!(parse_open_source(typo).is_err());
    }

    /// 2024-11-15 06:33:20 UTC, a modification time to give a file.
    #[cfg(feature = "ssr")]
    const MTIME: u64 = 1_731_652_400;

    /// A file at `dir/rel`, its folders made on the way, last modified at [`MTIME`].
    #[cfg(feature = "ssr")]
    fn file_at(dir: &std::path::Path, rel: &str) {
        let path = dir.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, b"webp").unwrap();
        std::fs::File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_secs(MTIME))
            .unwrap();
    }

    #[cfg(feature = "ssr")]
    fn with_images(image: Option<&str>, image_card: Option<&str>) -> OpenSourceEntry {
        OpenSourceEntry {
            image_card: image_card.map(str::to_string),
            ..with_image(image, None)
        }
    }

    #[cfg(feature = "ssr")]
    #[test]
    fn readme_images_carry_their_files_version() {
        let readme = tempfile::tempdir().unwrap();
        file_at(readme.path(), "x/full.webp");
        file_at(readme.path(), "x/card.webp");
        file_at(readme.path(), "my project/shot.webp");
        let content = OpenSourceContent {
            intro: None,
            projects: vec![with_images(
                Some("/readme/x/full.webp"),
                Some(" /readme/x/card.webp "),
            )],
            contributions: vec![with_images(Some("/readme/my%20project/shot.webp"), None)],
        }
        .with_readme_versions(readme.path());

        let images = |e: &OpenSourceEntry| (e.image.clone(), e.image_card.clone());
        assert_eq!(
            images(&content.projects[0]),
            (
                Some("/readme/x/full.webp?v=1731652400".to_string()),
                Some("/readme/x/card.webp?v=1731652400".to_string())
            )
        );
        // The file is found by the decoded address, as the file server finds it.
        assert_eq!(
            images(&content.contributions[0]),
            (
                Some("/readme/my%20project/shot.webp?v=1731652400".to_string()),
                None
            )
        );
    }

    /// Only a file the `/readme` mount would serve gets a version. Any other address
    /// stays as written.
    #[cfg(feature = "ssr")]
    #[test]
    fn other_images_keep_their_address() {
        let root = tempfile::tempdir().unwrap();
        let readme = root.path().join("readme");
        file_at(&readme, "x/shot.webp");
        // There, but not a type the mount serves.
        file_at(&readme, "x/notes.toml");
        // Two folders up from x/: outside the mount, which refuses `..`.
        file_at(root.path(), "outside.webp");
        let kept = [
            "https://example.com/readme/x/shot.webp",
            "/content/x/shot.webp",
            "readme/x/shot.webp",
            "/readme/x/gone.webp",
            "/readme/x/notes.toml",
            "/readme/x",
            "/readme/x/shot.webp?w=1",
            "/readme/x/shot.webp#top",
            "/readme/x/../../outside.webp",
            "/readme/x/..%2F..%2Foutside.webp",
            "  ",
        ];
        let content = OpenSourceContent {
            intro: None,
            projects: kept
                .iter()
                .map(|&address| with_images(Some(address), Some(address)))
                .collect(),
            contributions: Vec::new(),
        }
        .with_readme_versions(&readme);

        for (entry, address) in content.projects.iter().zip(kept) {
            assert_eq!(entry.image.as_deref(), Some(address));
            assert_eq!(entry.image_card.as_deref(), Some(address));
        }
    }

    #[cfg(feature = "ssr")]
    fn dir_with(contents: Option<&str>) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        if let Some(text) = contents {
            std::fs::write(dir.path().join(OPEN_SOURCE_FILE), text).unwrap();
        }
        dir
    }

    #[cfg(feature = "ssr")]
    #[test]
    fn missing_file_loads_as_none() {
        let dir = dir_with(None);
        assert_eq!(load_open_source_from(dir.path()), None);
    }

    #[cfg(feature = "ssr")]
    #[test]
    fn malformed_file_loads_as_none() {
        let dir = dir_with(Some("not = [toml"));
        assert_eq!(load_open_source_from(dir.path()), None);
    }

    #[cfg(feature = "ssr")]
    #[test]
    fn file_without_entries_loads_as_none() {
        // An intro alone would render a page with a heading and nothing under it.
        let dir = dir_with(Some("intro = \"hi\"\n"));
        assert_eq!(load_open_source_from(dir.path()), None);
    }

    #[cfg(feature = "ssr")]
    #[test]
    fn valid_file_loads_as_some() {
        let dir = dir_with(Some("[[projects]]\nname = \"x\"\nsummary = \"y\"\n"));
        let content = load_open_source_from(dir.path()).unwrap();
        assert_eq!(content.projects.len(), 1);
        assert_eq!(content.projects[0].name, "x");
    }

    #[cfg(feature = "ssr")]
    #[test]
    fn example_file_parses_and_has_entries() {
        // The example is the format's documentation; a schema change that breaks it
        // must fail here rather than in a deployer's hands.
        let example = include_str!("../public/content/opensource.toml.example");
        let content = parse_open_source(example).unwrap();
        assert!(!content.is_empty());
    }
}
