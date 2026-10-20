//! Pairing this PC with another of the person's devices, drawn from the app's pairing
//! (`view.devices.pairing`): the code shown, the code typed, the words to compare and how it
//! went are the app's, and the pairing itself runs on the app's network.
//!
//! On one network the two find each other: both wait, and DNS-SD does the rest. Anywhere
//! else, one shows a code and the other enters it. Either way both show the same three words,
//! and only if the person says they match on both does anything get paired — the one
//! mechanism for every platform. What the app says while the dialog is open goes to its own
//! status line.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use lumenna_app::network::PairingView;
use lumenna_app::{DevicesEvent, PairEvent};
use lumenna_ffi::ShellEvent;
use lumenna_records::PairingWords;
use windows::Win32::Foundation::HWND;
use windows::Win32::System::SystemServices::SS_NOPREFIX;
use windows::Win32::UI::WindowsAndMessaging::{BS_PUSHBUTTON, EN_CHANGE, ES_AUTOHSCROLL, ES_READONLY, EndDialog, IDCANCEL, WS_BORDER, WS_TABSTOP};

use super::app::{self, App};
use super::controls;
use super::dialog::{self, Class, Dialog, Template};
use super::{a11y, prompts, system};
use crate::devices;

const WAIT: u16 = 101;
const MY_CODE: u16 = 102;
const MY_CODE_LABEL: u16 = 103;
const THEIR_CODE: u16 = 104;
const WITH_CODE: u16 = 105;
const COPY: u16 = 106;
const STATUS: u16 = 199;

/// The pairing dialog, while it is open.
pub struct Pairing {
    hwnd: Cell<HWND>,
    status: Cell<HWND>,
    /// Every sentence and button, the app's, as it calls this PC; the access keys are this
    /// dialog's.
    words: PairingWords,
    /// The words last asked about, so each set is asked once.
    asked: RefCell<Option<Vec<String>>>,
    /// Whether the app has closed it, or the person has.
    closed: Cell<bool>,
}

impl Pairing {
    /// Draws the app's pairing as it now is, or closes the dialog when the app has closed it:
    /// paired, which is then said where it goes next.
    pub fn draw(&self, app: &App, view: Option<&PairingView>) {
        let hwnd = self.hwnd.get();
        if hwnd.is_invalid() || self.closed.get() {
            return;
        }
        let Some(view) = view else {
            self.closed.set(true);
            app.closing_live(self.status.get());
            unsafe {
                let _ = EndDialog(hwnd, 1);
            }
            return;
        };
        // This device's code, while it waits to be dialled.
        let mine = view.mine.clone().unwrap_or_default();
        let code = dialog::item(hwnd, MY_CODE);
        if controls::text(code) != mine {
            controls::set_text(code, &mine);
            if !mine.is_empty() {
                controls::focus(code);
            }
        }
        for id in [MY_CODE_LABEL, MY_CODE, COPY] {
            controls::show(dialog::item(hwnd, id), view.mine.is_some());
        }
        // While waiting, a code can still be entered: it gives up the wait and dials. One
        // pairing runs at a time, so while dialling there is nothing more to enter.
        let wait = !view.running;
        let join = !view.running || view.waiting;
        let focus = controls::focused();
        if (!wait && focus == dialog::item(hwnd, WAIT)) || (!join && focus == dialog::item(hwnd, WITH_CODE)) {
            // Somewhere that stays: a disabled button that had focus leaves it nowhere.
            controls::focus(dialog::item(hwnd, IDCANCEL.0 as u16));
        }
        controls::enable(dialog::item(hwnd, WAIT), wait);
        controls::enable(dialog::item(hwnd, WITH_CODE), join);
        // The words to compare, asked once each, over the dialog.
        if let Some(words) = &view.words
            && self.asked.borrow().as_ref() != Some(words)
        {
            *self.asked.borrow_mut() = Some(words.clone());
            let said = self.words.clone();
            let words = words.clone();
            app.defer(move |_| {
                let message = format!("{} {}.", said.match_message, words.join(", "));
                let answers = [said.match_yes.as_str(), said.match_no.as_str()];
                let matched = prompts::choose(hwnd, &said.match_title, &message, &answers, false) == Some(0);
                app::send(ShellEvent::Devices(DevicesEvent::Pair(PairEvent::Confirm(matched))));
            });
        }
    }

