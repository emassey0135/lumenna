//! Adding or changing a block (§16.1: block editor; §13: creating is a form).

use std::cell::RefCell;

use lumenna_surface::{BlockEdit, BlockScope, Change, Lumenna, NewBlock};
use windows::Win32::Foundation::HWND;
use windows::Win32::System::SystemServices::SS_NOPREFIX;
use windows::Win32::UI::WindowsAndMessaging::{
    BS_DEFPUSHBUTTON, BS_PUSHBUTTON, CB_ADDSTRING, CB_GETCURSEL, CB_SETCURSEL, CBS_DROPDOWNLIST,
    ES_AUTOHSCROLL, ES_NUMBER, IDCANCEL, IDOK, WS_BORDER, WS_TABSTOP, WS_VSCROLL,
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

const KINDS: [(&str, &str); 3] = [("work", "Work, takes tasks"), ("break", "Break"), ("event", "Event")];

/// What the form is for.
pub enum Purpose {
    /// A new block, starting on this date phrase.
    Add { date: String },
    /// Every occurrence of a series.
    Series { id: String },
    /// One day of a repeating series alone.
    Occurrence { series: String, date: String },
}

/// A block's fields as the form shows them.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct Fields {
    pub title: String,
    pub start: String,
    pub minutes: String,
    pub kind: String,
    pub repeat: String,
    /// The RFC 5545 rule it repeats by. When `repeat` is empty, it is a rule the repetition
    /// words cannot say: the field starts empty, the rule is said beside it, and it is left
    /// alone unless something is typed there.
    pub rule: Option<String>,
}

struct Form<'a> {
    lumenna: &'a Lumenna,
    purpose: Purpose,
    initial: Fields,
    heading: String,
    result: RefCell<Option<Change>>,
}

impl Form<'_> {
    fn read(&self, hwnd: HWND) -> Fields {
        let kind = controls::send(dialog::item(hwnd, KIND), CB_GETCURSEL, 0, 0);
        Fields {
            title: controls::text(dialog::item(hwnd, TITLE)).trim().to_owned(),
            start: controls::text(dialog::item(hwnd, START)).trim().to_owned(),
            minutes: controls::text(dialog::item(hwnd, MINUTES)).trim().to_owned(),
            kind: usize::try_from(kind).ok().and_then(|i| KINDS.get(i)).map_or("work", |k| k.0).to_owned(),
            repeat: controls::text(dialog::item(hwnd, REPEAT)).trim().to_owned(),
            rule: self.initial.rule.clone(),
        }
    }

    fn save(&self, hwnd: HWND) -> Result<Change, String> {
        let fields = self.read(hwnd);
        let minutes: u32 = fields.minutes.parse().map_err(|_| "Minutes has to be a whole number.".to_owned())?;
        let changed = |now: &String, was: &String| (now != was).then(|| now.clone());
        let result = match &self.purpose {
            Purpose::Add { .. } => self.lumenna.add_block(NewBlock {
                title: fields.title.clone(),
                at: fields.start.clone(),
                minutes,
                date: Some(controls::text(dialog::item(hwnd, DATE)).trim().to_owned()).filter(|d| !d.is_empty()),
                kind: fields.kind.clone(),
                repeat: Some(fields.repeat.clone()).filter(|r| !r.is_empty()),
            }),
            // Only what changed, so a concurrent edit to another field elsewhere stands.
            Purpose::Series { id } => self.lumenna.edit_block(
                id,
                BlockEdit {
                    title: changed(&fields.title, &self.initial.title),
                    at: changed(&fields.start, &self.initial.start),
                    minutes: (fields.minutes != self.initial.minutes).then_some(minutes),
                    kind: changed(&fields.kind, &self.initial.kind),
                    repeat: changed(&fields.repeat, &self.initial.repeat)
                        .map(|r| if r.is_empty() { "none".to_owned() } else { r }),
                },
                BlockScope::Series,
            ),
            Purpose::Occurrence { series, date } => self.lumenna.edit_block(
                series,
                BlockEdit {
                    title: changed(&fields.title, &self.initial.title),
                    at: changed(&fields.start, &self.initial.start),
                    minutes: (fields.minutes != self.initial.minutes).then_some(minutes),
                    kind: changed(&fields.kind, &self.initial.kind),
                    repeat: None,
                },
                BlockScope::Occurrence { date: date.clone() },
            ),
        };
        result.map_err(|error| sentence(&error))
    }
}

