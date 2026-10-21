//! Asking: a line of text, one of a list, yes or no, which of several actions.

use std::cell::RefCell;

use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, WPARAM};
use windows::Win32::Graphics::Gdi::{
    COLOR_HIGHLIGHT, COLOR_HIGHLIGHTTEXT, COLOR_WINDOW, COLOR_WINDOWTEXT, CreateSolidBrush, DT_END_ELLIPSIS, DT_NOPREFIX,
    DT_SINGLELINE, DT_VCENTER, DeleteObject, DrawFocusRect, DrawTextW, FillRect, GetDC, GetSysColor, GetTextMetricsW,
    HGDIOBJ, ReleaseDC, SelectObject, SetBkMode, SetTextColor, TEXTMETRICW, TRANSPARENT,
};
use windows::Win32::UI::Controls::{
    TASKDIALOG_BUTTON, TASKDIALOGCONFIG, TD_WARNING_ICON, TDCBF_CANCEL_BUTTON, TDF_ALLOW_DIALOG_CANCELLATION,
    DRAWITEMSTRUCT, ODS_FOCUS, ODS_SELECTED, TDF_POSITION_RELATIVE_TO_WINDOW, TaskDialogIndirect,
};
use windows::Win32::UI::WindowsAndMessaging::{
    BS_DEFPUSHBUTTON, BS_PUSHBUTTON, ES_AUTOHSCROLL, IDCANCEL, IDOK, LB_ADDSTRING, LB_GETCURSEL,
    LB_SETCURSEL, LB_SETITEMHEIGHT, LBN_DBLCLK, LBS_HASSTRINGS, LBS_NOINTEGRALHEIGHT, LBS_NOTIFY, LBS_OWNERDRAWFIXED,
    MB_ICONWARNING, MB_OK, MessageBoxW, WM_DRAWITEM, WM_GETFONT,
    WS_BORDER, WS_TABSTOP, WS_VSCROLL,
};
use windows::Win32::System::SystemServices::SS_NOPREFIX;
use windows::core::{HSTRING, PCWSTR};

use lumenna_records::{Swatch, Syntax};

use super::{a11y, completion, controls, dark, picker, swatch};
use super::dialog::{self, Class, Dialog, Template};

const FIELD: u16 = 100;
const LIST: u16 = 101;
const CALENDAR: u16 = 102;

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
    takes: Takes<'a>,
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
        template = template.item(Class::Static, self.label, u16::MAX, 0, 7, top, 246, 9);
        let field = (ES_AUTOHSCROLL as u32) | WS_BORDER.0 | WS_TABSTOP.0;
        // A day has the system's calendar beside it, after it in tab order, to fill it.
        template = match self.takes.day {
            Some(_) => template
                .item(Class::Edit, "", FIELD, field, 7, top + 10, 162, 14)
                .named(picker::CLASS, "", CALENDAR, picker::Kind::Date.style(), 173, top + 10, 80, 14),
            None => template.item(Class::Edit, "", FIELD, field, 7, top + 10, 246, 14),
        };
        template
            .item(Class::Button, self.yes, IDOK.0 as u16, (BS_DEFPUSHBUTTON as u32) | WS_TABSTOP.0, 149, top + 32, 50, 14)
            .item(Class::Button, "Cancel", IDCANCEL.0 as u16, (BS_PUSHBUTTON as u32) | WS_TABSTOP.0, 203, top + 32, 50, 14)
    }

    fn init(&self, hwnd: HWND) -> bool {
        let field = dialog::item(hwnd, FIELD);
        controls::set_text(field, self.initial);
        // Everything selected, so typing replaces it and an arrow keeps it.
        controls::send(field, windows::Win32::UI::Controls::EM_SETSEL, 0, -1);
        if let Some(syntax) = self.takes.syntax {
            completion::attach(field, syntax);
        }
        if let Some(day) = self.takes.day {
            let calendar = dialog::item(hwnd, CALENDAR);
            let name = self.label.replace('&', "");
            picker::ready(calendar, picker::Kind::Date, name.trim_end_matches(':'), &picker::Kind::Date.choose());
            picker::show(calendar, picker::Kind::Date, day);
        }
        controls::focus(field);
        true
    }

    fn message(&self, hwnd: HWND, message: u32, _wparam: WPARAM, lparam: LPARAM) -> Option<isize> {
        // A day picked is the field's, as the core reads it.
        let id = picker::changed(message, lparam).filter(|id| *id == CALENDAR)?;
        let day = picker::read(dialog::item(hwnd, id), picker::Kind::Date)?;
        controls::set_text(dialog::item(hwnd, FIELD), &day);
        Some(0)
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

/// What a line asks for beyond text.
#[derive(Clone, Copy, Default)]
pub struct Takes<'a> {
    /// A day, ISO, which a calendar beside the field opens on.
    pub day: Option<&'a str>,
    /// A language the core completes, offered on Down or Ctrl+Space as quick add's is.
    pub syntax: Option<Syntax>,
}

/// Asks for a line of text. `label` names the field, with `&` before its mnemonic; `message`,
/// if any, is read when the dialog opens; `yes` is the button that answers, a verb where
/// there is one ("Rename"), as Windows' own dialogs name theirs. An empty answer is an
/// answer, and only Cancel is `None`. What else the line takes (`takes`) is offered beside it.
pub fn ask(owner: HWND, title: &str, label: &str, message: &str, initial: &str, yes: &str, takes: Takes) -> Option<String> {
    let ask = AskText { title, label, message, initial, yes, takes, answer: RefCell::new(String::new()) };
    (dialog::run(Some(owner), &ask) == 1).then(|| ask.answer.into_inner().trim().to_owned())
}

