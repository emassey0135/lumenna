//! Asking: a line of text, one of a list, yes or no, which of several actions.

use std::cell::RefCell;

use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Controls::{
    TASKDIALOG_BUTTON, TASKDIALOGCONFIG, TD_WARNING_ICON, TDCBF_CANCEL_BUTTON, TDF_ALLOW_DIALOG_CANCELLATION,
    TDF_POSITION_RELATIVE_TO_WINDOW, TaskDialogIndirect,
};
use windows::Win32::UI::WindowsAndMessaging::{
    BS_DEFPUSHBUTTON, BS_PUSHBUTTON, ES_AUTOHSCROLL, IDCANCEL, IDOK, LB_ADDSTRING, LB_GETCURSEL,
    LB_SETCURSEL, LBN_DBLCLK, LBS_NOINTEGRALHEIGHT, LBS_NOTIFY, MB_ICONWARNING, MB_OK, MessageBoxW,
    WS_BORDER, WS_TABSTOP, WS_VSCROLL,
};
use windows::Win32::System::SystemServices::SS_NOPREFIX;
use windows::core::{HSTRING, PCWSTR};

use super::controls;
use super::dialog::{self, Class, Dialog, Template};

const FIELD: u16 = 100;
const LIST: u16 = 101;

/// Dialog units for a message of this length, at roughly fifty characters a line.
fn message_height(message: &str) -> i16 {
    let lines: usize = message.lines().map(|line| line.chars().count() / 50 + 1).sum();
    (lines.max(1) * 9) as i16
}

struct AskText<'a> {
    title: &'a str,
    label: &'a str,
    message: &'a str,
    initial: &'a str,
    yes: &'a str,
    answer: RefCell<String>,
}

impl Dialog for AskText<'_> {
    fn template(&self) -> Template {
        let message = if self.message.is_empty() { 0 } else { message_height(self.message) + 6 };
        let top = 7 + message;
        let mut template = Template::new(self.title, 260, top + 52);
        if message > 0 {
            template = template.item(Class::Static, self.message, u16::MAX, SS_NOPREFIX.0, 7, 7, 246, message - 6);
        }
        template
            .item(Class::Static, self.label, u16::MAX, 0, 7, top, 246, 9)
            .item(Class::Edit, "", FIELD, (ES_AUTOHSCROLL as u32) | WS_BORDER.0 | WS_TABSTOP.0, 7, top + 10, 246, 14)
            .item(Class::Button, self.yes, IDOK.0 as u16, (BS_DEFPUSHBUTTON as u32) | WS_TABSTOP.0, 149, top + 32, 50, 14)
            .item(Class::Button, "Cancel", IDCANCEL.0 as u16, (BS_PUSHBUTTON as u32) | WS_TABSTOP.0, 203, top + 32, 50, 14)
    }

    fn init(&self, hwnd: HWND) -> bool {
        let field = dialog::item(hwnd, FIELD);
        controls::set_text(field, self.initial);
        // Everything selected, so typing replaces it and an arrow keeps it.
        controls::send(field, windows::Win32::UI::Controls::EM_SETSEL, 0, -1);
        controls::focus(field);
        true
    }

    fn command(&self, hwnd: HWND, id: u16, _code: u16) -> Option<isize> {
        match i32::from(id) {
            id if id == IDOK.0 => {
                *self.answer.borrow_mut() = controls::text(dialog::item(hwnd, FIELD));
                Some(1)
            }
            id if id == IDCANCEL.0 => Some(0),
            _ => None,
        }
    }
}

/// Asks for a line of text. `label` names the field, with `&` before its mnemonic; `message`,
/// if any, is read when the dialog opens; `yes` is the button that answers, a verb where
/// there is one ("Rename"), as Windows' own dialogs name theirs. `None` if cancelled or left
/// empty.
pub fn ask_text(owner: HWND, title: &str, label: &str, message: &str, initial: &str, yes: &str) -> Option<String> {
    ask(owner, title, label, message, initial, yes).filter(|answer| !answer.is_empty())
}

/// Asks for a line of text that may be left empty on purpose: `Some("")` is an answer, and
/// only Cancel is `None`.
pub fn ask(owner: HWND, title: &str, label: &str, message: &str, initial: &str, yes: &str) -> Option<String> {
    let ask = AskText { title, label, message, initial, yes, answer: RefCell::new(String::new()) };
    (dialog::run(Some(owner), &ask) == 1).then(|| ask.answer.into_inner().trim().to_owned())
}

struct Pick<'a> {
    title: &'a str,
    label: &'a str,
    items: &'a [String],
    chosen: RefCell<Option<usize>>,
}

