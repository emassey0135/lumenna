//! A task's or a block's reminders: the core's `reminders_of`, its own or the defaults it
//! takes, with what the list offers (Add Reminder) and what the reminder chosen offers
//! (Remove Reminder) as buttons. The list is the app's form for the Reminders action; every
//! word in it and every action it runs are the core's.

use std::cell::RefCell;

use lumenna_surface::{Action, ActionKind, ReminderList, Subject, not_offered};
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::System::SystemServices::SS_NOPREFIX;
use windows::Win32::UI::Input::KeyboardAndMouse::VK_DELETE;
use windows::Win32::UI::WindowsAndMessaging::{
    BN_CLICKED, BS_PUSHBUTTON, IDCANCEL, LB_ADDSTRING, LB_GETCURSEL, LB_RESETCONTENT, LB_SETCURSEL, LBN_SELCHANGE,
    LBS_NOINTEGRALHEIGHT, LBS_NOTIFY, LBS_WANTKEYBOARDINPUT, WM_VKEYTOITEM, WS_BORDER, WS_TABSTOP, WS_VSCROLL,
};
use windows::core::HSTRING;

use super::actions;
use super::app::App;
use super::core::sentence;
use super::dialog::{self, Class, Dialog, Template};
use super::{a11y, controls, prompts};
use crate::{devices, speech};

const STATUS: u16 = 100;
const ANNOUNCED: u16 = 101;
const LIST: u16 = 102;
/// The chosen reminder's own action.
const ITS: u16 = 103;
/// The list's actions, from here on.
const LISTS: u16 = 110;

struct Reminders<'a> {
    app: &'a App,
    target: String,
    title: String,
    shown: RefCell<ReminderList>,
}

impl Reminders<'_> {
    fn chosen(&self, hwnd: HWND) -> Option<lumenna_surface::ReminderView> {
        let index = usize::try_from(controls::send(dialog::item(hwnd, LIST), LB_GETCURSEL, 0, 0)).ok()?;
        self.shown.borrow().reminders.get(index).cloned()
    }

    /// The action a chosen reminder's button stands for: the first any of them offers, since
    /// a default offers none.
    fn its_action(&self) -> Option<Action> {
        self.shown.borrow().reminders.iter().find_map(|r| r.actions.first().cloned())
    }

    /// Reads the list again, keeping the place in it.
    fn load(&self, hwnd: HWND) {
        let Ok(list) = self.app.core.lumenna.reminders_of(self.target.clone()) else { return };
        let announced = speech::sentence(&speech::announcement(&list.announcement, &list.notices));
        controls::set_text(dialog::item(hwnd, ANNOUNCED), &announced);
        let field = dialog::item(hwnd, LIST);
        // Said with the list, as well as shown above it.
        a11y::set_description(field, &announced);
        let kept = usize::try_from(controls::send(field, LB_GETCURSEL, 0, 0)).unwrap_or(0);
        controls::send(field, LB_RESETCONTENT, 0, 0);
        for reminder in &list.reminders {
            let text = HSTRING::from(reminder.words.as_str());
            controls::send(field, LB_ADDSTRING, 0, text.as_ptr() as isize);
        }
        controls::send(field, LB_SETCURSEL, kept.min(list.reminders.len().saturating_sub(1)), 0);
        *self.shown.borrow_mut() = list;
        self.offer(hwnd);
    }

    /// The chosen reminder's button, named by its action and enabled when it has one. The
    /// button with focus stays as it is, since disabling it would send focus nowhere.
    fn offer(&self, hwnd: HWND) {
        let button = dialog::item(hwnd, ITS);
        let Some(action) = self.its_action() else {
            if controls::focused() == button {
                controls::focus(dialog::item(hwnd, LIST));
            }
            controls::show(button, false);
            return;
        };
        controls::set_text(button, &devices::marked(&actions::label(&action), 'm', '&'));
        controls::show(button, true);
        let has = self.chosen(hwnd).is_some_and(|r| !r.actions.is_empty());
        if has || controls::focused() != button {
            controls::enable(button, has);
        }
    }

    fn run(&self, hwnd: HWND, action: &Action) {
        if let Some((change, _)) = actions::run(self.app, hwnd, action, || {}) {
            self.load(hwnd);
            let status = dialog::item(hwnd, STATUS);
            controls::set_text(status, &speech::sentence(&speech::announcement(&change.announcement, &change.notices)));
            a11y::changed(status);
        }
    }

    /// The chosen reminder's action of `kind`, or why it has none.
    fn act(&self, hwnd: HWND, kind: ActionKind) {
        let Some(reminder) = self.chosen(hwnd) else { return };
        match actions::of_kind(&reminder.actions, &[kind]) {
            Some(action) => self.run(hwnd, &action),
            None => {
                let status = dialog::item(hwnd, STATUS);
                controls::set_text(status, &speech::sentence(&not_offered(kind, Subject::Reminder, false)));
                a11y::changed(status);
            }
        }
    }
}

