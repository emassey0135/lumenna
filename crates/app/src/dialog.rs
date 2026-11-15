//! A form or a dialog as its controls in tab order, for a shell that draws its own controls and
//! speaks for itself: a question, the task form, the block form, a settings page, quick add.
//!
//! Every control says what it is in Lumenna's own terms (`ControlType`: a line, several lines, a
//! choice, a check box, a list, a button, a line to read), which each device maps to its own roles
//! (the BrailleSense's edit box and combo box, Orbit's, a BTSpeak dialog's), its label, its value
//! and what it sends (`ControlSends`). A shell never builds an event for a control itself: it hands
//! the control and what the person did with it to [`control_event`], and sends what comes back.
//! Every word is the core's, in sentence case, as the rest of the screen's description.

use lumenna_records::fields::{FieldKind, FormField, SettingsPage};
use lumenna_records::types::{SettingKind, Syntax};
use lumenna_records::{ActionKind, Answer, Choice, Question};

use crate::lines::Lines;
use crate::{ActingOn, Asking, BlockEvent, DetailsEvent, SettingsEvent, ShellEvent, Typing, ViewModel};

/// A form or a dialog: its controls in the order Tab moves through them.
#[boltffi::data]
#[derive(Debug, Clone, PartialEq)]
pub struct DialogShown {
    /// Which it is, for a shell to tell one apart from the next: `question:3`, `task:<id>`,
    /// `block`, `quick-add`, `settings:<page>`.
    pub id: String,
    /// What it is called, said as it opens.
    pub title: String,
    /// Its controls, in tab order.
    pub controls: Vec<ControlShown>,
    /// Which control it opens on, by position: the first that takes anything.
    pub focus: u32,
    /// What closing it unanswered sends (Escape, Back): its Cancel's events; none where nothing
    /// is to be undone (a settings page, whose settings apply as they change).
    pub cancel: Vec<ShellEvent>,
}

/// What a control is, in Lumenna's terms: each device's shell names it its own way.
#[boltffi::data]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlType {
    /// A line to type in.
    Line,
    /// Text of several lines to type in, where Enter starts a new line.
    Lines,
    /// One of its `options`, changed in place.
    Choice,
    /// On or off.
    Check,
    /// A list to pick one from (a question's choices), each of its `options`.
    List,
    /// Pressed.
    Button,
    /// A line to read: a caption, a refusal, a readback, a line of the task's details.
    Text,
}

/// What a control sends, which [`control_event`] turns into events.
#[boltffi::data]
#[derive(Debug, Clone, PartialEq)]
pub enum ControlSends {
    /// Nothing: a line to read.
    Nothing,
    /// A field of the task form, by its key: what is typed or chosen, as it changes.
    TaskField(String),
    /// A field of the block form, by its key: what is typed or chosen, as it changes; a check as
    /// `true` or `false`.
    BlockField(String),
    /// A setting, by its key: a line as it is typed, then applied when it is left; a choice or a
    /// check as it changes.
    Setting(String),
    /// The quick-add line: what is typed, as it changes; pressed (the line's Enter, or the Add
    /// button, given the line's text), the task added.
    QuickAdd,
    /// A question's line: pressed (Enter, or its button given the line's text), the answer.
    Answer,
    /// A question's choices: the one chosen (by its `id`), the answer; its button is given the
    /// one in hand.
    Pick,
    /// These events, as they are, when pressed.
    Press(Vec<ShellEvent>),
    /// The shell's own to do when pressed, by what it is: a file to pick (`import`,
    /// `restore-backup`), a backup to take into the backup folder (`back-up`), a device to pair
    /// (`pair`), a folder to choose (a setting's key).
    Shell(String),
}

/// What the person did with a control.
#[boltffi::data]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlMove {
    /// Typed in a line: its text as it now is, the cursor in UTF-8 bytes.
    Typed {
        /// Where the cursor is.
        cursor: u32,
    },
    /// Left a line, chose an option, changed a check: the value it now holds (an option's
    /// `id`, a check's `true` or `false`).
    Set,
    /// Pressed it: a button, or Enter in a line that submits.
    Pressed,
}

