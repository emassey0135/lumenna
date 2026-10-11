//! The day's two proposals, Replan from Now and Plan the Day: the app's own screen for the
//! day's `Question::Form` actions of those kinds. Nothing changes until the person applies
//! what they left checked.
//!
//! The screen says the whole proposal when it opens (its announcement, on the live status
//! line), then lists each change or sitting as a checked row of a checkbox tree, a sitting's
//! reason as the row's description and beside the list. What fits nowhere is plain lines
//! under the core's heading. Applying makes the rows still checked; the core then proposes
//! again around what was turned down, and while that proposes anything the screen shows it.
//! Every word here is the core's (`proposal_words`).

use std::cell::{Cell, RefCell};

use lumenna_surface::{
    ActionKind, Change, DayPins, DayPlan, ProposalWords, Replan, ReplanPins, Result, TimeStyle, proposal_words,
};
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::System::SystemServices::SS_NOPREFIX;
use windows::Win32::UI::Controls::{
    NMHDR, TVN_SELCHANGEDW, TVS_DISABLEDRAGDROP, TVS_NOHSCROLL, TVS_SHOWSELALWAYS,
};
use windows::Win32::UI::WindowsAndMessaging::{
    BS_DEFPUSHBUTTON, BS_PUSHBUTTON, ES_AUTOVSCROLL, ES_MULTILINE, ES_READONLY, IDCANCEL, IDOK, PostMessageW, WM_APP,
    WM_NOTIFY, WS_BORDER, WS_TABSTOP, WS_VSCROLL,
};

use super::app::App;
use super::clock::Locale;
use super::core::sentence;
use super::dialog::{self, Class, Dialog, Template};
use super::tree::{Item, Tree};
use super::{a11y, controls, prompts};
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
/// The announcement is said once the screen is up: said while it is being made, it went
/// unheard.
const WM_OPENED: u32 = WM_APP + 1;

/// What is proposed: the rest of today, or a day's plan.
#[derive(Clone)]
enum Proposed {
    Replan(Replan),
    Day(DayPlan),
}

impl Proposed {
    fn said(&self) -> String {
        let (announcement, notices) = match self {
            Self::Replan(replan) => (&replan.announcement, &replan.notices),
            Self::Day(plan) => (&plan.announcement, &plan.notices),
        };
        speech::sentence(&speech::announcement(announcement, notices))
    }

    /// Each row: what it says, and why, where the core says why.
    fn rows(&self) -> Vec<(String, String)> {
        match self {
            Self::Replan(replan) => replan.changes.iter().map(|c| (c.sentence.clone(), String::new())).collect(),
            Self::Day(plan) => plan.sittings.iter().map(|s| (s.sentence.clone(), s.reason.clone().unwrap_or_default())).collect(),
        }
    }

    fn unfit(&self) -> &[String] {
        match self {
            Self::Replan(replan) => &replan.unplaceable,
            Self::Day(plan) => &plan.unfit,
        }
    }

    /// Makes the rows at `checked`, and what the core proposes next.
    fn apply(&self, app: &App, checked: &[usize]) -> Result<(String, Change, Self)> {
        let lumenna = &app.core.lumenna;
        match self {
            Self::Replan(replan) => {
                let accepted = checked.iter().filter_map(|i| replan.changes.get(*i).cloned()).collect();
                let done = lumenna.apply_replan(replan.clone(), accepted, None)?;
                Ok((done.announcement, done.change, Self::Replan(done.next)))
            }
            Self::Day(plan) => {
                let accepted = checked.iter().filter_map(|i| plan.sittings.get(*i).cloned()).collect();
                let done = lumenna.apply_day_plan(plan.clone(), accepted, None)?;
                Ok((done.announcement, done.change, Self::Day(done.next)))
            }
        }
    }

    /// The same proposal asked again of the day as it is now, keeping what was decided.
    fn again(&self, app: &App) -> Result<Self> {
        let lumenna = &app.core.lumenna;
        Ok(match self {
            Self::Replan(replan) => Self::Replan(lumenna.replan(replan.style, replan.pins.clone(), None)?),
            Self::Day(plan) => {
                Self::Day(lumenna.propose_day_plan(Some(plan.date.clone()), plan.style, plan.pins.clone(), None)?)
            }
        })
    }
}

struct Screen<'a> {
    app: &'a App,
    words: ProposalWords,
    proposed: RefCell<Proposed>,
    list: RefCell<Option<Tree>>,
    /// Counts the proposals shown, so each one's rows are new rows, checked afresh.
    shown: Cell<u32>,
    /// What was last applied, said on the main window once the screen closes.
    applied: RefCell<Option<String>>,
}

