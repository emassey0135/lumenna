//! A task's or a block's reminders, drawn from the app's open reminder list
//! (`view.reminders`): its own or the defaults it takes, with what the list offers (Add
//! Reminder) and what the reminder chosen offers (Remove Reminder) as buttons, run as the
//! app's, the reminder held first so the change lands back on the list. Every word in it and
//! every action it runs are the core's; what the app says while it is open goes to its own
//! status line.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use lumenna_app::{ActingOn, Side};
use lumenna_app::lines::Lines;
use lumenna_ffi::ShellEvent;
use lumenna_records::reminders::{ReminderList, ReminderView};
use lumenna_records::{ActionKind, Offered};
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::System::SystemServices::SS_NOPREFIX;
use windows::Win32::UI::Input::KeyboardAndMouse::VK_DELETE;
use windows::Win32::UI::WindowsAndMessaging::{
    BN_CLICKED, BS_PUSHBUTTON, EndDialog, IDCANCEL, LB_ADDSTRING, LB_GETCURSEL, LB_RESETCONTENT, LB_SETCURSEL,
    LBN_SELCHANGE, LBS_NOINTEGRALHEIGHT, LBS_NOTIFY, LBS_WANTKEYBOARDINPUT, WM_VKEYTOITEM, WS_BORDER, WS_TABSTOP, WS_VSCROLL,
};
use windows::core::HSTRING;

use super::actions;
use super::app::{self, App};
use super::dialog::{self, Class, Dialog, Template};
use super::{a11y, controls};
use crate::{devices, speech};

const STATUS: u16 = 100;
const ANNOUNCED: u16 = 101;
const LIST: u16 = 102;
/// The chosen reminder's own action.
const ITS: u16 = 103;
/// The list's actions, from here on.
const LISTS: u16 = 110;

/// The list open, while it is.
pub struct Reminders {
    hwnd: Cell<HWND>,
    status: Cell<HWND>,
    /// Its title, the core's: "Reminders for Pay rent".
    title: String,
    /// What it was first drawn from: which buttons it has.
    first: ReminderList,
    shown: RefCell<ReminderList>,
    /// The landing count of the list's row in hand when last drawn.
    landing: Cell<u32>,
    closed: Cell<bool>,
}

impl Reminders {
    fn chosen(&self, hwnd: HWND) -> Option<ReminderView> {
        let index = usize::try_from(controls::send(dialog::item(hwnd, LIST), LB_GETCURSEL, 0, 0)).ok()?;
        self.shown.borrow().reminders.get(index).cloned()
    }

    /// The action a chosen reminder's button stands for: the first any of them offers, since
    /// a default offers none.
    fn its_action(&self) -> Option<Offered> {
        self.shown.borrow().reminders.iter().find_map(|r| r.offered.first().cloned())
    }

    /// Draws the app's list as it now is, or closes the dialog when the app has closed it.
    pub fn draw(&self, app: &App, list: Option<&ReminderList>, lines: &Lines, held: &lumenna_app::HeldView) {
        let hwnd = self.hwnd.get();
        if hwnd.is_invalid() || self.closed.get() {
            return;
        }
        let Some(list) = list else {
            self.closed.set(true);
            app.closing_live(self.status.get());
            unsafe {
                let _ = EndDialog(hwnd, 1);
            }
            return;
        };
        self.fill(hwnd, list, lines);
        // Where a change made here lands: the same reminder, or the one now in its place.
        if held.landing != self.landing.replace(held.landing)
            && let Some(at) = held.focus.as_ref().and_then(|key| list.reminders.iter().position(|r| &r.id == key))
        {
            controls::send(dialog::item(hwnd, LIST), LB_SETCURSEL, at, 0);
            self.offer(hwnd);
        }
    }

    fn fill(&self, hwnd: HWND, list: &ReminderList, lines: &Lines) {
        let announced = speech::sentence(&speech::announcement(&list.announcement, &list.notices));
        let label = dialog::item(hwnd, ANNOUNCED);
        if controls::text(label) != announced {
            controls::set_text(label, &announced);
            // Said with the list, as well as shown above it.
            a11y::set_description(dialog::item(hwnd, LIST), &announced);
        }
        let same = self.shown.borrow().reminders == list.reminders;
        if !same || self.shown.borrow().reminders.is_empty() {
            let field = dialog::item(hwnd, LIST);
            let kept = usize::try_from(controls::send(field, LB_GETCURSEL, 0, 0)).unwrap_or(0);
            controls::send(field, LB_RESETCONTENT, 0, 0);
            for line in &lines.reminders {
                let text = HSTRING::from(line.text.as_str());
                controls::send(field, LB_ADDSTRING, 0, text.as_ptr() as isize);
            }
            controls::send(field, LB_SETCURSEL, kept.min(list.reminders.len().saturating_sub(1)), 0);
        }
        *self.shown.borrow_mut() = list.clone();
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
        let has = self.chosen(hwnd).is_some_and(|r| !r.offered.is_empty());
        if has || controls::focused() != button {
            controls::enable(button, has);
        }
    }

