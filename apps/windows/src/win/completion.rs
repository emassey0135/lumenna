//! Offering what could be typed next in a quick-add or filter field.
//!
//! Down arrow — or Ctrl+Space, as in an IDE — asks the core what fits at the cursor and opens
//! the candidates as a popup menu at the caret. Tab is never taken: it is how a screen reader
//! user leaves the field. Nothing opens by itself after a `#` or `@`, since typing a
//! name straight through is common and a popup would interrupt it.
//!
//! A menu rather than a combobox: a stock control, read with its
//! position and count, with focus back in the field after it. What has been typed narrows it
//! before it opens. Each item leads with its name, so its first letter finds it, and the
//! first is highlighted as the menu opens, so it is read at once.

use std::cell::Cell;
use std::sync::Arc;

use lumenna_surface::{Lumenna, Syntax};
use windows::Win32::Foundation::{HWND, LPARAM, POINT, WPARAM};
use windows::Win32::Graphics::Gdi::ClientToScreen;
use windows::Win32::UI::Controls::{EM_GETSEL, EM_POSFROMCHAR, EM_REPLACESEL, EM_SETSEL};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, MAPVK_VK_TO_VSC, MapVirtualKeyW, VK_CONTROL, VK_DOWN, VK_MENU, VK_SHIFT, VK_SPACE,
};
use windows::Win32::UI::Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, DestroyMenu, PostMessageW,
    MF_GRAYED, MF_STRING, TPM_LEFTALIGN, TPM_RETURNCMD, TPM_TOPALIGN, TrackPopupMenuEx, WM_CHAR,
    WM_KEYDOWN, WM_NCDESTROY,
};
use windows::core::HSTRING;

use super::controls;
use crate::speech;

/// A reserved bit of a key message's data, marking the Down queued to highlight a menu's first
/// item. Should the menu not take it, the field must not either: it would open the menu again,
/// which would queue another.
const QUEUED: isize = 1 << 25;

struct Completer {
    lumenna: Arc<Lumenna>,
    syntax: Syntax,
    /// Ctrl+Space also sends a space as a character, which must not be typed.
    swallow_space: Cell<bool>,
}

const SUBCLASS: usize = 0x4C75_6D31;

/// Makes an edit control offer completions.
pub fn attach(edit: HWND, lumenna: Arc<Lumenna>, syntax: Syntax) {
    let completer = Box::new(Completer { lumenna, syntax, swallow_space: Cell::new(false) });
    unsafe {
        let _ = SetWindowSubclass(edit, Some(procedure), SUBCLASS, Box::into_raw(completer) as usize);
    }
}

fn pressed(key: windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY) -> bool {
    unsafe { GetKeyState(i32::from(key.0)) < 0 }
}

unsafe extern "system" fn procedure(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    id: usize,
    data: usize,
) -> windows::Win32::Foundation::LRESULT {
    unsafe {
        let completer = &*(data as *const Completer);
        match message {
            WM_KEYDOWN if lparam.0 & QUEUED != 0 => return windows::Win32::Foundation::LRESULT(0),
            WM_KEYDOWN => {
                let key = wparam.0 as u16;
                let control = pressed(VK_CONTROL);
                let plain = !control && !pressed(VK_MENU) && !pressed(VK_SHIFT);
                if (key == VK_DOWN.0 && plain) || (key == VK_SPACE.0 && control && !pressed(VK_MENU)) {
                    completer.swallow_space.set(key == VK_SPACE.0);
                    offer(hwnd, completer);
                    return windows::Win32::Foundation::LRESULT(0);
                }
            }
            WM_CHAR if completer.swallow_space.replace(false) && wparam.0 == usize::from(b' ') => {
                return windows::Win32::Foundation::LRESULT(0);
            }
            WM_NCDESTROY => {
                let _ = RemoveWindowSubclass(hwnd, Some(procedure), id);
                drop(Box::from_raw(data as *mut Completer));
            }
            _ => {}
        }
        DefSubclassProc(hwnd, message, wparam, lparam)
    }
}

/// Asks what fits at the cursor and offers it.
fn offer(edit: HWND, completer: &Completer) {
    let text = controls::text(edit);
    let mut start = 0u32;
    let mut end = 0u32;
    controls::send(edit, EM_GETSEL, (&raw mut start) as usize, (&raw mut end) as isize);
    let cursor = controls::bytes_at(&text, end as usize);
    let Ok(found) = completer.lumenna.complete_text(&text, cursor as u32, completer.syntax) else {
        return;
    };

    let chosen = unsafe {
        let Ok(menu) = CreatePopupMenu() else { return };
        if found.candidates.is_empty() {
            // Said as a menu too, so the answer comes the same way either way.
            let _ = AppendMenuW(menu, MF_STRING | MF_GRAYED, 1, &HSTRING::from(menu_text(&found.announcement)));
        }
        for (index, candidate) in found.candidates.iter().enumerate() {
            let _ = AppendMenuW(menu, MF_STRING, index + 1, &HSTRING::from(menu_text(&speech::candidate(candidate))));
        }
        let point = caret_point(edit, &text, end as usize);
        // The first item highlighted as the menu opens, so it is read at once rather than
        // after a first arrow. A menu has no option for that; a Down key queued now is the
        // first thing the menu's own loop takes.
        let scan = MapVirtualKeyW(u32::from(VK_DOWN.0), MAPVK_VK_TO_VSC) as isize;
        let _ = PostMessageW(Some(edit), WM_KEYDOWN, WPARAM(usize::from(VK_DOWN.0)), LPARAM(1 | (scan << 16) | QUEUED));
        let chosen = TrackPopupMenuEx(menu, (TPM_RETURNCMD | TPM_LEFTALIGN | TPM_TOPALIGN).0, point.x, point.y, edit, None);
        let _ = DestroyMenu(menu);
        chosen.0 as usize
    };
    let Some(candidate) = chosen.checked_sub(1).and_then(|index| found.candidates.get(index)) else {
        return;
    };
    // Replaced as a selection, so the edit control's own undo takes it back and the parent
    // hears of the change as if it had been typed.
    let from = controls::units_at(&text, found.start as usize);
    let to = controls::units_at(&text, found.end as usize);
    controls::send(edit, EM_SETSEL, from, to as isize);
    let inserted = HSTRING::from(candidate.text.as_str());
    controls::send(edit, EM_REPLACESEL, 1, inserted.as_ptr() as isize);
}

/// Where the caret is, on screen, for the menu to open beside it.
fn caret_point(edit: HWND, text: &str, units: usize) -> POINT {
    // The position of the character before the caret; after the last one there is none.
    let at = units.saturating_sub(1);
    let packed = controls::send(edit, EM_POSFROMCHAR, at, 0) as usize;
    let mut point = if text.is_empty() || packed == usize::MAX {
        POINT::default()
    } else {
        POINT { x: i32::from(controls::low_word(packed) as i16), y: i32::from(controls::high_word(packed) as i16) }
    };
    point.y += 20;
    unsafe {
        let _ = ClientToScreen(edit, &mut point);
    }
    point
}

/// Menu text with ampersands doubled, so none is taken for a mnemonic.
fn menu_text(text: &str) -> String {
    text.replace('&', "&&")
}
