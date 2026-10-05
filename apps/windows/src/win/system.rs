//! What the app asks of Windows beyond its own window: the registry, the clipboard, and the
//! shell's own file dialogs.

use std::path::{Path, PathBuf};

use windows::Win32::Foundation::{HANDLE, HGLOBAL, HWND};
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance, CoTaskMemFree};
use windows::Win32::System::DataExchange::{CloseClipboard, EmptyClipboard, GetClipboardData, OpenClipboard, SetClipboardData};
use windows::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock};
use windows::Win32::System::Ole::CF_UNICODETEXT;
use windows::Win32::System::Registry::{
    HKEY_CURRENT_USER, REG_DWORD, REG_SZ, RRF_RT_REG_DWORD, RRF_RT_REG_SZ, RegDeleteKeyValueW, RegGetValueW,
    RegSetKeyValueW,
};
use windows::Win32::UI::Shell::Common::COMDLG_FILTERSPEC;
use windows::Win32::UI::Shell::{
    FOS_FORCEFILESYSTEM, FOS_OVERWRITEPROMPT, FOS_PICKFOLDERS, FileOpenDialog, FileSaveDialog, IFileDialog,
    IFileOpenDialog, IFileSaveDialog, IShellItem, SHCreateItemFromParsingName, SIGDN_FILESYSPATH,
};
use windows::core::{HSTRING, Interface, PCWSTR};

/// What Windows starts at sign-in.
pub const RUN_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";

pub fn read_u32(key: &str, name: &str) -> Option<u32> {
    let mut value = 0u32;
    let mut size = size_of::<u32>() as u32;
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            &HSTRING::from(key),
            &HSTRING::from(name),
            RRF_RT_REG_DWORD,
            None,
            Some((&raw mut value).cast()),
            Some(&mut size),
        )
    };
    status.is_ok().then_some(value)
}

pub fn write_u32(key: &str, name: &str, value: u32) -> bool {
    let bytes = value.to_le_bytes();
    let status = unsafe {
        RegSetKeyValueW(
            HKEY_CURRENT_USER,
            &HSTRING::from(key),
            &HSTRING::from(name),
            REG_DWORD.0,
            Some(bytes.as_ptr().cast()),
            bytes.len() as u32,
        )
    };
    status.is_ok()
}

pub fn read_string(key: &str, name: &str) -> Option<String> {
    let mut size = 0u32;
    let (key, name) = (HSTRING::from(key), HSTRING::from(name));
    unsafe {
        RegGetValueW(HKEY_CURRENT_USER, &key, &name, RRF_RT_REG_SZ, None, None, Some(&mut size)).ok().ok()?;
        let mut buffer = vec![0u16; size as usize / 2 + 1];
        RegGetValueW(HKEY_CURRENT_USER, &key, &name, RRF_RT_REG_SZ, None, Some(buffer.as_mut_ptr().cast()), Some(&mut size))
            .ok()
            .ok()?;
        let length = buffer.iter().position(|unit| *unit == 0).unwrap_or(buffer.len());
        Some(String::from_utf16_lossy(&buffer[..length]))
    }
}

pub fn write_string(key: &str, name: &str, value: &str) -> bool {
    let wide: Vec<u16> = value.encode_utf16().chain(std::iter::once(0)).collect();
    let status = unsafe {
        RegSetKeyValueW(
            HKEY_CURRENT_USER,
            &HSTRING::from(key),
            &HSTRING::from(name),
            REG_SZ.0,
            Some(wide.as_ptr().cast()),
            (wide.len() * 2) as u32,
        )
    };
    status.is_ok()
}

pub fn delete_value(key: &str, name: &str) {
    unsafe {
        let _ = RegDeleteKeyValueW(HKEY_CURRENT_USER, &HSTRING::from(key), &HSTRING::from(name));
    }
}

/// Puts text on the clipboard.
pub fn copy(owner: HWND, text: &str) -> bool {
    let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        if OpenClipboard(Some(owner)).is_err() {
            return false;
        }
        let _ = EmptyClipboard();
        let copied = GlobalAlloc(GMEM_MOVEABLE, wide.len() * 2).ok().is_some_and(|memory| {
            let target = GlobalLock(memory).cast::<u16>();
            if target.is_null() {
                return false;
            }
            std::ptr::copy_nonoverlapping(wide.as_ptr(), target, wide.len());
            let _ = GlobalUnlock(memory);
            // The clipboard owns the memory from here.
            SetClipboardData(u32::from(CF_UNICODETEXT.0), Some(HANDLE(memory.0))).is_ok()
        });
        let _ = CloseClipboard();
        copied
    }
}