    fn say(&self, text: &str) {
        let status = self.status.get();
        controls::set_text(status, text);
        a11y::changed(status);
    }
}

impl Dialog for Pairing {
    fn template(&self) -> Template {
        let field = ES_AUTOHSCROLL as u32 | WS_BORDER.0 | WS_TABSTOP.0;
        let button = BS_PUSHBUTTON as u32 | WS_TABSTOP.0;
        let words = &self.words;
        let key = |text: &str, letter| devices::marked(text, letter, '&');
        Template::new(&words.title, 280, 197)
            .item(Class::Static, &words.intro, u16::MAX, SS_NOPREFIX.0, 7, 7, 266, 36)
            .item(Class::Button, &key(&words.wait, 'W'), WAIT, button, 7, 46, 120, 14)
            .item(Class::Static, &format!("{}:", key(&words.my_code, 'c')), MY_CODE_LABEL, 0, 7, 66, 266, 9)
            .item(Class::Edit, "", MY_CODE, field | ES_READONLY as u32, 7, 76, 196, 14)
            .item(Class::Button, &key(&words.copy_code, 'd'), COPY, button, 207, 76, 66, 14)
            .item(Class::Static, &format!("{}:", key(&words.their_code, 'o')), u16::MAX, 0, 7, 96, 266, 9)
            .item(Class::Edit, "", THEIR_CODE, field, 7, 106, 266, 14)
            .item(Class::Button, &key(&words.join, 'P'), WITH_CODE, button, 7, 124, 120, 14)
            .item(Class::Static, "", STATUS, SS_NOPREFIX.0, 7, 144, 266, 28)
            .item(Class::Button, "Cancel", IDCANCEL.0 as u16, button, 223, 176, 50, 14)
    }

    fn init(&self, hwnd: HWND) -> bool {
        self.hwnd.set(hwnd);
        let status = dialog::item(hwnd, STATUS);
        self.status.set(status);
        a11y::make_live(status);
        app::with(|app| app.open_live(status));
        // This device's code shows once there is one.
        for id in [MY_CODE_LABEL, MY_CODE, COPY] {
            controls::show(dialog::item(hwnd, id), false);
        }
        false
    }

    fn command(&self, hwnd: HWND, id: u16, code: u16) -> Option<isize> {
        let pair = |event| app::send(ShellEvent::Devices(DevicesEvent::Pair(event)));
        match id {
            WAIT => pair(PairEvent::Wait),
            // An empty field is never read as the clipboard: nobody could tell it would be, and
            // it would send whatever happened to be copied. The app says what is wanted.
            WITH_CODE => pair(PairEvent::Join),
            THEIR_CODE if u32::from(code) == EN_CHANGE => pair(PairEvent::Code(controls::text(dialog::item(hwnd, THEIR_CODE)))),
            COPY => {
                // Copied only when asked for: the clipboard is the person's.
                let code = controls::text(dialog::item(hwnd, MY_CODE));
                if system::copy(hwnd, &code) {
                    self.say(&self.words.copied);
                }
            }
            id if i32::from(id) == IDCANCEL.0 => {
                self.closed.set(true);
                return Some(0);
            }
            _ => {}
        }
        None
    }
}

/// Runs the pairing dialog over `owner` until the app has paired or the person gives up.
pub fn run(app: &App, owner: HWND) {
    let name = system::computer_name();
    // Opened from the Devices page, which has shown the app's devices already.
    if app.crux.view().devices.is_none() {
        app.crux.send(ShellEvent::Devices(DevicesEvent::Show));
    }
    app.crux.send(ShellEvent::Devices(DevicesEvent::Pair(PairEvent::Open(name))));
    let Some(words) = app.crux.view().devices.map(|devices| devices.words) else { return };
    let pairing = Rc::new(Pairing {
        hwnd: Cell::new(HWND::default()),
        status: Cell::new(HWND::default()),
        words,
        asked: RefCell::new(None),
        closed: Cell::new(false),
    });
    app.open_pairing(Some(Rc::clone(&pairing)));
    dialog::run(Some(owner), &*pairing);
    app.open_pairing(None);
    // Given up by the person: the pairing under way, or else the screen.
    if app.crux.view().devices.is_some_and(|devices| devices.pairing.is_some()) {
        app.crux.send(ShellEvent::Devices(DevicesEvent::Pair(PairEvent::Cancel)));
    }
    app.close_live(pairing.status.get());
}
