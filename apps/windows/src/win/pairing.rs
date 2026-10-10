//! Pairing this PC with another of the person's devices.
//!
//! On one network the two find each other: both wait, and DNS-SD does the rest. Anywhere
//! else, one shows a code and the other enters it. Either way both show the same three words,
//! and only if the person says they match on both does anything get paired — the one
//! mechanism for every platform. The pairing runs on a thread of its own and asks its
//! questions back through this dialog, as the Mac's sheet does.

use std::cell::{Cell, RefCell};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{RecvTimeoutError, Sender, channel};
use std::sync::Arc;
use std::time::Duration;

use lumenna_surface::{Lumenna, LumennaError, PairedWith, PairingPrompt, PairingWords, Reach, pairing_words};
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::System::SystemServices::SS_NOPREFIX;
use windows::Win32::UI::WindowsAndMessaging::{
    BS_PUSHBUTTON, ES_AUTOHSCROLL, ES_READONLY, IDCANCEL, WM_APP, WS_BORDER, WS_TABSTOP,
};

use super::controls;
use super::core::{Poster, sentence, taken};
use super::dialog::{self, Class, Dialog, Template};
use super::{a11y, prompts, system};
use crate::{devices, speech};

const WAIT: u16 = 101;
const MY_CODE: u16 = 102;
const MY_CODE_LABEL: u16 = 103;
const THEIR_CODE: u16 = 104;
const WITH_CODE: u16 = 105;
const STATUS: u16 = 199;

/// The code to give the other device: a boxed `String`.
const WM_PAIR_CODE: u32 = WM_APP + 20;
/// The words to compare: a boxed `(Vec<String>, Sender<bool>)`.
const WM_PAIR_WORDS: u32 = WM_APP + 21;
/// The pairing ended: a boxed `Result<PairedWith, String>`.
const WM_PAIR_DONE: u32 = WM_APP + 22;

struct Pairing {
    lumenna: Arc<Lumenna>,
    /// Every sentence and button, the core's; the access keys are this dialog's.
    words: PairingWords,
    /// Set when the person gives up, which ends the wait for the other device.
    cancelled: RefCell<Option<Arc<AtomicBool>>>,
    running: Cell<bool>,
    /// Whether the pairing running is waiting to be found, rather than dialling a code.
    waiting: Cell<bool>,
    /// A code entered while waiting: the wait is given up, and this is dialled once it ends.
    then: RefCell<Option<String>>,
    paired: RefCell<Option<PairedWith>>,
}

impl Pairing {
    fn say(&self, hwnd: HWND, text: &str) {
        let status = dialog::item(hwnd, STATUS);
        controls::set_text(status, text);
        a11y::changed(status);
    }

    fn start(&self, hwnd: HWND, code: Option<String>) {
        if self.running.replace(true) {
            return;
        }
        // Focus somewhere that stays: a disabled button that had it leaves it nowhere.
        controls::focus(dialog::item(hwnd, IDCANCEL.0 as u16));
        controls::enable(dialog::item(hwnd, WAIT), false);
        // While waiting, a code can still be entered: it gives up the wait and dials. One
        // pairing runs at a time, so while dialling there is nothing more to enter.
        self.waiting.set(code.is_none());
        controls::enable(dialog::item(hwnd, WITH_CODE), code.is_none());
        self.say(hwnd, if code.is_none() { &self.words.opening } else { &self.words.connecting });
        let cancelled = Arc::new(AtomicBool::new(false));
        *self.cancelled.borrow_mut() = Some(Arc::clone(&cancelled));
        let window = Poster::new(hwnd);
        let prompt = Arc::new(Prompt { window, cancelled });
        let lumenna = Arc::clone(&self.lumenna);
        std::thread::spawn(move || {
            let result = lumenna
                .pair(code, Reach::Internet, system::computer_name(), "windows".to_owned(), prompt)
                .map_err(|error: LumennaError| sentence(&error));
            window.send(WM_PAIR_DONE, result);
        });
    }