impl Dialog for Form<'_> {
    fn template(&self) -> Template {
        let field = |template: Template, label: &str, id: u16, style: u32, y: i16| {
            template
                .item(Class::Static, label, u16::MAX, 0, 7, y, 216, 9)
                .item(Class::Edit, "", id, style | WS_BORDER.0 | WS_TABSTOP.0, 7, y + 10, 216, 14)
        };
        let adding = matches!(self.purpose, Purpose::Add { .. });
        let once = matches!(self.purpose, Purpose::Occurrence { .. });
        let mut y = 7;
        let mut template = Template::new(&self.heading, 230, 10);
        template = field(template, "&Title:", TITLE, ES_AUTOHSCROLL as u32, y);
        y += 28;
        template = field(template, "Starts &at, such as 9am:", START, ES_AUTOHSCROLL as u32, y);
        y += 28;
        template = field(template, "&Minutes:", MINUTES, ES_NUMBER as u32, y);
        y += 28;
        template = template
            .item(Class::Static, "&Kind:", u16::MAX, 0, 7, y, 216, 9)
            .item(Class::ComboBox, "", KIND, CBS_DROPDOWNLIST as u32 | WS_VSCROLL.0 | WS_TABSTOP.0, 7, y + 10, 216, 60);
        y += 28;
        if !once {
            template = field(template, "&Repeats, such as every weekday; empty for once:", REPEAT, ES_AUTOHSCROLL as u32, y);
            y += 28;
            if let Some(rule) = self.initial.rule.as_ref().filter(|_| self.initial.repeat.is_empty()) {
                let note = format!("It repeats by the rule {rule}, which the repetition words cannot say. Leave Repeats empty to keep it.");
                template = template.item(Class::Static, &note, u16::MAX, SS_NOPREFIX.0, 7, y, 216, 26);
                y += 30;
            }
        }
        if adding {
            template = field(template, "Starts &on:", DATE, ES_AUTOHSCROLL as u32, y);
            y += 28;
        }
        let ok = if adding { "Add" } else { "Save" };
        template = template
            .item(Class::Button, ok, IDOK.0 as u16, BS_DEFPUSHBUTTON as u32 | WS_TABSTOP.0, 119, y + 4, 50, 14)
            .item(Class::Button, "Cancel", IDCANCEL.0 as u16, BS_PUSHBUTTON as u32 | WS_TABSTOP.0, 173, y + 4, 50, 14);
        // The height is known only once the fields are in.
        template.resize(230, y + 25)
    }

    fn init(&self, hwnd: HWND) -> bool {
        controls::set_text(dialog::item(hwnd, TITLE), &self.initial.title);
        controls::set_text(dialog::item(hwnd, START), &self.initial.start);
        controls::set_text(dialog::item(hwnd, MINUTES), &self.initial.minutes);
        controls::set_text(dialog::item(hwnd, REPEAT), &self.initial.repeat);
        if let Purpose::Add { date } = &self.purpose {
            controls::set_text(dialog::item(hwnd, DATE), date);
        }
        let kind = dialog::item(hwnd, KIND);
        for (_, label) in KINDS {
            let text = HSTRING::from(label);
            controls::send(kind, CB_ADDSTRING, 0, text.as_ptr() as isize);
        }
        let selected = KINDS.iter().position(|(k, _)| *k == self.initial.kind).unwrap_or(0);
        controls::send(kind, CB_SETCURSEL, selected, 0);
        false
    }

    fn command(&self, hwnd: HWND, id: u16, _code: u16) -> Option<isize> {
        match i32::from(id) {
            id if id == IDOK.0 => match self.save(hwnd) {
                Ok(change) => {
                    *self.result.borrow_mut() = Some(change);
                    Some(1)
                }
                Err(message) => {
                    // The form stays, with what was typed, so it can be put right.
                    prompts::fail(hwnd, &message);
                    None
                }
            },
            id if id == IDCANCEL.0 => Some(0),
            _ => None,
        }
    }
}

/// Runs the form, and returns what saving it did.
pub fn run(owner: HWND, lumenna: &Lumenna, purpose: Purpose, initial: Fields) -> Option<Change> {
    let heading = match &purpose {
        Purpose::Add { .. } => "New Block".to_owned(),
        Purpose::Series { .. } => format!("Change {}, Every Occurrence", initial.title),
        Purpose::Occurrence { .. } => format!("Change {}, This Day Only", initial.title),
    };
    let form = Form { lumenna, purpose, initial, heading, result: RefCell::new(None) };
    dialog::run(Some(owner), &form);
    form.result.into_inner()
}
