//! The day's two proposals, Replan from Now and Plan the Day, drawn from the app's proposal
//! screen (`view.proposal`): what is proposed, what is checked, applying, the other options and
//! what comes next are the app's. Nothing changes until the person applies what they left
//! checked.
//!
//! The screen says the whole proposal when it opens (its announcement, on its own live status
//! line), then lists each change or sitting as a checked row of a checkbox tree, a sitting's
//! reason as the row's description and beside the list. What fits nowhere is plain lines
//! under the core's heading. Applying makes the rows still checked; the app then proposes
//! again around what was turned down, and while that proposes anything the screen shows it.
//! Replan from Now offers the core's other options too, a button each under its heading:
//! taking one shows the proposal it stands for in place. While the screen is open, what the
//! app says goes to its status line: the main window's is hidden behind it.
//! Every word here is the core's (`proposal_words`).

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use lumenna_app::{ProposalEvent, ProposalView};
use lumenna_ffi::ShellEvent;
use lumenna_records::ProposalWords;
use lumenna_records::proposals::TimeStyle;
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::System::SystemServices::SS_NOPREFIX;
use windows::Win32::UI::Controls::{
    NM_TVSTATEIMAGECHANGING, NMHDR, NMTVSTATEIMAGECHANGING, TVN_SELCHANGEDW, TVS_DISABLEDRAGDROP, TVS_NOHSCROLL,
    TVS_SHOWSELALWAYS,
};
use windows::Win32::UI::WindowsAndMessaging::{
    BN_CLICKED, BS_DEFPUSHBUTTON, BS_MULTILINE, BS_PUSHBUTTON, ES_AUTOVSCROLL, ES_MULTILINE, ES_READONLY, EndDialog, IDCANCEL,
    IDOK, PostMessageW, WM_APP, WM_NOTIFY, WS_BORDER, WS_TABSTOP, WS_VSCROLL,
};

use super::app::{self, App};
use super::clock::Locale;
use super::dialog::{self, Class, Dialog, Template};
use super::tree::{Item, Tree};
use super::{a11y, controls};
use crate::speech::{self, Clock};

const STATUS: u16 = 100;
const ANNOUNCED: u16 = 101;
const LIST: u16 = 102;
/// The row chosen's reason, shown beside the list for whoever reads the screen by eye.
const REASON: u16 = 103;
const NOTHING: u16 = 104;
const UNFIT_LABEL: u16 = 105;
const UNFIT: u16 = 106;
const HINT: u16 = 107;
const OTHERS_LABEL: u16 = 108;
/// The other options, a button each, from here on: at most three, as the core offers.
const OTHERS: u16 = 110;
const OTHER_COUNT: u16 = 3;
/// The announcement is said once the screen is up: said while it is being made, it went
/// unheard.
const WM_OPENED: u32 = WM_APP + 1;

/// The screen open, while it is.
pub struct Screen {
    hwnd: Cell<HWND>,
    words: ProposalWords,
    /// What it was first drawn from, until the dialog exists.
    first: ProposalView,
    list: RefCell<Option<Tree>>,
    /// The proposal last drawn, by the app's count: a new one is a new list.
    shown: Cell<u32>,
    /// Each row's reason, as last drawn.
    reasons: RefCell<Vec<String>>,
    /// Whether the app has closed it.
    closed: Cell<bool>,
    /// Its status line, which outlives nothing: kept to say where it went once it has gone.
    status: Cell<HWND>,
}

impl Screen {
    /// Draws the app's screen as it now is, or closes the dialog when the app has closed it:
    /// what the app says as it closes is said on the window once it has gone.
    pub fn draw(&self, app: &App, view: Option<&ProposalView>) {
        let hwnd = self.hwnd.get();
        if hwnd.is_invalid() {
            return;
        }
        match view {
            Some(view) => self.fill(hwnd, view),
            None => {
                if !self.closed.replace(true) {
                    app.closing_live(self.status.get());
                    unsafe {
                        let _ = EndDialog(hwnd, 1);
                    }
                }
            }
        }
    }

