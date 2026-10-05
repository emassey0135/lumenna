//! Where the store lives, and how the app was started.

use std::path::PathBuf;

/// What the app was started with.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Arguments {
    /// `--profile <directory>`: a store other than the usual one, as `lum --profile` takes.
    pub profile: Option<PathBuf>,
    /// `--background`: start in the notification area without showing the window — how it is
    /// started at sign-in.
    pub background: bool,
}

/// Reads the arguments after the program's name. Anything not understood is ignored, so an
/// older shortcut with an option since removed still starts the app.
pub fn arguments(args: impl IntoIterator<Item = String>) -> Arguments {
    let mut parsed = Arguments::default();
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--background" => parsed.background = true,
            "--profile" => parsed.profile = args.next().map(PathBuf::from),
            other => {
                if let Some(path) = other.strip_prefix("--profile=") {
                    parsed.profile = Some(PathBuf::from(path));
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

/// Where the store lives when nothing names another.
///
/// The same place as the command line's, as on the Mac, so the app and `lum` on one computer
/// are one device with one store rather than two that would have to pair with each other. This
/// must agree with `default_directory` in `apps/cli/src/profile.rs`: the *local* data
/// directory, because Roaming AppData is copied to a server at every logon on a domain with
/// roaming profiles.
pub fn default_directory() -> Option<PathBuf> {
    directories::ProjectDirs::from("", "", "lumenna").map(|dirs| dirs.data_local_dir().to_path_buf())
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
    fn an_option_not_understood_is_ignored() {
        assert_eq!(parse(&["--minimised"]), Arguments::default());
    }

    #[test]
    fn a_profile_on_the_command_line_wins() {
        assert_eq!(directory(Some(PathBuf::from("D:\\work"))), Some(PathBuf::from("D:\\work")));
    }
}
