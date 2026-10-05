//! Modal dialogs from templates built in memory.
//!
//! The dialog manager is Windows' own: Tab and Shift-Tab, Enter for the default button,
//! Escape to cancel, Alt and a mnemonic to reach a field, and a static label naming the field
//! after it — all of which screen readers have handled for thirty years. Building the
//! template here rather than in a `.rc` file keeps the app free of a resource compiler, and
//! keeps each dialog's layout beside the code that runs it.

use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    DLGTEMPLATE, DS_CENTER, DS_MODALFRAME, DS_SETFONT, DialogBoxIndirectParamW, EndDialog,
    GWLP_USERDATA, GetDlgItem, GetWindowLongPtrW, IDCANCEL, IDOK, SetWindowLongPtrW,
    WM_COMMAND, WM_INITDIALOG, WS_CAPTION, WS_CHILD, WS_POPUP, WS_SYSMENU, WS_VISIBLE,
};

use super::controls;

/// The stock classes a dialog item can be.
#[derive(Clone, Copy)]
pub enum Class {
    Button = 0x80,
    Edit = 0x81,
    Static = 0x82,
    ListBox = 0x83,
    ComboBox = 0x85,
}

/// A dialog template: the dialog, then its items in tab order. Sizes are dialog units, which
/// scale with the dialog's font.
pub struct Template {
    words: Vec<u16>,
    count: u16,
}

/// Where the item count sits in the header, to be filled in last.
const COUNT_AT: usize = 4;
/// Where the width and height sit, after the count and the position.
const SIZE_AT: usize = 7;

impl Template {
    pub fn new(title: &str, width: i16, height: i16) -> Self {
        let mut template = Self { words: Vec::new(), count: 0 };
        let style = WS_POPUP.0 | WS_CAPTION.0 | WS_SYSMENU.0 | (DS_MODALFRAME | DS_SETFONT | DS_CENTER) as u32;
        template.dword(style);
        template.dword(0);
        template.words.push(0); // the item count
        for value in [0, 0, width, height] {
            template.words.push(value as u16);
        }
        template.words.push(0); // no menu
        template.words.push(0); // the standard dialog class
        template.string(title);
        // The shell's font, at the size dialogs use.
        template.words.push(9);
        template.string("Segoe UI");
        template
    }

    /// Adds an item. `style` gets `WS_CHILD | WS_VISIBLE` added.
    #[allow(clippy::too_many_arguments)]
    pub fn item(mut self, class: Class, text: &str, id: u16, style: u32, x: i16, y: i16, width: i16, height: i16) -> Self {
        if self.words.len() % 2 == 1 {
            self.words.push(0); // each item starts on a four-byte boundary
        }
        self.dword(WS_CHILD.0 | WS_VISIBLE.0 | style);
        self.dword(0);
        for value in [x, y, width, height] {
            self.words.push(value as u16);
        }
        self.words.push(id);
        self.words.push(0xFFFF);
        self.words.push(class as u16);
        self.string(text);
        self.words.push(0); // no creation data
        self.count += 1;
        self
    }

    /// Sets the dialog's size, for one whose height depends on which items it has.
    pub fn resize(mut self, width: i16, height: i16) -> Self {
        self.words[SIZE_AT] = width as u16;
        self.words[SIZE_AT + 1] = height as u16;
        self
    }

    fn dword(&mut self, value: u32) {
        self.words.push((value & 0xFFFF) as u16);
        self.words.push((value >> 16) as u16);
    }

    fn string(&mut self, text: &str) {
        self.words.extend(text.encode_utf16());
        self.words.push(0);
    }

    /// The template, four-byte aligned as `DialogBoxIndirectParamW` requires.
    fn build(&self) -> Vec<u32> {
        let mut words = self.words.clone();
        words[COUNT_AT] = self.count;
        if words.len() % 2 == 1 {
            words.push(0);
        }
        words.chunks(2).map(|pair| u32::from(pair[0]) | (u32::from(pair[1]) << 16)).collect()
    }
}

/// What a dialog does.
pub trait Dialog {
    fn template(&self) -> Template;

    /// Fills the dialog in once its controls exist. Returns whether it placed the focus
    /// itself; otherwise the first field gets it.
    fn init(&self, _hwnd: HWND) -> bool {
        false
    }

    /// A control's command. `Some` closes the dialog with that result.
    fn command(&self, _hwnd: HWND, id: u16, _code: u16) -> Option<isize> {
        match i32::from(id) {
            id if id == IDOK.0 => Some(1),
            id if id == IDCANCEL.0 => Some(0),
            _ => None,
        }
    }
}

/// Runs a dialog until it closes, and returns what it closed with.
pub fn run(owner: Option<HWND>, dialog: &dyn Dialog) -> isize {
    let template = dialog.template().build();
    // The dialog procedure gets a pointer to this reference, which lives until this returns,
    // and the dialog cannot outlive the call.
    let reference: &dyn Dialog = dialog;
    let pointer = &raw const reference;
    unsafe {
        DialogBoxIndirectParamW(
            Some(controls::instance()),
            template.as_ptr().cast::<DLGTEMPLATE>(),
            owner,
            Some(procedure),
            LPARAM(pointer as isize),
        )
    }
}

/// A control in a dialog, by identifier.
pub fn item(hwnd: HWND, id: u16) -> HWND {
    unsafe { GetDlgItem(Some(hwnd), i32::from(id)).unwrap_or_default() }
}

unsafe extern "system" fn procedure(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> isize {
    unsafe {
        if message == WM_INITDIALOG {
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, lparam.0);
        }
        let pointer = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const &dyn Dialog;
        if pointer.is_null() {
            return 0;
        }
        let dialog = *pointer;
        match message {
            WM_INITDIALOG => isize::from(!dialog.init(hwnd)),
            WM_COMMAND => {
                let id = controls::low_word(wparam.0);
                let code = controls::high_word(wparam.0);
                if let Some(result) = dialog.command(hwnd, id, code) {
                    let _ = EndDialog(hwnd, result);
                }
                1
            }
            _ => 0,
        }
    }
}
