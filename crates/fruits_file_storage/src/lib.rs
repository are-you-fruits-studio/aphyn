//! # fruits_file_storage
//!
//! Absolute, OS-aware paths of the directories an app works with. The functions
//! only compute paths: they never create, read, or write the directories.
//!
//! # How to use
//!
//! | Function | Directory | Access |
//! |---|---|---|
//! | [`app_path`] | folder where the app executable lies | readonly |
//! | [`assets_path`] | folder where assets lie (`<app_path>/assets`) | readonly |
//! | [`save_path`] | folder where progress lies, per app | writable |
//! | [`cache_path`] | folder where cache lies, per app | writable |
//!
//! Access is a convention for callers, not enforced by the code. [`save_path`] and
//! [`cache_path`] take the app name, which becomes the last path segment; create the
//! directory yourself before writing into it.
//!
//! ```
//! use fruits_file_storage::*;
//!
//! assert!(app_path().is_absolute());
//! assert!(assets_path().ends_with("assets"));
//!
//! let save = save_path("my_game");
//! assert!(save.is_absolute());
//! assert!(save.ends_with("my_game"));
//! assert!(cache_path("my_game").is_absolute());
//! ```
//!
//! # How to maintain
//!
//! - [`app_path`] is the parent of [`std::env::current_exe`]. With the dynamic launch
//!   this is the launcher exe's folder, which `fruits_editor build` fills with `assets/`.
//! - Writable roots come from env vars:
//!   - Windows: `%APPDATA%` (save), `%LOCALAPPDATA%` (cache).
//!   - Linux: `$XDG_DATA_HOME` (save), `$XDG_CACHE_HOME` (cache); per the XDG spec, empty or
//!     relative values are ignored in favor of `$HOME/.local/share` and `$HOME/.cache`.
//! - A missing executable path or required env var panics.
//! - Other OSes compile but hit `todo!()` when a writable path is requested.

use std::path::PathBuf;

// todo: refactor

pub fn app_path() -> PathBuf {
    let exe = std::env::current_exe().expect("failed to get the app executable path");
    exe.parent().expect("app executable has no parent directory").to_path_buf()
}

pub fn assets_path() -> PathBuf {
    app_path().join("assets")
}

pub fn save_path(app_name: &str) -> PathBuf {
    os_save_root().join(app_name)
}

pub fn cache_path(app_name: &str) -> PathBuf {
    os_cache_root().join(app_name)
}

#[cfg(windows)]
fn os_save_root() -> PathBuf {
    env_path("APPDATA").expect("%APPDATA% is not set")
}

#[cfg(windows)]
fn os_cache_root() -> PathBuf {
    env_path("LOCALAPPDATA").expect("%LOCALAPPDATA% is not set")
}

#[cfg(target_os = "linux")]
fn os_save_root() -> PathBuf {
    env_path("XDG_DATA_HOME").unwrap_or_else(|| home_path().join(".local").join("share"))
}

#[cfg(target_os = "linux")]
fn os_cache_root() -> PathBuf {
    env_path("XDG_CACHE_HOME").unwrap_or_else(|| home_path().join(".cache"))
}

#[cfg(target_os = "linux")]
fn home_path() -> PathBuf {
    env_path("HOME").expect("$HOME is not set")
}

// todo: support other OS (macOS, ...)
#[cfg(not(any(windows, target_os = "linux")))]
fn os_save_root() -> PathBuf {
    todo!("save path is not supported on this OS")
}

// todo: support other OS (macOS, ...)
#[cfg(not(any(windows, target_os = "linux")))]
fn os_cache_root() -> PathBuf {
    todo!("cache path is not supported on this OS")
}

#[cfg(any(windows, target_os = "linux"))]
fn env_path(var: &str) -> Option<PathBuf> {
    std::env::var_os(var).map(PathBuf::from).filter(|p| p.is_absolute())
}
