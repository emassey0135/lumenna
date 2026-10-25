//! Where the store lives, for `lum` and the resident apps alike, and how a resident app was
//! started.

use std::path::PathBuf;

/// What the app was started with.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Arguments {
    /// `--profile <directory>`: a store other than the usual one, as `lum --profile` takes.
    pub profile: Option<PathBuf>,
    /// `--background`: start in the notification area without showing the window — how it is
    /// started at sign-in.
    pub background: bool,
    /// `--no-shortcuts`: leave the shortcuts from anywhere to another copy — one open on
    /// another profile, or the UI tests', which must not take the person's keys.
    pub no_shortcuts: bool,
    /// `--launch <id>`: a launch shortcut chosen from outside (`lumenna_records::launch`): a
    /// desktop entry's action, a Jump List's task. The app does it as it opens, or a copy already
    /// running does.
    pub launch: Option<String>,
}

/// Reads the arguments after the program's name. Anything not understood is ignored, so an
/// older shortcut with an option since removed still starts the app.
pub fn arguments(args: impl IntoIterator<Item = String>) -> Arguments {
    let mut parsed = Arguments::default();
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--background" => parsed.background = true,
            "--no-shortcuts" => parsed.no_shortcuts = true,
            "--profile" => parsed.profile = args.next().map(PathBuf::from),
            "--launch" => parsed.launch = args.next(),
            other => {
                if let Some(path) = other.strip_prefix("--profile=") {
                    parsed.profile = Some(PathBuf::from(path));
                } else if let Some(id) = other.strip_prefix("--launch=") {
                    parsed.launch = Some(id.to_owned());
                }
            }
        }
    }
    parsed
}

/// The profile directory: the one named on the command line, else `LUMENNA_PROFILE`, else
/// where `lum` keeps it — the same order of precedence as `lum`'s.
pub fn directory(explicit: Option<PathBuf>) -> Option<PathBuf> {
    explicit
        .or_else(|| std::env::var_os("LUMENNA_PROFILE").filter(|v| !v.is_empty()).map(PathBuf::from))
        .or_else(default_directory)
}

/// Where the store lives when nothing names another: one place for `lum` and the apps, so the
/// app and `lum` on one computer are one device with one store rather than two that would have
/// to pair with each other. The *local* data directory: on Windows the other, Roaming AppData,
/// is copied to a server at every logon on a domain with roaming profiles, carrying the store
/// and its backups off the machine.
pub fn default_directory() -> Option<PathBuf> {
    if let Some(family) = package_family() {
        return packaged_directory(&family);
    }
    directories::ProjectDirs::from("", "", "lumenna").map(|dirs| dirs.data_local_dir().to_path_buf())
}

/// A packaged install's store (the Microsoft Store's MSIX): the package's own folder, named
/// outright. Its writes to AppData land there anyway, while it still believes it writes the
/// usual path; named, the app's one-instance lock and the profile's pipe, which are named from
/// the path, are its own, and an install from the installer beside it on the same PC is
/// another device rather than one fighting this one for a name. `lum`, run through the
/// package's alias, is in the package too, and opens the same store.
fn packaged_directory(family: &str) -> Option<PathBuf> {
    let local = directories::BaseDirs::new()?.data_local_dir().to_path_buf();
    Some(local.join("Packages").join(family).join("LocalCache").join("Local").join("lumenna").join("data"))
}

/// The package this process runs in (an MSIX's, as the Microsoft Store installs), by its family
/// name; `None` for an ordinary install.
#[cfg(windows)]
#[must_use]
pub fn package_family() -> Option<String> {
    use windows_sys::Win32::Foundation::ERROR_INSUFFICIENT_BUFFER;
    use windows_sys::Win32::Storage::Packaging::Appx::GetCurrentPackageFamilyName;
    let mut length = 0u32;
    // Asked with no buffer, a packaged process is told the length; any other, that it has none.
    if unsafe { GetCurrentPackageFamilyName(&mut length, std::ptr::null_mut()) } != ERROR_INSUFFICIENT_BUFFER {
        return None;
    }
    let mut name = vec![0u16; length as usize];
    if unsafe { GetCurrentPackageFamilyName(&mut length, name.as_mut_ptr()) } != 0 {
        return None;
    }
    let end = name.iter().position(|&unit| unit == 0).unwrap_or(name.len());
    Some(String::from_utf16_lossy(&name[..end]))
}

/// No other system packages an app this way.
#[cfg(not(windows))]
#[must_use]
pub fn package_family() -> Option<String> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Arguments {
        arguments(args.iter().map(|a| (*a).to_owned()))
    }

    #[test]
    fn nothing_given_is_the_usual_profile_shown() {
        assert_eq!(parse(&[]), Arguments::default());
    }

    #[test]
    fn a_profile_can_be_named_either_way() {
        assert_eq!(parse(&["--profile", "D:\\work"]).profile, Some(PathBuf::from("D:\\work")));
        assert_eq!(parse(&["--profile=D:\\work"]).profile, Some(PathBuf::from("D:\\work")));
    }

    #[test]
    fn starting_at_sign_in_is_in_the_background() {
        let parsed = parse(&["--background", "--profile", "D:\\work"]);
        assert!(parsed.background);
        assert_eq!(parsed.profile, Some(PathBuf::from("D:\\work")));
    }

    #[test]
    fn a_copy_can_leave_the_shortcuts_to_another() {
        assert!(parse(&["--no-shortcuts"]).no_shortcuts);
        assert!(!parse(&[]).no_shortcuts, "the shortcuts are taken unless asked not to");
    }

    #[test]
    fn a_launch_shortcut_can_be_named_either_way() {
        assert_eq!(parse(&["--launch", "new-task"]).launch.as_deref(), Some("new-task"));
        assert_eq!(parse(&["--launch=today", "--no-shortcuts"]).launch.as_deref(), Some("today"));
        assert_eq!(parse(&[]).launch, None);
    }

    #[test]
    fn an_option_not_understood_is_ignored() {
        assert_eq!(parse(&["--minimised"]), Arguments::default());
    }

    #[test]
    fn a_profile_on_the_command_line_wins() {
        assert_eq!(directory(Some(PathBuf::from("D:\\work"))), Some(PathBuf::from("D:\\work")));
    }

    #[test]
    fn a_packaged_install_keeps_its_store_in_its_own_package_folder() {
        let path = packaged_directory("dev.emassey0135.Lumenna_8wekyb3d8bbwe").unwrap();
        assert!(
            path.ends_with(std::path::Path::new("Packages/dev.emassey0135.Lumenna_8wekyb3d8bbwe/LocalCache/Local/lumenna/data")),
            "{}",
            path.display()
        );
    }

    #[test]
    fn a_process_run_outside_a_package_has_no_package() {
        assert_eq!(package_family(), None);
    }
}
