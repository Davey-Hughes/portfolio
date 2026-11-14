//! Where the server's files live on disk.
//!
//! The `/images` mount and the gallery listing both read [`images_root`], so they agree
//! on where the photos are as long as the answer does not change while the server runs.
//! `main` resolves it once at startup, while the gallery code resolves it again on every
//! call: with `IMAGES_DIR` unset, a `public/images` created or removed under a running
//! server would split the two.
//!
//! Every function reads the environment on each call rather than caching it: tests set
//! these variables, and the cost is nothing next to the directory walk that follows.

use std::path::{Path, PathBuf};

/// Resolve a directory path from an env var, falling back to the first
/// existing path in `fallbacks` (last entry is used if none exist).
pub fn resolve_dir(env_var: &str, fallbacks: &[&str]) -> String {
    if let Ok(val) = std::env::var(env_var) {
        return val;
    }
    for candidate in fallbacks {
        if Path::new(candidate).exists() {
            return (*candidate).to_string();
        }
    }
    fallbacks.last().copied().unwrap_or(".").to_string()
}

/// Root of the photo tree, one subdirectory per gallery.
///
/// `IMAGES_DIR` when set, else `public/images` when it exists, else `./images`.
pub fn images_root() -> PathBuf {
    PathBuf::from(resolve_dir("IMAGES_DIR", &["public/images", "./images"]))
}

/// The homepage's gallery: `GALLERY_PATH` when set, else `home` under [`images_root`].
pub fn home_gallery_dir() -> PathBuf {
    std::env::var("GALLERY_PATH").map_or_else(|_| images_root().join("home"), PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::EnvVarGuard;
    use serial_test::serial;

    #[test]
    #[serial]
    fn home_gallery_dir_follows_images_dir_when_gallery_path_is_unset() {
        let root = tempfile::TempDir::new().unwrap();
        unsafe { std::env::remove_var("GALLERY_PATH") };
        let _images_dir = EnvVarGuard::set("IMAGES_DIR", root.path());

        assert_eq!(home_gallery_dir(), root.path().join("home"));
    }

    #[test]
    #[serial]
    fn home_gallery_dir_follows_gallery_path_when_set() {
        let root = tempfile::TempDir::new().unwrap();
        let landing = root.path().join("landing");
        let _images_dir = EnvVarGuard::set("IMAGES_DIR", root.path());
        let _gallery_path = EnvVarGuard::set("GALLERY_PATH", &landing);

        assert_eq!(home_gallery_dir(), landing);
    }
}
