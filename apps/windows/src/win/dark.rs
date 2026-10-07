//! Dark mode, when Windows' app mode is dark (Settings, Personalisation, Colours) — and never
//! under a high-contrast theme, which takes precedence and already works, since every control
//! is stock and draws in system colours.
//!
//! Win32 has no dark mode of its own. What is used:
//!
//! - **Title bars**: `DWMWA_USE_IMMERSIVE_DARK_MODE`, documented.
//! - **Menus**: uxtheme's `SetPreferredAppMode` and `FlushMenuThemes` (ordinals 135 and 136),
//!   undocumented but what Explorer and Notepad use. The menu bar itself stays light: only
//!   drawing it by hand would change that.
//! - **Trees, lists, fields, combo boxes and push buttons**: the `DarkMode_Explorer` and
//!   `DarkMode_CFD` visual styles, and the colours given through `WM_CTLCOLOR*`.
//! - **Check boxes and group boxes** draw unthemed in dark mode: their themed text ignores the
//!   colour given and stays black, which cannot be read on a dark background. Unthemed, they
//!   take the colours.
//!
//! Left light, being legible as they are and impossible to make dark cleanly: the property
//! sheet's tab strip, the hotkey control, task dialogs and message boxes.
//!
//! Every pair of colours meets WCAG AA: white on the background is 16:1, on a field 14:1, and
//! disabled text 6.2:1 and 5.4:1.
//!
//! `LUMENNA_DARK` set to 1 or 0 overrides the system's choice, for the tests, which check the
//! app in dark mode without changing the person's own setting. A high-contrast theme still wins.

use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};

use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Dwm::{DWMWA_USE_IMMERSIVE_DARK_MODE, DwmSetWindowAttribute};
use windows::Win32::Graphics::Gdi::{
    CreateSolidBrush, HBRUSH, HDC, RDW_ALLCHILDREN, RDW_ERASE, RDW_FRAME, RDW_INVALIDATE, RedrawWindow, SetBkColor,
    SetTextColor,
};
use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
use windows::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_DWORD, RegGetValueW};
use windows::Win32::UI::Accessibility::{HCF_HIGHCONTRASTON, HIGHCONTRASTW};
use windows::Win32::UI::Controls::{SetWindowTheme, TVM_SETBKCOLOR, TVM_SETLINECOLOR, TVM_SETTEXTCOLOR};
use windows::Win32::UI::Shell::{RemoveWindowSubclass, SetWindowSubclass};
use windows::Win32::UI::Input::KeyboardAndMouse::IsWindowEnabled;
use windows::Win32::UI::WindowsAndMessaging::{
    BS_TYPEMASK, ES_MULTILINE, EnumChildWindows, GWL_STYLE, GetClassNameW, GetWindowLongW,
    SPI_GETHIGHCONTRAST, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, SystemParametersInfoW, WM_CTLCOLORBTN, WM_CTLCOLORDLG,
    WM_CTLCOLOREDIT, WM_CTLCOLORLISTBOX, WM_CTLCOLORSTATIC,
};
use windows::core::{PCSTR, PCWSTR, w};

use super::controls;

/// The window and dialog background.
const BACKGROUND: COLORREF = rgb(0x20, 0x20, 0x20);
/// A field's background, a step lighter.
const FIELD: COLORREF = rgb(0x2B, 0x2B, 0x2B);
const TEXT: COLORREF = rgb(0xFF, 0xFF, 0xFF);
/// Disabled text.
const QUIET: COLORREF = rgb(0xA0, 0xA0, 0xA0);
/// A tree's lines, which are not text.
const LINES: COLORREF = rgb(0x80, 0x80, 0x80);
/// A field's border: 4.8:1 on the background and 4.2:1 on the field, where the theme's own
/// is 1.4:1 and a field could not be told from the window around it.
const BORDER: COLORREF = rgb(0x8B, 0x8B, 0x8B);

const fn rgb(r: u8, g: u8, b: u8) -> COLORREF {
    COLORREF(r as u32 | (g as u32) << 8 | (b as u32) << 16)
}

static ON: AtomicBool = AtomicBool::new(false);

/// Whether the app is drawn dark now.
pub fn on() -> bool {
    ON.load(Ordering::Relaxed)
}

fn high_contrast() -> bool {
    let mut contrast = HIGHCONTRASTW { cbSize: size_of::<HIGHCONTRASTW>() as u32, ..Default::default() };
    unsafe {
        let _ = SystemParametersInfoW(
            SPI_GETHIGHCONTRAST,
            contrast.cbSize,
            Some((&raw mut contrast).cast()),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        );
    }
    contrast.dwFlags.contains(HCF_HIGHCONTRASTON)
}

/// Whether Windows wants dark apps: `AppsUseLightTheme` is 0.
fn system_dark() -> bool {
    let mut value = 1u32;
    let mut size = size_of::<u32>() as u32;
    let read = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            w!(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize"),
            w!("AppsUseLightTheme"),
            RRF_RT_REG_DWORD,
            None,
            Some((&raw mut value).cast()),
            Some(&mut size),
        )
    };
    read.is_ok() && value == 0
}

