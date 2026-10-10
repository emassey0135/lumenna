//! The selected task's details, in the right-hand pane.
//!
//! Labelled stock fields: a static label before each control is what Windows names the
//! control by, and its mnemonic is the field's Alt shortcut. What each field takes is its
//! accessible description. Saving sends only the fields that changed (the surface's
//! `task_edit`), and the buttons are the Task menu's own actions (`task_actions.rs`).
//!
//! The fields follow the store while nobody is editing them: a change from another device or
//! another process refills them, unless they hold typing not yet saved, which it would lose.

use std::cell::RefCell;

use lumenna_surface::{ActionKind, TaskDetail, TaskFields, priorities, task_edit, task_fields};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::SystemServices::{SS_CENTER, SS_NOPREFIX};
use windows::Win32::UI::Controls::{WC_BUTTONW, WC_COMBOBOXW, WC_EDITW, WC_LISTBOXW, WC_STATICW};
use windows::Win32::UI::Input::KeyboardAndMouse::{VK_ESCAPE, VK_TAB};
use windows::Win32::UI::Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass};
use windows::Win32::UI::WindowsAndMessaging::{
    BN_CLICKED, BS_PUSHBUTTON, CB_ADDSTRING, CB_FINDSTRINGEXACT, CB_GETCURSEL, CB_GETLBTEXT,
    CB_GETLBTEXTLEN, CB_RESETCONTENT, CB_SETCURSEL, CBS_DROPDOWNLIST, DLGC_WANTALLKEYS,
    DLGC_WANTMESSAGE, DLGC_WANTTAB, ES_AUTOHSCROLL, ES_AUTOVSCROLL, ES_MULTILINE, ES_READONLY,
    ES_WANTRETURN, LB_ADDSTRING, LB_GETCURSEL, LB_RESETCONTENT, LB_SETCURSEL, LBS_NOINTEGRALHEIGHT,
    LBS_NOTIFY, MSG, WM_GETDLGCODE, WM_KEYDOWN, WM_NCDESTROY, WS_EX_CLIENTEDGE, WS_TABSTOP,
    WS_VSCROLL,
};
use windows::core::HSTRING;

use super::a11y;
use super::app::App;
use super::controls::{self, rect};
use super::view::Metrics;
use super::actions;

const TITLE: u16 = 300;
const DUE: u16 = 301;
const REPEAT: u16 = 302;
const PRIORITY: u16 = 303;
const ESTIMATE: u16 = 304;
const PROJECT: u16 = 305;
const LABELS: u16 = 306;
const NOTES: u16 = 307;
const WAITS: u16 = 308;
const STATE: u16 = 309;

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

/// What a button does: saving, the task's action of these kinds, or stopping waiting for the
/// task chosen in the list above it.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Action {
    Save,
    Kinds(&'static [ActionKind]),
    StopWaiting,
}

pub struct Detail {
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
    /// "Add…" and "Stop Waiting", under the list of what it waits for.
    wait_buttons: Vec<(HWND, Action)>,
    state: HWND,
    /// Save, Mark Done and the rest, in rows of three.
    buttons: Vec<(HWND, Action)>,
    shown: RefCell<Option<TaskDetail>>,
}

