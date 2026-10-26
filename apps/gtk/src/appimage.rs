//! Run from an AppImage: a file the person downloaded, which nothing installed, so the desktop
//! does not know it. The first window shown offers, once, to add it to the applications menu
//! (`lumenna_records::launch::menu_entry_words`): a desktop entry in the user's applications
//! folder naming the AppImage where it is, with its actions and its icon, as `install.sh` writes
//! one for a checkout. The entry is also how the shortcuts portal knows an unsandboxed app. The
//! answer is kept in the user's configuration, so it is never asked again; once added, each start
//! writes the entry again if the AppImage has moved, or a newer one lists other actions.

use std::cell::Cell;
use std::path::{Path, PathBuf};

use gtk::glib;
use gtk::prelude::*;
use lumenna_records::launch::{menu_entry_failed, menu_entry_words};

/// The desktop entry this build was made with, whose `Exec` lines name `lumenna-gtk`.
const ENTRY: &str = include_str!("../data/dev.emassey0135.Lumenna.desktop");
/// KRunner's runner, written as it is. GNOME Shell's search provider is not: the shell reads
/// providers only from the system's data folders, never the user's.
const RUNNER: &str = include_str!("../data/dev.emassey0135.Lumenna.krunner.desktop");

/// Marks an entry as the AppImage's own, which a start may write again; one `install.sh` or a
/// package wrote is left alone.
const MARK: &str = "X-Lumenna-AppImage=true";

thread_local! {
    /// Whether this process has offered already: a window shown again asks nothing.
    static OFFERED: Cell<bool> = const { Cell::new(false) };
}

/// The AppImage this runs from, as its runtime names it (`APPIMAGE`), if it is one.
pub fn path() -> Option<PathBuf> {
    std::env::var_os("APPIMAGE").map(PathBuf::from).filter(|path| path.is_absolute())
}

/// The program to name where something starts the app again (an autostart entry): the AppImage,
/// not the program inside it, whose path is a mount that is gone once it stops.
pub fn program() -> std::io::Result<PathBuf> {
    path().map_or_else(std::env::current_exe, Ok)
}

/// What the person answered, kept beside the app's other configuration.
fn answer_file() -> PathBuf {
    glib::user_config_dir().join("lumenna").join("applications-menu")
}

fn entry_file() -> PathBuf {
    glib::user_data_dir().join("applications").join(format!("{}.desktop", crate::ID))
}

fn runner_file() -> PathBuf {
    glib::user_data_dir().join("krunner/dbusplugins").join(format!("{}.desktop", crate::ID))
}

/// The session bus's service file, which starts the app in the background to answer a search.
fn service_file() -> PathBuf {
    glib::user_data_dir().join("dbus-1/services").join(format!("{}.service", crate::ID))
}

/// Whether the offer is this process's to make: run from an AppImage on the usual profile, whose
/// identifier the entry is named by. Another profile's copy is a second one, for testing.
fn ours(application: Option<gtk::Application>) -> Option<PathBuf> {
    path().filter(|_| usual(application))
}

/// Whether this is the usual profile's copy, whose identifier the files are named by.
fn usual(application: Option<gtk::Application>) -> bool {
    application.and_then(|application| application.application_id()).is_some_and(|id| id == crate::ID)
}

/// On the first window shown, offers the applications menu, unless it was answered before or the
/// menu has Lumenna already.
pub fn offer(window: &gtk::ApplicationWindow) {
    if OFFERED.replace(true) {
        return;
    }
    let Some(appimage) = ours(window.application()) else { return };
    if answer_file().exists() || entry_file().exists() {
        return;
    }
    let window = window.clone();
    crate::window::spawn(async move {
        let words = menu_entry_words();
        let yes = crate::prompts::yes_or_no(&window, &words.heading, &words.detail, &words.no, &words.yes).await;
        let said = if yes {
            match write(&appimage) {
                Ok(()) => {
                    remember("added");
                    words.added
                }
                // Not kept, so it is offered again next time.
                Err(error) => menu_entry_failed(&error.to_string()),
            }
        } else {
            remember("declined");
            return;
        };
        if let Some(app) = crate::window::app() {
            app.say(&said);
        }
    });
}

