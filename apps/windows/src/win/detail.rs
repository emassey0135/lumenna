//! The selected task's details (§16.1: task detail / edit), in the right-hand pane.
//!
//! Labelled stock fields: a static label before each control is what Windows names the
//! control by, and its mnemonic is the field's Alt shortcut. What each field takes is its
//! accessible description. Saving sends only the fields that changed (`form.rs`).
//!
//! The fields follow the store while nobody is editing them: a change from another device or
//! another process refills them, unless they hold typing not yet saved, which it would lose.

use std::cell::RefCell;

use lumenna_surface::TaskDetail;
use windows::Win32::System::SystemServices::{SS_CENTER, SS_NOPREFIX};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Controls::{WC_BUTTONW, WC_COMBOBOXW, WC_EDITW, WC_STATICW};
use windows::Win32::UI::Input::KeyboardAndMouse::{VK_ESCAPE, VK_TAB};
use windows::Win32::UI::Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass};
use windows::Win32::UI::WindowsAndMessaging::{
    BN_CLICKED, BS_PUSHBUTTON, CB_ADDSTRING, CB_FINDSTRINGEXACT, CB_GETCURSEL, CB_GETLBTEXT,
    CB_GETLBTEXTLEN, CB_RESETCONTENT, CB_SETCURSEL, CBS_DROPDOWNLIST, DLGC_WANTALLKEYS,
    DLGC_WANTMESSAGE, DLGC_WANTTAB, ES_AUTOHSCROLL, ES_AUTOVSCROLL, ES_MULTILINE, ES_READONLY,
    ES_WANTRETURN, MSG, WM_GETDLGCODE, WM_KEYDOWN, WM_NCDESTROY,
    WS_EX_CLIENTEDGE, WS_TABSTOP, WS_VSCROLL,
};
use windows::core::HSTRING;

use super::app::App;
use super::controls::{self, rect};
use super::a11y;
use super::view::Metrics;
use crate::form::TaskFields;

const TITLE: u16 = 300;
const DUE: u16 = 301;
const REPEAT: u16 = 302;
const PRIORITY: u16 = 303;
const ESTIMATE: u16 = 304;
const PROJECT: u16 = 305;
const LABELS: u16 = 306;
const NOTES: u16 = 307;
const STATE: u16 = 308;
const SAVE: u16 = 309;
const DONE: u16 = 310;

const PRIORITIES: [&str; 4] = ["Priority 1, highest", "Priority 2", "Priority 3", "Priority 4, none"];

/// A field: its label, its control, and how many lines tall the control is.
struct Field {
    label: HWND,
    control: HWND,
    lines: i32,
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
    state: HWND,
    save: HWND,
    done: HWND,
    shown: RefCell<Option<TaskDetail>>,
}