/// One control.
#[boltffi::data]
#[derive(Debug, Clone, PartialEq)]
pub struct ControlShown {
    /// What tells it apart in its dialog: `field:due`, `setting:clock`, `yes`, `cancel`.
    pub key: String,
    /// What it is.
    pub kind: ControlType,
    /// Its name, ahead of its value; for a button, what it says; for a line to read, empty.
    pub label: String,
    /// What it holds: a line's text, a choice's option's title, a line to read's text; empty for
    /// a button and a check.
    pub value: String,
    /// What it takes, for after it. Empty for nothing.
    pub hint: String,
    /// A placeholder showing an answer's shape. Empty for none.
    pub example: String,
    /// A choice's options or a list's, each its `id` (what is sent) and `title`; for a question's
    /// line asking how long, the usual lengths.
    pub options: Vec<Choice>,
    /// Which of `options` it holds, by position.
    pub selected: Option<u32>,
    /// A check's state.
    pub checked: bool,
    /// What a line holds, where a shell offers its own picker beside it: `Date`, `Time`.
    pub field: Option<FieldKind>,
    /// For a line written in the filter language, which a shell offers completion in.
    pub syntax: Option<Syntax>,
    /// A button's letter on a braille device; empty for none.
    pub letter: String,
    /// Whether a button asks something first, which a device marks its own way.
    pub asks: bool,
    /// Whether it is the dialog's default button: what Enter in it presses.
    pub default: bool,
    /// Whether it is the dialog's Cancel.
    pub cancel: bool,
    /// What it sends.
    pub sends: ControlSends,
}

impl ControlShown {
    fn new(key: impl Into<String>, kind: ControlType, label: impl Into<String>, sends: ControlSends) -> Self {
        Self {
            key: key.into(),
            kind,
            label: label.into(),
            value: String::new(),
            hint: String::new(),
            example: String::new(),
            options: Vec::new(),
            selected: None,
            checked: false,
            field: None,
            syntax: None,
            letter: String::new(),
            asks: false,
            default: false,
            cancel: false,
            sends,
        }
    }

    fn text(key: impl Into<String>, text: impl Into<String>) -> Self {
        Self { value: text.into(), ..Self::new(key, ControlType::Text, "", ControlSends::Nothing) }
    }

    fn button(key: impl Into<String>, label: impl Into<String>, sends: ControlSends) -> Self {
        Self::new(key, ControlType::Button, label, sends)
    }

    fn press(key: impl Into<String>, label: impl Into<String>, events: Vec<ShellEvent>) -> Self {
        Self::button(key, label, ControlSends::Press(events))
    }

    fn default(mut self) -> Self {
        self.default = true;
        self
    }

    fn cancelling(mut self) -> Self {
        self.cancel = true;
        self
    }
}

/// What doing `how` to a control that sends `sends`, holding `value`, sends, in order; nothing
/// where that does nothing (typing in a question's line, which is answered when pressed).
#[must_use]
pub fn control_event(sends: &ControlSends, value: &str, how: ControlMove) -> Vec<ShellEvent> {
    let value = value.to_owned();
    match (sends, how) {
        (ControlSends::TaskField(key), ControlMove::Typed { .. } | ControlMove::Set) => {
            vec![ShellEvent::Details(DetailsEvent::Field { key: key.clone(), value })]
        }
        (ControlSends::BlockField(key), ControlMove::Typed { .. } | ControlMove::Set) => {
            vec![ShellEvent::Block(BlockEvent::Field { key: key.clone(), value })]
        }
        (ControlSends::Setting(key), ControlMove::Typed { .. }) => vec![ShellEvent::Settings(SettingsEvent::Type { key: key.clone(), value })],
        (ControlSends::Setting(key), ControlMove::Set) => vec![ShellEvent::Settings(SettingsEvent::Set { key: key.clone(), value })],
        (ControlSends::QuickAdd, ControlMove::Typed { cursor }) => vec![ShellEvent::Typing(Typing::Text { text: value, cursor })],
        (ControlSends::QuickAdd, ControlMove::Set) => {
            let cursor = u32::try_from(value.len()).unwrap_or(u32::MAX);
            vec![ShellEvent::Typing(Typing::Text { text: value, cursor })]
        }
        (ControlSends::QuickAdd, ControlMove::Pressed) => vec![ShellEvent::QuickAdd(value)],
        (ControlSends::Answer, ControlMove::Pressed) => vec![ShellEvent::Answer(Answer::Text { text: value })],
        (ControlSends::Pick, ControlMove::Set | ControlMove::Pressed) if !value.is_empty() => {
            vec![ShellEvent::Answer(Answer::Picked { id: value, length: None })]
        }
        (ControlSends::Press(events), ControlMove::Pressed) => events.clone(),
        _ => Vec::new(),
    }
}

