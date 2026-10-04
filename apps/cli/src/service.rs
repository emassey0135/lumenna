//! `lum daemon install|uninstall|start|stop|status` (§8): running `lum sync-daemon` as a
//! service, so nobody hand-writes a service definition.
//!
//! CLI-only users have no tray app to hold the sync endpoint all day, and on a BTSpeak nothing
//! else is resident at all (§16.11), so something has to keep sync running. Each platform has
//! its own way, and §8 names them:
//!
//! - **Linux** — a `systemctl --user` unit, **plus `loginctl enable-linger`**: without
//!   lingering the service stops when the last session ends, which defeats the purpose.
//! - **BTSpeak** — a system unit with `User=` the person installing it, the device's own
//!   convention, which needs no lingering. Writing it takes `sudo`.
//! - **macOS** — a LaunchAgent in `~/Library/LaunchAgents`.
//! - **Windows** — a Task Scheduler entry at logon, which needs no administrator rights,
//!   unlike a real Windows service.
//!
//! The service is told its profile with `--profile`, never left to work it out, so it syncs
//! the store it was installed from whatever its environment says. A profile other than the
//! default gets a service name of its own, so each profile on a machine can have a daemon.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::api::{Affected, Response};
use crate::error::{CliError, Result};
use crate::profile::{Profile, default_directory};

/// Where and how the service runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Service {
    /// The `lum` binary the service starts.
    pub exe: PathBuf,
    /// The profile it syncs, absolute.
    pub profile: PathBuf,
    /// Whether it stays off the relays and the lookup service.
    pub local_only: bool,
    /// A short tag for a profile other than the default, so its service has a name of its
    /// own; `None` for the default profile.
    pub tag: Option<String>,
}

impl Service {
    /// The service for this profile, started from the running binary.
    pub(crate) fn for_profile(profile: &Profile, local_only: bool) -> Result<Self> {
        let exe = std::env::current_exe()?.canonicalize()?;
        let directory = profile.directory().canonicalize()?;
        let default = default_directory().and_then(|d| d.canonicalize().ok());
        let tag = (default.as_deref() != Some(directory.as_path())).then(|| tag_for(&directory));
        Ok(Self { exe, profile: directory, local_only, tag })
    }

    /// What the service runs: `lum --profile <dir> sync-daemon [--local-only]`.
    fn arguments(&self) -> Vec<String> {
        let mut args = vec![
            self.exe.display().to_string(),
            "--profile".to_owned(),
            self.profile.display().to_string(),
            "sync-daemon".to_owned(),
        ];
        if self.local_only {
            args.push("--local-only".to_owned());
        }
        args
    }

    /// `lumenna-sync`, or `lumenna-sync-<tag>`: the systemd unit's name, without `.service`.
    fn unit_name(&self) -> String {
        match &self.tag {
            Some(tag) => format!("lumenna-sync-{tag}"),
            None => "lumenna-sync".to_owned(),
        }
    }

    /// `lumenna.sync`, or `lumenna.sync.<tag>`: the launchd label.
    fn label(&self) -> String {
        match &self.tag {
            Some(tag) => format!("lumenna.sync.{tag}"),
            None => "lumenna.sync".to_owned(),
        }
    }

    /// `Lumenna Sync`, or `Lumenna Sync <tag>`: the scheduled task's name.
    fn task_name(&self) -> String {
        match &self.tag {
            Some(tag) => format!("Lumenna Sync {tag}"),
            None => "Lumenna Sync".to_owned(),
        }
    }

    /// The systemd unit. `user` is set for BTSpeak's system unit and absent for a user unit.
    fn systemd_unit(&self, user: Option<&str>) -> String {
        let exec = self.arguments().iter().map(|a| systemd_quote(a)).collect::<Vec<_>>().join(" ");
        let mut unit = format!(
            "# Written by `lum daemon install`; `lum daemon uninstall` removes it.\n\
             [Unit]\n\
             Description=Lumenna sync daemon for {}\n\
             Wants=network-online.target\n\
             After=network-online.target\n\
             \n\
             [Service]\n\
             ExecStart={exec}\n\
             Restart=on-failure\n\
             RestartSec=10\n",
            self.profile.display()
        );
        if let Some(user) = user {
            unit.push_str(&format!("User={user}\n"));
        }
        let wanted_by = if user.is_some() { "multi-user.target" } else { "default.target" };
        unit.push_str(&format!("\n[Install]\nWantedBy={wanted_by}\n"));
        unit
    }