    /// Shows a proposal: a new one as a new list, its rows as checked as the app has them, or
    /// the core's words for nothing to do.
    fn fill(&self, hwnd: HWND, view: &ProposalView) {
        let new = self.shown.replace(view.shown) != view.shown;
        let any = !view.changes.is_empty();
        let unfit = view.unfit.join("\r\n");
        let reasons: Vec<String> = view.changes.iter().map(|change| change.reason.clone().unwrap_or_default()).collect();
        let has_reasons = reasons.iter().any(|reason| !reason.is_empty());
        let mut shown = vec![
            (LIST, any),
            (HINT, any),
            (IDOK.0 as u16, any),
            (REASON, any && has_reasons),
            (NOTHING, !any),
            (UNFIT_LABEL, !unfit.is_empty()),
            (UNFIT, !unfit.is_empty()),
            (OTHERS_LABEL, !view.others.is_empty()),
        ];
        shown.extend((0..OTHER_COUNT).map(|i| (OTHERS + i, usize::from(i) < view.others.len())));
        // Focus first leaves whatever is about to be hidden, or it would go nowhere.
        let focused = controls::focused();
        if shown.iter().any(|(id, visible)| !visible && dialog::item(hwnd, *id) == focused) {
            controls::focus(dialog::item(hwnd, if any { LIST } else { IDCANCEL.0 as u16 }));
        }
        let announced = speech::sentence(&view.announcement);
        set_if_changed(dialog::item(hwnd, ANNOUNCED), &announced);
        if let Some(tree) = self.list.borrow().as_ref() {
            let items = view
                .changes
                .iter()
                .enumerate()
                .map(|(index, change)| Item {
                    key: format!("{}:{index}", view.shown),
                    text: change.sentence.clone(),
                    depth: 0,
                    checked: Some(change.checked),
                    expanded: None,
                    swatch: None,
                })
                .collect();
            tree.set(items);
            if new {
                for (index, reason) in reasons.iter().enumerate().filter(|(_, reason)| !reason.is_empty()) {
                    tree.describe(index, reason);
                }
                if any {
                    tree.select_quietly(0);
                }
            }
        }
        *self.reasons.borrow_mut() = reasons;
        for (index, option) in view.others.iter().enumerate().take(usize::from(OTHER_COUNT)) {
            // Named by the whole option, which a button's text is; an ampersand is its own.
            set_if_changed(dialog::item(hwnd, OTHERS + index as u16), &option.replace('&', "&&"));
        }
        for (id, visible) in shown {
            controls::show(dialog::item(hwnd, id), visible);
        }
        // The core's words for nothing to do, unless the announcement already is them.
        let nothing = if announced.trim_end_matches('.') == self.words.nothing.trim_end_matches('.') { "" } else { &self.words.nothing };
        set_if_changed(dialog::item(hwnd, NOTHING), nothing);
        set_if_changed(dialog::item(hwnd, UNFIT), &unfit);
        self.show_reason(hwnd);
        // A new proposal after one was applied or another option taken: on it, in the list.
        if new && view.shown > 1 {
            controls::focus(dialog::item(hwnd, if any { LIST } else { IDCANCEL.0 as u16 }));
        }
    }

    /// The chosen row's reason, beside the list.
    fn show_reason(&self, hwnd: HWND) {
        let index = self.list.borrow().as_ref().and_then(Tree::selected);
        let reason = index.and_then(|i| self.reasons.borrow().get(i).cloned()).unwrap_or_default();
        set_if_changed(dialog::item(hwnd, REASON), &reason);
    }
}

/// Writes a control's text only when it differs.
fn set_if_changed(control: HWND, text: &str) {
    if controls::text(control) != text {
        controls::set_text(control, text);
    }
}

impl Dialog for Screen {
    fn template(&self) -> Template {
        let button = BS_PUSHBUTTON as u32 | WS_TABSTOP.0;
        let tree = (TVS_SHOWSELALWAYS | TVS_DISABLEDRAGDROP | TVS_NOHSCROLL) | WS_BORDER.0 | WS_TABSTOP.0;
        let lines = (ES_MULTILINE | ES_READONLY | ES_AUTOVSCROLL) as u32 | WS_VSCROLL.0 | WS_BORDER.0 | WS_TABSTOP.0;
        // The status line first, so Windows does not name it after the label before it. The
        // list's name is the screen's title, which says what checking a row does.
        let others = if self.words.others.is_empty() { String::new() } else { format!("{}:", self.words.others) };
        let mut template = Template::new(&self.words.title, 300, 318)
            .item(Class::Static, "", STATUS, SS_NOPREFIX.0, 7, 290, 230, 21)
            .item(Class::Static, "", ANNOUNCED, SS_NOPREFIX.0, 7, 7, 286, 36)
            .named("SysTreeView32", "", LIST, tree, 7, 46, 286, 84)
            .item(Class::Static, "", REASON, SS_NOPREFIX.0, 7, 133, 286, 18)
            .item(Class::Static, "", NOTHING, SS_NOPREFIX.0, 7, 46, 286, 18)
            .item(Class::Static, &format!("{}:", self.words.unfit), UNFIT_LABEL, SS_NOPREFIX.0, 7, 154, 286, 9)
            .item(Class::Edit, "", UNFIT, lines, 7, 164, 286, 22)
            .item(Class::Static, &self.words.hint, HINT, SS_NOPREFIX.0, 7, 190, 166, 18)
            .item(Class::Button, &self.words.apply, IDOK.0 as u16, (BS_DEFPUSHBUTTON as u32) | WS_TABSTOP.0, 177, 190, 116, 14)
            .item(Class::Static, &others, OTHERS_LABEL, SS_NOPREFIX.0, 7, 212, 286, 9);
        // Two lines each: an option says what it costs and what follows.
        for index in 0..OTHER_COUNT {
            let y = 222 + 22 * index as i16;
            template = template.item(Class::Button, "", OTHERS + index, button | BS_MULTILINE as u32, 7, y, 286, 20);
        }
        template.item(Class::Button, &self.words.cancel, IDCANCEL.0 as u16, button, 243, 294, 50, 14)
    }