/// The first control that takes anything: where a dialog opens.
fn first_taking(controls: &[ControlShown]) -> u32 {
    let at = controls.iter().position(|control| control.kind != ControlType::Text).unwrap_or(0);
    u32::try_from(at).unwrap_or(0)
}

fn dialog(id: String, title: String, controls: Vec<ControlShown>) -> DialogShown {
    let cancel = controls.iter().find(|control| control.cancel).map_or_else(Vec::new, |control| match &control.sends {
        ControlSends::Press(events) => events.clone(),
        _ => Vec::new(),
    });
    DialogShown { id, title, focus: first_taking(&controls), controls, cancel }
}

fn cancel_word() -> String {
    lumenna_records::words::cancel(true)
}

/// A form's field as its control, holding `value`, sending as `sends` does.
fn field_control(field: &FormField, value: String, options: &[Choice], checked: bool, sends: ControlSends) -> ControlShown {
    let key = format!("field:{}", field.key);
    match field.kind {
        FieldKind::Toggle => ControlShown { checked, ..ControlShown::new(key, ControlType::Check, field.label.clone(), sends) },
        _ if !options.is_empty() => {
            let selected = options.iter().position(|option| option.id == value || option.title == value);
            ControlShown {
                value: selected.and_then(|at| options.get(at)).map_or(value, |option| option.title.clone()),
                options: options.to_vec(),
                selected: selected.map(|at| u32::try_from(at).unwrap_or(0)),
                hint: field.hint.clone(),
                ..ControlShown::new(key, ControlType::Choice, field.label.clone(), sends)
            }
        }
        kind => ControlShown {
            value,
            hint: field.hint.clone(),
            example: field.example.clone(),
            field: matches!(kind, FieldKind::Date | FieldKind::Time).then_some(kind),
            syntax: field.syntax,
            ..ControlShown::new(key, if kind == FieldKind::Lines { ControlType::Lines } else { ControlType::Line }, field.label.clone(), sends)
        },
    }
}

/// A question open, as a dialog: its message, its line or its choices, then its buttons in the
/// order the screen's description gives them. A task's form asked for (`Question::Form`) is the
/// task form, once the details show that task; any other form is the shell's own.
#[must_use]
pub fn question(asking: &Asking, view: &ViewModel, lines: &Lines) -> Option<DialogShown> {
    let id = format!("question:{}", asking.number);
    let cancel = || ControlShown::press("cancel", cancel_word(), vec![ShellEvent::Cancel]).cancelling();
    let mut controls = Vec::new();
    if let Some(refusal) = &asking.refusal {
        controls.push(ControlShown::text("refusal", refusal.clone()));
    }
    let title = match &asking.question {
        Question::Immediate => return None,
        Question::Form => {
            let action = asking.action.as_ref()?;
            let shown = matches!(action.kind, ActionKind::Edit | ActionKind::EditTask)
                && view.details.as_ref().is_some_and(|details| details.task.id == action.target);
            if !shown {
                return None;
            }
            let mut form = task_form(view, lines)?;
            form.id = id;
            // Saving it or closing it closes the question as well as the form.
            for control in form.controls.iter_mut().filter(|control| control.cancel || control.default) {
                if let ControlSends::Press(events) = &mut control.sends {
                    events.push(ShellEvent::Cancel);
                }
            }
            form.cancel.push(ShellEvent::Cancel);
            return Some(form);
        }
        Question::Confirm { title, message, yes } => {
            if !message.is_empty() {
                controls.push(ControlShown::text("message", message.clone()));
            }
            controls.push(cancel());
            controls.push(ControlShown::press("yes", yes.clone(), vec![ShellEvent::Answer(Answer::Yes)]).default());
            title.clone()
        }
        Question::Text { title, label, initial, hint, yes, .. } => {
            controls.push(ControlShown {
                value: asking.typed.clone().unwrap_or_else(|| initial.clone()),
                hint: hint.clone(),
                options: asking.lengths.clone(),
                field: asking.day.is_some().then_some(FieldKind::Date),
                syntax: asking.syntax,
                ..ControlShown::new("line", ControlType::Line, label.clone(), ControlSends::Answer)
            });
            controls.push(ControlShown::button("yes", yes.clone(), ControlSends::Answer).default());
            controls.push(cancel());
            title.clone()
        }
        Question::Pick { title, yes, .. } => {
            let choices = asking.choices.as_ref().map(|choices| choices.choices.clone()).unwrap_or_default();
            // Each choice by the core's line, as a list says it.
            let options: Vec<Choice> = choices
                .iter()
                .enumerate()
                .map(|(at, choice)| Choice { title: lines.choices.get(at).map_or_else(|| choice.title.clone(), |line| line.text.clone()), ..choice.clone() })
                .collect();
            controls.push(ControlShown {
                selected: (!options.is_empty()).then_some(0),
                options,
                ..ControlShown::new("choices", ControlType::List, title.clone(), ControlSends::Pick)
            });
            controls.push(ControlShown::button("yes", yes.clone(), ControlSends::Pick).default());
            controls.push(cancel());
            title.clone()
        }
        Question::Choose { title, message, answers } => {
            if !message.is_empty() {
                controls.push(ControlShown::text("message", message.clone()));
            }
            for answer in answers {
                controls.push(ControlShown::press(
                    format!("answer:{}", answer.id),
                    answer.title.clone(),
                    vec![ShellEvent::Answer(Answer::Picked { id: answer.id.clone(), length: None })],
                ));
            }
            controls.push(cancel());
            title.clone()
        }
    };
    Some(dialog(id, title, controls))
}

