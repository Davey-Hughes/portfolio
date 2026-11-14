//! Helpers shared by the unit tests.

/// Sets an environment variable for the life of one test, removing it again on drop,
/// so a failing assertion cannot leak the value into the tests that run after it.
///
/// Only for `#[serial]` tests: that is what keeps them from racing each other on the
/// process environment.
#[must_use = "the variable is removed as soon as the guard is dropped"]
pub struct EnvVarGuard(&'static str);

impl EnvVarGuard {
    pub fn set(key: &'static str, value: impl AsRef<std::ffi::OsStr>) -> Self {
        unsafe { std::env::set_var(key, value) };
        Self(key)
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        unsafe { std::env::remove_var(self.0) };
    }
}
