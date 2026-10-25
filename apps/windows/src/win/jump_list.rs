//! The Jump List, on the taskbar button and in Start: the core's ways in
//! (`lumenna_records::launch::launch_shortcuts`), in its order and words, each starting the app
//! with `--launch <id>`, or handing the id to the copy already running (`tray::claim`).
//!
//! A package's list is the package's own (`Windows.UI.StartScreen.JumpList`), whose items start
//! the package's app with their arguments; an ordinary install's is the shell's list for the
//! identity its Start menu shortcut carries (`toast::app_id`), each task a link to this
//! executable. Made off the window's thread, since either can take a moment.

use lumenna_records::launch::{LaunchShortcut, launch_shortcuts};
use windows::UI::StartScreen::{JumpList, JumpListItem, JumpListSystemGroupKind};
use windows::Win32::Storage::EnhancedStorage::PKEY_Title;
use windows::Win32::System::Com::StructuredStorage::{PROPVARIANT, PropVariantChangeType, PVCHF_DEFAULT};
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx};
use windows::Win32::System::Variant::VT_LPWSTR;
use windows::Win32::UI::Shell::Common::{IObjectArray, IObjectCollection};
use windows::Win32::UI::Shell::PropertiesSystem::IPropertyStore;
use windows::Win32::UI::Shell::{DestinationList, EnumerableObjectCollection, ICustomDestinationList, IShellLinkW, ShellLink};
use windows::core::{HSTRING, Interface};

use super::{package, toast};

/// Makes the Jump List for the profile at `directory`, as the core lists its ways in.
pub fn publish(directory: std::path::PathBuf) {
    std::thread::spawn(move || {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        }
        let shortcuts = launch_shortcuts();
        if package::packaged() && JumpList::IsSupported().unwrap_or(false) {
            let _ = packaged(&shortcuts);
        } else {
            let _ = ordinary(&shortcuts, &directory);
        }
    });
}

/// What a task starts the app with: the way in, and the profile when it is not the usual one.
fn arguments(id: &str, directory: &std::path::Path) -> String {
    let mut arguments = format!("--launch {id}");
    if crate::profile::default_directory().as_deref() != Some(directory) {
        arguments.push_str(&format!(" --profile \"{}\"", directory.display()));
    }
    arguments
}

fn packaged(shortcuts: &[LaunchShortcut]) -> windows::core::Result<()> {
    let list = JumpList::LoadCurrentAsync()?.join()?;
    list.SetSystemGroupKind(JumpListSystemGroupKind::None)?;
    let items = list.Items()?;
    items.Clear()?;
    for shortcut in shortcuts {
        let item = JumpListItem::CreateWithArguments(&HSTRING::from(format!("--launch {}", shortcut.id)), &HSTRING::from(&shortcut.title))?;
        item.SetDescription(&HSTRING::from(&shortcut.description))?;
        items.Append(&item)?;
    }
    list.SaveAsync()?.join()
}

fn ordinary(shortcuts: &[LaunchShortcut], directory: &std::path::Path) -> windows::core::Result<()> {
    let exe = HSTRING::from(std::env::current_exe().map_err(|_| windows::core::Error::empty())?.as_os_str());
    unsafe {
        let list: ICustomDestinationList = CoCreateInstance(&DestinationList, None, CLSCTX_INPROC_SERVER)?;
        list.SetAppID(&HSTRING::from(toast::app_id()))?;
        let mut slots = 0u32;
        let _removed: IObjectArray = list.BeginList(&mut slots)?;
        let tasks: IObjectCollection = CoCreateInstance(&EnumerableObjectCollection, None, CLSCTX_INPROC_SERVER)?;
        for shortcut in shortcuts {
            let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)?;
            link.SetPath(&exe)?;
            link.SetArguments(&HSTRING::from(arguments(&shortcut.id, directory)))?;
            link.SetDescription(&HSTRING::from(&shortcut.description))?;
            link.SetIconLocation(&exe, 0)?;
            // A task's name is its link's title, as a string the shell reads.
            let mut title = PROPVARIANT::default();
            PropVariantChangeType(&mut title, &PROPVARIANT::from(shortcut.title.as_str()), PVCHF_DEFAULT, VT_LPWSTR)?;
            let store: IPropertyStore = link.cast()?;
            store.SetValue(&PKEY_Title, &title)?;
            store.Commit()?;
            tasks.AddObject(&link)?;
        }
        list.AddUserTasks(&tasks.cast::<IObjectArray>()?)?;
        list.CommitList()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_task_names_the_profile_only_when_it_is_not_the_usual_one() {
        let usual = crate::profile::default_directory().unwrap();
        assert_eq!(arguments("today", &usual), "--launch today");
        assert_eq!(arguments("new-task", std::path::Path::new("D:\\work")), "--launch new-task --profile \"D:\\work\"");
    }
}
