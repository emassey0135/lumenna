//! Adding or changing a block (§16.1: block editor; §13: creating is a form).
//!
//! Every setting a block has (§3.6), in two columns: what it is and when on the left — title,
//! start, length, kind, and the three flags — and how it repeats and behaves on the right. The
//! flags start from the kind, and follow it while it changes. The form for one day of a
//! repeating block has only the left column: that is all one day can change.
//!
//! What saving sends is the surface's (`new_block`, `block_edit`): only what changed, so a
//! concurrent edit to another field elsewhere stands.

use std::cell::RefCell;

use lumenna_surface::{BlockFields, BlockScope, Change, Lumenna, block_defaults, block_edit, new_block};
use windows::Win32::Foundation::HWND;
use windows::Win32::System::SystemServices::SS_NOPREFIX;
use windows::Win32::UI::WindowsAndMessaging::{
    BS_AUTOCHECKBOX, BS_DEFPUSHBUTTON, BS_PUSHBUTTON, CB_ADDSTRING, CB_GETCURSEL, CB_SETCURSEL,
    CBN_SELCHANGE, CBS_DROPDOWNLIST, ES_AUTOHSCROLL, ES_AUTOVSCROLL, ES_MULTILINE, ES_NUMBER,
    ES_WANTRETURN, IDCANCEL, IDOK, WS_BORDER, WS_TABSTOP, WS_VSCROLL,
};
use windows::core::HSTRING;

use super::controls;
use super::core::sentence;
use super::dialog::{self, Class, Dialog, Template};
use super::prompts;

const TITLE: u16 = 100;
const START: u16 = 101;
const MINUTES: u16 = 102;
const KIND: u16 = 103;
const REPEAT: u16 = 104;
const DATE: u16 = 105;
const TAKES_TASKS: u16 = 106;
const CAPACITY: u16 = 107;
const ANCHORED: u16 = 108;
const UNTIL: u16 = 109;
const SHORTEST: u16 = 110;
const FILTER: u16 = 111;
const COLOUR: u16 = 112;
const NOTES: u16 = 113;

const KINDS: [(&str, &str); 3] = [("work", "Work"), ("break", "Break"), ("event", "Event")];

/// A column's width, and where the second starts, in dialog units.
const COLUMN: i16 = 216;
const RIGHT: i16 = 7 + COLUMN + 14;

/// What the form is for.
pub enum Purpose {
    /// A new block, starting on this date phrase.
    Add { date: String },
    /// Every occurrence of a series.
    Series { id: String },
    /// One day of a repeating series alone.
    Occurrence { series: String, date: String },
}

/// A new block's fields: a work block at `start` for `minutes`, with a work block's flags.
pub fn fresh(start: &str, minutes: u32) -> BlockFields {
    let mut fields = BlockFields {
        start: start.to_owned(),
        minutes: minutes.to_string(),
        kind: "work".to_owned(),
        ..BlockFields::default()
    };
    if let Some(defaults) = block_defaults(fields.kind.clone()) {
        fields.accepts_tasks = defaults.accepts_tasks;
        fields.counts_capacity = defaults.counts_capacity;
        fields.anchored = defaults.anchored;
    }
    fields
}

struct Form<'a> {
    lumenna: &'a Lumenna,
    purpose: Purpose,
    initial: BlockFields,
    /// The RFC 5545 rule it repeats by, when the repetition words cannot say it: Repeats starts
    /// empty, the rule is said beside it, and it is left alone unless something is typed there.
    rule: Option<String>,
    heading: String,
    result: RefCell<Option<Change>>,
}

