//! The task in hand's details, in the right-hand pane, drawn from the app's view
//! (`view.details`): which task, its fields as typed or as stored, and saving are the app's.
//!
//! Labelled stock fields: a static label before each control is what Windows names the
//! control by, and its mnemonic is the field's Alt shortcut. What each field takes is its
//! accessible description. Typing goes to the app as it happens (`DetailsEvent::Field`), which
//! keeps it through a change elsewhere and saves only what changed; the buttons are the task's
//! own actions, run as the app's.
//!
//! A field is written only when what the app holds differs from what it shows, so drawing
//! never moves the caret in a field being typed in.

use std::cell::{Cell, RefCell};

use lumenna_app::details::DetailsEvent;
use lumenna_app::lines::Lines;
use lumenna_app::{ActingOn, DetailsView, ViewModel};
use lumenna_ffi::ShellEvent;
use lumenna_records::fields::{DetailsWords, details_words};
use lumenna_records::{ActionKind, Asks, Choice, Offered, TaskDetail, priorities, task_form};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::System::SystemServices::{SS_CENTER, SS_NOPREFIX};
use windows::Win32::UI::Controls::{WC_BUTTONW, WC_COMBOBOXW, WC_EDITW, WC_LISTBOXW, WC_STATICW};
use windows::Win32::UI::Input::KeyboardAndMouse::{VK_ESCAPE, VK_TAB};
use windows::Win32::UI::Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass};
use windows::Win32::UI::WindowsAndMessaging::{
    BN_CLICKED, BS_PUSHBUTTON, CB_ADDSTRING, CB_GETCURSEL, CB_RESETCONTENT, CB_SETCURSEL, CBN_SELCHANGE, CBS_DROPDOWNLIST,
    DLGC_WANTALLKEYS, DLGC_WANTMESSAGE, DLGC_WANTTAB, EN_CHANGE, ES_AUTOHSCROLL, ES_AUTOVSCROLL, ES_MULTILINE, ES_READONLY,
    ES_WANTRETURN, LB_ADDSTRING, LB_GETCURSEL, LB_RESETCONTENT, LB_SETCURSEL, LBS_NOINTEGRALHEIGHT, LBS_NOTIFY, MSG,
    DestroyWindow, GetWindowRect, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SetWindowPos, WM_GETDLGCODE, WM_GETFONT, WM_KEYDOWN,
    WM_NCDESTROY, WM_SETFONT, WS_EX_CLIENTEDGE, WS_TABSTOP, WS_VSCROLL,
};
use windows::core::HSTRING;

use super::{a11y, dark};
use super::app::App;
use super::controls::{self, rect};
use super::view::Metrics;
use super::actions;
use crate::devices;

const TITLE: u16 = 300;
const DUE: u16 = 301;
const REPEAT: u16 = 302;
const PRIORITY: u16 = 303;
const ESTIMATE: u16 = 304;
const PROJECT: u16 = 305;
const LABELS: u16 = 306;
const NOTES: u16 = 307;
const WAITS: u16 = 308;
const ABOUT: u16 = 309;

/// Where a field sits: across the pane, or in one of its two columns.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Column {
    Full,
    Left,
    Right,
}

/// A field: its label, its control, where it sits, and how many lines tall it is.
struct Field {
    label: HWND,
    control: HWND,
    column: Column,
    lines: i32,
}

pub struct Detail {
    pane: HWND,
    /// Its words, the core's (`details_words`).
    words: DetailsWords,
    placeholder: HWND,
    fields: Vec<Field>,
    title: HWND,
    due: HWND,
    repeat: HWND,
    priority: HWND,
    estimate: HWND,
    project: HWND,
    labels: HWND,
    notes: HWND,
    waits: HWND,
    /// Wait for Another Task and Stop Waiting for the Selected Task, under the list of what it
    /// waits for.
    add_wait: HWND,
    stop_wait: HWND,
    /// What the details list besides the form, a line each (`Lines::details`): its state, what
    /// it waits for, its history, on the task's own screen and never in a list.
    about: HWND,
    save: HWND,
    /// The task's primary actions, as it offers them now, a button each after Save.
    offered: RefCell<Vec<(HWND, Offered)>>,
    /// Every action it offers, a menu away.
    more: HWND,
    /// Where the pane was last laid out, to lay it out again when its buttons change.
    laid: Cell<Option<(i32, i32, Metrics)>>,
    shown: RefCell<Option<TaskDetail>>,
    /// What the Project list offers, as the core gives it.
    projects: RefCell<Vec<Choice>>,
    /// The fields are being written from the app, not typed.
    filling: Cell<bool>,
}