fn wanted() -> bool {
    if high_contrast() {
        return false;
    }
    match std::env::var("LUMENNA_DARK").as_deref() {
        Ok("1") => true,
        Ok("0") => false,
        _ => system_dark(),
    }
}

/// Reads the system's choice again, and sets menus to it; true if it changed.
pub fn refresh() -> bool {
    let dark = wanted();
    let changed = ON.swap(dark, Ordering::Relaxed) != dark;
    app_mode(dark);
    changed
}

/// Menus, through uxtheme's undocumented `SetPreferredAppMode`: force dark, or force light.
fn app_mode(dark: bool) {
    type SetPreferredAppMode = unsafe extern "system" fn(i32) -> i32;
    type FlushMenuThemes = unsafe extern "system" fn();
    unsafe {
        let Ok(uxtheme) = LoadLibraryW(w!("uxtheme.dll")) else { return };
        let set = GetProcAddress(uxtheme, PCSTR(135 as *const u8));
        let flush = GetProcAddress(uxtheme, PCSTR(136 as *const u8));
        if let (Some(set), Some(flush)) = (set, flush) {
            let set: SetPreferredAppMode = std::mem::transmute(set);
            let flush: FlushMenuThemes = std::mem::transmute(flush);
            // ForceDark is 2, ForceLight 3.
            set(if dark { 2 } else { 3 });
            flush();
        }
    }
}

/// A top-level window or dialog: its title bar, and every control in it.
pub fn window(hwnd: HWND) {
    let dark = i32::from(on());
    unsafe {
        let _ = DwmSetWindowAttribute(hwnd, DWMWA_USE_IMMERSIVE_DARK_MODE, (&raw const dark).cast(), size_of::<i32>() as u32);
    }
    controls_in(hwnd);
}

/// Every control under `parent`, themed for the mode, then everything redrawn.
pub fn controls_in(parent: HWND) {
    unsafe extern "system" fn visit(hwnd: HWND, _: LPARAM) -> windows::core::BOOL {
        control(hwnd);
        true.into()
    }
    unsafe {
        let _ = EnumChildWindows(Some(parent), Some(visit), LPARAM(0));
        let _ = RedrawWindow(Some(parent), None, None, RDW_ERASE | RDW_INVALIDATE | RDW_FRAME | RDW_ALLCHILDREN);
    }
}

fn control(hwnd: HWND) {
    let mut name = [0u16; 64];
    let length = unsafe { GetClassNameW(hwnd, &mut name) };
    let class = String::from_utf16_lossy(&name[..usize::try_from(length).unwrap_or(0)]);
    let dark = on();
    // `None` restores the class's own theme.
    let theme = |theme: Option<PCWSTR>, list: Option<PCWSTR>| unsafe {
        let _ = SetWindowTheme(hwnd, theme.unwrap_or(PCWSTR::null()), list.unwrap_or(PCWSTR::null()));
    };
    match class.as_str() {
        "SysTreeView32" => {
            // Light, the tree keeps the Explorer style it was made with (`tree.rs`).
            theme(Some(if dark { w!("DarkMode_Explorer") } else { w!("Explorer") }), None);
            let colour = |message, value: COLORREF| {
                controls::send(hwnd, message, 0, if dark { value.0 as isize } else { -1 });
            };
            colour(TVM_SETBKCOLOR, BACKGROUND);
            colour(TVM_SETTEXTCOLOR, TEXT);
            colour(TVM_SETLINECOLOR, LINES);
        }
        "Edit" => {
            // A multi-line field's scroll bar is dark only in the Explorer style.
            let multiline = unsafe { GetWindowLongW(hwnd, GWL_STYLE) } as u32 & ES_MULTILINE as u32 != 0;
            theme(dark.then(|| if multiline { w!("DarkMode_Explorer") } else { w!("DarkMode_CFD") }), None);
            unsafe {
                if dark {
                    let _ = SetWindowSubclass(hwnd, Some(field_border), 2, 0);
                } else {
                    let _ = RemoveWindowSubclass(hwnd, Some(field_border), 2);
                }
                let _ = RedrawWindow(Some(hwnd), None, None, RDW_FRAME | RDW_INVALIDATE);
            }
        }
        "ComboBox" => theme(dark.then(|| w!("DarkMode_CFD")), None),
        "ListBox" => theme(dark.then(|| w!("DarkMode_Explorer")), None),
        "Button" => {
            let kind = unsafe { GetWindowLongW(hwnd, GWL_STYLE) } as u32 & BS_TYPEMASK as u32;
            // Check boxes, radio buttons, three-state boxes and group boxes: styles 2 to 7,
            // and 9 for an automatic radio button. 0 and 1 are push buttons.
            if matches!(kind, 2..=7 | 9) {
                // Themed, their text stays black whatever colour is given: unthemed, they
                // take it.
                if dark {
                    theme(Some(w!("")), Some(w!("")));
                } else {
                    theme(None, None);
                }
            } else {
                theme(dark.then(|| w!("DarkMode_Explorer")), None);
            }
        }
        _ => {}
    }
}