/// At each start, once added: the entry written again if it no longer says what it would, the
/// AppImage having moved or been replaced by a newer one. An entry the person removed stays gone.
/// Started otherwise (a checkout's build), what an AppImage since deleted left is taken away.
pub fn keep_current(application: &gtk::Application) {
    forget_if_gone(application);
    let Some(appimage) = ours(Some(application.clone())) else { return };
    if std::fs::read_to_string(answer_file()).map(|answer| answer.trim().to_owned()).ok().as_deref() != Some("added") {
        return;
    }
    let Ok(current) = std::fs::read_to_string(entry_file()) else { return };
    let stale = current != entry(&appimage)
        || std::fs::read_to_string(service_file()).ok() != Some(service(&appimage))
        || std::fs::read_to_string(runner_file()).ok().as_deref() != Some(RUNNER);
    if current.lines().any(|line| line == MARK)
        && stale
        && let Err(error) = write(&appimage)
    {
        eprintln!("Lumenna could not bring its applications menu entry up to date: {error}");
    }
}

/// Takes away the AppImage's own entry, KRunner's runner and the search's service once the
/// AppImage they name is gone: the entry hides itself (`TryExec`), but KRunner would go on asking
/// the bus to start a program that is not there. Only from a start that is not an AppImage: one
/// is, it writes them again naming itself. The answer goes too, so a later AppImage offers again.
fn forget_if_gone(application: &gtk::Application) {
    // A sandbox's data folder is its own, never the one these were written to.
    if path().is_some() || crate::sandboxed() || !usual(Some(application.clone())) {
        return;
    }
    let Ok(current) = std::fs::read_to_string(entry_file()) else { return };
    if named_appimage(&current).is_none_or(|appimage| appimage.exists()) {
        return;
    }
    for file in [service_file(), runner_file(), entry_file(), answer_file()] {
        if let Err(error) = std::fs::remove_file(&file)
            && error.kind() != std::io::ErrorKind::NotFound
        {
            eprintln!("Lumenna could not take away {}, left by an AppImage since deleted: {error}", file.display());
        }
    }
}

/// The AppImage an entry names, if it is the AppImage's own (`MARK`): its `TryExec`, as `entry`
/// wrote it.
fn named_appimage(entry: &str) -> Option<PathBuf> {
    if !entry.lines().any(|line| line == MARK) {
        return None;
    }
    entry.lines().find_map(|line| line.strip_prefix("TryExec=")).map(|path| PathBuf::from(path.replace("\\\\", "\\")))
}

fn remember(answer: &str) {
    let file = answer_file();
    let written = file.parent().map_or(Ok(()), std::fs::create_dir_all).and_then(|()| std::fs::write(&file, format!("{answer}\n")));
    if let Err(error) = written {
        eprintln!("Lumenna could not keep the answer to its applications menu offer: {error}");
    }
}

/// Writes the entry, KRunner's runner and the service that starts the AppImage, and copies the
/// icon in beside them, from the AppImage's own.
fn write(appimage: &Path) -> std::io::Result<()> {
    if let Some(appdir) = std::env::var_os("APPDIR") {
        copy_icons(&Path::new(&appdir).join("usr/share/icons/hicolor"), &glib::user_data_dir().join("icons/hicolor"))?;
    }
    replace(&runner_file(), RUNNER)?;
    replace(&service_file(), &service(appimage))?;
    // The entry last, so an entry there means the rest is too.
    replace(&entry_file(), &entry(appimage))
}

/// Written beside it and renamed, so nothing ever reads half a file.
fn replace(file: &Path, text: &str) -> std::io::Result<()> {
    if let Some(parent) = file.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut partial = file.as_os_str().to_owned();
    partial.push(".partial");
    std::fs::write(&partial, text)?;
    std::fs::rename(&partial, file)
}

/// The service file naming the AppImage, its `Exec` split as a shell would split it.
fn service(appimage: &Path) -> String {
    let path = appimage.display().to_string();
    let program = if path.chars().all(|c| c.is_ascii_alphanumeric() || "/._+-".contains(c)) {
        path
    } else {
        format!("'{}'", path.replace('\'', "'\\''"))
    };
    format!("[D-BUS Service]\nName={}\nExec={program} --background\n", crate::ID)
}

/// Every image of the app's icon under `from` (`<size>/apps/<id>.png`), to the same place under
/// `to`.
fn copy_icons(from: &Path, to: &Path) -> std::io::Result<()> {
    let Ok(sizes) = std::fs::read_dir(from) else { return Ok(()) };
    for size in sizes.flatten() {
        let Ok(icons) = std::fs::read_dir(size.path().join("apps")) else { continue };
        for icon in icons.flatten() {
            let name = icon.file_name();
            if Path::new(&name).file_stem().is_some_and(|stem| stem == crate::ID) {
                let target = to.join(size.file_name()).join("apps").join(&name);
                if let Some(parent) = target.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::copy(icon.path(), target)?;
            }
        }
    }
    Ok(())
}