    /// The launchd property list.
    fn launchd_plist(&self) -> String {
        let args: String = self
            .arguments()
            .iter()
            .map(|a| format!("        <string>{}</string>\n", xml_escape(a)))
            .collect();
        let log = xml_escape(&self.profile.join("sync-daemon.log").display().to_string());
        format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
             <!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \
             \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
             <!-- Written by `lum daemon install`; `lum daemon uninstall` removes it. -->\n\
             <plist version=\"1.0\">\n\
             <dict>\n\
             \x20   <key>Label</key>\n\
             \x20   <string>{label}</string>\n\
             \x20   <key>ProgramArguments</key>\n\
             \x20   <array>\n\
             {args}\
             \x20   </array>\n\
             \x20   <key>RunAtLoad</key>\n\
             \x20   <true/>\n\
             \x20   <!-- Restarted if it fails, not when it is stopped: it exits cleanly then. -->\n\
             \x20   <key>KeepAlive</key>\n\
             \x20   <dict>\n\
             \x20       <key>SuccessfulExit</key>\n\
             \x20       <false/>\n\
             \x20   </dict>\n\
             \x20   <key>ProcessType</key>\n\
             \x20   <string>Background</string>\n\
             \x20   <key>StandardErrorPath</key>\n\
             \x20   <string>{log}</string>\n\
             </dict>\n\
             </plist>\n",
            label = xml_escape(&self.label()),
        )
    }

    /// The command line Task Scheduler runs, quoted for Windows.
    fn windows_command_line(&self) -> String {
        self.arguments().iter().map(|a| windows_quote(a)).collect::<Vec<_>>().join(" ")
    }
}

/// Eight hex digits that tell profiles apart, from the profile's path. FNV-1a: stable across
/// builds and platforms, which a service name has to be.
fn tag_for(directory: &Path) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in directory.display().to_string().bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{:08x}", hash & 0xffff_ffff)
}

/// One argument for systemd's `ExecStart=`: quoted, with `\`, `"` and `%` escaped.
fn systemd_quote(arg: &str) -> String {
    let escaped = arg.replace('\\', "\\\\").replace('"', "\\\"").replace('%', "%%");
    format!("\"{escaped}\"")
}

fn xml_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// One argument quoted the way `CommandLineToArgvW` reads it back.
fn windows_quote(arg: &str) -> String {
    if !arg.is_empty() && !arg.contains([' ', '\t', '"']) {
        return arg.to_owned();
    }
    let mut out = String::from('"');
    let mut backslashes = 0;
    for c in arg.chars() {
        match c {
            '\\' => backslashes += 1,
            '"' => {
                out.push_str(&"\\".repeat(backslashes * 2 + 1));
                out.push('"');
                backslashes = 0;
            }
            other => {
                out.push_str(&"\\".repeat(backslashes));
                out.push(other);
                backslashes = 0;
            }
        }
    }
    out.push_str(&"\\".repeat(backslashes * 2));
    out.push('"');
    out
}

// ---------------------------------------------------------------------------------------
// Running the platform's tools
// ---------------------------------------------------------------------------------------