impl Dialog for Pick<'_> {
    fn template(&self) -> Template {
        let style = (LBS_NOTIFY | LBS_NOINTEGRALHEIGHT) as u32 | WS_VSCROLL.0 | WS_BORDER.0 | WS_TABSTOP.0;
        Template::new(self.title, 260, 196)
            .item(Class::Static, self.label, u16::MAX, 0, 7, 7, 246, 9)
            .item(Class::ListBox, "", LIST, style, 7, 17, 246, 152)
            .item(Class::Button, "OK", IDOK.0 as u16, (BS_DEFPUSHBUTTON as u32) | WS_TABSTOP.0, 149, 175, 50, 14)
            .item(Class::Button, "Cancel", IDCANCEL.0 as u16, (BS_PUSHBUTTON as u32) | WS_TABSTOP.0, 203, 175, 50, 14)
    }

    fn init(&self, hwnd: HWND) -> bool {
        let list = dialog::item(hwnd, LIST);
        for item in self.items {
            let text = HSTRING::from(item.as_str());
            controls::send(list, LB_ADDSTRING, 0, text.as_ptr() as isize);
        }
        controls::send(list, LB_SETCURSEL, 0, 0);
        controls::focus(list);
        true
    }

    fn command(&self, hwnd: HWND, id: u16, code: u16) -> Option<isize> {
        let choose = || {
            let index = controls::send(dialog::item(hwnd, LIST), LB_GETCURSEL, 0, 0);
            *self.chosen.borrow_mut() = usize::try_from(index).ok();
            Some(1)
        };
        match i32::from(id) {
            id if id == IDOK.0 => choose(),
            id if id == IDCANCEL.0 => Some(0),
            id if id == i32::from(LIST) && u32::from(code) == LBN_DBLCLK => choose(),
            _ => None,
        }
    }
}

/// Offers a list and returns the position of the one chosen. `label` names the list.
pub fn pick(owner: HWND, title: &str, label: &str, items: &[String]) -> Option<usize> {
    if items.is_empty() {
        fail(owner, &format!("{title}: there is nothing to choose from."));
        return None;
    }
    let pick = Pick { title, label, items, chosen: RefCell::new(None) };
    if dialog::run(Some(owner), &pick) == 1 { pick.chosen.into_inner().filter(|i| *i < items.len()) } else { None }
}

/// Offers several actions and Cancel, and returns the position of the action chosen.
///
/// A task dialog's buttons say what each does — "Delete and Keep Its Tasks" — so the choice
/// is read as a sentence rather than as Yes or No to a question.
pub fn choose(owner: HWND, title: &str, message: &str, actions: &[&str], warning: bool) -> Option<usize> {
    // The question is the main instruction; the title bar names the app, as Windows' own
    // task dialogs do, so a screen reader does not read the question twice.
    let caption = HSTRING::from("Lumenna");
    let title = HSTRING::from(title);
    let message = HSTRING::from(message);
    let texts: Vec<HSTRING> = actions.iter().map(|a| HSTRING::from(*a)).collect();
    let buttons: Vec<TASKDIALOG_BUTTON> = texts
        .iter()
        .enumerate()
        .map(|(index, text)| TASKDIALOG_BUTTON { nButtonID: 1000 + index as i32, pszButtonText: PCWSTR(text.as_ptr()) })
        .collect();
    let mut config = TASKDIALOGCONFIG {
        cbSize: size_of::<TASKDIALOGCONFIG>() as u32,
        hwndParent: owner,
        dwFlags: TDF_ALLOW_DIALOG_CANCELLATION | TDF_POSITION_RELATIVE_TO_WINDOW,
        dwCommonButtons: TDCBF_CANCEL_BUTTON,
        pszWindowTitle: PCWSTR(caption.as_ptr()),
        pszMainInstruction: PCWSTR(title.as_ptr()),
        pszContent: PCWSTR(message.as_ptr()),
        cButtons: buttons.len() as u32,
        pButtons: buttons.as_ptr(),
        nDefaultButton: IDCANCEL.0,
        ..Default::default()
    };
    if warning {
        config.Anonymous1.pszMainIcon = TD_WARNING_ICON;
    }
    let mut pressed = 0;
    unsafe { TaskDialogIndirect(&config, Some(&mut pressed), None, None).ok()? };
    usize::try_from(pressed - 1000).ok().filter(|index| *index < actions.len())
}

/// Asks before something that cannot be taken back. Cancel is the default, so Enter alone
/// never does it.
pub fn confirm(owner: HWND, title: &str, message: &str, action: &str) -> bool {
    choose(owner, title, message, &[action], true) == Some(0)
}

/// Says why something could not be done.
pub fn fail(owner: HWND, message: &str) {
    unsafe {
        MessageBoxW(Some(owner), &HSTRING::from(message), &HSTRING::from("Lumenna"), MB_OK | MB_ICONWARNING);
    }
}
