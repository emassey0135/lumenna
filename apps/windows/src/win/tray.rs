//! Staying resident: the notification-area icon, and one instance per profile.
//!
//! Tray icons are poorly exposed to screen readers, so the icon is never the only way back
//! to the window: a global shortcut shows it (`shortcuts.rs`), and starting Lumenna again
//! shows the running one rather than opening a second.

use windows::Win32::Foundation::{ERROR_ALREADY_EXISTS, GetLastError, HANDLE, HWND, LPARAM, WPARAM};
use windows::Win32::System::Threading::CreateMutexW;
use windows::Win32::UI::Shell::{
    NIF_ICON, NIF_MESSAGE, NIF_SHOWTIP, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_SETVERSION,
    NOTIFYICON_VERSION_4, NOTIFYICONDATAW, Shell_NotifyIconW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AllowSetForegroundWindow, FindWindowW, GetWindowThreadProcessId, HICON, PostMessageW,
    RegisterWindowMessageW, WM_APP,
};
use windows::core::{HSTRING, w};

/// The icon's messages.
pub const WM_TRAY: u32 = WM_APP + 4;
/// Sent by a second instance: show the window.
pub const WM_SHOW_RUNNING: u32 = WM_APP + 5;

fn icon_data(main: HWND, icon: HICON) -> NOTIFYICONDATAW {
    let mut data = NOTIFYICONDATAW {
        cbSize: size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: main,
        uID: 1,
        uFlags: NIF_ICON | NIF_TIP | NIF_MESSAGE | NIF_SHOWTIP,
        uCallbackMessage: WM_TRAY,
        hIcon: icon,
        ..Default::default()
    };
    for (slot, unit) in data.szTip.iter_mut().zip("Lumenna".encode_utf16()) {
        *slot = unit;
    }
    data
}

/// Puts the icon in the notification area. Called again when Explorer restarts.
pub fn add_icon(main: HWND, icon: HICON) {
    let mut data = icon_data(main, icon);
    unsafe {
        let _ = Shell_NotifyIconW(NIM_ADD, &data);
        data.Anonymous.uVersion = NOTIFYICON_VERSION_4;
        let _ = Shell_NotifyIconW(NIM_SETVERSION, &data);
    }
}

pub fn remove_icon(main: HWND) {
    let data = icon_data(main, HICON::default());
    unsafe {
        let _ = Shell_NotifyIconW(NIM_DELETE, &data);
    }
}

/// The message Explorer broadcasts when the taskbar is made again, after which the icon
/// has to be added again.
pub fn taskbar_created() -> u32 {
    unsafe { RegisterWindowMessageW(w!("TaskbarCreated")) }
}

/// What every main window's class begins with, whatever its profile — how one copy of
/// Lumenna knows another is open.
pub const CLASS_PREFIX: &str = "LumennaMain-";

/// Held for as long as this is the profile's running instance.
pub struct Instance(#[allow(dead_code)] HANDLE);

/// The window class for this profile's main window, which is how a second instance finds
/// the first. One per profile, so two profiles can run side by side.
pub fn class_name(profile: &std::path::Path) -> String {
    format!("{CLASS_PREFIX}{:016x}", fnv(&profile.display().to_string().to_lowercase()))
}

/// Becomes the profile's running instance, or — if there already is one — shows it and
/// returns `None`.
pub fn claim(profile: &std::path::Path) -> Option<Instance> {
    let name = format!("Local\\Lumenna-{:016x}", fnv(&profile.display().to_string().to_lowercase()));
    let mutex = unsafe { CreateMutexW(None, true, &HSTRING::from(name)) }.ok()?;
    if unsafe { GetLastError() } != ERROR_ALREADY_EXISTS {
        return Some(Instance(mutex));
    }
    unsafe {
        if let Ok(running) = FindWindowW(&HSTRING::from(class_name(profile)), None) {
            let mut process = 0;
            GetWindowThreadProcessId(running, Some(&mut process));
            // This process was just started by the person, so it may hand the foreground on.
            let _ = AllowSetForegroundWindow(process);
            let _ = PostMessageW(Some(running), WM_SHOW_RUNNING, WPARAM(0), LPARAM(0));
        }
    }
    None
}

/// FNV-1a, for a short stable name from a path.
fn fnv(text: &str) -> u64 {
    text.bytes().fold(0xcbf2_9ce4_8422_2325, |hash, byte| (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3))
}