    /// Pairs with the code typed in — or, if nothing was typed, the one on the clipboard,
    /// which is how a code sent from the other device usually arrives. Not this device's own
    /// code, which waiting copied there: that would pair it with itself.
    fn with_code(&self, hwnd: HWND) {
        let field = dialog::item(hwnd, THEIR_CODE);
        let mut code = controls::text(field).trim().to_owned();
        let mine = controls::text(dialog::item(hwnd, MY_CODE)).trim().to_owned();
        if code.is_empty()
            && let Some(pasted) = system::pasted(hwnd)
                .map(|p| p.trim().to_owned())
                .filter(|p| !p.is_empty() && *p != mine)
        {
            controls::set_text(field, &pasted);
            code = pasted;
        }
        if code.is_empty() {
            prompts::fail(hwnd, &self.words.need_code);
            controls::focus(field);
            return;
        }
        if self.running.get() {
            if self.waiting.get() {
                *self.then.borrow_mut() = Some(code);
                if let Some(cancelled) = self.cancelled.borrow().as_ref() {
                    cancelled.store(true, Ordering::Relaxed);
                }
                self.say(hwnd, &self.words.switching);
            }
            return;
        }
        self.start(hwnd, Some(code));
    }
}

impl Dialog for Pairing {
    fn template(&self) -> Template {
        let field = ES_AUTOHSCROLL as u32 | WS_BORDER.0 | WS_TABSTOP.0;
        let button = BS_PUSHBUTTON as u32 | WS_TABSTOP.0;
        let words = &self.words;
        let key = |text: &str, letter| devices::marked(text, letter, '&');
        Template::new(&words.title, 280, 207)
            .item(Class::Static, &words.intro, u16::MAX, SS_NOPREFIX.0, 7, 7, 266, 36)
            .item(Class::Button, &key(&words.wait, 'W'), WAIT, button, 7, 46, 120, 14)
            .item(Class::Static, &format!("{}:", key(&words.my_code, 'c')), MY_CODE_LABEL, 0, 7, 66, 266, 9)
            .item(Class::Edit, "", MY_CODE, field | ES_READONLY as u32, 7, 76, 266, 14)
            .item(Class::Static, &format!("{}:", key(&words.their_code, 'o')), u16::MAX, 0, 7, 96, 266, 9)
            .item(Class::Edit, "", THEIR_CODE, field, 7, 106, 266, 14)
            .item(Class::Static, &words.empty_means, u16::MAX, SS_NOPREFIX.0, 7, 123, 266, 9)
            .item(Class::Button, &key(&words.join, 'P'), WITH_CODE, button, 7, 134, 120, 14)
            .item(Class::Static, "", STATUS, SS_NOPREFIX.0, 7, 154, 266, 28)
            .item(Class::Button, "Cancel", IDCANCEL.0 as u16, button, 223, 186, 50, 14)
    }

    fn init(&self, hwnd: HWND) -> bool {
        a11y::make_live(dialog::item(hwnd, STATUS));
        // Read with the field, as well as shown under it.
        a11y::set_description(dialog::item(hwnd, THEIR_CODE), &self.words.empty_means);
        // This device's code shows once there is one.
        controls::show(dialog::item(hwnd, MY_CODE_LABEL), false);
        controls::show(dialog::item(hwnd, MY_CODE), false);
        false
    }

    fn command(&self, hwnd: HWND, id: u16, _code: u16) -> Option<isize> {
        match id {
            WAIT => self.start(hwnd, None),
            WITH_CODE => self.with_code(hwnd),
            id if i32::from(id) == IDCANCEL.0 => {
                if let Some(cancelled) = self.cancelled.borrow().as_ref() {
                    cancelled.store(true, Ordering::Relaxed);
                }
                return Some(0);
            }
            _ => {}
        }
        None
    }