impl Screen<'_> {
    fn say(&self, hwnd: HWND, text: &str) {
        let status = dialog::item(hwnd, STATUS);
        controls::set_text(status, text);
        a11y::changed(status);
    }

    /// Shows the proposal held: its rows all checked, or the core's words for nothing to do.
    fn fill(&self, hwnd: HWND) {
        let proposed = self.proposed.borrow().clone();
        let rows = proposed.rows();
        let generation = self.shown.get() + 1;
        self.shown.set(generation);
        let said = proposed.said();
        controls::set_text(dialog::item(hwnd, ANNOUNCED), &said);
        let any = !rows.is_empty();
        let apply = dialog::item(hwnd, IDOK.0 as u16);
        let list = dialog::item(hwnd, LIST);
        if !any && [apply, list].contains(&controls::focused()) {
            controls::focus(dialog::item(hwnd, IDCANCEL.0 as u16));
        }
        if let Some(tree) = self.list.borrow().as_ref() {
            let items = rows
                .iter()
                .enumerate()
                .map(|(index, (text, _))| Item { key: format!("{generation}:{index}"), text: text.clone(), depth: 0, checked: Some(true) })
                .collect();
            tree.set(items);
            for (index, (_, reason)) in rows.iter().enumerate().filter(|(_, (_, reason))| !reason.is_empty()) {
                tree.describe(index, reason);
            }
            if any {
                tree.select_quietly(0);
            }
        }
        let reasons = rows.iter().any(|(_, reason)| !reason.is_empty());
        for (id, visible) in [(LIST, any), (HINT, any), (IDOK.0 as u16, any), (REASON, any && reasons), (NOTHING, !any)] {
            controls::show(dialog::item(hwnd, id), visible);
        }
        // The core's words for nothing to do, unless the announcement already is them.
        let nothing = if said.trim_end_matches('.') == self.words.nothing.trim_end_matches('.') { "" } else { &self.words.nothing };
        controls::set_text(dialog::item(hwnd, NOTHING), nothing);
        let unfit = proposed.unfit().join("\r\n");
        controls::set_text(dialog::item(hwnd, UNFIT), &unfit);
        for id in [UNFIT_LABEL, UNFIT] {
            controls::show(dialog::item(hwnd, id), !unfit.is_empty());
        }
        self.show_reason(hwnd);
    }

    /// The chosen row's reason, beside the list.
    fn show_reason(&self, hwnd: HWND) {
        let index = self.list.borrow().as_ref().and_then(Tree::selected);
        let rows = self.proposed.borrow().rows();
        let reason = index.and_then(|i| rows.get(i)).map(|(_, reason)| reason.clone()).unwrap_or_default();
        controls::set_text(dialog::item(hwnd, REASON), &reason);
    }

    /// Makes what is checked. While the core proposes more, the screen shows that; once it
    /// proposes nothing the screen closes. A proposal the day has moved past is refused, and
    /// asked again.
    fn apply(&self, hwnd: HWND) -> Option<isize> {
        let checked: Vec<usize> = {
            let list = self.list.borrow();
            let tree = list.as_ref()?;
            (0..tree.len()).filter(|index| tree.is_checked(*index)).collect()
        };
        let proposed = self.proposed.borrow().clone();
        match proposed.apply(self.app, &checked) {
            Ok((announcement, change, next)) => {
                if change.changed {
                    self.app.store_changed();
                }
                let done = next.rows().is_empty();
                *self.proposed.borrow_mut() = next;
                if done {
                    *self.applied.borrow_mut() = Some(announcement);
                    return Some(1);
                }
                self.fill(hwnd);
                if let Some(tree) = self.list.borrow().as_ref() {
                    controls::focus(tree.hwnd);
                }
                self.say(hwnd, &announcement);
            }
            Err(error) => {
                prompts::fail(hwnd, &sentence(&error));
                match proposed.again(self.app) {
                    Ok(again) => {
                        *self.proposed.borrow_mut() = again;
                        self.fill(hwnd);
                        self.say(hwnd, &self.proposed.borrow().said());
                    }
                    Err(error) => {
                        prompts::fail(hwnd, &sentence(&error));
                        return Some(0);
                    }
                }
            }
        }
        None
    }
}