impl Detail {
    pub fn create(pane: HWND) -> Self {
        let words = details_words(false);
        let placeholder = controls::create(pane, WC_STATICW, &words.none, SS_CENTER.0 | SS_NOPREFIX.0, 0, 0);
        let mut fields = Vec::new();
        let mut field = |label: &str, class, style: u32, id: u16, column: Column, lines: i32, description: &str| {
            let label = controls::create(pane, WC_STATICW, label, 0, 0, 0);
            let control = controls::create(pane, class, "", style | WS_TABSTOP.0, WS_EX_CLIENTEDGE.0, id);
            if !description.is_empty() {
                a11y::set_description(control, description);
            }
            fields.push(Field { label, control, column, lines });
            control
        };
        let line = ES_AUTOHSCROLL as u32;
        let list = CBS_DROPDOWNLIST as u32 | WS_VSCROLL.0;
        // The names and what each takes are the core's; the access keys are this pane's.
        let form = task_form();
        let named = |key: &str, letter: char| {
            let found = form.iter().find(|f| f.key == key);
            let label = found.map_or(key, |f| f.label.as_str());
            // A colon after, as every Windows label has.
            (format!("{}:", devices::marked(label, letter, '&')), found.map(|f| f.hint.clone()).unwrap_or_default())
        };
        // Made in reading order, which is also Tab's.
        let (label, hint) = named("title", 'i');
        let title = field(&label, WC_EDITW, line, TITLE, Column::Full, 1, &hint);
        let (label, hint) = named("due", 'u');
        let due = field(&label, WC_EDITW, line, DUE, Column::Left, 1, &hint);
        let (label, hint) = named("repeat", 'p');
        let repeat = field(&label, WC_EDITW, line, REPEAT, Column::Right, 1, &hint);
        let (label, hint) = named("priority", 'o');
        let priority = field(&label, WC_COMBOBOXW, list, PRIORITY, Column::Left, 1, &hint);
        let (label, hint) = named("estimate", 'm');
        let estimate = field(&label, WC_EDITW, line, ESTIMATE, Column::Right, 1, &hint);
        let (label, hint) = named("project", 'j');
        let project = field(&label, WC_COMBOBOXW, list, PROJECT, Column::Left, 1, &hint);
        let (label, hint) = named("labels", 'L');
        let labels = field(&label, WC_EDITW, line, LABELS, Column::Right, 1, &hint);
        let multiline = (ES_MULTILINE | ES_WANTRETURN | ES_AUTOVSCROLL) as u32 | WS_VSCROLL.0;
        let (label, hint) = named("notes", 'N');
        let notes = field(&label, WC_EDITW, multiline, NOTES, Column::Full, 4, &hint);
        let waits_style = (LBS_NOTIFY | LBS_NOINTEGRALHEIGHT) as u32 | WS_VSCROLL.0;
        let waits = field(&format!("{}:", devices::marked(&words.waits, 'W', '&')), WC_LISTBOXW, waits_style, WAITS, Column::Full, 3, &words.waits_hint);
        let button = |text: &str| controls::create(pane, WC_BUTTONW, text, BS_PUSHBUTTON as u32 | WS_TABSTOP.0, 0, 0);
        // Wait for Another Task asks which: an ellipsis says so.
        let add_wait = button(&format!("{}...", words.add_wait.replace('&', "&&")));
        let stop_wait = button(&words.stop_wait.replace('&', "&&"));
        let read_only = (ES_MULTILINE | ES_AUTOVSCROLL | ES_READONLY) as u32 | WS_VSCROLL.0;
        let about = field(&format!("{}:", devices::marked(&words.about, 'b', '&')), WC_EDITW, read_only, ABOUT, Column::Full, 4, "");
        let save = button(&devices::marked(&words.save, 'S', '&'));
        // Every action the task offers, as its row's menu offers them.
        let more = button(&format!("{}...", devices::marked(&words.actions, 'A', '&')));

        // An example of what each line takes, greyed while it is empty.
        for (key, edit) in [("title", title), ("due", due), ("repeat", repeat), ("estimate", estimate), ("labels", labels)] {
            if let Some(field) = form.iter().find(|f| f.key == key) {
                controls::cue(edit, &field.example);
            }
        }
        for choice in priorities() {
            let text = HSTRING::from(choice.title);
            controls::send(priority, CB_ADDSTRING, 0, text.as_ptr() as isize);
        }
        unsafe {
            let _ = SetWindowSubclass(notes, Some(leaves_on_tab), 1, 0);
            let _ = SetWindowSubclass(about, Some(leaves_on_tab), 1, 0);
        }
        let detail = Self {
            pane,
            words,
            placeholder,
            fields,
            title,
            due,
            repeat,
            priority,
            estimate,
            project,
            labels,
            notes,
            waits,
            add_wait,
            stop_wait,
            about,
            save,
            offered: RefCell::new(Vec::new()),
            more,
            laid: Cell::new(None),
            shown: RefCell::new(None),
            projects: RefCell::new(Vec::new()),
            filling: Cell::new(false),
        };
        detail.show_fields(false);
        detail
    }

