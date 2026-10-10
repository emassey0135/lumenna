//! Adding or changing a block.
//!
//! Every setting a block has, in two columns: what it is and when on the left — title,
//! start, length, kind, and the three flags — and how it repeats and behaves on the right. The
//! flags start from the kind, and follow it while it changes. The form for one day of a
//! repeating block has only the left column: that is all one day can change.
//!
//! What saving sends is the surface's (`new_block`, `block_edit`): only what changed, so a
//! concurrent edit to another field elsewhere stands.

use std::cell::{Cell, RefCell};

use lumenna_surface::{BlockFields, BlockScope, Change, Choice, FieldKind, Lumenna, block_defaults, block_edit, block_form, new_block};
use windows::Win32::Foundation::HWND;
use windows::Win32::System::SystemServices::SS_NOPREFIX;
use windows::Win32::UI::WindowsAndMessaging::{
    BS_AUTOCHECKBOX, BS_DEFPUSHBUTTON, BS_PUSHBUTTON, CB_ADDSTRING, CB_GETCURSEL, CB_SETCURSEL,
    CBN_SELCHANGE, CBS_DROPDOWNLIST, EN_CHANGE, ES_AUTOHSCROLL, ES_AUTOVSCROLL, ES_MULTILINE, ES_NUMBER,
    ES_WANTRETURN, IDCANCEL, IDOK, WS_BORDER, WS_TABSTOP, WS_VSCROLL,
};
use windows::core::HSTRING;

use super::{a11y, controls};
use super::core::sentence;
use super::dialog::{self, Class, Dialog, Template};
use super::prompts;
use crate::devices;

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

/// Each field's control, by the key the core's `block_form()` names it by, with the letter
/// that is its access key here: the words are the core's, the keys this dialog's.
const FIELDS: [(&str, u16, char); 14] = [
    ("title", TITLE, 'N'),
    ("start", START, 'a'),
    ("minutes", MINUTES, 'L'),
    ("kind", KIND, 'K'),
    ("accepts_tasks", TAKES_TASKS, 'T'),
    ("counts_capacity", CAPACITY, 'C'),
    ("anchored", ANCHORED, 'h'),
    ("date", DATE, 'D'),
    ("repeat", REPEAT, 'R'),
    ("until", UNTIL, 'U'),
    ("min_minutes", SHORTEST, 'g'),
    ("task_filter", FILTER, 'f'),
    ("colour", COLOUR, 'o'),
    ("notes", NOTES, 'e'),
];

/// The kinds a block can be, as the core names them.
fn kinds() -> Vec<Choice> {
    block_form().into_iter().find(|f| f.key == "kind").map(|f| f.options).unwrap_or_default()
}

/// A column's width, and where the second starts, in dialog units.
const COLUMN: i16 = 216;
const RIGHT: i16 = 7 + COLUMN + 14;

/// What the form is for.
pub enum Purpose {
    /// A new block, starting on this date phrase. While `follow` holds, Starts at is the
    /// core's `new_block_start` for whatever Day says, until the person changes it.
    Add { date: String, follow: bool },
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
    /// The core's note for a block repeating by a rule the repetition words cannot say:
    /// Repeats starts empty, the note is shown under it, and the rule is left alone unless
    /// something is typed there.
    note: Option<String>,
    heading: String,
    result: RefCell<Option<Change>>,
    /// Whether Starts at still follows Day: a new block's, until the person changes it.
    follow: Cell<bool>,
    /// Set while the form writes Starts at itself, so that is not taken as the person's.
    writing: Cell<bool>,
}

