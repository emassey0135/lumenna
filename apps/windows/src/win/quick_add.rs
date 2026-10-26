//! Adding a task the way it would be said: one line, read back as it is typed, drawn from the
//! app's quick-add line (`view.line`): what it will add, and adding it, are the app's.
//!
//! The readback is what a sighted user gets from inline highlighting: what will be saved,
//! with the date resolved. It is a read-only field after the line, not spoken as it changes —
//! speech on every keystroke buries the typing — but Tab reaches it, and it is what is
//! announced when the task is added. A line refused stays as typed, why said once.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use lumenna_app::{LineView, Typing};
use lumenna_ffi::ShellEvent;
use lumenna_records::Syntax;
use lumenna_records::fields::QuickAddWords;
use windows::Win32::Foundation::HWND;
use windows::Win32::System::SystemServices::SS_NOPREFIX;
use windows::Win32::UI::Controls::{EM_GETSEL, EM_SETSEL};
use windows::Win32::UI::WindowsAndMessaging::{
    BS_DEFPUSHBUTTON, BS_PUSHBUTTON, EN_CHANGE, ES_AUTOHSCROLL, ES_AUTOVSCROLL, ES_MULTILINE, ES_READONLY, EndDialog,
    IDCANCEL, IDOK, SetForegroundWindow, WS_BORDER, WS_TABSTOP, WS_VSCROLL,
};

use super::app::{self, App};
use super::dialog::{self, Class, Dialog, Template};
use super::{completion, controls, prompts};
use crate::{devices, speech};

const LINE: u16 = 100;
const READBACK: u16 = 101;

/// Quick add open, while it is.
pub struct QuickAdd {
    hwnd: Cell<HWND>,
    /// Its words, the core's.
    words: QuickAddWords,
    /// What the line starts with: the app's, the place's own (`#Work `).
    initial: String,
    /// The refusal last said, so it is said once.
    refused: RefCell<Option<String>>,
    closed: Cell<bool>,
}

impl QuickAdd {
    /// Draws the app's line as it now is: its readback, and whether it is refused. The app
    /// closing it — the task added — closes the dialog.
    pub fn draw(&self, app: &App, line: Option<&LineView>, refused: Option<&String>) {
        let hwnd = self.hwnd.get();
        if hwnd.is_invalid() || self.closed.get() {
            return;
        }
        let Some(line) = line else {
            self.closed.set(true);
            unsafe {
                let _ = EndDialog(hwnd, 1);
            }
            return;
        };
        Self::read_back(hwnd, line);
        if refused.is_some() && self.refused.borrow().as_ref() != refused {
            *self.refused.borrow_mut() = refused.cloned();
            if let Some(why) = refused.cloned() {
                app.defer(move |_| prompts::fail(hwnd, &speech::sentence(&why)));
            }
        } else if refused.is_none() {
            self.refused.replace(None);
        }
    }

    /// The line and the cursor in it, for the app.
    fn typed(hwnd: HWND) {
        let line = dialog::item(hwnd, LINE);
        let text = controls::text(line);
        let mut start = 0u32;
        let mut end = 0u32;
        controls::send(line, EM_GETSEL, (&raw mut start) as usize, (&raw mut end) as isize);
        let cursor = controls::bytes_at(&text, end as usize) as u32;
        controls::enable(dialog::item(hwnd, IDOK.0 as u16), !text.trim().is_empty());
        app::with(|app| {
            app.crux.send(ShellEvent::Typing(Typing::Text { text, cursor }));
            // The app reads the line back as it takes it, so the readback is written now: left
            // to the turn, which reaches the window later, a Tab straight after typing found
            // the readback of the line before the last keystroke.
            if let Some(line) = app.crux.view().line {
                Self::read_back(hwnd, &line);
            }
        });
    }

    fn read_back(hwnd: HWND, line: &LineView) {
        let readback = dialog::item(hwnd, READBACK);
        if controls::text(readback) != line.readback {
            controls::set_text(readback, &line.readback);
        }
    }
}

impl Dialog for QuickAdd {
    fn template(&self) -> Template {
        let readback = (ES_MULTILINE | ES_READONLY | ES_AUTOVSCROLL) as u32 | WS_VSCROLL.0 | WS_BORDER.0 | WS_TABSTOP.0;
        let words = &self.words;
        // The core's words, with this dialog's access keys.
        let label = |text: &str, letter| format!("{}:", devices::marked(text, letter, '&'));
        Template::new(&words.title, 300, 128)
            .item(Class::Static, &label(&words.label, 'T'), u16::MAX, 0, 7, 7, 286, 9)
            .item(Class::Edit, "", LINE, ES_AUTOHSCROLL as u32 | WS_BORDER.0 | WS_TABSTOP.0, 7, 17, 286, 14)
            .item(Class::Static, &words.hint, u16::MAX, SS_NOPREFIX.0, 7, 35, 286, 18)
            .item(Class::Static, &label(&words.will_add, 'W'), u16::MAX, 0, 7, 56, 286, 9)
            .item(Class::Edit, "", READBACK, readback, 7, 66, 286, 36)
            .item(Class::Button, &words.add.replace('&', "&&"), IDOK.0 as u16, BS_DEFPUSHBUTTON as u32 | WS_TABSTOP.0, 189, 108, 50, 14)
            .item(Class::Button, &words.cancel.replace('&', "&&"), IDCANCEL.0 as u16, BS_PUSHBUTTON as u32 | WS_TABSTOP.0, 243, 108, 50, 14)
    }

    fn init(&self, hwnd: HWND) -> bool {
        self.hwnd.set(hwnd);
        let line = dialog::item(hwnd, LINE);
        completion::attach(line, Syntax::QuickAdd);
        controls::set_text(line, &self.initial);
        // The cursor after the prefix, so typing carries on from `#Work `.
        let end = self.initial.encode_utf16().count();
        controls::send(line, EM_SETSEL, end, end as isize);
        Self::typed(hwnd);
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
                Self::typed(hwnd);
                None
            }
            // Adding is the app's: the dialog closes once the task is added.
            id if id == IDOK.0 => {
                let text = controls::text(dialog::item(hwnd, LINE));
                if !text.trim().is_empty() {
                    app::send(ShellEvent::QuickAdd(text));
                }
                None
            }
            id if id == IDCANCEL.0 => {
                self.closed.set(true);
                app::send(ShellEvent::Typing(Typing::Close));
                Some(0)
            }
            _ => None,
        }
    }
}

/// Asks for a task in quick-add words, starting from the line the app opens (the place's own
/// prefix), and has the app add it.
pub fn run(app: &App, owner: Option<HWND>) {
    app.crux.send(ShellEvent::Typing(Typing::Open));
    let initial = app.crux.view().line.map(|line| line.text).unwrap_or_default();
    let open = Rc::new(QuickAdd {
        hwnd: Cell::new(HWND::default()),
        words: lumenna_records::fields::quick_add_words(false),
        initial,
        refused: RefCell::new(None),
        closed: Cell::new(false),
    });
    app.open_quick_add(Some(Rc::clone(&open)));
    dialog::run(owner, &*open);
    app.open_quick_add(None);
}
