//! Small helpers over Win32 windows and controls.

use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::InvalidateRect;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{EnableWindow, GetFocus, SetFocus};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, GetWindowTextLengthW, GetWindowTextW, HMENU, IsChild, IsWindowVisible,
    MoveWindow, SW_HIDE, SW_SHOW, SendMessageW, SetWindowTextW, ShowWindow, WINDOW_EX_STYLE,
    WINDOW_STYLE, WS_CHILD, WS_VISIBLE,
};
use windows::core::{HSTRING, PCWSTR};

/// The module's own instance.
pub fn instance() -> HINSTANCE {
    unsafe { GetModuleHandleW(None).map(Into::into).unwrap_or_default() }
}

/// Creates a visible child control.
pub fn create(parent: HWND, class: PCWSTR, text: &str, style: u32, ex: u32, id: u16) -> HWND {
    unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE(ex),
            class,
            &HSTRING::from(text),
            WINDOW_STYLE(WS_CHILD.0 | WS_VISIBLE.0 | style),
            0,
            0,
            0,
            0,
            Some(parent),
            Some(HMENU(usize::from(id) as _)),
            Some(instance()),
            None,
        )
        .unwrap_or_default()
    }
}

/// Sends a message and returns its result.
pub fn send(hwnd: HWND, message: u32, wparam: usize, lparam: isize) -> isize {
    unsafe { SendMessageW(hwnd, message, Some(WPARAM(wparam)), Some(LPARAM(lparam))).0 }
}

/// A window's text.
pub fn text(hwnd: HWND) -> String {
    unsafe {
        let length = GetWindowTextLengthW(hwnd);
        if length <= 0 {
            return String::new();
        }
        let mut buffer = vec![0u16; length as usize + 1];
        let copied = GetWindowTextW(hwnd, &mut buffer);
        String::from_utf16_lossy(&buffer[..copied.max(0) as usize])
    }
}

/// Sets a window's text, if it differs — setting the same text still raises a name change,
/// which a screen reader may read again.
pub fn set_text(hwnd: HWND, value: &str) {
    if text(hwnd) != value {
        unsafe {
            let _ = SetWindowTextW(hwnd, &HSTRING::from(value));
        }
    }
}


pub fn show(hwnd: HWND, visible: bool) {
    unsafe {
        let _ = ShowWindow(hwnd, if visible { SW_SHOW } else { SW_HIDE });
    }
}

pub fn is_visible(hwnd: HWND) -> bool {
    unsafe { IsWindowVisible(hwnd).as_bool() }
}

pub fn enable(hwnd: HWND, enabled: bool) {
    unsafe {
        let _ = EnableWindow(hwnd, enabled);
    }
}

pub fn place(hwnd: HWND, rect: RECT) {
    unsafe {
        let _ = MoveWindow(hwnd, rect.left, rect.top, rect.right - rect.left, rect.bottom - rect.top, true);
        let _ = InvalidateRect(Some(hwnd), None, true);
    }
}

pub fn focus(hwnd: HWND) {
    unsafe {
        let _ = SetFocus(Some(hwnd));
    }
}

pub fn focused() -> HWND {
    unsafe { GetFocus() }
}

/// Whether `hwnd` is `ancestor` or inside it.
pub fn within(ancestor: HWND, hwnd: HWND) -> bool {
    !hwnd.is_invalid() && (hwnd == ancestor || unsafe { IsChild(ancestor, hwnd).as_bool() })
}

pub const fn low_word(value: usize) -> u16 {
    (value & 0xFFFF) as u16
}

pub const fn high_word(value: usize) -> u16 {
    ((value >> 16) & 0xFFFF) as u16
}

/// A rectangle from its origin and size.
pub const fn rect(x: i32, y: i32, width: i32, height: i32) -> RECT {
    RECT { left: x, top: y, right: x + width, bottom: y + height }
}

/// A UTF-16 offset — what an edit control counts — as the UTF-8 byte offset the core counts
/// in, never splitting a character.
pub fn bytes_at(text: &str, units: usize) -> usize {
    let mut counted = 0;
    for (byte, character) in text.char_indices() {
        if counted >= units {
            return byte;
        }
        counted += character.len_utf16();
    }
    text.len()
}

/// A UTF-8 byte offset from the core as the UTF-16 offset an edit control takes.
pub fn units_at(text: &str, bytes: usize) -> usize {
    let mut end = bytes.min(text.len());
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].encode_utf16().count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offsets_convert_both_ways_across_characters_of_every_width() {
        let text = "é📅 #Work";
        // é is one unit and two bytes; 📅 is two units and four bytes.
        assert_eq!(bytes_at(text, 1), 2);
        assert_eq!(bytes_at(text, 3), 6);
        assert_eq!(units_at(text, 6), 3);
        assert_eq!(units_at(text, text.len()), text.encode_utf16().count());
        assert_eq!(units_at(text, 4), 1, "an offset inside a character goes back to its start");
    }
}