/// The desktop entry for the AppImage at `appimage`: every `Exec` naming it, the main one's
/// `TryExec` too, so the entry hides itself while the file is gone, and marked as the AppImage's.
fn entry(appimage: &Path) -> String {
    let program = exec_argument(appimage);
    let mut text = String::new();
    for line in ENTRY.lines() {
        match line.strip_prefix("Exec=lumenna-gtk") {
            Some(rest) => {
                text.push_str(&format!("Exec={program}{rest}\n"));
                if text.lines().filter(|line| line.starts_with('[')).count() == 1 {
                    text.push_str(&format!("TryExec={}\n{MARK}\n", appimage.display().to_string().replace('\\', "\\\\")));
                }
            }
            None => {
                text.push_str(line);
                text.push('\n');
            }
        }
    }
    text
}

/// A path as one argument of a desktop entry's `Exec`: quoted where it holds a reserved character,
/// `%` doubled, then escaped as a key file's string, which doubles each backslash again.
pub fn exec_argument(path: &Path) -> String {
    let text = path.display().to_string();
    let quoted = if text.chars().any(|c| c.is_whitespace() || "\"'\\><~|&;$*?#()`".contains(c)) {
        let mut inner = String::new();
        for c in text.chars() {
            if "\"`$\\".contains(c) {
                inner.push('\\');
            }
            inner.push(c);
        }
        format!("\"{inner}\"")
    } else {
        text
    };
    quoted.replace('%', "%%").replace('\\', "\\\\")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_exec_names_the_appimage_and_keeps_its_launch_shortcut() {
        let text = entry(Path::new("/home/someone/Applications/Lumenna-1.0-x86_64.AppImage"));
        let execs: Vec<&str> = text.lines().filter(|line| line.starts_with("Exec=")).collect();
        assert_eq!(execs.len(), 1 + lumenna_records::launch::launch_shortcuts().len());
        assert!(execs.iter().all(|line| line.starts_with("Exec=/home/someone/Applications/Lumenna-1.0-x86_64.AppImage")));
        assert!(text.contains("Exec=/home/someone/Applications/Lumenna-1.0-x86_64.AppImage --launch new-task\n"));
        assert_eq!(text.matches("TryExec=").count(), 1, "the main entry's alone");
        let main = &text[..text.find("[Desktop Action").unwrap()];
        assert!(main.contains("TryExec=/home/someone/Applications/Lumenna-1.0-x86_64.AppImage\n"));
        assert!(main.contains(MARK));
        assert!(text.contains("Actions=new-task;today;what-now;"));
    }

    #[test]
    fn an_entry_of_the_appimage_names_the_file_it_runs_and_another_entry_names_none() {
        let appimage = Path::new("/home/someone/odd\\name/Lumenna.AppImage");
        assert_eq!(named_appimage(&entry(appimage)).as_deref(), Some(appimage));
        assert_eq!(named_appimage(ENTRY), None, "one install.sh or a package wrote is never taken away");
    }

    #[test]
    fn the_search_service_starts_the_appimage_in_the_background() {
        assert_eq!(
            service(Path::new("/opt/Lumenna.AppImage")),
            "[D-BUS Service]\nName=dev.emassey0135.Lumenna\nExec=/opt/Lumenna.AppImage --background\n"
        );
        assert!(service(Path::new("/home/a b/it's.AppImage")).contains("Exec='/home/a b/it'\\''s.AppImage' --background"));
    }

    #[test]
    fn a_path_with_spaces_or_a_percent_is_quoted_as_a_desktop_entry_takes_it() {
        assert_eq!(exec_argument(Path::new("/opt/Lumenna.AppImage")), "/opt/Lumenna.AppImage");
        assert_eq!(exec_argument(Path::new("/home/a b/100%/L.AppImage")), "\"/home/a b/100%%/L.AppImage\"");
        // `$` is escaped for Exec (`\$`), and the backslash again for the key file (`\\$`).
        assert_eq!(exec_argument(Path::new("/home/$x/L.AppImage")), "\"/home/\\\\$x/L.AppImage\"");
    }
}