impl Form<'_> {
    fn once(&self) -> bool {
        matches!(self.purpose, Purpose::Occurrence { .. })
    }

    fn read(&self, hwnd: HWND) -> BlockFields {
        let text = |id| controls::text(dialog::item(hwnd, id));
        let kind = controls::send(dialog::item(hwnd, KIND), CB_GETCURSEL, 0, 0);
        let mut fields = BlockFields {
            title: text(TITLE),
            start: text(START),
            minutes: text(MINUTES),
            kind: usize::try_from(kind).ok().and_then(|i| KINDS.get(i)).map_or("work", |k| k.0).to_owned(),
            accepts_tasks: controls::checked(dialog::item(hwnd, TAKES_TASKS)),
            counts_capacity: controls::checked(dialog::item(hwnd, CAPACITY)),
            anchored: controls::checked(dialog::item(hwnd, ANCHORED)),
            ..self.initial.clone()
        };
        if !self.once() {
            fields.repeat = text(REPEAT);
            fields.until = text(UNTIL);
            fields.min_minutes = text(SHORTEST);
            fields.task_filter = text(FILTER);
            fields.colour = text(COLOUR);
            fields.notes = text(NOTES).replace("\r\n", "\n");
        }
        fields
    }

    /// What saving did: a change, or `None` when nothing differed.
    fn save(&self, hwnd: HWND) -> Result<Option<Change>, String> {
        let fields = self.read(hwnd);
        let saved = match &self.purpose {
            Purpose::Add { .. } => {
                let date = controls::text(dialog::item(hwnd, DATE));
                new_block(fields, Some(date)).and_then(|block| self.lumenna.add_block(block)).map(Some)
            }
            Purpose::Series { id } => block_edit(self.initial.clone(), fields)
                .and_then(|edit| edit.map(|edit| self.lumenna.edit_block(id, edit, BlockScope::Series)).transpose()),
            Purpose::Occurrence { series, date } => block_edit(self.initial.clone(), fields).and_then(|edit| {
                edit.map(|edit| self.lumenna.edit_block(series, edit, BlockScope::Occurrence { date: date.clone() }))
                    .transpose()
            }),
        };
        saved.map_err(|error| sentence(&error))
    }

    /// Sets the three flags to `kind`'s own.
    fn follow_kind(hwnd: HWND, kind: &str) {
        if let Some(defaults) = block_defaults(kind.to_owned()) {
            controls::check(dialog::item(hwnd, TAKES_TASKS), defaults.accepts_tasks);
            controls::check(dialog::item(hwnd, CAPACITY), defaults.counts_capacity);
            controls::check(dialog::item(hwnd, ANCHORED), defaults.anchored);
        }
    }
}