impl Detail {
    pub fn create(pane: HWND) -> Self {
        let placeholder = controls::create(pane, WC_STATICW, "No task selected", SS_CENTER.0 | SS_NOPREFIX.0, 0, 0);
        let mut fields = Vec::new();
        let mut field = |label: &str, class, style: u32, id: u16, lines: i32, description: &str| {
            let label = controls::create(pane, WC_STATICW, label, 0, 0, 0);
            let control = controls::create(pane, class, "", style | WS_TABSTOP.0, WS_EX_CLIENTEDGE.0, id);
            if !description.is_empty() {
                a11y::set_description(control, description);
            }
            fields.push(Field { label, control, lines });
            control
        };
        let line = ES_AUTOHSCROLL as u32;
        let title = field("T&itle", WC_EDITW, line, TITLE, 1, "");
        let due = field("D&ue", WC_EDITW, line, DUE, 1, "A date, such as tomorrow or next Friday. Empty for none. A new date keeps how it repeats.");
        let repeat = field("Re&peats", WC_EDITW, line, REPEAT, 1, "Such as every Monday, or every! 2 weeks to count from when it is done. Empty for no repetition.");
        let priority = field("Pri&ority", WC_COMBOBOXW, CBS_DROPDOWNLIST as u32 | WS_VSCROLL.0, PRIORITY, 1, "");
        let estimate = field("Esti&mate", WC_EDITW, line, ESTIMATE, 1, "Such as 45m or 1h30m. Empty for none.");
        let project = field("Pro&ject", WC_COMBOBOXW, CBS_DROPDOWNLIST as u32 | WS_VSCROLL.0, PROJECT, 1, "");
        let labels = field("&Labels", WC_EDITW, line, LABELS, 1, "Names separated by commas. A new name becomes a label.");
        let multiline = (ES_MULTILINE | ES_WANTRETURN | ES_AUTOVSCROLL) as u32 | WS_VSCROLL.0;
        let notes = field("&Notes", WC_EDITW, multiline, NOTES, 5, "");
        let state = field("State", WC_EDITW, line | ES_READONLY as u32, STATE, 1, "");
        let save = controls::create(pane, WC_BUTTONW, "&Save", BS_PUSHBUTTON as u32 | WS_TABSTOP.0, 0, SAVE);
        let done = controls::create(pane, WC_BUTTONW, "Mark Done", BS_PUSHBUTTON as u32 | WS_TABSTOP.0, 0, DONE);

        for text in PRIORITIES {
            let text = HSTRING::from(text);
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
            state,
            save,
            done,
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
        controls::show(self.save, visible);
        controls::show(self.done, visible);
        if let Some(task) = &task {
            let fields = TaskFields::of(task);
            controls::set_text(self.title, &fields.title);
            controls::set_text(self.due, &fields.due);
            controls::set_text(self.repeat, &fields.repeat);
            controls::send(self.priority, CB_SETCURSEL, usize::from(fields.priority.clamp(1, 4) - 1), 0);
            controls::set_text(self.estimate, &fields.estimate);
            select_text(self.project, &fields.project);
            controls::set_text(self.labels, &fields.labels);
            // An edit control's lines end in CR LF.
            controls::set_text(self.notes, &fields.notes.replace("\r\n", "\n").replace('\n', "\r\n"));
            let states: Vec<&str> = task.state.iter().map(String::as_str).filter(|s| *s != "ready").collect();
            let state = if states.is_empty() { "open".to_owned() } else { states.join(", ") };
            controls::set_text(self.state, &state);
            let completed = task.state.iter().any(|s| s == "completed");
            controls::set_text(self.done, if completed { "Mark Not Done" } else { "Mark Done" });
        }
        *self.shown.borrow_mut() = task;
    }

    /// The fields as they are now.
    fn read(&self) -> TaskFields {
        let priority = controls::send(self.priority, CB_GETCURSEL, 0, 0);
        TaskFields {
            title: controls::text(self.title),
            due: controls::text(self.due),
            repeat: controls::text(self.repeat),
            priority: u8::try_from(priority + 1).unwrap_or(4).clamp(1, 4),
            estimate: controls::text(self.estimate),
            project: selected_text(self.project),
            labels: controls::text(self.labels),
            notes: controls::text(self.notes).replace("\r\n", "\n"),
        }
    }

    /// Whether the fields hold typing not yet saved.
    pub fn has_changes(&self) -> bool {
        self.shown.borrow().as_ref().is_some_and(|task| self.read().edit(task).is_some())
    }

    /// The store changed: the fields follow unless someone is part way through editing them.
    pub fn reload(&self, app: &App) {
        if let Some(id) = self.task_id()
            && !self.has_changes() {
                self.load(app, &id);
            }
    }

    /// Saves what changed, and says what that did.
    pub fn save(&self, app: &App) {
        let Some(task) = self.shown.borrow().clone() else { return };
        let Some(edit) = self.read().edit(&task) else {
            app.say("Nothing changed");
            return;
        };
        if let Some(change) = app.perform(|lumenna| lumenna.edit_task(&task.id, edit)) {
            self.load(app, &task.id);
            app.say_change(&change);
        }
    }

    pub fn toggle_done(&self, app: &App) {
        let Some(task) = self.shown.borrow().clone() else { return };
        let completed = task.state.iter().any(|s| s == "completed");
        if let Some(change) = app.perform(|lumenna| {
            if completed { lumenna.uncomplete_task(&task.id) } else { lumenna.complete_task(&task.id) }
        }) {
            self.load(app, &task.id);
            app.say_change(&change);
        }
    }

    pub fn command(&self, app: &App, control: HWND, _id: u16, code: u16) -> bool {
        if u32::from(code) != BN_CLICKED {
            return false;
        }
        if control == self.save {
            self.save(app);
        } else if control == self.done {
            self.toggle_done(app);
        } else {
            return false;
        }
        true
    }

    pub fn layout(&self, width: i32, height: i32, m: Metrics) {
        let gap = m.gap();
        let inner = width - 2 * gap;
        controls::place(self.placeholder, rect(gap, height / 3, inner, m.line * 2));
        let mut y = gap;
        for field in &self.fields {
            controls::place(field.label, rect(gap, y, inner, m.line));
            y += m.line + m.px(2);
            let tall = if field.lines > 1 { m.line * field.lines + m.px(8) } else { m.field };
            // A drop-down list's height includes its list.
            let list = if [self.priority, self.project].contains(&field.control) { m.line * 8 } else { 0 };
            controls::place(field.control, rect(gap, y, inner, tall + list));
            y += tall + m.px(6);
        }
        let button = m.px(110);
        controls::place(self.save, rect(gap, y + m.px(4), button, m.button));
        controls::place(self.done, rect(gap + button + m.px(8), y + m.px(4), button, m.button));
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
/// otherwise Tab is typed into the notes and a keyboard user cannot leave them (§6.4's rule,
/// for a different field).
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
