//! Adding or changing a block, drawn from the app's block form (`view.block_form`): what it is
//! for, which fields it shows, what they hold and why saving was refused are the app's.
//!
//! Every setting a block has, in two columns: what it is and when on the left — title,
//! start, length, kind, and the three flags — and how it repeats and behaves on the right. The
//! flags start from the kind, and follow it while it changes; a new block's start follows its
//! day until the person changes it. The form for one day of a repeating block has only the
//! left column: that is all one day can change. Until and the shortest length show only while
//! it repeats.
//!
//! What is typed goes to the app as it is typed; Save is the app's, which sends only what
//! changed and closes the form, or keeps it open with why. The dialog closes when the app's
//! form does.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use lumenna_app::{BlockEvent, BlockFormView};
use lumenna_ffi::ShellEvent;
use lumenna_records::{BlockFields, Choice, FieldKind, FormField, block_form};
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    BN_CLICKED, BS_AUTOCHECKBOX, BS_DEFPUSHBUTTON, BS_PUSHBUTTON, CB_ADDSTRING, CB_GETCURSEL, CB_SETCURSEL, CBN_SELCHANGE,
    CBS_DROPDOWNLIST, EN_CHANGE, ES_AUTOHSCROLL, ES_AUTOVSCROLL, ES_MULTILINE, ES_NUMBER, ES_WANTRETURN, EndDialog, IDCANCEL,
    IDOK, WS_BORDER, WS_TABSTOP, WS_VSCROLL,
};
use windows::core::HSTRING;

use super::app::{self, App};
use super::dialog::{self, Class, Dialog, Template};
use super::{a11y, controls, picker, prompts};
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
const START_PICKER: u16 = 120;
const DATE_PICKER: u16 = 121;

/// The fields a picker fills, beside them: its key, the field, the picker and what it picks.
const PICKERS: [(&str, u16, u16, picker::Kind); 2] =
    [("start", START, START_PICKER, picker::Kind::Time), ("date", DATE, DATE_PICKER, picker::Kind::Date)];

/// How wide a picker is, beside its field, in dialog units.
const PICKER: i16 = 80;

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

/// The right-hand column: how it repeats and behaves, which one day alone cannot change.
const RIGHT_KEYS: [&str; 6] = ["repeat", "until", "min_minutes", "task_filter", "colour", "notes"];

/// A column's width, and where the second starts, in dialog units.
const COLUMN: i16 = 216;
const RIGHT: i16 = 7 + COLUMN + 14;

/// The kinds a block can be, as the core names them.
fn kinds() -> Vec<Choice> {
    block_form().into_iter().find(|f| f.key == "kind").map(|f| f.options).unwrap_or_default()
}

/// What a field of the form holds.
fn value(values: &BlockFields, date: &str, key: &str) -> String {
    match key {
        "title" => values.title.clone(),
        "start" => values.start.clone(),
        "minutes" => values.minutes.clone(),
        "kind" => values.kind.clone(),
        "accepts_tasks" => values.accepts_tasks.to_string(),
        "counts_capacity" => values.counts_capacity.to_string(),
        "anchored" => values.anchored.to_string(),
        "date" => date.to_owned(),
        "repeat" => values.repeat.clone(),
        "until" => values.until.clone(),
        "min_minutes" => values.min_minutes.clone(),
        "task_filter" => values.task_filter.clone(),
        "colour" => values.colour.clone(),
        // A multi-line edit control breaks lines at CR LF.
        "notes" => values.notes.replace('\n', "\r\n"),
        _ => String::new(),
    }
}

/// The form open, while it is.
pub struct Form {
    hwnd: Cell<HWND>,
    /// What it was first drawn from: its heading, its button, and which fields it can show.
    first: BlockFormView,
    /// The fields it has controls for: what the app shows, and while a series' form, what it
    /// shows only while the block repeats.
    keys: Vec<String>,
    /// The fields are being written from the app, not typed.
    filling: Cell<bool>,
    /// The last refusal said, so it is said once.
    refused: RefCell<Option<String>>,
    /// Whether the app has closed it.
    closed: Cell<bool>,
    /// Each field's description as last set.
    hints: RefCell<std::collections::HashMap<String, String>>,
}


