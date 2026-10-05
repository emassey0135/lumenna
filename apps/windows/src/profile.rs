//! Where the store lives.

use std::path::PathBuf;

/// The profile directory: `LUMENNA_PROFILE` if set, else where `lum` keeps it.
///
/// The same place as the command line's, as on the Mac, so the app and `lum` on one PC are
/// one device with one store rather than two that would have to pair with each other. This
/// must agree with `default_directory` in `apps/cli/src/profile.rs`: the *local* data
/// directory, because Roaming AppData is copied to a server at every logon on a domain with
/// roaming profiles.
pub fn directory() -> Option<PathBuf> {
    if let Some(explicit) = std::env::var_os("LUMENNA_PROFILE").filter(|v| !v.is_empty()) {
        return Some(PathBuf::from(explicit));
    }
    directories::ProjectDirs::from("", "", "lumenna").map(|dirs| dirs.data_local_dir().to_path_buf())
}
