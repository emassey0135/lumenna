//! Adding a task the way it would be said: one line, read back as it is typed.
//!
//! The readback is what a sighted user gets from inline highlighting: what will be saved,
//! with the date resolved. It is a read-only field after the line, not spoken as it changes —
//! speech on every keystroke buries the typing — but Tab reaches it, and it is what is
//! announced when the task is added.

use std::cell::RefCell;
use std::sync::Arc;

use lumenna_surface::{Change, Lumenna, Syntax};
use windows::Win32::Foundation::HWND;
use windows::Win32::System::SystemServices::SS_NOPREFIX;
use windows::Win32::UI::Controls::EM_SETSEL;
use windows::Win32::UI::WindowsAndMessaging::{
    BS_DEFPUSHBUTTON, BS_PUSHBUTTON, EN_CHANGE, ES_AUTOHSCROLL, ES_AUTOVSCROLL,
    ES_MULTILINE, ES_READONLY, IDCANCEL, IDOK, SetForegroundWindow, WS_BORDER,
    WS_TABSTOP, WS_VSCROLL,
};

use super::controls;
use super::core::sentence;
use super::dialog::{self, Class, Dialog, Template};
use super::{completion, prompts};

const LINE: u16 = 100;
const READBACK: u16 = 101;

struct QuickAdd {
    lumenna: Arc<Lumenna>,
    initial: String,
    added: RefCell<Option<Change>>,
}

impl QuickAdd {
    fn preview(&self, hwnd: HWND) {
        let text = controls::text(dialog::item(hwnd, LINE));
        let readback = if text.trim().is_empty() {
            String::new()
        } else {
            match self.lumenna.preview_task(&text) {
                // Everything worth saying before confirming, errors included: there is no
                // squiggle under the text, so this is the only channel.
                Ok(preview) => std::iter::once(preview.announcement)
                    .chain(preview.diagnostics.into_iter().map(|d| d.message))
                    .collect::<Vec<_>>()
                    .join(". "),
                Err(error) => sentence(&error),
            }
        };
        controls::set_text(dialog::item(hwnd, READBACK), &readback);
        controls::enable(dialog::item(hwnd, IDOK.0 as u16), !text.trim().is_empty());
    }
}

impl Dialog for QuickAdd {
    fn template(&self) -> Template {
        let readback = (ES_MULTILINE | ES_READONLY | ES_AUTOVSCROLL) as u32 | WS_VSCROLL.0 | WS_BORDER.0 | WS_TABSTOP.0;
        Template::new("New Task", 300, 128)
            .item(Class::Static, "&Task:", u16::MAX, 0, 7, 7, 286, 9)
            .item(Class::Edit, "", LINE, ES_AUTOHSCROLL as u32 | WS_BORDER.0 | WS_TABSTOP.0, 7, 17, 286, 14)
            .item(
                Class::Static,
                "Such as: write the chapter tomorrow p1 #Work. Down arrow offers what could come next.",
                u16::MAX,
                SS_NOPREFIX.0,
                7,
                35,
                286,
                18,
            )
            .item(Class::Static, "&Will add:", u16::MAX, 0, 7, 56, 286, 9)
            .item(Class::Edit, "", READBACK, readback, 7, 66, 286, 36)
            .item(Class::Button, "Add", IDOK.0 as u16, BS_DEFPUSHBUTTON as u32 | WS_TABSTOP.0, 189, 108, 50, 14)
            .item(Class::Button, "Cancel", IDCANCEL.0 as u16, BS_PUSHBUTTON as u32 | WS_TABSTOP.0, 243, 108, 50, 14)
    }

    fn init(&self, hwnd: HWND) -> bool {
        let line = dialog::item(hwnd, LINE);
        completion::attach(line, Arc::clone(&self.lumenna), Syntax::QuickAdd);
        controls::set_text(line, &self.initial);
        // The cursor after the prefix, so typing carries on from `#Work `.
        let end = self.initial.encode_utf16().count();
        controls::send(line, EM_SETSEL, end, end as isize);
        self.preview(hwnd);
        controls::focus(line);
        unsafe {
            // Opened by the global shortcut, the dialog has to come in front of whatever was.
            let _ = SetForegroundWindow(hwnd);
        }
        true
    }

    fn command(&self, hwnd: HWND, id: u16, code: u16) -> Option<isize> {
        match i32::from(id) {
            id if id == i32::from(LINE) && u32::from(code) == EN_CHANGE => {
                self.preview(hwnd);
                None
            }
            id if id == IDOK.0 => {
                let text = controls::text(dialog::item(hwnd, LINE));
                if text.trim().is_empty() {
                    return None;
                }
                match self.lumenna.add_task(&text) {
                    Ok(change) => {
                        *self.added.borrow_mut() = Some(change);
                        Some(1)
                    }
                    Err(error) => {
                        prompts::fail(hwnd, &sentence(&error));
                        None
                    }
                }
            }
            id if id == IDCANCEL.0 => Some(0),
            _ => None,
        }
    }
}

/// Asks for a task in quick-add words, starting from `initial`, and adds it.
pub fn run(owner: Option<HWND>, lumenna: Arc<Lumenna>, initial: &str) -> Option<Change> {
    let dialog = QuickAdd { lumenna, initial: initial.to_owned(), added: RefCell::new(None) };
    dialog::run(owner, &dialog);
    dialog.added.into_inner()
}