/// The text on the clipboard, if there is any.
pub fn pasted(owner: HWND) -> Option<String> {
    unsafe {
        OpenClipboard(Some(owner)).ok()?;
        let text = GetClipboardData(u32::from(CF_UNICODETEXT.0)).ok().and_then(|handle| {
            let memory = HGLOBAL(handle.0);
            let source = GlobalLock(memory).cast::<u16>();
            if source.is_null() {
                return None;
            }
            let length = (0..).take_while(|i| *source.add(*i) != 0).count();
            let text = String::from_utf16_lossy(std::slice::from_raw_parts(source, length));
            let _ = GlobalUnlock(memory);
            Some(text)
        });
        let _ = CloseClipboard();
        text
    }
}

/// The path a dialog chose.
fn chosen(dialog: &IFileDialog, owner: HWND) -> Option<PathBuf> {
    unsafe {
        // Cancelled is an error here, and so is anything else that stopped it: no path.
        dialog.Show(Some(owner)).ok()?;
        let item: IShellItem = dialog.GetResult().ok()?;
        let name = item.GetDisplayName(SIGDN_FILESYSPATH).ok()?;
        let path = name.to_string().ok();
        CoTaskMemFree(Some(name.0.cast()));
        path.map(PathBuf::from)
    }
}

/// Starts a dialog in `folder`, when there is one.
fn start_in(dialog: &IFileDialog, folder: Option<&Path>) {
    if let Some(folder) = folder {
        unsafe {
            if let Ok(item) = SHCreateItemFromParsingName::<_, _, IShellItem>(&HSTRING::from(folder.as_os_str()), None) {
                let _ = dialog.SetFolder(&item);
            }
        }
    }
}

/// Asks for a folder: where backups go.
pub fn choose_folder(owner: HWND, title: &str, start: Option<&Path>) -> Option<PathBuf> {
    unsafe {
        let dialog: IFileOpenDialog = CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER).ok()?;
        let dialog: IFileDialog = dialog.cast().ok()?;
        let options = dialog.GetOptions().ok()?;
        dialog.SetOptions(options | FOS_PICKFOLDERS | FOS_FORCEFILESYSTEM).ok()?;
        dialog.SetTitle(&HSTRING::from(title)).ok()?;
        start_in(&dialog, start);
        chosen(&dialog, owner)
    }
}

/// Asks for a file to open. `types` are `(name, pattern)`: `("Lumenna exports and backups",
/// "*.json;*.lumbak")`.
pub fn open_file(owner: HWND, title: &str, types: &[(&str, &str)]) -> Option<PathBuf> {
    unsafe {
        let dialog: IFileOpenDialog = CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER).ok()?;
        let dialog: IFileDialog = dialog.cast().ok()?;
        dialog.SetTitle(&HSTRING::from(title)).ok()?;
        set_types(&dialog, types)?;
        chosen(&dialog, owner)
    }
}

/// Asks where to save a file, offering `name`; the dialog asks before replacing one.
pub fn save_file(owner: HWND, title: &str, name: &str, types: &[(&str, &str)]) -> Option<PathBuf> {
    unsafe {
        let dialog: IFileSaveDialog = CoCreateInstance(&FileSaveDialog, None, CLSCTX_INPROC_SERVER).ok()?;
        let dialog: IFileDialog = dialog.cast().ok()?;
        let options = dialog.GetOptions().ok()?;
        dialog.SetOptions(options | FOS_OVERWRITEPROMPT | FOS_FORCEFILESYSTEM).ok()?;
        dialog.SetTitle(&HSTRING::from(title)).ok()?;
        dialog.SetFileName(&HSTRING::from(name)).ok()?;
        set_types(&dialog, types)?;
        chosen(&dialog, owner)
    }
}

fn set_types(dialog: &IFileDialog, types: &[(&str, &str)]) -> Option<()> {
    if types.is_empty() {
        return Some(());
    }
    let strings: Vec<(HSTRING, HSTRING)> = types.iter().map(|(n, p)| (HSTRING::from(*n), HSTRING::from(*p))).collect();
    let specs: Vec<COMDLG_FILTERSPEC> = strings
        .iter()
        .map(|(name, pattern)| COMDLG_FILTERSPEC { pszName: PCWSTR(name.as_ptr()), pszSpec: PCWSTR(pattern.as_ptr()) })
        .collect();
    unsafe { dialog.SetFileTypes(&specs).ok() }
}

/// What this PC is called, to name it to another device when pairing.
pub fn computer_name() -> String {
    std::env::var("COMPUTERNAME").ok().filter(|n| !n.is_empty()).unwrap_or_else(|| "Windows PC".to_owned())
}