struct Pick<'a> {
    title: &'a str,
    label: &'a str,
    items: &'a [String],
    /// Why each is offered, said after it; empty where nothing is.
    reasons: &'a [String],
    /// Each one's colour, drawn beside its text where any has one: the list then draws its own
    /// items, keeping their strings for what a screen reader reads.
    swatches: &'a [Option<Swatch>],
    yes: &'a str,
    chosen: RefCell<Option<usize>>,
}

impl Pick<'_> {
    fn coloured(&self) -> bool {
        self.swatches.iter().any(Option::is_some)
    }

    /// Draws one choice: its colour's mark, then its text, in the list's colours, the system's
    /// for the selection.
    fn draw(&self, item: &DRAWITEMSTRUCT) {
        let Some(text) = usize::try_from(item.itemID).ok().and_then(|index| self.items.get(index)) else { return };
        let swatch = self.swatches.get(item.itemID as usize).and_then(Option::as_ref);
        let selected = item.itemState.0 & ODS_SELECTED.0 != 0;
        let (back, fore) = if selected {
            unsafe { (COLORREF(GetSysColor(COLOR_HIGHLIGHT)), COLORREF(GetSysColor(COLOR_HIGHLIGHTTEXT))) }
        } else {
            dark::field().unwrap_or_else(|| unsafe { (COLORREF(GetSysColor(COLOR_WINDOW)), COLORREF(GetSysColor(COLOR_WINDOWTEXT))) })
        };
        let dc = item.hDC;
        let mut area = item.rcItem;
        unsafe {
            let brush = CreateSolidBrush(back);
            FillRect(dc, &area, brush);
            let _ = DeleteObject(brush.into());
            let side = swatch::side(item.hwndItem);
            area.left += 2;
            swatch::draw(dc, &area, swatch, side);
            // "No colour" lines up with the colours' names.
            area.left += side + 4;
            SetTextColor(dc, fore);
            SetBkMode(dc, TRANSPARENT);
            let mut wide: Vec<u16> = text.encode_utf16().collect();
            DrawTextW(dc, &mut wide, &mut area, DT_SINGLELINE | DT_VCENTER | DT_NOPREFIX | DT_END_ELLIPSIS);
            if item.itemState.0 & ODS_FOCUS.0 != 0 {
                let _ = DrawFocusRect(dc, &item.rcItem);
            }
        }
    }
}

impl Dialog for Pick<'_> {
    fn template(&self) -> Template {
        let drawn = if self.coloured() { (LBS_OWNERDRAWFIXED | LBS_HASSTRINGS) as u32 } else { 0 };
        let style = (LBS_NOTIFY | LBS_NOINTEGRALHEIGHT) as u32 | drawn | WS_VSCROLL.0 | WS_BORDER.0 | WS_TABSTOP.0;
        Template::new(self.title, 260, 196)
            .item(Class::Static, self.label, u16::MAX, 0, 7, 7, 246, 9)
            .item(Class::ListBox, "", LIST, style, 7, 17, 246, 152)
            .item(Class::Button, self.yes, IDOK.0 as u16, (BS_DEFPUSHBUTTON as u32) | WS_TABSTOP.0, 149, 175, 50, 14)
            .item(Class::Button, "Cancel", IDCANCEL.0 as u16, (BS_PUSHBUTTON as u32) | WS_TABSTOP.0, 203, 175, 50, 14)
    }

    fn init(&self, hwnd: HWND) -> bool {
        let list = dialog::item(hwnd, LIST);
        for item in self.items {
            let text = HSTRING::from(item.as_str());
            controls::send(list, LB_ADDSTRING, 0, text.as_ptr() as isize);
        }
        for (index, reason) in self.reasons.iter().enumerate().filter(|(_, reason)| !reason.is_empty()) {
            a11y::set_item_description(list, index as u32 + 1, reason);
        }
        controls::send(list, LB_SETCURSEL, 0, 0);
        if self.coloured() {
            // As tall as a line of its font, or its mark, with a little room.
            let side = swatch::side(list);
            let line = unsafe {
                let dc = GetDC(Some(list));
                let font = HGDIOBJ(controls::send(list, WM_GETFONT, 0, 0) as _);
                let old = SelectObject(dc, font);
                let mut metrics = TEXTMETRICW::default();
                let _ = GetTextMetricsW(dc, &mut metrics);
                SelectObject(dc, old);
                ReleaseDC(Some(list), dc);
                metrics.tmHeight
            };
            controls::send(list, LB_SETITEMHEIGHT, 0, (line.max(side) + 4) as isize);
        }
        controls::focus(list);
        true
    }

    fn message(&self, _hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> Option<isize> {
        if message != WM_DRAWITEM || wparam.0 != usize::from(LIST) {
            return None;
        }
        self.draw(unsafe { &*(lparam.0 as *const DRAWITEMSTRUCT) });
        Some(1)
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

/// Offers a list and returns the position of the one chosen. `label` names the list, and
/// `yes` is the button that takes the one chosen ("Move"). `reasons`, where not empty, say why
/// each is offered: its description, said after it as a hint is; `swatches`, each one's colour.
pub fn pick(owner: HWND, title: &str, label: &str, items: &[String], reasons: &[String], swatches: &[Option<Swatch>], yes: &str) -> Option<usize> {
    if items.is_empty() {
        fail(owner, &format!("{title}: there is nothing to choose from."));
        return None;
    }
    let pick = Pick { title, label, items, reasons, swatches, yes, chosen: RefCell::new(None) };
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