impl Dialog for Form<'_> {
    fn template(&self) -> Template {
        let field = |template: Template, label: &str, id: u16, style: u32, x: i16, y: i16| {
            template
                .item(Class::Static, label, u16::MAX, 0, x, y, COLUMN, 9)
                .item(Class::Edit, "", id, style | WS_BORDER.0 | WS_TABSTOP.0, x, y + 10, COLUMN, 14)
        };
        let check = |template: Template, label: &str, id: u16, y: i16| {
            template.item(Class::Button, label, id, BS_AUTOCHECKBOX as u32 | WS_TABSTOP.0, 7, y, COLUMN, 10)
        };
        let line = ES_AUTOHSCROLL as u32;
        let adding = matches!(self.purpose, Purpose::Add { .. });
        let once = self.once();
        let width = if once { 7 + COLUMN + 7 } else { RIGHT + COLUMN + 7 };
        let mut template = Template::new(&self.heading, width, 10);

        // What it is and when.
        let mut y = 7;
        template = field(template, "&Title:", TITLE, line, 7, y);
        y += 28;
        template = field(template, "Starts &at, such as 9am:", START, line, 7, y);
        y += 28;
        template = field(template, "&Minutes:", MINUTES, ES_NUMBER as u32, 7, y);
        y += 28;
        template = template
            .item(Class::Static, "&Kind:", u16::MAX, 0, 7, y, COLUMN, 9)
            .item(Class::ComboBox, "", KIND, CBS_DROPDOWNLIST as u32 | WS_VSCROLL.0 | WS_TABSTOP.0, 7, y + 10, COLUMN, 60);
        y += 30;
        template = check(template, "Takes ta&sks", TAKES_TASKS, y);
        y += 14;
        template = check(template, "&Counts toward the hours for work", CAPACITY, y);
        y += 14;
        template = check(template, "A&nchored: stays put when the day runs late", ANCHORED, y);
        y += 18;
        if adding {
            template = field(template, "Starts &on:", DATE, line, 7, y);
            y += 28;
        }
        let mut bottom = y;

        // How it repeats and behaves: every occurrence's alone.
        if !once {
            let mut y = 7;
            template = field(template, "&Repeats, such as every weekday; empty for once:", REPEAT, line, RIGHT, y);
            y += 28;
            if let Some(rule) = self.rule.as_ref().filter(|_| self.initial.repeat.is_empty()) {
                let note = format!("It repeats by the rule {rule}, which the repetition words cannot say. Leave Repeats empty to keep it.");
                template = template.item(Class::Static, &note, u16::MAX, SS_NOPREFIX.0, RIGHT, y, COLUMN, 26);
                y += 30;
            }
            template = field(template, "Last &day it repeats; empty for good:", UNTIL, line, RIGHT, y);
            y += 28;
            template = field(template, "Shortest &length when the day runs late, in minutes; empty for its kind's:", SHORTEST, ES_NUMBER as u32, RIGHT, y);
            y += 28;
            template = field(template, "Offers tasks matching this &filter, such as #Work; empty for any:", FILTER, line, RIGHT, y);
            y += 28;
            template = field(template, "Colo&ur, by name; empty for none:", COLOUR, line, RIGHT, y);
            y += 28;
            let notes = (ES_MULTILINE | ES_WANTRETURN | ES_AUTOVSCROLL) as u32 | WS_VSCROLL.0;
            template = template
                .item(Class::Static, "Not&es:", u16::MAX, 0, RIGHT, y, COLUMN, 9)
                .item(Class::Edit, "", NOTES, notes | WS_BORDER.0 | WS_TABSTOP.0, RIGHT, y + 10, COLUMN, 40);
            y += 54;
            bottom = bottom.max(y);
        }

        let ok = if adding { "Add" } else { "Save" };
        template = template
            .item(Class::Button, ok, IDOK.0 as u16, BS_DEFPUSHBUTTON as u32 | WS_TABSTOP.0, width - 111, bottom + 4, 50, 14)
            .item(Class::Button, "Cancel", IDCANCEL.0 as u16, BS_PUSHBUTTON as u32 | WS_TABSTOP.0, width - 57, bottom + 4, 50, 14);
        // The height is known only once the fields are in.
        template.resize(width, bottom + 25)
    }

    fn init(&self, hwnd: HWND) -> bool {
        let set = |id, text: &str| controls::set_text(dialog::item(hwnd, id), text);
        let initial = &self.initial;
        set(TITLE, &initial.title);
        set(START, &initial.start);
        set(MINUTES, &initial.minutes);
        if !self.once() {
            set(REPEAT, &initial.repeat);
            set(UNTIL, &initial.until);
            set(SHORTEST, &initial.min_minutes);
            set(FILTER, &initial.task_filter);
            set(COLOUR, &initial.colour);
            // A multi-line edit control breaks lines at CR LF.
            set(NOTES, &initial.notes.replace('\n', "\r\n"));
        }
        if let Purpose::Add { date } = &self.purpose {
            set(DATE, date);
        }
        let kind = dialog::item(hwnd, KIND);
        for (_, label) in KINDS {
            let text = HSTRING::from(label);
            controls::send(kind, CB_ADDSTRING, 0, text.as_ptr() as isize);
        }
        let selected = KINDS.iter().position(|(k, _)| *k == initial.kind).unwrap_or(0);
        controls::send(kind, CB_SETCURSEL, selected, 0);
        controls::check(dialog::item(hwnd, TAKES_TASKS), initial.accepts_tasks);
        controls::check(dialog::item(hwnd, CAPACITY), initial.counts_capacity);
        controls::check(dialog::item(hwnd, ANCHORED), initial.anchored);
        false
    }

    fn command(&self, hwnd: HWND, id: u16, code: u16) -> Option<isize> {
        match i32::from(id) {
            id if id == IDOK.0 => match self.save(hwnd) {
                Ok(change) => {
                    *self.result.borrow_mut() = change;
                    Some(1)
                }
                Err(message) => {
                    // The form stays, with what was typed, so it can be put right.
                    prompts::fail(hwnd, &message);
                    None
                }
            },
            id if id == IDCANCEL.0 => Some(0),
            // A new kind brings its own flags, which can then be set apart from it.
            id if id == i32::from(KIND) && u32::from(code) == CBN_SELCHANGE => {
                Self::follow_kind(hwnd, &self.read(hwnd).kind);
                None
            }
            _ => None,
        }
    }
}

/// Runs the form, and returns what saving it did; `None` if it was cancelled or nothing
/// changed. `rule` is the rule a series repeats by when the repetition words cannot say it.
pub fn run(owner: HWND, lumenna: &Lumenna, purpose: Purpose, initial: BlockFields, rule: Option<String>) -> Option<Change> {
    let heading = match &purpose {
        Purpose::Add { .. } => "New Block".to_owned(),
        Purpose::Series { .. } => format!("Change {}, Every Occurrence", initial.title),
        Purpose::Occurrence { .. } => format!("Change {}, This Day Only", initial.title),
    };
    let form = Form { lumenna, purpose, initial, rule, heading, result: RefCell::new(None) };
    dialog::run(Some(owner), &form);
    form.result.into_inner()
}