impl Form {
    /// The dialog, once it exists.
    pub fn window(&self) -> Option<HWND> {
        Some(self.hwnd.get()).filter(|hwnd| !hwnd.is_invalid())
    }

    fn once(&self) -> bool {
        self.first.second_column.is_none()
    }

    fn has(&self, key: &str) -> bool {
        self.keys.iter().any(|k| k == key)
    }

    /// The core's words for a field, and whether it is a check box.
    fn field(&self, key: &str) -> Option<FormField> {
        self.first.fields.iter().find(|f| f.key == key).cloned().or_else(|| block_form().into_iter().find(|f| f.key == key))
    }

    /// Draws the app's form as it now is, or closes the dialog when the app has closed it.
    /// Returns whether it held back the field being typed in, for once the app is `still`.
    pub fn draw(&self, app: &App, view: Option<&BlockFormView>, still: bool) -> bool {
        let hwnd = self.hwnd.get();
        if hwnd.is_invalid() {
            return false;
        }
        let Some(view) = view else {
            if !self.closed.replace(true) {
                unsafe {
                    let _ = EndDialog(hwnd, 1);
                }
            }
            return false;
        };
        let typing = self.fill(hwnd, view, still);
        // A refusal is said once, after this turn, over the form, which stays as typed.
        let refusal = view.refusal.clone();
        if refusal.is_some() && *self.refused.borrow() != refusal {
            *self.refused.borrow_mut() = refusal.clone();
            if let Some(refusal) = refusal {
                app.defer(move |app| prompts::fail(app.block_form_window().unwrap_or(app.main), &crate::speech::sentence(&refusal)));
            }
        } else if refusal.is_none() {
            self.refused.replace(None);
        }
        typing
    }

    /// The fields as the app holds them: which are shown, and what each holds, written only
    /// where it differs, so the caret in a field being typed in stays put.
    fn fill(&self, hwnd: HWND, view: &BlockFormView, still: bool) -> bool {
        self.filling.set(true);
        let mut typing = false;
        for (key, id, _) in FIELDS {
            if !self.has(key) {
                continue;
            }
            let control = dialog::item(hwnd, id);
            let shown = view.fields.iter().find(|f| f.key == key);
            // Until and the shortest length come and go as the block repeats or not; never the
            // one with focus, which would leave focus nowhere.
            let picked = PICKERS.iter().find(|(k, _, _, _)| *k == key).map(|(_, _, picker, kind)| (dialog::item(hwnd, *picker), *kind));
            if control != controls::focused() || shown.is_some() {
                controls::show(control, shown.is_some());
                controls::show(label_of(hwnd, control), shown.is_some());
            }
            if let Some((picker, _)) = picked
                && (picker != controls::focused() || shown.is_some())
            {
                controls::show(picker, shown.is_some());
            }
            // What it takes, read with it: set again only when the app words it otherwise.
            if let Some(field) = shown
                && !field.hint.is_empty()
                && self.hints.borrow().get(key) != Some(&field.hint)
            {
                a11y::set_description(control, &field.hint);
                self.hints.borrow_mut().insert(key.to_owned(), field.hint.clone());
            }
            let held = value(&view.values, &view.date, key);
            match key {
                "kind" => {
                    let at = kinds().iter().position(|k| k.id == held);
                    if usize::try_from(controls::send(control, CB_GETCURSEL, 0, 0)).ok() != at {
                        controls::send(control, CB_SETCURSEL, at.unwrap_or(usize::MAX), 0);
                    }
                }
                "accepts_tasks" | "counts_capacity" | "anchored" => {
                    if controls::checked(control) != (held == "true") {
                        controls::check(control, held == "true");
                    }
                }
                _ => typing |= controls::write_typed(control, &held, still),
            }
            if let Some((picker, kind)) = picked {
                picker::show(picker, kind, &controls::text(control));
            }
        }
        self.filling.set(false);
        typing
    }