/// Runs a command, turning a failure into a sentence that names the command and says what it
/// printed — a service manager's own message is usually the most useful thing to read.
fn run(program: &str, args: &[&str]) -> Result<String> {
    let output = Command::new(program).args(args).output().map_err(|e| {
        CliError::Message(format!("could not run {program}: {e}"))
    })?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        let said = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        Err(CliError::Message(format!(
            "`{program} {}` failed{}",
            args.join(" "),
            if said.is_empty() { String::new() } else { format!(": {said}") }
        )))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Platform {
    LinuxUser,
    BtSpeak,
    MacOs,
    Windows,
    Other,
}

fn platform() -> Platform {
    if cfg!(target_os = "macos") {
        Platform::MacOs
    } else if cfg!(target_os = "windows") {
        Platform::Windows
    } else if cfg!(target_os = "linux") {
        if Path::new("/BTSpeak").exists() { Platform::BtSpeak } else { Platform::LinuxUser }
    } else {
        Platform::Other
    }
}

fn unsupported() -> CliError {
    CliError::Message(
        "this platform has no service manager `lum` knows how to use; run `lum sync-daemon` \
         from whatever starts programs at login"
            .to_owned(),
    )
}

fn home() -> Result<PathBuf> {
    directories::BaseDirs::new()
        .map(|dirs| dirs.home_dir().to_path_buf())
        .ok_or_else(|| CliError::Message("cannot find the home directory".to_owned()))
}

fn user_unit_path(service: &Service) -> Result<PathBuf> {
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .map_or_else(|| home().map(|h| h.join(".config")), Ok)?;
    Ok(config.join("systemd/user").join(format!("{}.service", service.unit_name())))
}

fn system_unit_path(service: &Service) -> PathBuf {
    PathBuf::from("/etc/systemd/system").join(format!("{}.service", service.unit_name()))
}

fn plist_path(service: &Service) -> Result<PathBuf> {
    Ok(home()?.join("Library/LaunchAgents").join(format!("{}.plist", service.label())))
}

fn current_user() -> Result<String> {
    Ok(run("id", &["-un"])?.trim().to_owned())
}

fn launchd_domain() -> Result<String> {
    Ok(format!("gui/{}", run("id", &["-u"])?.trim()))
}

// ---------------------------------------------------------------------------------------
// The commands
// ---------------------------------------------------------------------------------------

/// `lum daemon install`.
pub(crate) fn install(profile: &Profile, local_only: bool) -> Result<Response> {
    let service = Service::for_profile(profile, local_only)?;
    let mut notes = Vec::new();
    let where_ = match platform() {
        Platform::LinuxUser => {
            let path = user_unit_path(&service)?;
            write_file(&path, &service.systemd_unit(None))?;
            run("systemctl", &["--user", "daemon-reload"])?;
            let unit = format!("{}.service", service.unit_name());
            run("systemctl", &["--user", "enable", "--now", &unit])?;
            // §8: without lingering the service stops with the last session.
            let user = current_user()?;
            if let Err(error) = run("loginctl", &["enable-linger", &user]) {
                notes.push(format!(
                    "could not let it keep running after you log out ({error}); \
                     `loginctl enable-linger {user}` as an administrator fixes that"
                ));
            }
            format!("a user service, {unit}; `journalctl --user -u {unit}` shows its log")
        }
        Platform::BtSpeak => {
            let user = current_user()?;
            let staged = profile.directory().join(format!("{}.service", service.unit_name()));
            write_file(&staged, &service.systemd_unit(Some(&user)))?;
            let target = system_unit_path(&service);
            let result = (|| {
                run("sudo", &["install", "-m", "0644", &staged.display().to_string(), &target.display().to_string()])?;
                run("sudo", &["systemctl", "daemon-reload"])?;
                run("sudo", &["systemctl", "enable", "--now", &format!("{}.service", service.unit_name())])
            })();
            let _ = std::fs::remove_file(&staged);
            result?;
            format!(
                "a system service, {}.service, running as {user}",
                service.unit_name()
            )
        }
        Platform::MacOs => {
            let path = plist_path(&service)?;
            write_file(&path, &service.launchd_plist())?;
            let domain = launchd_domain()?;
            // Installing again replaces what is there.
            let _ = run("launchctl", &["bootout", &format!("{domain}/{}", service.label())]);
            run("launchctl", &["bootstrap", &domain, &path.display().to_string()])?;
            format!(
                "a LaunchAgent, {}; its log is {}",
                service.label(),
                service.profile.join("sync-daemon.log").display()
            )
        }
        Platform::Windows => {
            let name = service.task_name();
            let command = service.windows_command_line();
            run("schtasks", &["/Create", "/TN", &name, "/TR", &command, "/SC", "ONLOGON", "/RL", "LIMITED", "/F"])?;
            run("schtasks", &["/Run", "/TN", &name])?;
            format!("a scheduled task, {name}, run at logon")
        }
        Platform::Other => return Err(unsupported()),
    };
    if service.exe.components().any(|c| c.as_os_str() == "target") {
        notes.push(format!(
            "it runs {}, which looks like a build directory; install again after installing \
             `lum` for good, or a rebuild will change what the service runs",
            service.exe.display()
        ));
    }
    let mut response = Response::touched(
        format!("The sync daemon now runs as {where_}"),
        Affected::default(),
    );
    for note in notes {
        response = response.note(note);
    }
    Ok(response)
}

/// `lum daemon uninstall`.
pub(crate) fn uninstall(profile: &Profile) -> Result<Response> {
    let service = Service::for_profile(profile, false)?;
    let removed = match platform() {
        Platform::LinuxUser => {
            let path = user_unit_path(&service)?;
            if !path.exists() {
                return Ok(Response::unchanged("the sync daemon is not installed as a service"));
            }
            let unit = format!("{}.service", service.unit_name());
            let _ = run("systemctl", &["--user", "disable", "--now", &unit]);
            std::fs::remove_file(&path)?;
            run("systemctl", &["--user", "daemon-reload"])?;
            unit
        }
        Platform::BtSpeak => {
            let target = system_unit_path(&service);
            if !target.exists() {
                return Ok(Response::unchanged("the sync daemon is not installed as a service"));
            }
            let unit = format!("{}.service", service.unit_name());
            let _ = run("sudo", &["systemctl", "disable", "--now", &unit]);
            run("sudo", &["rm", "-f", &target.display().to_string()])?;
            run("sudo", &["systemctl", "daemon-reload"])?;
            unit
        }
        Platform::MacOs => {
            let path = plist_path(&service)?;
            if !path.exists() {
                return Ok(Response::unchanged("the sync daemon is not installed as a service"));
            }
            let _ = run("launchctl", &["bootout", &format!("{}/{}", launchd_domain()?, service.label())]);
            std::fs::remove_file(&path)?;
            service.label()
        }
        Platform::Windows => {
            let name = service.task_name();
            let _ = run("schtasks", &["/End", "/TN", &name]);
            run("schtasks", &["/Delete", "/TN", &name, "/F"])?;
            name
        }
        Platform::Other => return Err(unsupported()),
    };
    Ok(Response::touched(
        format!("Removed the sync daemon service, {removed}; nothing syncs in the background now"),
        Affected::default(),
    ))
}

/// `lum daemon start` and `lum daemon stop`.
pub(crate) fn start_or_stop(profile: &Profile, start: bool) -> Result<Response> {
    let service = Service::for_profile(profile, false)?;
    let verb = if start { "start" } else { "stop" };
    match platform() {
        Platform::LinuxUser => {
            run("systemctl", &["--user", verb, &format!("{}.service", service.unit_name())])?;
        }
        Platform::BtSpeak => {
            run("sudo", &["systemctl", verb, &format!("{}.service", service.unit_name())])?;
        }
        Platform::MacOs => {
            let target = format!("{}/{}", launchd_domain()?, service.label());
            if start {
                run("launchctl", &["kickstart", &target])?;
            } else {
                // A clean exit, which launchd's KeepAlive does not restart. It starts again
                // at the next login, or with `lum daemon start`.
                run("launchctl", &["kill", "SIGTERM", &target])?;
            }
        }
        Platform::Windows => {
            let flag = if start { "/Run" } else { "/End" };
            run("schtasks", &[flag, "/TN", &service.task_name()])?;
        }
        Platform::Other => return Err(unsupported()),
    }
    Ok(Response::touched(
        if start { "Started the sync daemon" } else { "Stopped the sync daemon until the next login, or `lum daemon start`" },
        Affected::default(),
    ))
}

/// `lum daemon status`: whether the service is installed, and whether anything holds the
/// endpoint.
pub(crate) fn status(profile: &Profile) -> Result<Response> {
    let service = Service::for_profile(profile, false)?;
    let installed = match platform() {
        Platform::LinuxUser => user_unit_path(&service)?.exists(),
        Platform::BtSpeak => system_unit_path(&service).exists(),
        Platform::MacOs => plist_path(&service)?.exists(),
        Platform::Windows => run("schtasks", &["/Query", "/TN", &service.task_name()]).is_ok(),
        Platform::Other => false,
    };
    let running = crate::network::endpoint_held(profile)?;
    let announcement = match (installed, running) {
        (true, true) => "The sync daemon is installed as a service and running",
        (true, false) => "The sync daemon is installed as a service but not running; `lum daemon start` starts it",
        (false, true) => "Sync is running, but not as a service; `lum daemon install` keeps it running",
        (false, false) => "The sync daemon is not installed or running; `lum daemon install` sets it up",
    };
    Ok(Response::unchanged(announcement))
}

fn write_file(path: &Path, contents: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, contents)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn service() -> Service {
        Service {
            exe: PathBuf::from("/usr/local/bin/lum"),
            profile: PathBuf::from("/home/pi/.local/share/my tasks"),
            local_only: true,
            tag: Some("0a1b2c3d".to_owned()),
        }
    }

    #[test]
    fn a_systemd_unit_quotes_its_arguments_and_names_its_profile() {
        let unit = service().systemd_unit(None);
        assert!(unit.contains(
            "ExecStart=\"/usr/local/bin/lum\" \"--profile\" \"/home/pi/.local/share/my tasks\" \"sync-daemon\" \"--local-only\""
        ), "{unit}");
        assert!(unit.contains("WantedBy=default.target"));
        assert!(!unit.contains("User="));

        let system = service().systemd_unit(Some("pi"));
        assert!(system.contains("User=pi\n"));
        assert!(system.contains("WantedBy=multi-user.target"));
    }

    #[test]
    fn a_percent_sign_does_not_become_a_systemd_specifier() {
        assert_eq!(systemd_quote("50%\"x\""), "\"50%%\\\"x\\\"\"");
    }

    #[test]
    fn a_plist_escapes_its_paths_and_restarts_only_on_failure() {
        let mut odd = service();
        odd.profile = PathBuf::from("/Users/me/Tasks & <Stuff>");
        let plist = odd.launchd_plist();
        assert!(plist.contains("<string>/Users/me/Tasks &amp; &lt;Stuff&gt;</string>"), "{plist}");
        assert!(plist.contains("<string>lumenna.sync.0a1b2c3d</string>"));
        assert!(plist.contains("<key>SuccessfulExit</key>"));
    }

    #[test]
    fn windows_arguments_survive_spaces_and_quotes() {
        assert_eq!(windows_quote("plain"), "plain");
        assert_eq!(windows_quote("C:\\Program Files\\lum.exe"), "\"C:\\Program Files\\lum.exe\"");
        assert_eq!(windows_quote("a \"b\""), "\"a \\\"b\\\"\"");
        assert_eq!(windows_quote("ends\\ "), "\"ends\\ \"");
    }

    #[test]
    fn the_default_profile_keeps_the_plain_names_and_others_are_told_apart() {
        let mut default = service();
        default.tag = None;
        assert_eq!(default.unit_name(), "lumenna-sync");
        assert_eq!(default.label(), "lumenna.sync");
        assert_eq!(service().unit_name(), "lumenna-sync-0a1b2c3d");
        assert_ne!(tag_for(Path::new("/a")), tag_for(Path::new("/b")));
        assert_eq!(tag_for(Path::new("/a")), tag_for(Path::new("/a")), "stable");
    }
}