impl Form<'_> {
    fn once(&self) -> bool {
        matches!(self.purpose, Purpose::Occurrence { .. })
    }

    /// Whether the form has the field `key`: every one, but for one day of a repeating block
    /// only what the core says one day can change.
    fn shows(&self, key: &str) -> bool {
        !self.once() || block_form().iter().any(|f| f.key == key && f.one_day)
    }

    fn read(&self, hwnd: HWND) -> BlockFields {
        let text = |id| controls::text(dialog::item(hwnd, id));
        let kind = controls::send(dialog::item(hwnd, KIND), CB_GETCURSEL, 0, 0);
        let mut fields = BlockFields {
            title: text(TITLE),
            start: text(START),
            minutes: text(MINUTES),
            kind: usize::try_from(kind).ok().and_then(|i| kinds().get(i).map(|k| k.id.clone())).unwrap_or_else(|| "work".to_owned()),
            accepts_tasks: controls::checked(dialog::item(hwnd, TAKES_TASKS)),
            counts_capacity: controls::checked(dialog::item(hwnd, CAPACITY)),
            anchored: controls::checked(dialog::item(hwnd, ANCHORED)),
            ..self.initial.clone()
        };
        // What the form does not show keeps what it had.
        let read = |key: &str, id: u16, field: &mut String| {
            if self.shows(key) {
                *field = text(id).replace("\r\n", "\n");
            }
        };
        read("repeat", REPEAT, &mut fields.repeat);
        read("until", UNTIL, &mut fields.until);
        read("min_minutes", SHORTEST, &mut fields.min_minutes);
        read("task_filter", FILTER, &mut fields.task_filter);
        read("colour", COLOUR, &mut fields.colour);
        read("notes", NOTES, &mut fields.notes);
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

    /// Writes Starts at, as the form's own write rather than the person's.
    fn set_start(&self, hwnd: HWND, start: &str) {
        self.writing.set(true);
        controls::set_text(dialog::item(hwnd, START), start);
        self.writing.set(false);
    }

    /// Starts at as the core has it for the day Day says, while it still follows Day. A day
    /// half typed, which the core cannot read yet, leaves it as it is.
    fn follow_day(&self, hwnd: HWND) {
        if !self.follow.get() {
            return;
        }
        let date = controls::text(dialog::item(hwnd, DATE));
        if let Ok(start) = self.lumenna.new_block_start(Some(date)) {
            self.set_start(hwnd, &start);
        }
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
        let form = block_form();
        // A field's name as the core gives it, with this dialog's access key; a colon before a
        // field, none on a check box.
        let label = |key: &str, colon: bool| {
            let name = form.iter().find(|f| f.key == key).map_or(key, |f| f.label.as_str());
            let letter = FIELDS.iter().find(|(k, _, _)| *k == key).map_or(' ', |(_, _, l)| *l);
            format!("{}{}", devices::marked(name, letter, '&'), if colon { ":" } else { "" })
        };
        let line = ES_AUTOHSCROLL as u32;
        let adding = matches!(self.purpose, Purpose::Add { .. });
        // The right-hand column: how it repeats and behaves, which one day alone cannot change.
        const RIGHT_KEYS: [&str; 6] = ["repeat", "until", "min_minutes", "task_filter", "colour", "notes"];
        let right = RIGHT_KEYS.iter().any(|key| self.shows(key));
        let width = if right { RIGHT + COLUMN + 7 } else { 7 + COLUMN + 7 };
        let mut template = Template::new(&self.heading, width, 10);

        // What it is and when.
        let mut y = 7;
        template = field(template, &label("title", true), TITLE, line, 7, y);
        y += 28;
        template = field(template, &label("start", true), START, line, 7, y);
        y += 28;
        template = field(template, &label("minutes", true), MINUTES, ES_NUMBER as u32, 7, y);
        y += 28;
        template = template
            .item(Class::Static, &label("kind", true), u16::MAX, 0, 7, y, COLUMN, 9)
            .item(Class::ComboBox, "", KIND, CBS_DROPDOWNLIST as u32 | WS_VSCROLL.0 | WS_TABSTOP.0, 7, y + 10, COLUMN, 60);
        y += 30;
        template = check(template, &label("accepts_tasks", false), TAKES_TASKS, y);
        y += 14;
        template = check(template, &label("counts_capacity", false), CAPACITY, y);
        y += 14;
        template = check(template, &label("anchored", false), ANCHORED, y);
        y += 18;
        if adding {
            template = field(template, &label("date", true), DATE, line, 7, y);
            y += 28;
        }
        let mut bottom = y;

        if right {
            let mut y = 7;
            if self.shows("repeat") {
                template = field(template, &label("repeat", true), REPEAT, line, RIGHT, y);
                y += 28;
                if let Some(note) = &self.note {
                    template = template.item(Class::Static, note, u16::MAX, SS_NOPREFIX.0, RIGHT, y, COLUMN, 26);
                    y += 30;
                }
            }
            for (key, id, style) in [
                ("until", UNTIL, line),
                ("min_minutes", SHORTEST, ES_NUMBER as u32),
                ("task_filter", FILTER, line),
                ("colour", COLOUR, line),
            ] {
                if self.shows(key) {
                    template = field(template, &label(key, true), id, style, RIGHT, y);
                    y += 28;
                }
            }
            if self.shows("notes") {
                let notes = (ES_MULTILINE | ES_WANTRETURN | ES_AUTOVSCROLL) as u32 | WS_VSCROLL.0;
                template = template
                    .item(Class::Static, &label("notes", true), u16::MAX, 0, RIGHT, y, COLUMN, 9)
                    .item(Class::Edit, "", NOTES, notes | WS_BORDER.0 | WS_TABSTOP.0, RIGHT, y + 10, COLUMN, 40);
                y += 54;
            }
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
        self.set_start(hwnd, &initial.start);
        set(MINUTES, &initial.minutes);
        for (key, id, value) in [
            ("repeat", REPEAT, &initial.repeat),
            ("until", UNTIL, &initial.until),
            ("min_minutes", SHORTEST, &initial.min_minutes),
            ("task_filter", FILTER, &initial.task_filter),
            ("colour", COLOUR, &initial.colour),
            // A multi-line edit control breaks lines at CR LF.
            ("notes", NOTES, &initial.notes.replace('\n', "\r\n")),
        ] {
            if self.shows(key) {
                set(id, value);
            }
        }
        if let Purpose::Add { date, .. } = &self.purpose {
            // Starts at is already the core's for this day; setting Day asks again, for the same.
            set(DATE, date);
        }
        let kind = dialog::item(hwnd, KIND);
        let kinds = kinds();
        for choice in &kinds {
            let text = HSTRING::from(choice.title.as_str());
            controls::send(kind, CB_ADDSTRING, 0, text.as_ptr() as isize);
        }
        let selected = kinds.iter().position(|k| k.id == initial.kind).unwrap_or(0);
        controls::send(kind, CB_SETCURSEL, selected, 0);
        controls::check(dialog::item(hwnd, TAKES_TASKS), initial.accepts_tasks);
        controls::check(dialog::item(hwnd, CAPACITY), initial.counts_capacity);
        controls::check(dialog::item(hwnd, ANCHORED), initial.anchored);
        // What each takes, read with it, and an example of it, greyed while it is empty.
        for field in block_form() {
            let Some((_, id, _)) = FIELDS.iter().find(|(k, _, _)| *k == field.key) else { continue };
            let control = dialog::item(hwnd, *id);
            if control.is_invalid() {
                continue;
            }
            if !field.hint.is_empty() {
                a11y::set_description(control, &field.hint);
            }
            if matches!(field.kind, FieldKind::Line | FieldKind::Time | FieldKind::Date | FieldKind::Minutes) {
                controls::cue(control, &field.example);
            }
        }
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
            // A start the person chose stays theirs whatever day they then choose.
            id if id == i32::from(START) && u32::from(code) == EN_CHANGE => {
                if !self.writing.get() {
                    self.follow.set(false);
                }
                None
            }
            id if id == i32::from(DATE) && u32::from(code) == EN_CHANGE => {
                self.follow_day(hwnd);
                None
            }
            _ => None,
        }
    }
}

/// Runs the form, and returns what saving it did; `None` if it was cancelled or nothing
/// changed. `note` is the core's `unsayable_repeat_note` for the block being changed.
pub fn run(owner: HWND, lumenna: &Lumenna, purpose: Purpose, initial: BlockFields, note: Option<String>) -> Option<Change> {
    let heading = match &purpose {
        Purpose::Add { .. } => "New Block".to_owned(),
        Purpose::Series { .. } => format!("Change {}, Every Occurrence", initial.title),
        Purpose::Occurrence { .. } => format!("Change {}, This Day Only", initial.title),
    };
    let follow = matches!(purpose, Purpose::Add { follow: true, .. });
    let form = Form {
        lumenna,
        purpose,
        initial,
        note,
        heading,
        result: RefCell::new(None),
        follow: Cell::new(follow),
        writing: Cell::new(false),
    };
    dialog::run(Some(owner), &form);
    form.result.into_inner()
}