    /// What a control now holds, as the app takes it: a flag as `true` or `false`, the kind as
    /// its identifier, the notes with plain line ends.
    fn read(&self, hwnd: HWND, key: &str, id: u16) -> String {
        let control = dialog::item(hwnd, id);
        match key {
            "kind" => {
                let at = usize::try_from(controls::send(control, CB_GETCURSEL, 0, 0)).ok();
                at.and_then(|i| kinds().get(i).map(|k| k.id.clone())).unwrap_or_else(|| "work".to_owned())
            }
            "accepts_tasks" | "counts_capacity" | "anchored" => controls::checked(control).to_string(),
            _ => controls::text(control).replace("\r\n", "\n"),
        }
    }
}

/// The static label made just before a control: what names it, shown and hidden with it.
fn label_of(hwnd: HWND, control: HWND) -> HWND {
    unsafe {
        windows::Win32::UI::WindowsAndMessaging::GetWindow(control, windows::Win32::UI::WindowsAndMessaging::GW_HWNDPREV)
            .ok()
            .filter(|previous| windows::Win32::UI::WindowsAndMessaging::GetParent(*previous).ok() == Some(hwnd))
            .filter(|previous| {
                let mut class = [0u16; 16];
                let length = windows::Win32::UI::WindowsAndMessaging::GetClassNameW(*previous, &mut class);
                String::from_utf16_lossy(&class[..length.max(0) as usize]).eq_ignore_ascii_case("Static")
            })
            .unwrap_or_default()
    }
}

impl Dialog for Form {
    fn template(&self) -> Template {
        let field = |template: Template, label: &str, id: u16, style: u32, x: i16, y: i16| {
            let template = template.item(Class::Static, label, u16::MAX, 0, x, y, COLUMN, 9);
            // A day or a time has the system's picker beside it, after it in tab order.
            match PICKERS.iter().find(|(_, field, _, _)| *field == id) {
                Some((_, _, picker, kind)) => template
                    .item(Class::Edit, "", id, style | WS_BORDER.0 | WS_TABSTOP.0, x, y + 10, COLUMN - PICKER - 4, 14)
                    .named(picker::CLASS, "", *picker, kind.style(), x + COLUMN - PICKER, y + 10, PICKER, 14),
                None => template.item(Class::Edit, "", id, style | WS_BORDER.0 | WS_TABSTOP.0, x, y + 10, COLUMN, 14),
            }
        };
        let check = |template: Template, label: &str, id: u16, y: i16| {
            template.item(Class::Button, label, id, BS_AUTOCHECKBOX as u32 | WS_TABSTOP.0, 7, y, COLUMN, 10)
        };
        // A field's name as the core gives it, with this dialog's access key; a colon before a
        // field, none on a check box.
        let label = |key: &str, colon: bool| {
            let name = self.field(key).map_or_else(|| key.to_owned(), |f| f.label);
            let letter = FIELDS.iter().find(|(k, _, _)| *k == key).map_or(' ', |(_, _, l)| *l);
            format!("{}{}", devices::marked(&name, letter, '&'), if colon { ":" } else { "" })
        };
        let line = ES_AUTOHSCROLL as u32;
        let right = !self.once();
        let width = if right { RIGHT + COLUMN + 7 } else { 7 + COLUMN + 7 };
        let mut template = Template::new(&self.first.heading, width, 10);

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
        if self.has("date") {
            template = field(template, &label("date", true), DATE, line, 7, y);
            y += 28;
        }
        let mut bottom = y;

        if right {
            let mut y = 7;
            for (key, id, style) in [
                ("repeat", REPEAT, line),
                ("until", UNTIL, line),
                ("min_minutes", SHORTEST, ES_NUMBER as u32),
                ("task_filter", FILTER, line),
                ("colour", COLOUR, line),
            ] {
                template = field(template, &label(key, true), id, style, RIGHT, y);
                y += 28;
            }
            let notes = (ES_MULTILINE | ES_WANTRETURN | ES_AUTOVSCROLL) as u32 | WS_VSCROLL.0;
            template = template
                .item(Class::Static, &label("notes", true), u16::MAX, 0, RIGHT, y, COLUMN, 9)
                .item(Class::Edit, "", NOTES, notes | WS_BORDER.0 | WS_TABSTOP.0, RIGHT, y + 10, COLUMN, 40);
            y += 54;
            bottom = bottom.max(y);
        }

        template = template
            .item(Class::Button, &self.first.yes, IDOK.0 as u16, BS_DEFPUSHBUTTON as u32 | WS_TABSTOP.0, width - 111, bottom + 4, 50, 14)
            .item(Class::Button, "Cancel", IDCANCEL.0 as u16, BS_PUSHBUTTON as u32 | WS_TABSTOP.0, width - 57, bottom + 4, 50, 14);
        // The height is known only once the fields are in.
        template.resize(width, bottom + 25)
    }