/// The task form for the task the details show: its fields, the lines of its details, its own
/// actions shown by themselves (`primary`), then Save and Cancel, which forgets what was typed.
#[must_use]
pub fn task_form(view: &ViewModel, lines: &Lines) -> Option<DialogShown> {
    let details = view.details.as_ref()?;
    let fields = &details.fields;
    let mut controls: Vec<ControlShown> = details
        .form
        .iter()
        .map(|field| {
            let value = crate::details::get(fields, &field.key);
            let options = if field.key == "project" { &details.projects } else { &field.options };
            field_control(field, value, options, false, ControlSends::TaskField(field.key.clone()))
        })
        .collect();
    let said: Vec<String> = if lines.details.len() == details.lines.len() {
        lines.details.iter().map(|line| line.text.clone()).collect()
    } else {
        details.lines.iter().map(|line| format!("{}: {}", line.label, line.value)).collect()
    };
    controls.extend(details.lines.iter().zip(said).map(|(line, text)| ControlShown::text(format!("line:{}", line.key), text)));
    for offered in details.task.offered.iter().filter(|offered| offered.primary) {
        controls.push(ControlShown {
            letter: crate::braille::action_letter(offered.kind).unwrap_or_default().to_owned(),
            asks: offered.asks != crate::Asks::Nothing,
            ..ControlShown::press(
                format!("action:{:?}", offered.kind),
                if offered.sentence.is_empty() { offered.title.clone() } else { offered.sentence.clone() },
                vec![ShellEvent::ActOn(ActingOn::Details, offered.clone())],
            )
        });
    }
    let words = lumenna_records::fields::details_words(true);
    controls.push(ControlShown::press("save", words.save, vec![ShellEvent::Details(DetailsEvent::Save)]).default());
    controls.push(ControlShown::press("cancel", cancel_word(), vec![ShellEvent::Details(DetailsEvent::Discard)]).cancelling());
    Some(dialog(format!("task:{}", details.task.id), details.task.title.clone(), controls))
}