impl Detail {
    pub fn create(pane: HWND) -> Self {
        let placeholder = controls::create(pane, WC_STATICW, "No task selected", SS_CENTER.0 | SS_NOPREFIX.0, 0, 0);
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
        // Made in reading order, which is also Tab's.
        let title = field("T&itle", WC_EDITW, line, TITLE, Column::Full, 1, "");
        let due = field("D&ue", WC_EDITW, line, DUE, Column::Left, 1, "A date, such as tomorrow or next Friday. Empty for none. A new date keeps how it repeats.");
        let repeat = field("Re&peats", WC_EDITW, line, REPEAT, Column::Right, 1, "Such as every Monday, or every! 2 weeks to count from when it is done. Empty for no repetition.");
        let priority = field("Pri&ority", WC_COMBOBOXW, list, PRIORITY, Column::Left, 1, "");
        let estimate = field("Esti&mate", WC_EDITW, line, ESTIMATE, Column::Right, 1, "Such as 45m or 1h30m. Empty for none.");
        let project = field("Pro&ject", WC_COMBOBOXW, list, PROJECT, Column::Left, 1, "");
        let labels = field("&Labels", WC_EDITW, line, LABELS, Column::Right, 1, "Names separated by commas. A new name becomes a label.");
        let multiline = (ES_MULTILINE | ES_WANTRETURN | ES_AUTOVSCROLL) as u32 | WS_VSCROLL.0;
        let notes = field("&Notes", WC_EDITW, multiline, NOTES, Column::Full, 4, "");
        let waits_style = (LBS_NOTIFY | LBS_NOINTEGRALHEIGHT) as u32 | WS_VSCROLL.0;
        let waits = field("&Waits for", WC_LISTBOXW, waits_style, WAITS, Column::Full, 3, "The tasks this one waits for. It is blocked until they are done.");
        let button = |text: &str, action: Action| {
            (controls::create(pane, WC_BUTTONW, text, BS_PUSHBUTTON as u32 | WS_TABSTOP.0, 0, 0), action)
        };
        let wait_buttons = vec![button("Add...", Action::Kinds(&[ActionKind::WaitFor])), button("Stop Waiting", Action::StopWaiting)];
        a11y::set_name(wait_buttons[0].0, "Add something it waits for");
        a11y::set_name(wait_buttons[1].0, "Stop waiting for the selected task");
        let state = field("State", WC_EDITW, line | ES_READONLY as u32, STATE, Column::Full, 1, "");
        let buttons = vec![
            button("&Save", Action::Save),
            // Named again from the task's own actions whenever one is shown.
            button("Mark Done", Action::Kinds(&[ActionKind::MarkDone, ActionKind::MarkNotDone])),
            button("Put in a Block...", Action::Kinds(&[ActionKind::PutInBlock])),
            button("Make Subtask Of...", Action::Kinds(&[ActionKind::MakeSubtaskOf])),
            button("Move to Top Level", Action::Kinds(&[ActionKind::MoveToTopLevel])),
            button("Move to Trash", Action::Kinds(&[ActionKind::Delete])),
        ];

        for choice in priorities() {
            let text = HSTRING::from(choice.title);
            controls::send(priority, CB_ADDSTRING, 0, text.as_ptr() as isize);
        }
        unsafe {
            let _ = SetWindowSubclass(notes, Some(leaves_on_tab), 1, 0);
        }
        let detail = Self {
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
            wait_buttons,
            state,
            buttons,
            shown: RefCell::new(None),
        };
        detail.fill(None);
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

    /// Shows a task, or none. The same task stays as it is, so typing not yet saved is kept
    /// when the list reloads around a change made here.
    pub fn show(&self, app: &App, id: Option<&str>) {
        if self.task_id().as_deref() == id {
            return;
        }
        match id {
            Some(id) => self.load(app, id),
            None => self.fill(None),
        }
    }

    /// Reads the task again, and refills the fields with it.
    fn load(&self, app: &App, id: &str) {
        let projects: Vec<String> =
            app.core.lumenna.list_projects().map(|r| r.rows.into_iter().map(|p| p.title).collect()).unwrap_or_default();
        controls::send(self.project, CB_RESETCONTENT, 0, 0);
        for name in &projects {
            let text = HSTRING::from(name.as_str());
            controls::send(self.project, CB_ADDSTRING, 0, text.as_ptr() as isize);
        }
        match app.core.lumenna.show_task(id) {
            Ok(shown) => self.fill(Some(shown.task)),
            Err(_) => self.fill(None),
        }
    }

    fn fill(&self, task: Option<TaskDetail>) {
        let visible = task.is_some();
        controls::show(self.placeholder, !visible);
        for field in &self.fields {
            controls::show(field.label, visible);
            controls::show(field.control, visible);
        }
        for (button, _) in self.wait_buttons.iter().chain(&self.buttons) {
            controls::show(*button, visible);
        }
        if let Some(task) = &task {
            let fields = task_fields(task.clone());
            controls::set_text(self.title, &fields.title);
            controls::set_text(self.due, &fields.due);
            controls::set_text(self.repeat, &fields.repeat);
            let position = priorities().iter().position(|p| p.id == fields.priority.to_string());
            controls::send(self.priority, CB_SETCURSEL, position.unwrap_or(usize::MAX), 0);
            controls::set_text(self.estimate, &fields.estimate);
            select_text(self.project, &fields.project);
            controls::set_text(self.labels, &fields.labels);
            // An edit control's lines end in CR LF.
            controls::set_text(self.notes, &fields.notes.replace("\r\n", "\n").replace('\n', "\r\n"));
            controls::send(self.waits, LB_RESETCONTENT, 0, 0);
            for other in &task.depends {
                let text = HSTRING::from(other.title.as_str());
                controls::send(self.waits, LB_ADDSTRING, 0, text.as_ptr() as isize);
            }
            controls::send(self.waits, LB_SETCURSEL, 0, 0);
            controls::enable(self.wait_buttons[1].0, !task.depends.is_empty());
            controls::set_text(self.state, &crate::speech::task_state(task));
            for (button, action) in self.wait_buttons.iter().chain(&self.buttons) {
                if let Action::Kinds(kinds) = action {
                    let offered = actions::of_kind(&task.actions, kinds);
                    // "Add…" beside the list it adds to says enough; the rest are named as offered.
                    if let Some(offered) = offered.as_ref().filter(|_| !self.wait_buttons.iter().any(|(b, _)| b == button)) {
                        controls::set_text(*button, &actions::label(offered));
                    }
                    // Never the button that has focus: focus would go nowhere.
                    if offered.is_some() || controls::focused() != *button {
                        controls::enable(*button, offered.is_some());
                    }
                }
            }
        }
        *self.shown.borrow_mut() = task;
    }

    /// The fields as they are now.
    fn read(&self) -> TaskFields {
        // The priority chosen, as the core numbers it; nothing chosen reads as none.
        let chosen = usize::try_from(controls::send(self.priority, CB_GETCURSEL, 0, 0)).ok();
        let priority = chosen.and_then(|i| priorities().get(i).and_then(|p| p.id.parse().ok())).unwrap_or(4);
        TaskFields {
            title: controls::text(self.title),
            due: controls::text(self.due),
            repeat: controls::text(self.repeat),
            priority,
            estimate: controls::text(self.estimate),
            project: selected_text(self.project),
            labels: controls::text(self.labels),
            notes: controls::text(self.notes).replace("\r\n", "\n"),
        }
    }

    /// Whether the fields hold typing not yet saved.
    pub fn has_changes(&self) -> bool {
        self.shown.borrow().as_ref().is_some_and(|task| task_edit(task.clone(), self.read()).is_some())
    }

    /// The store changed: the fields follow unless someone is part way through editing them.
    pub fn reload(&self, app: &App) {
        if let Some(id) = self.task_id()
            && !self.has_changes()
        {
            self.load(app, &id);
        }
    }

    /// Saves what changed, and says what that did.
    pub fn save(&self, app: &App) {
        let Some(task) = self.shown.borrow().clone() else { return };
        let Some(edit) = task_edit(task.clone(), self.read()) else {
            app.say("Nothing changed");
            return;
        };
        if let Some(change) = app.perform(|lumenna| lumenna.edit_task(&task.id, edit)) {
            self.load(app, &task.id);
            app.say_change(&change);
        }
    }

    pub fn command(&self, app: &App, control: HWND, _id: u16, code: u16) -> bool {
        if u32::from(code) != BN_CLICKED {
            return false;
        }
        let action = self.wait_buttons.iter().chain(&self.buttons).find(|(b, _)| *b == control).map(|(_, a)| *a);
        let Some(action) = action else { return false };
        let Some(task) = self.shown.borrow().clone() else { return true };
        let offered = match action {
            Action::Save => return {
                self.save(app);
                true
            },
            Action::Kinds(kinds) => actions::of_kind(&task.actions, kinds),
            Action::StopWaiting => {
                // The task chosen in the list, whose Stop Waiting names it.
                let index = controls::send(self.waits, LB_GETCURSEL, 0, 0);
                let other = usize::try_from(index).ok().and_then(|i| task.depends.get(i)).map(|d| d.id.clone());
                task.actions.iter().find(|a| a.kind == ActionKind::StopWaiting && a.other == other).cloned()
            }
        };
        if let Some(offered) = offered
            && let Some((change, _)) = actions::run(app, app.main, &offered, || app.open_detail())
        {
            app.say_change(&change);
        }
        true
    }

    /// What can be done to the task shown: for the Task menu, while focus is here.
    pub fn actions(&self) -> Option<Vec<lumenna_surface::Action>> {
        self.shown.borrow().as_ref().map(|task| task.actions.clone())
    }

    pub fn layout(&self, width: i32, height: i32, m: Metrics) {
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
                row_of_buttons(&self.wait_buttons, gap, y - m.px(2), inner, m);
                y += m.button + m.px(8);
            }
        }
        for (row, chunk) in self.buttons.chunks(3).enumerate() {
            row_of_buttons(chunk, gap, y + row as i32 * (m.button + m.px(6)), inner, m);
        }
    }
}