    /// The chosen reminder's action of `kind`, as the app's, or why it has none.
    fn act(&self, hwnd: HWND, kind: ActionKind) {
        let Some(reminder) = self.chosen(hwnd) else { return };
        match actions::of_kind(&reminder.offered, &[kind]) {
            Some(action) => app::send(ShellEvent::ActOn(ActingOn::Held(Side::Reminders, reminder.id), action)),
            // Said by the app, on this dialog's status line.
            None => {
                app::send(ShellEvent::Hold(Side::Reminders, reminder.id));
                app::send(ShellEvent::Run(vec![kind]));
            }
        }
    }
}

impl Dialog for Reminders {
    fn template(&self) -> Template {
        let list = (LBS_NOTIFY | LBS_NOINTEGRALHEIGHT | LBS_WANTKEYBOARDINPUT) as u32 | WS_VSCROLL.0 | WS_BORDER.0 | WS_TABSTOP.0;
        let button = BS_PUSHBUTTON as u32 | WS_TABSTOP.0;
        // The status line first, so Windows does not name it after the label before it.
        let mut template = Template::new(&self.title, 260, 192)
            .item(Class::Static, "", STATUS, SS_NOPREFIX.0, 7, 148, 246, 18)
            .item(Class::Static, &self.first.announcement, ANNOUNCED, SS_NOPREFIX.0, 7, 7, 246, 9)
            .item(Class::Static, "&Reminders:", u16::MAX, 0, 7, 20, 246, 9)
            .item(Class::ListBox, "", LIST, list, 7, 30, 246, 90);
        let mut x = 7;
        for (index, action) in self.first.offered.iter().enumerate() {
            template = template.item(Class::Button, &devices::marked(&actions::label(action), 'A', '&'), LISTS + index as u16, button, x, 126, 80, 14);
            x += 84;
        }
        template
            .item(Class::Button, "", ITS, button, x, 126, 80, 14)
            .item(Class::Button, &lumenna_records::words::close(false), IDCANCEL.0 as u16, button, 203, 172, 50, 14)
    }

    fn init(&self, hwnd: HWND) -> bool {
        self.hwnd.set(hwnd);
        let status = dialog::item(hwnd, STATUS);
        self.status.set(status);
        a11y::make_live(status);
        app::with(|app| {
            app.open_live(status);
            self.fill(hwnd, &self.first, &app.crux.lines());
        });
        // Where there is nothing to choose, the first thing to do is add one.
        let first = if self.first.reminders.is_empty() { LISTS } else { LIST };
        controls::focus(dialog::item(hwnd, first));
        true
    }

    fn command(&self, hwnd: HWND, id: u16, code: u16) -> Option<isize> {
        if id == LIST && u32::from(code) == LBN_SELCHANGE {
            if let Some(reminder) = self.chosen(hwnd) {
                app::send(ShellEvent::Hold(Side::Reminders, reminder.id));
            }
            self.offer(hwnd);
            return None;
        }
        if i32::from(id) == IDCANCEL.0 {
            self.closed.set(true);
            return Some(0);
        }
        if u32::from(code) != BN_CLICKED {
            return None;
        }
        if id == ITS {
            if let Some(kind) = self.its_action().map(|a| a.kind) {
                self.act(hwnd, kind);
            }
        } else if let Some(action) = id.checked_sub(LISTS).and_then(|i| self.shown.borrow().offered.get(usize::from(i)).cloned()) {
            // The list's own, acted on from it, so what it adds lands on the list.
            app::send(ShellEvent::ActOn(ActingOn::Reminders, action));
        }
        None
    }

    fn message(&self, hwnd: HWND, message: u32, wparam: WPARAM, _lparam: LPARAM) -> Option<isize> {
        match message {
            // Delete removes, as in every other list.
            WM_VKEYTOITEM if controls::low_word(wparam.0) == VK_DELETE.0 => {
                // What Delete runs is the core's (`keys::runs`).
                if let Some(reminder) = self.chosen(hwnd) {
                    app::send(ShellEvent::Hold(Side::Reminders, reminder.id));
                    app::send(ShellEvent::Key("delete".into()));
                }
                Some(-2)
            }
            WM_VKEYTOITEM => Some(-1),
            _ => None,
        }
    }
}

/// Runs the app's open reminder list as a dialog until the app closes it or the person does.
pub fn run(app: &App, list: ReminderList) {
    let reminders = Rc::new(Reminders {
        title: app.crux.view().reminders_title.unwrap_or_default(),
        hwnd: Cell::new(HWND::default()),
        status: Cell::new(HWND::default()),
        shown: RefCell::new(ReminderList { reminders: Vec::new(), ..list.clone() }),
        first: list,
        landing: Cell::new(app.crux.view().reminders_held.landing),
        closed: Cell::new(false),
    });
    app.open_reminders(Some(Rc::clone(&reminders)));
    dialog::run(Some(app.owner()), &*reminders);
    app.open_reminders(None);
    if app.crux.view().reminders.is_some() {
        app.crux.send(ShellEvent::CloseReminders);
    }
    app.close_live(reminders.status.get());
}