/// The block form, while it is open: why saving was refused, its fields, then its Save (Add) and
/// Cancel.
#[must_use]
pub fn block_form(view: &ViewModel) -> Option<DialogShown> {
    let form = view.block_form.as_ref()?;
    let values = &form.values;
    let mut controls = Vec::new();
    if let Some(refusal) = &form.refusal {
        controls.push(ControlShown::text("refusal", refusal.clone()));
    }
    for field in &form.fields {
        let (value, checked) = match field.key.as_str() {
            "title" => (values.title.clone(), false),
            "start" => (values.start.clone(), false),
            "minutes" => (values.minutes.clone(), false),
            "kind" => (values.kind.clone(), false),
            "repeat" => (values.repeat.clone(), false),
            "until" => (values.until.clone(), false),
            "min_minutes" => (values.min_minutes.clone(), false),
            "task_filter" => (values.task_filter.clone(), false),
            "colour" => (values.colour.clone(), false),
            "notes" => (values.notes.clone(), false),
            "date" => (form.date.clone(), false),
            "accepts_tasks" => (String::new(), values.accepts_tasks),
            "counts_capacity" => (String::new(), values.counts_capacity),
            "anchored" => (String::new(), values.anchored),
            _ => (String::new(), false),
        };
        controls.push(field_control(field, value, &field.options, checked, ControlSends::BlockField(field.key.clone())));
    }
    controls.push(ControlShown::press("save", form.yes.clone(), vec![ShellEvent::Block(BlockEvent::Save)]).default());
    controls.push(ControlShown::press("cancel", cancel_word(), vec![ShellEvent::Block(BlockEvent::Cancel)]).cancelling());
    Some(dialog("block".to_owned(), form.heading.clone(), controls))
}

/// Quick add, while it is open: the line, what fits at the cursor (each a button putting it in),
/// what the line will add, why the last was refused, then Add and Cancel.
#[must_use]
pub fn quick_add(view: &ViewModel, lines: &Lines) -> Option<DialogShown> {
    let line = view.line.as_ref()?;
    let words = lumenna_records::fields::quick_add_words(true);
    let mut controls = vec![ControlShown {
        value: line.text.clone(),
        hint: words.hint.clone(),
        example: words.example.clone(),
        syntax: Some(Syntax::QuickAdd),
        ..ControlShown::new("line", ControlType::Line, words.label.clone(), ControlSends::QuickAdd)
    }];
    for (at, candidate) in line.offer.candidates.iter().take(lumenna_records::words::COMPLETIONS_SHOWN).enumerate() {
        let said = lines.candidates.get(at).cloned().unwrap_or_else(|| crate::speech::candidate(candidate));
        controls.push(ControlShown::press(
            format!("complete:{at}"),
            lumenna_records::fields::complete_with(&said),
            vec![ShellEvent::Typing(Typing::Choose(u32::try_from(at).unwrap_or(0)))],
        ));
    }
    if !line.readback.is_empty() {
        controls.push(ControlShown::text("readback", lumenna_records::words::capitalised(&line.readback)));
    }
    if let Some(refused) = &view.add_refused {
        controls.push(ControlShown::text("refused", refused.clone()));
    }
    controls.push(ControlShown::button("add", words.add.clone(), ControlSends::QuickAdd).default());
    controls.push(ControlShown::press("cancel", words.cancel.clone(), vec![ShellEvent::Typing(Typing::Close)]).cancelling());
    Some(dialog("quick-add".to_owned(), words.title, controls))
}