    /// The task shown, if any.
    pub fn task_id(&self) -> Option<String> {
        self.shown.borrow().as_ref().map(|task| task.id.clone())
    }

    /// Where focus goes when F6 reaches the pane; nowhere while it shows nothing.
    pub fn focus_target(&self) -> Option<HWND> {
        self.shown.borrow().is_some().then_some(self.title)
    }

    fn show_fields(&self, visible: bool) {
        controls::show(self.placeholder, !visible);
        for field in &self.fields {
            controls::show(field.label, visible);
            controls::show(field.control, visible);
        }
        let offered: Vec<HWND> = self.offered.borrow().iter().map(|(button, _)| *button).collect();
        for button in [self.add_wait, self.stop_wait, self.save, self.more].into_iter().chain(offered) {
            controls::show(button, visible);
        }
    }

    /// The buttons after Save: the task's primary actions as it offers them now, made again
    /// only when they change, focus moving to the title first if it was on one going. What it
    /// waits for has its own buttons, and its own form is this pane.
    fn draw_offered(&self, task: &TaskDetail) {
        let wanted: Vec<Offered> = task
            .offered
            .iter()
            .filter(|action| action.primary && !matches!(action.kind, ActionKind::WaitFor | ActionKind::StopWaiting))
            .filter(|action| !(action.asks == Asks::Form && matches!(action.kind, ActionKind::Edit | ActionKind::EditTask)))
            .cloned()
            .collect();
        let same = {
            let had = self.offered.borrow();
            had.len() == wanted.len() && had.iter().zip(&wanted).all(|((_, a), b)| a.kind == b.kind && a.title == b.title)
        };
        if same {
            // The same buttons, each now standing for the task's own.
            for ((_, action), offered) in self.offered.borrow_mut().iter_mut().zip(wanted) {
                *action = offered;
            }
            return;
        }
        let old: Vec<HWND> = self.offered.borrow_mut().drain(..).map(|(button, _)| button).collect();
        if old.contains(&controls::focused()) {
            controls::focus(self.title);
        }
        for button in old {
            unsafe {
                let _ = DestroyWindow(button);
            }
        }
        let font = controls::send(self.save, WM_GETFONT, 0, 0);
        let mut after = self.save;
        let mut made = Vec::new();
        for action in wanted {
            let button = controls::create(self.pane, WC_BUTTONW, &actions::label(&action), BS_PUSHBUTTON as u32 | WS_TABSTOP.0, 0, 0);
            controls::send(button, WM_SETFONT, font as usize, 0);
            // In Tab's order after Save, or the one before it, and before Actions.
            unsafe {
                let _ = SetWindowPos(button, Some(after), 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
            }
            after = button;
            made.push((button, action));
        }
        *self.offered.borrow_mut() = made;
        dark::controls_in(self.pane);
        if let Some((width, height, m)) = self.laid.get() {
            self.layout(width, height, m);
        }
    }

    /// Draws the app's details of the task in hand, or none.
    pub fn draw(&self, view: &ViewModel, lines: &Lines, still: bool) -> bool {
        let Some(details) = &view.details else {
            // Never hide the control that has focus: focus would go nowhere, and the task stays
            // shown, as it was, until focus leaves.
            if self.fields.iter().any(|f| f.control == controls::focused()) {
                return false;
            }
            if self.shown.replace(None).is_some() {
                self.show_fields(false);
            }
            return false;
        };
        let other = self.task_id().as_deref() != Some(details.task.id.as_str());
        if other {
            self.show_fields(true);
        }
        self.filling.set(true);
        let held = self.fill(details, lines, other || still);
        self.filling.set(false);
        *self.shown.borrow_mut() = Some(details.task.clone());
        held
    }

    fn fill(&self, details: &DetailsView, lines: &Lines, still: bool) -> bool {
        let mut held = false;
        let fields = &details.fields;
        let task = &details.task;
        held |= controls::write_typed(self.title, &fields.title, still);
        held |= controls::write_typed(self.due, &fields.due, still);
        held |= controls::write_typed(self.repeat, &fields.repeat, still);
        let position = priorities().iter().position(|p| p.id == fields.priority.to_string());
        if usize::try_from(controls::send(self.priority, CB_GETCURSEL, 0, 0)).ok() != position {
            controls::send(self.priority, CB_SETCURSEL, position.unwrap_or(usize::MAX), 0);
        }
        held |= controls::write_typed(self.estimate, &fields.estimate, still);
        if *self.projects.borrow() != details.projects || self.chosen_project() != fields.project {
            self.fill_projects(&details.projects, &fields.project);
        }
        held |= controls::write_typed(self.labels, &fields.labels, still);
        // An edit control's lines end in CR LF.
        held |= controls::write_typed(self.notes, &fields.notes.replace("\r\n", "\n").replace('\n', "\r\n"), still);
        let waits: Vec<&str> = task.depends.iter().map(|other| other.title.as_str()).collect();
        let had: Vec<String> = self.shown.borrow().as_ref().map(|t| t.depends.iter().map(|d| d.title.clone()).collect()).unwrap_or_default();
        if had != waits || self.task_id().as_deref() != Some(task.id.as_str()) {
            controls::send(self.waits, LB_RESETCONTENT, 0, 0);
            // An empty list says so, in the core's word.
            let listed = if waits.is_empty() { vec![self.words.waits_none.as_str()] } else { waits.clone() };
            for title in listed {
                let text = HSTRING::from(title);
                controls::send(self.waits, LB_ADDSTRING, 0, text.as_ptr() as isize);
            }
            controls::send(self.waits, LB_SETCURSEL, 0, 0);
        }
        // Never disable the button that has focus: focus would go nowhere.
        for (button, on) in [
            (self.add_wait, actions::of_kind(&task.offered, &[ActionKind::WaitFor]).is_some()),
            (self.stop_wait, !task.depends.is_empty()),
        ] {
            if !on && controls::focused() == button {
                controls::focus(self.waits);
            }
            controls::enable(button, on);
        }
        // Its state, the rule it repeats by, what it waits for and its history, a line each.
        let about: Vec<String> = lines.details.iter().map(|line| line.text.clone()).collect();
        set_if_changed(self.about, &about.join("\r\n"));
        self.draw_offered(task);
        held
    }

    /// The projects to choose from, in tree order, the task's own chosen. A subproject says
    /// its level, which a drop-down list has no other way to; typing its first letters still
    /// finds it. The app lists an archived project the task is in too, so it is not lost.
    fn fill_projects(&self, projects: &[Choice], current: &str) {
        *self.projects.borrow_mut() = projects.to_vec();
        controls::send(self.project, CB_RESETCONTENT, 0, 0);
        for project in projects {
            let text = if project.depth > 0 { format!("{}, {}", project.title, lumenna_records::words::level(project.depth)) } else { project.title.clone() };
            let text = HSTRING::from(text);
            controls::send(self.project, CB_ADDSTRING, 0, text.as_ptr() as isize);
        }
        let position = projects.iter().position(|p| p.id == current);
        controls::send(self.project, CB_SETCURSEL, position.unwrap_or(usize::MAX), 0);
    }

    /// The project chosen, by name; none chosen is empty.
    fn chosen_project(&self) -> String {
        let index = usize::try_from(controls::send(self.project, CB_GETCURSEL, 0, 0)).ok();
        index.and_then(|i| self.projects.borrow().get(i).map(|p| p.id.clone())).unwrap_or_default()
    }

    /// Saves what changed, as the app does: it says what that did, or that nothing changed.
    pub fn save(&self, app: &App) {
        if self.shown.borrow().is_some() {
            app.crux.send(ShellEvent::Details(DetailsEvent::Save));
        }
    }

    pub fn command(&self, app: &App, control: HWND, _id: u16, code: u16) -> bool {
        let code = u32::from(code);
        // Typing and choosing, as it happens: the app keeps it until it is saved.
        let edits = [
            (self.title, "title"),
            (self.due, "due"),
            (self.repeat, "repeat"),
            (self.estimate, "estimate"),
            (self.labels, "labels"),
            (self.notes, "notes"),
        ];
        if let Some((_, key)) = edits.iter().find(|(edit, _)| *edit == control) {
            if code == EN_CHANGE && !self.filling.get() {
                let value = controls::text(control).replace("\r\n", "\n");
                app.crux.send(ShellEvent::Details(DetailsEvent::Field { key: (*key).to_owned(), value }));
            }
            return true;
        }
        if control == self.priority || control == self.project {
            if code == CBN_SELCHANGE && !self.filling.get() {
                let (key, value) = if control == self.priority {
                    let chosen = usize::try_from(controls::send(self.priority, CB_GETCURSEL, 0, 0)).ok();
                    ("priority", chosen.and_then(|i| priorities().get(i).map(|p| p.id.clone())).unwrap_or_default())
                } else {
                    ("project", self.chosen_project())
                };
                app.crux.send(ShellEvent::Details(DetailsEvent::Field { key: key.to_owned(), value }));
            }
            return true;
        }
        if code != BN_CLICKED {
            return false;
        }
        if control == self.save {
            self.save(app);
            return true;
        }
        let Some(task) = self.shown.borrow().clone() else {
            return [self.add_wait, self.stop_wait, self.more].contains(&control) || self.offered.borrow().iter().any(|(b, _)| *b == control);
        };
        let offered = if control == self.add_wait {
            actions::of_kind(&task.offered, &[ActionKind::WaitFor])
        } else if control == self.stop_wait {
            // The task chosen in the list, whose Stop Waiting names it.
            let index = controls::send(self.waits, LB_GETCURSEL, 0, 0);
            let other = usize::try_from(index).ok().and_then(|i| task.depends.get(i)).map(|d| d.id.clone());
            task.offered.iter().find(|a| a.kind == ActionKind::StopWaiting && a.other == other).cloned()
        } else if control == self.more {
            // Every action the task offers, as its row's menu offers them, under the button.
            let mut placed = RECT::default();
            unsafe {
                let _ = GetWindowRect(self.more, &mut placed);
            }
            let items = actions::menu(&task.offered);
            let items: Vec<(u16, &str)> = items.iter().map(|(id, text)| (*id, text.as_str())).collect();
            app.popup(&items, POINT { x: placed.left, y: placed.bottom }).and_then(|command| actions::chosen(&task.offered, command))
        } else {
            let found = self.offered.borrow().iter().find(|(b, _)| *b == control).map(|(_, action)| action.clone());
            let Some(found) = found else { return false };
            Some(found)
        };
        if let Some(offered) = offered {
            self.act(app, offered);
        }
        true
    }

    /// Runs one of the task's own actions as the app's; its form is this pane.
    pub fn act(&self, app: &App, action: Offered) {
        if action.asks == Asks::Form && action.kind != ActionKind::Reminders {
            app.open_detail();
        } else {
            app.crux.send(ShellEvent::ActOn(ActingOn::Details, action));
        }
    }

    pub fn layout(&self, width: i32, height: i32, m: Metrics) {
        self.laid.set(Some((width, height, m)));
        let gap = m.gap();
        let inner = width - 2 * gap;
        let half = (inner - gap) / 2;
        controls::place(self.placeholder, rect(gap, height / 3, inner, m.line * 2));
        let mut y = gap;
        for field in &self.fields {
            let (x, w) = match field.column {
                Column::Full => (gap, inner),
                Column::Left => (gap, half),
                Column::Right => (gap + half + gap, half),
            };
            let tall = if field.lines > 1 { m.line * field.lines + m.px(8) } else { m.field };
            controls::place(field.label, rect(x, y, w, m.line));
            // A drop-down list's height includes its list.
            let list = if [self.priority, self.project].contains(&field.control) { m.line * 8 } else { 0 };
            controls::place(field.control, rect(x, y + m.line + m.px(2), w, tall + list));
            // A left field shares its row with the right one after it.
            if field.column != Column::Left {
                y += m.line + m.px(2) + tall + m.px(6);
            }
            if field.control == self.waits {
                // Their names are long: a row each.
                for button in [self.add_wait, self.stop_wait] {
                    controls::place(button, rect(gap, y - m.px(2), inner, m.button));
                    y += m.button + m.px(6);
                }
                y += m.px(2);
            }
        }
        let buttons: Vec<HWND> =
            std::iter::once(self.save).chain(self.offered.borrow().iter().map(|(button, _)| *button)).chain([self.more]).collect();
        for (row, chunk) in buttons.chunks(3).enumerate() {
            row_of_buttons(chunk, gap, y + row as i32 * (m.button + m.px(6)), inner, m);
        }
    }
}

/// Writes a control's text only when it differs, so the caret in a field being typed in stays.
fn set_if_changed(control: HWND, text: &str) {
    if controls::text(control) != text {
        controls::set_text(control, text);
    }
}

/// Lays buttons out in a row, three to its width.
fn row_of_buttons(buttons: &[HWND], x: i32, y: i32, width: i32, m: Metrics) {
    let each = (width - 2 * m.px(6)) / 3;
    for (index, button) in buttons.iter().enumerate() {
        controls::place(*button, rect(x + index as i32 * (each + m.px(6)), y, each, m.button));
    }
}

/// A multi-line field keeps Enter for new lines but gives Tab and Escape back to the window:
/// otherwise Tab is typed into the notes and a keyboard user cannot leave them.
unsafe extern "system" fn leaves_on_tab(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    id: usize,
    _data: usize,
) -> LRESULT {
    unsafe {
        match message {
            WM_GETDLGCODE => {
                let code = DefSubclassProc(hwnd, message, wparam, lparam);
                let asked = lparam.0 as *const MSG;
                if !asked.is_null() && (*asked).message == WM_KEYDOWN {
                    let key = (*asked).wParam.0 as u16;
                    if key == VK_TAB.0 || key == VK_ESCAPE.0 {
                        return LRESULT(code.0 & !((DLGC_WANTALLKEYS | DLGC_WANTTAB | DLGC_WANTMESSAGE) as isize));
                    }
                }
                code
            }
            WM_NCDESTROY => {
                let _ = RemoveWindowSubclass(hwnd, Some(leaves_on_tab), id);
                DefSubclassProc(hwnd, message, wparam, lparam)
            }
            _ => DefSubclassProc(hwnd, message, wparam, lparam),
        }
    }
}