    fn init(&self, hwnd: HWND) -> bool {
        self.hwnd.set(hwnd);
        let kind = dialog::item(hwnd, KIND);
        for choice in kinds() {
            let text = HSTRING::from(choice.title.as_str());
            controls::send(kind, CB_ADDSTRING, 0, text.as_ptr() as isize);
        }
        // An example of what each takes, greyed while it is empty.
        for field in block_form() {
            let Some((_, id, _)) = FIELDS.iter().find(|(k, _, _)| *k == field.key) else { continue };
            let control = dialog::item(hwnd, *id);
            if !control.is_invalid() && matches!(field.kind, FieldKind::Line | FieldKind::Time | FieldKind::Date | FieldKind::Minutes) {
                controls::cue(control, &field.example);
            }
        }
        for (key, _, id, kind) in PICKERS {
            let picker = dialog::item(hwnd, id);
            if !picker.is_invalid() {
                let name = self.field(key).map_or_else(|| key.to_owned(), |f| f.label);
                picker::ready(picker, kind, &name, &kind.choose());
            }
        }
        self.fill(hwnd, &self.first, true);
        false
    }

    fn message(&self, hwnd: HWND, message: u32, _wparam: WPARAM, lparam: LPARAM) -> Option<isize> {
        // A day or time picked fills its field, which sends it to the app as typing does.
        // Not while the app's value is shown in it, which is no one's typing: a new block's
        // start follows its day until the person changes it.
        let id = picker::changed(message, lparam)?;
        let (_, field, _, kind) = PICKERS.iter().find(|(_, _, picker, _)| *picker == id)?;
        let value = picker::read(dialog::item(hwnd, id), *kind)?;
        let field = dialog::item(hwnd, *field);
        if !self.filling.get() && controls::text(field) != value {
            controls::set_text(field, &value);
        }
        Some(0)
    }

    fn command(&self, hwnd: HWND, id: u16, code: u16) -> Option<isize> {
        let code = u32::from(code);
        match i32::from(id) {
            // Saving is the app's; the form closes when the app's does.
            id if id == IDOK.0 => {
                app::send(ShellEvent::Block(BlockEvent::Save));
                None
            }
            id if id == IDCANCEL.0 => {
                self.closed.set(true);
                app::send(ShellEvent::Block(BlockEvent::Cancel));
                Some(0)
            }
            _ => {
                let (key, _, _) = FIELDS.iter().find(|(_, field, _)| *field == id)?;
                let changed = match *key {
                    "kind" => code == CBN_SELCHANGE,
                    "accepts_tasks" | "counts_capacity" | "anchored" => code == BN_CLICKED,
                    _ => code == EN_CHANGE,
                };
                if changed && !self.filling.get() {
                    let value = self.read(hwnd, key, id);
                    app::send(ShellEvent::Block(BlockEvent::Field { key: (*key).to_owned(), value }));
                }
                None
            }
        }
    }
}

/// Runs the app's open block form as a dialog until the app closes it or the person cancels.
pub fn run(app: &App, view: BlockFormView) {
    let mut keys: Vec<String> = view.fields.iter().map(|f| f.key.clone()).collect();
    if view.second_column.is_some() {
        for key in RIGHT_KEYS {
            if !keys.iter().any(|k| k == key) {
                keys.push(key.to_owned());
            }
        }
    }
    let form = Rc::new(Form {
        hwnd: Cell::new(HWND::default()),
        first: view,
        keys,
        filling: Cell::new(false),
        refused: RefCell::new(None),
        closed: Cell::new(false),
        hints: RefCell::new(std::collections::HashMap::new()),
    });
    app.open_block_form(Some(Rc::clone(&form)));
    dialog::run(Some(app.main), &*form);
    app.open_block_form(None);
}