/// Lays buttons out in a row, three to its width.
fn row_of_buttons(buttons: &[(HWND, Action)], x: i32, y: i32, width: i32, m: Metrics) {
    let each = (width - 2 * m.px(6)) / 3;
    for (index, (button, _)) in buttons.iter().enumerate() {
        controls::place(*button, rect(x + index as i32 * (each + m.px(6)), y, each, m.button));
    }
}

/// Selects the item with this text in a drop-down list, adding it if missing.
fn select_text(combo: HWND, text: &str) {
    let wide = HSTRING::from(text);
    let mut index = controls::send(combo, CB_FINDSTRINGEXACT, usize::MAX, wide.as_ptr() as isize);
    if index < 0 && !text.is_empty() {
        index = controls::send(combo, CB_ADDSTRING, 0, wide.as_ptr() as isize);
    }
    controls::send(combo, CB_SETCURSEL, index.max(-1) as usize, 0);
}

/// The selected item's text in a drop-down list.
fn selected_text(combo: HWND) -> String {
    let index = controls::send(combo, CB_GETCURSEL, 0, 0);
    if index < 0 {
        return String::new();
    }
    let length = controls::send(combo, CB_GETLBTEXTLEN, index as usize, 0);
    let mut buffer = vec![0u16; usize::try_from(length).unwrap_or(0) + 1];
    let copied = controls::send(combo, CB_GETLBTEXT, index as usize, buffer.as_mut_ptr() as isize);
    String::from_utf16_lossy(&buffer[..usize::try_from(copied).unwrap_or(0)])
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