impl Dialog for Reminders<'_> {
    fn template(&self) -> Template {
        let list = (LBS_NOTIFY | LBS_NOINTEGRALHEIGHT | LBS_WANTKEYBOARDINPUT) as u32 | WS_VSCROLL.0 | WS_BORDER.0 | WS_TABSTOP.0;
        let button = BS_PUSHBUTTON as u32 | WS_TABSTOP.0;
        let shown = self.shown.borrow();
        // The status line first, so Windows does not name it after the label before it.
        let mut template = Template::new(&self.title, 260, 192)
            .item(Class::Static, "", STATUS, SS_NOPREFIX.0, 7, 148, 246, 18)
            .item(Class::Static, &shown.announcement, ANNOUNCED, SS_NOPREFIX.0, 7, 7, 246, 9)
            .item(Class::Static, "&Reminders:", u16::MAX, 0, 7, 20, 246, 9)
            .item(Class::ListBox, "", LIST, list, 7, 30, 246, 90);
        let mut x = 7;
        for (index, action) in shown.actions.iter().enumerate() {
            template = template.item(Class::Button, &devices::marked(&actions::label(action), 'A', '&'), LISTS + index as u16, button, x, 126, 80, 14);
            x += 84;
        }
        template
            .item(Class::Button, "", ITS, button, x, 126, 80, 14)
            .item(Class::Button, "Close", IDCANCEL.0 as u16, button, 203, 172, 50, 14)
    }

    fn init(&self, hwnd: HWND) -> bool {
        a11y::make_live(dialog::item(hwnd, STATUS));
        self.load(hwnd);
        // Where there is nothing to choose, the first thing to do is add one.
        let first = if self.shown.borrow().reminders.is_empty() { LISTS } else { LIST };
        controls::focus(dialog::item(hwnd, first));
        true
    }

    fn command(&self, hwnd: HWND, id: u16, code: u16) -> Option<isize> {
        if id == LIST && u32::from(code) == LBN_SELCHANGE {
            self.offer(hwnd);
            return None;
        }
        if i32::from(id) == IDCANCEL.0 {
            return Some(0);
        }
        if u32::from(code) != BN_CLICKED {
            return None;
        }
        if id == ITS {
            if let Some(kind) = self.its_action().map(|a| a.kind) {
                self.act(hwnd, kind);
            }
        } else if let Some(action) = id.checked_sub(LISTS).and_then(|i| self.shown.borrow().actions.get(usize::from(i)).cloned()) {
            self.run(hwnd, &action);
        }
        None
    }

    fn message(&self, hwnd: HWND, message: u32, wparam: WPARAM, _lparam: LPARAM) -> Option<isize> {
        match message {
            // Delete removes, as in every other list.
            WM_VKEYTOITEM if controls::low_word(wparam.0) == VK_DELETE.0 => {
                self.act(hwnd, ActionKind::Delete);
                Some(-2)
            }
            WM_VKEYTOITEM => Some(-1),
            _ => None,
        }
    }
}

/// Opens the list of reminders that `action` (Reminders, on a task or a block) is for.
pub fn show(app: &App, owner: HWND, action: &Action) {
    let shown = match app.core.lumenna.reminders_of(action.target.clone()) {
        Ok(shown) => shown,
        Err(error) => return prompts::fail(owner, &sentence(&error)),
    };
    let reminders = Reminders { app, target: action.target.clone(), title: action.title.clone(), shown: RefCell::new(shown) };
    dialog::run(Some(owner), &reminders);
}