    fn message(&self, hwnd: HWND, message: u32, _wparam: WPARAM, lparam: LPARAM) -> Option<isize> {
        match message {
            WM_PAIR_CODE => {
                let code: String = unsafe { taken(lparam) };
                controls::set_text(dialog::item(hwnd, MY_CODE), &code);
                controls::show(dialog::item(hwnd, MY_CODE_LABEL), true);
                controls::show(dialog::item(hwnd, MY_CODE), true);
                controls::focus(dialog::item(hwnd, MY_CODE));
                let copied = system::copy(hwnd, &code);
                let waiting = &self.words.waiting;
                self.say(hwnd, &if copied { format!("{waiting} {}", self.words.copied) } else { waiting.clone() });
            }
            WM_PAIR_WORDS => {
                let (words, answer): (Vec<String>, Sender<bool>) = unsafe { taken(lparam) };
                let said = &self.words;
                let message = format!("{} {}.", said.match_message, words.join(", "));
                let answers = [said.match_yes.as_str(), said.match_no.as_str()];
                let chosen = prompts::choose(hwnd, &said.match_title, &message, &answers, false);
                let _ = answer.send(chosen == Some(0));
            }
            WM_PAIR_DONE => {
                let result: Result<PairedWith, String> = unsafe { taken(lparam) };
                self.running.set(false);
                let then = self.then.borrow_mut().take();
                match result {
                    Ok(paired) => {
                        *self.paired.borrow_mut() = Some(paired);
                        unsafe {
                            let _ = windows::Win32::UI::WindowsAndMessaging::EndDialog(hwnd, 1);
                        }
                    }
                    // The wait was given up for a code entered meanwhile: dial it now.
                    Err(_) if then.is_some() => {
                        controls::show(dialog::item(hwnd, MY_CODE_LABEL), false);
                        controls::show(dialog::item(hwnd, MY_CODE), false);
                        self.start(hwnd, then);
                    }
                    Err(message) => {
                        controls::show(dialog::item(hwnd, MY_CODE_LABEL), false);
                        controls::show(dialog::item(hwnd, MY_CODE), false);
                        controls::enable(dialog::item(hwnd, WAIT), true);
                        controls::enable(dialog::item(hwnd, WITH_CODE), true);
                        controls::focus(dialog::item(hwnd, WAIT));
                        self.say(hwnd, &message);
                    }
                }
            }
            _ => return None,
        }
        Some(0)
    }
}

/// The pairing's questions, asked through the dialog while the pairing thread waits.
struct Prompt {
    window: Poster,
    cancelled: Arc<AtomicBool>,
}

impl PairingPrompt for Prompt {
    fn show_code(&self, code: String) {
        self.window.send(WM_PAIR_CODE, code);
    }

    fn confirm(&self, words: Vec<String>) -> bool {
        let (answer, answered) = channel();
        if !self.window.send(WM_PAIR_WORDS, (words, answer)) {
            return false;
        }
        // Waits for the person, but not past their giving up: a dialog closed meanwhile never
        // answers.
        loop {
            match answered.recv_timeout(Duration::from_millis(250)) {
                Ok(matched) => return matched,
                Err(RecvTimeoutError::Timeout) if !self.cancelled.load(Ordering::Relaxed) => {}
                Err(_) => return false,
            }
        }
    }

    fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }
}

/// Runs the pairing dialog, and returns what was said of the device paired, if one was.
pub fn run(owner: HWND, lumenna: Arc<Lumenna>) -> Option<String> {
    let pairing = Pairing {
        lumenna,
        words: pairing_words("this PC".to_owned(), true),
        cancelled: RefCell::new(None),
        running: Cell::new(false),
        waiting: Cell::new(false),
        then: RefCell::new(None),
        paired: RefCell::new(None),
    };
    dialog::run(Some(owner), &pairing);
    let paired = pairing.paired.into_inner()?;
    Some(speech::announcement(&paired.announcement, &paired.notices))
}