    fn init(&self, hwnd: HWND) -> bool {
        self.hwnd.set(hwnd);
        a11y::make_live(dialog::item(hwnd, STATUS));
        self.status.set(dialog::item(hwnd, STATUS));
        app::with(|app| app.open_live(self.status.get()));
        let tree = Tree::adopt(dialog::item(hwnd, LIST), &self.words.title, true);
        // What leaving a row unchecked does, said with the button that acts on the rest.
        a11y::set_description(dialog::item(hwnd, IDOK.0 as u16), &self.words.hint);
        *self.list.borrow_mut() = Some(tree);
        self.fill(hwnd, &self.first);
        let first = if self.first.changes.is_empty() { IDCANCEL.0 as u16 } else { LIST };
        controls::focus(dialog::item(hwnd, first));
        unsafe {
            let _ = PostMessageW(Some(hwnd), WM_OPENED, WPARAM(0), LPARAM(0));
        }
        true
    }

    fn command(&self, _hwnd: HWND, id: u16, code: u16) -> Option<isize> {
        match i32::from(id) {
            id if id == IDOK.0 => {
                app::send(ShellEvent::Proposal(ProposalEvent::Apply));
                None
            }
            id if id == IDCANCEL.0 => {
                self.closed.set(true);
                app::send(ShellEvent::Proposal(ProposalEvent::Close));
                Some(0)
            }
            _ => {
                if let Some(index) = id.checked_sub(OTHERS).filter(|i| *i < OTHER_COUNT && u32::from(code) == BN_CLICKED) {
                    app::send(ShellEvent::Proposal(ProposalEvent::Take(u32::from(index))));
                }
                None
            }
        }
    }

    fn message(&self, hwnd: HWND, message: u32, _wparam: WPARAM, lparam: LPARAM) -> Option<isize> {
        match message {
            WM_OPENED => {
                let status = dialog::item(hwnd, STATUS);
                controls::set_text(status, &speech::sentence(&self.first.announcement));
                a11y::changed(status);
                Some(0)
            }
            WM_NOTIFY => {
                let header = unsafe { &*(lparam.0 as *const NMHDR) };
                if header.idFrom != usize::from(LIST) {
                    return None;
                }
                let list = self.list.borrow();
                let tree = list.as_ref()?;
                match header.code {
                    TVN_SELCHANGEDW => self.show_reason(hwnd),
                    // Space or a click on a row's box, which the tree flips itself.
                    NM_TVSTATEIMAGECHANGING if !tree.busy() => {
                        let notice = unsafe { &*(lparam.0 as *const NMTVSTATEIMAGECHANGING) };
                        if let Some(index) = tree.index_of_handle(notice.hti) {
                            let on = !tree.is_checked(index);
                            app::send(ShellEvent::Proposal(ProposalEvent::Check { index: index as u32, on }));
                        }
                    }
                    _ => {}
                }
                None
            }
            _ => None,
        }
    }
}

/// The clock the person's Windows says times in, for the core to say the proposal in.
pub fn style() -> TimeStyle {
    if Locale.time("13:00").contains("13") { TimeStyle::TwentyFourHour } else { TimeStyle::TwelveHour }
}

/// Runs the app's open proposal screen as a dialog until the app closes it or the person does.
pub fn run(app: &App, view: ProposalView) {
    let screen = Rc::new(Screen {
        hwnd: Cell::new(HWND::default()),
        words: view.words.clone(),
        first: view,
        list: RefCell::new(None),
        shown: Cell::new(0),
        reasons: RefCell::new(Vec::new()),
        closed: Cell::new(false),
        status: Cell::new(HWND::default()),
    });
    app.open_proposal(Some(Rc::clone(&screen)));
    dialog::run(Some(app.main), &*screen);
    app.open_proposal(None);
    app.close_live(screen.status.get());
}
