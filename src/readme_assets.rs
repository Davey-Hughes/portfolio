//! Which files the `/readme` mount serves.
//!
//! The directory holds screenshots and short clips for the READMEs of the owner's
//! repos, so only those media types go out; anything else that ends up there (a
//! stray `notes.toml`, an `index.html`) is answered with 404. SVG is left out on
//! purpose: opened directly on the site's origin, an SVG can run script.

/// File extensions the mount serves, compared without regard to ASCII case.
pub const ALLOWED_EXTENSIONS: &[&str] =
    &["webp", "png", "jpg", "jpeg", "gif", "avif", "mp4", "webm"];

/// Whether a request path under the `/readme` mount names a file of an allowed type.
///
/// `path` is the path as received, still percent-encoded. It is judged after
/// decoding, so `notes%2Etoml` is refused like `notes.toml`, and a path that does
/// not decode to UTF-8 is refused outright.
pub fn is_servable(path: &str) -> bool {
    let Ok(decoded) = urlencoding::decode(path) else {
        return false;
    };
    let name = decoded.rsplit('/').next().unwrap_or_default();
    // A leading dot marks a dotfile, not an extension: `.webp` has none.
    let Some((stem, extension)) = name.rsplit_once('.') else {
        return false;
    };
    !stem.is_empty()
        && ALLOWED_EXTENSIONS
            .iter()
            .any(|allowed| allowed.eq_ignore_ascii_case(extension))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_servable(paths: &[&str]) {
        for path in paths {
            assert!(is_servable(path), "expected {path:?} to be served");
        }
    }

    fn assert_refused(paths: &[&str]) {
        for path in paths {
            assert!(!is_servable(path), "expected {path:?} to be refused");
        }
    }

    #[test]
    fn allows_listed_extensions() {
        assert_servable(&[
            "/fixture/shot.webp",
            "/a/b.png",
            "/a/b.jpg",
            "/a/b.jpeg",
            "/a/b.gif",
            "/a/b.avif",
            "/a/b.mp4",
            "/a/b.webm",
        ]);
    }

    #[test]
    fn extension_match_ignores_case() {
        assert_servable(&["/a/SHOT.WEBP", "/a/b.Png"]);
    }

    #[test]
    fn refuses_other_types() {
        // `/a/.webp` is a dotfile: it has no extension at all.
        assert_refused(&[
            "/a/notes.toml",
            "/a/b.svg",
            "/a/b.html",
            "/a/b.webp.toml",
            "/a/README",
            "/a/.webp",
        ]);
    }

    #[test]
    fn judges_last_extension() {
        assert_servable(&["/a/b.toml.webp"]);
    }

    #[test]
    fn refuses_directories() {
        assert_refused(&["/fixture/", "/fixture", "/", ""]);
    }

    #[test]
    fn judges_the_decoded_path() {
        assert_refused(&["/a/notes%2Etoml"]);
        assert_servable(&["/a/shot%2Ewebp"]);
        // Decodes to a trailing slash, so there is no file name to judge.
        assert_refused(&["/a/b.webp%2F"]);
        // %FF on its own is not valid UTF-8.
        assert_refused(&["/a/%FF.webp"]);
    }
}
