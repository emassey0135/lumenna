//! What Lumenna offers from outside it, before it is open: the app icon's shortcuts (iOS and
//! Android), the Dock's menu on a Mac, the taskbar's Jump List on Windows, a desktop entry's
//! actions on Linux, and a web app's manifest. Each platform lists these and no others, in
//! this order and these words, and hands the one chosen to the app (`Event::Launch`), which
//! does it. A platform whose list is a file written ahead (the desktop entry, the web's
//! manifest) keeps it as this says, which a test of the core checks; the rest set it at run time.

use serde::{Deserialize, Serialize};

/// One way into Lumenna from outside it.
#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LaunchShortcut {
    /// What the platform hands back when it is chosen: "new-task".
    pub id: String,
    /// Its name, in Title Case: "New Task".
    pub title: String,
    /// The same in sentence case.
    pub sentence: String,
    /// What it does, for a platform that shows a second line or a tooltip.
    pub description: String,
    /// A generic name for its icon, which each platform draws its own way: "add", "today",
    /// "next".
    pub icon: String,
}

/// Every launch shortcut, in the order a platform lists them.
#[must_use]
pub fn launch_shortcuts() -> Vec<LaunchShortcut> {
    [
        ("new-task", "New Task", "Add a task with one line", "add"),
        ("today", "Today", "The day's blocks and what is in them", "today"),
        ("what-now", "What Should I Work On?", "What to work on now, and starting on it", "next"),
    ]
    .into_iter()
    .map(|(id, title, description, icon)| LaunchShortcut {
        id: id.to_owned(),
        title: title.to_owned(),
        sentence: crate::fields::sentence_case(title.to_owned()),
        description: description.to_owned(),
        icon: icon.to_owned(),
    })
    .collect()
}

/// What an app that runs from a file of its own, not installed (Linux's AppImage), asks the first
/// time it runs: whether to add itself to the desktop's applications menu, which is also how the
/// desktop comes to know it (its shortcuts from anywhere, its launch shortcuts). Asked once.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MenuEntryWords {
    /// The question: "Add Lumenna to the applications menu?"
    pub heading: String,
    /// What adding it does, and that it is asked once.
    pub detail: String,
    /// The answer that adds it, in Title Case.
    pub yes: String,
    /// The answer that leaves it out, in Title Case, and the safe one.
    pub no: String,
    /// Said once it is added.
    pub added: String,
}

/// The words of the applications menu's offer.
#[must_use]
pub fn menu_entry_words() -> MenuEntryWords {
    MenuEntryWords {
        heading: "Add Lumenna to the applications menu?".to_owned(),
        detail: "Lumenna runs from the file you downloaded, so the applications menu does not list it yet. Added, \
                 it opens from there like any other app, with New Task, Today and What Should I Work On? beside \
                 it, and the desktop can offer its shortcuts from anywhere. This is asked once."
            .to_owned(),
        yes: "Add to Menu".to_owned(),
        no: "Don't Add".to_owned(),
        added: "Lumenna is in the applications menu".to_owned(),
    }
}

/// Said when the applications menu's entry could not be written, with the platform's reason.
#[must_use]
pub fn menu_entry_failed(reason: &str) -> String {
    let reason = reason.trim();
    if reason.is_empty() {
        "Lumenna could not be added to the applications menu".to_owned()
    } else {
        format!("Lumenna could not be added to the applications menu: {reason}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_menu_offer_names_every_launch_shortcut_it_promises() {
        let detail = menu_entry_words().detail;
        for shortcut in launch_shortcuts() {
            assert!(detail.contains(&shortcut.title), "{} in {detail}", shortcut.title);
        }
        assert_eq!(menu_entry_failed(" "), "Lumenna could not be added to the applications menu");
    }

    /// The repository's root, from this crate.
    fn root() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    #[test]
    fn the_linux_desktop_entry_lists_every_launch_shortcut_as_the_core_words_it() {
        let entry = std::fs::read_to_string(root().join("apps/gtk/data/dev.emassey0135.Lumenna.desktop")).unwrap();
        let ids: Vec<String> = launch_shortcuts().iter().map(|s| s.id.clone()).collect();
        let listed = entry.lines().find_map(|line| line.strip_prefix("Actions=")).map(|actions| {
            actions.split(';').filter(|a| !a.is_empty()).map(str::to_owned).collect::<Vec<_>>()
        });
        assert_eq!(listed.as_deref(), Some(ids.as_slice()), "Actions= in the desktop entry");
        for shortcut in launch_shortcuts() {
            let section = format!("[Desktop Action {}]", shortcut.id);
            let at = entry.find(&section).unwrap_or_else(|| panic!("no {section}"));
            let name = entry[at..].lines().find_map(|line| line.strip_prefix("Name=")).unwrap_or_default();
            assert_eq!(name, shortcut.title, "{section}");
        }
    }

    #[test]
    fn the_web_app_manifest_lists_every_launch_shortcut_as_the_core_words_it() {
        let Some(manifest) = ["apps/web/manifest.json"]
            .iter()
            .find_map(|path| std::fs::read_to_string(root().join(path)).ok())
        else {
            panic!("no web app manifest");
        };
        let manifest: serde_json::Value = serde_json::from_str(&manifest).unwrap();
        let listed: Vec<(String, String)> = manifest["shortcuts"]
            .as_array()
            .map(|shortcuts| {
                shortcuts
                    .iter()
                    .map(|s| (s["name"].as_str().unwrap_or_default().to_owned(), s["url"].as_str().unwrap_or_default().to_owned()))
                    .collect()
            })
            .unwrap_or_default();
        let wanted: Vec<(String, String)> =
            launch_shortcuts().into_iter().map(|s| (s.sentence, format!("/?launch={}", s.id))).collect();
        assert_eq!(listed, wanted);
    }
}
