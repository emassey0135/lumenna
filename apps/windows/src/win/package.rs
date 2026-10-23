//! What differs when the app runs as an MSIX package, as the Microsoft Store installs it: Windows
//! opens it at sign-in through the package's startup task rather than the Run key, which a
//! package's registry writes never reach, and its notifications are the package's own. Its store
//! is the package's too (`profile::default_directory`), and with it the one-instance lock and
//! the pipe named from its path.

use windows::ApplicationModel::Activation::ActivationKind;
use windows::ApplicationModel::{AppInstance, StartupTask, StartupTaskState};
use windows::core::HSTRING;

use crate::profile;

/// The startup task the package's manifest declares (`installer/msix/AppxManifest.xml`).
const STARTUP_TASK: &str = "LumennaAtSignIn";

/// Whether this process runs in a package.
pub fn packaged() -> bool {
    profile::package_family().is_some()
}

/// Whether Windows started this process at sign-in, through the package's startup task: as
/// `--background` does for the Run key, it then opens in the notification area alone.
pub fn started_at_sign_in() -> bool {
    packaged()
        && AppInstance::GetActivatedEventArgs()
            .and_then(|arguments| arguments.Kind())
            .is_ok_and(|kind| kind == ActivationKind::StartupTask)
}

fn task() -> windows::core::Result<StartupTask> {
    StartupTask::GetAsync(&HSTRING::from(STARTUP_TASK))?.join()
}

/// Whether the startup task opens the app at sign-in.
pub fn opens_at_sign_in() -> bool {
    task()
        .and_then(|task| task.State())
        .is_ok_and(|state| state == StartupTaskState::Enabled || state == StartupTaskState::EnabledByPolicy)
}

/// Turns the startup task on or off, or says why Windows would not. Asking can take a moment,
/// so it is asked off the window's thread.
pub fn open_at_sign_in(on: bool) -> Result<(), String> {
    let reason = |error: windows::core::Error| error.message().trim().trim_end_matches('.').to_owned();
    let task = task().map_err(reason)?;
    if !on {
        return task.Disable().map_err(reason);
    }
    match task.RequestEnableAsync().and_then(|asked| asked.join()).map_err(reason)? {
        StartupTaskState::Enabled | StartupTaskState::EnabledByPolicy => Ok(()),
        StartupTaskState::DisabledByUser => {
            Err("it is turned off in Startup apps, in Settings or Task Manager, which is where it is turned back on".to_owned())
        }
        StartupTaskState::DisabledByPolicy => Err("your organisation's policy turns it off".to_owned()),
        _ => Err("Windows did not turn it on".to_owned()),
    }
}