impl Dialog for Screen<'_> {
    fn template(&self) -> Template {
        let button = BS_PUSHBUTTON as u32 | WS_TABSTOP.0;
        let tree = (TVS_SHOWSELALWAYS | TVS_DISABLEDRAGDROP | TVS_NOHSCROLL) | WS_BORDER.0 | WS_TABSTOP.0;
        let lines = (ES_MULTILINE | ES_READONLY | ES_AUTOVSCROLL) as u32 | WS_VSCROLL.0 | WS_BORDER.0 | WS_TABSTOP.0;
        // The status line first, so Windows does not name it after the label before it. The
        // list's name is the screen's title, which says what checking a row does.
        Template::new(&self.words.title, 300, 236)
            .item(Class::Static, "", STATUS, SS_NOPREFIX.0, 7, 208, 230, 21)
            .item(Class::Static, "", ANNOUNCED, SS_NOPREFIX.0, 7, 7, 286, 36)
            .named("SysTreeView32", "", LIST, tree, 7, 46, 286, 84)
            .item(Class::Static, "", REASON, SS_NOPREFIX.0, 7, 133, 286, 18)
            .item(Class::Static, "", NOTHING, SS_NOPREFIX.0, 7, 46, 286, 18)
            .item(Class::Static, &format!("{}:", self.words.unfit), UNFIT_LABEL, SS_NOPREFIX.0, 7, 154, 286, 9)
            .item(Class::Edit, "", UNFIT, lines, 7, 164, 286, 22)
            .item(Class::Static, &self.words.hint, HINT, SS_NOPREFIX.0, 7, 190, 166, 18)
            .item(Class::Button, &self.words.apply, IDOK.0 as u16, (BS_DEFPUSHBUTTON as u32) | WS_TABSTOP.0, 177, 190, 116, 14)
            .item(Class::Button, &self.words.cancel, IDCANCEL.0 as u16, button, 243, 212, 50, 14)
    }

    fn init(&self, hwnd: HWND) -> bool {
        a11y::make_live(dialog::item(hwnd, STATUS));
        let tree = Tree::adopt(dialog::item(hwnd, LIST), &self.words.title, true);
        // What leaving a row unchecked does, said with the button that acts on the rest.
        a11y::set_description(dialog::item(hwnd, IDOK.0 as u16), &self.words.hint);
        *self.list.borrow_mut() = Some(tree);
        self.fill(hwnd);
        let first = if self.proposed.borrow().rows().is_empty() { IDCANCEL.0 as u16 } else { LIST };
        controls::focus(dialog::item(hwnd, first));
        unsafe {
            let _ = PostMessageW(Some(hwnd), WM_OPENED, WPARAM(0), LPARAM(0));
        }
        true
    }

    fn command(&self, hwnd: HWND, id: u16, _code: u16) -> Option<isize> {
        match i32::from(id) {
            id if id == IDOK.0 => self.apply(hwnd),
            id if id == IDCANCEL.0 => Some(0),
            _ => None,
        }
    }

    fn message(&self, hwnd: HWND, message: u32, _wparam: WPARAM, lparam: LPARAM) -> Option<isize> {
        match message {
            WM_OPENED => {
                self.say(hwnd, &self.proposed.borrow().said());
                Some(0)
            }
            WM_NOTIFY => {
                let header = unsafe { &*(lparam.0 as *const NMHDR) };
                if header.idFrom == usize::from(LIST) && header.code == TVN_SELCHANGEDW {
                    self.show_reason(hwnd);
                }
                None
            }
            _ => None,
        }
    }
}

/// The clock the person's Windows says times in, for the core to say the proposal in.
fn style() -> TimeStyle {
    if Locale.time("13:00").contains("13") { TimeStyle::TwentyFourHour } else { TimeStyle::TwelveHour }
}

fn show(app: &App, kind: ActionKind, proposed: Result<Proposed>) {
    let proposed = match proposed {
        Ok(proposed) => proposed,
        Err(error) => return prompts::fail(app.main, &sentence(&error)),
    };
    let screen = Screen {
        app,
        words: proposal_words(kind),
        proposed: RefCell::new(proposed),
        list: RefCell::new(None),
        shown: Cell::new(0),
        applied: RefCell::new(None),
    };
    dialog::run(Some(app.main), &screen);
    if let Some(applied) = screen.applied.into_inner() {
        app.say(&applied);
    }
}

/// Replan from Now: the rest of today, worked out again from now.
pub fn replan(app: &App) {
    let proposed = app.core.lumenna.replan(style(), ReplanPins::default(), None).map(Proposed::Replan);
    show(app, ActionKind::Replan, proposed);
}

/// Plan the Day: `date`'s work blocks still to come, each given its most urgent tasks.
pub fn plan_day(app: &App, date: &str) {
    let proposed = app.core.lumenna.propose_day_plan(Some(date.to_owned()), style(), DayPins::default(), None).map(Proposed::Day);
    show(app, ActionKind::PlanDay, proposed);
}