fn brushes() -> &'static (isize, isize) {
    static BRUSHES: OnceLock<(isize, isize)> = OnceLock::new();
    BRUSHES.get_or_init(|| unsafe { (CreateSolidBrush(BACKGROUND).0 as isize, CreateSolidBrush(FIELD).0 as isize) })
}

/// The window background brush, while dark.
pub fn background() -> Option<HBRUSH> {
    on().then(|| HBRUSH(brushes().0 as _))
}

/// Colours for a control asking through `WM_CTLCOLOR*`, while dark: the brush to answer with.
pub fn colour(message: u32, wparam: WPARAM, lparam: LPARAM) -> Option<LRESULT> {
    if !on() {
        return None;
    }
    let dc = HDC(wparam.0 as _);
    let control = HWND(lparam.0 as _);
    let enabled = unsafe { IsWindowEnabled(control) }.as_bool();
    let (back, brush) = match message {
        WM_CTLCOLOREDIT | WM_CTLCOLORLISTBOX => (FIELD, brushes().1),
        WM_CTLCOLORSTATIC | WM_CTLCOLORBTN | WM_CTLCOLORDLG => (BACKGROUND, brushes().0),
        _ => return None,
    };
    unsafe {
        SetTextColor(dc, if enabled { TEXT } else { QUIET });
        SetBkColor(dc, back);
    }
    Some(LRESULT(brush))
}

/// Paints a window's background dark, answering `WM_ERASEBKGND`, while dark.
pub fn erase(hwnd: HWND, wparam: WPARAM) -> Option<LRESULT> {
    let brush = background()?;
    let mut area = windows::Win32::Foundation::RECT::default();
    unsafe {
        let _ = windows::Win32::UI::WindowsAndMessaging::GetClientRect(hwnd, &mut area);
        windows::Win32::Graphics::Gdi::FillRect(HDC(wparam.0 as _), &area, brush);
    }
    Some(LRESULT(1))
}

/// Answers a control's colours or the background for a window whose own procedure would
/// not: a property sheet's frame, subclassed when the sheet starts.
pub unsafe extern "system" fn sheet_colours(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _id: usize,
    _data: usize,
) -> LRESULT {
    let answered = match message {
        WM_CTLCOLORSTATIC | WM_CTLCOLORBTN | WM_CTLCOLORDLG | WM_CTLCOLOREDIT | WM_CTLCOLORLISTBOX => {
            colour(message, wparam, lparam)
        }
        windows::Win32::UI::WindowsAndMessaging::WM_ERASEBKGND => erase(hwnd, wparam),
        _ => None,
    };
    answered.unwrap_or_else(|| unsafe { windows::Win32::UI::Shell::DefSubclassProc(hwnd, message, wparam, lparam) })
}

/// Draws a field's border over the theme's, while dark: white when it has focus, so focus
/// shows, and otherwise light grey.
unsafe extern "system" fn field_border(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM, _id: usize, _data: usize) -> LRESULT {
    use windows::Win32::Graphics::Gdi::{FrameRect, GetWindowDC, ReleaseDC};
    use windows::Win32::UI::Input::KeyboardAndMouse::GetFocus;
    use windows::Win32::UI::Shell::DefSubclassProc;
    use windows::Win32::UI::WindowsAndMessaging::{GWL_EXSTYLE, GetWindowRect, WM_KILLFOCUS, WM_NCPAINT, WM_SETFOCUS, WS_EX_CLIENTEDGE};

    let result = unsafe { DefSubclassProc(hwnd, message, wparam, lparam) };
    match message {
        WM_NCPAINT if on() => unsafe {
            let mut place = windows::Win32::Foundation::RECT::default();
            let _ = GetWindowRect(hwnd, &mut place);
            let edge = windows::Win32::Foundation::RECT { left: 0, top: 0, right: place.right - place.left, bottom: place.bottom - place.top };
            let dc = GetWindowDC(Some(hwnd));
            // A client edge is two pixels, whose inner one the theme draws as it likes: white
            // around a multi-line field, as if it had focus. Focus thickens the border to both.
            let focused = GetFocus() == hwnd;
            let outer = CreateSolidBrush(if focused { TEXT } else { BORDER });
            FrameRect(dc, &edge, outer);
            let extended = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;
            if extended & WS_EX_CLIENTEDGE.0 != 0 {
                let inner = CreateSolidBrush(if focused { TEXT } else { FIELD });
                let inside = windows::Win32::Foundation::RECT { left: 1, top: 1, right: edge.right - 1, bottom: edge.bottom - 1 };
                FrameRect(dc, &inside, inner);
                let _ = windows::Win32::Graphics::Gdi::DeleteObject(inner.into());
            }
            let _ = windows::Win32::Graphics::Gdi::DeleteObject(outer.into());
            ReleaseDC(Some(hwnd), dc);
        },
        // Focus changes the border's colour.
        WM_SETFOCUS | WM_KILLFOCUS => unsafe {
            let _ = RedrawWindow(Some(hwnd), None, None, RDW_FRAME | RDW_INVALIDATE);
        },
        _ => {}
    }
    result
}