/// A settings page (by `SettingsPage::id`), while settings are open: each setting as its control
/// and why a line was refused after it, the page's caption, its exports, its buttons, and the
/// store's own actions with their caption. A setting applies as it changes, so a page has no Save
/// or Cancel.
#[must_use]
pub fn settings_page(view: &ViewModel, page: &str) -> Option<DialogShown> {
    let settings = view.settings.as_ref()?;
    let page: &SettingsPage = settings.pages.iter().find(|found| found.id == page)?;
    let mut controls = Vec::new();
    for key in &page.keys {
        let Some(shown) = settings.settings.iter().find(|shown| &shown.setting.key == key) else { continue };
        let setting = &shown.setting;
        let sends = ControlSends::Setting(setting.key.clone());
        let id = format!("setting:{}", setting.key);
        controls.push(match setting.kind {
            SettingKind::Toggle => ControlShown { checked: setting.value == "true", hint: setting.hint.clone(), ..ControlShown::new(id, ControlType::Check, setting.title.clone(), sends) },
            SettingKind::Folder => ControlShown {
                value: setting.value.clone(),
                hint: setting.hint.clone(),
                ..ControlShown::new(id, ControlType::Button, setting.title.clone(), ControlSends::Shell(setting.key.clone()))
            },
            _ if !setting.options.is_empty() => {
                let selected = setting.options.iter().position(|option| option.id == setting.value);
                ControlShown {
                    value: selected.map_or_else(|| setting.value.clone(), |at| setting.options[at].title.clone()),
                    options: setting.options.clone(),
                    selected: selected.map(|at| u32::try_from(at).unwrap_or(0)),
                    hint: setting.hint.clone(),
                    ..ControlShown::new(id, ControlType::Choice, setting.title.clone(), sends)
                }
            }
            kind => ControlShown {
                value: shown.typed.clone(),
                hint: if !setting.hint.is_empty() {
                    setting.hint.clone()
                } else if kind == SettingKind::Time {
                    settings.words.time_hint.clone()
                } else if kind == SettingKind::Number {
                    settings.words.number_hint.clone()
                } else {
                    String::new()
                },
                field: (kind == SettingKind::Time).then_some(FieldKind::Time),
                ..ControlShown::new(id, ControlType::Line, setting.title.clone(), sends)
            },
        });
        if let Some(problem) = &shown.problem {
            controls.push(ControlShown::text(format!("problem:{}", setting.key), problem.clone()));
        }
    }
    if !page.caption.is_empty() {
        controls.push(ControlShown::text("caption", page.caption.clone()));
    }
    if page.id == "export" {
        for export in &settings.exports {
            controls.push(ControlShown::press(
                format!("export:{:?}", export.format),
                export.label.clone(),
                vec![ShellEvent::Settings(SettingsEvent::Export(export.format))],
            ));
        }
    }
    for button in &page.buttons {
        let sends = match button.id.as_str() {
            "sync-now" => ControlSends::Press(vec![ShellEvent::Key("sync-now".to_owned())]),
            other => ControlSends::Shell(other.to_owned()),
        };
        controls.push(ControlShown {
            letter: crate::braille::button_letter(&button.id).unwrap_or_default().to_owned(),
            asks: button.asks,
            ..ControlShown::button(button.id.clone(), button.title.clone(), sends)
        });
    }
    if page.id == "export" {
        for offered in &settings.offered {
            controls.push(ControlShown {
                asks: offered.asks != crate::Asks::Nothing,
                ..ControlShown::press(
                    format!("action:{:?}", offered.kind),
                    if offered.sentence.is_empty() { offered.title.clone() } else { offered.sentence.clone() },
                    vec![ShellEvent::ActOn(ActingOn::Settings, offered.clone())],
                )
            });
        }
        if !settings.offered_caption.is_empty() {
            controls.push(ControlShown::text("offered-caption", settings.offered_caption.clone()));
        }
    }
    Some(dialog(format!("settings:{}", page.id), page.title.clone(), controls))
}

/// The dialog the app has open on top, if one is: a question, else the block form, else quick
/// add. The task form and a settings page are opened by the shell ([`task_form`],
/// [`settings_page`]), the app holding no record of which page is shown.
#[must_use]
pub fn open(view: &ViewModel, lines: &Lines) -> Option<DialogShown> {
    if let Some(asking) = &view.asking {
        return question(asking, view, lines);
    }
    block_form(view).or_else(|| quick_add(view, lines))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_line_set_in_quick_add_puts_the_cursor_at_its_end_and_a_pick_needs_a_choice() {
        assert_eq!(
            control_event(&ControlSends::QuickAdd, "Buy milk", ControlMove::Set),
            [ShellEvent::Typing(Typing::Text { text: "Buy milk".to_owned(), cursor: 8 })]
        );
        assert!(control_event(&ControlSends::Pick, "", ControlMove::Pressed).is_empty(), "nothing chosen answers nothing");
        assert!(control_event(&ControlSends::Answer, "Office", ControlMove::Set).is_empty(), "a question is answered when pressed");
        assert!(control_event(&ControlSends::Shell("import".to_owned()), "", ControlMove::Pressed).is_empty(), "the shell's own");
        assert_eq!(
            control_event(&ControlSends::Setting("clock".to_owned()), "24", ControlMove::Typed { cursor: 2 }),
            [ShellEvent::Settings(SettingsEvent::Type { key: "clock".to_owned(), value: "24".to_owned() })]
        );
    }
}
