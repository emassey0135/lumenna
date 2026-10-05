//! Reading and driving the app through native UI Automation — the `IUIAutomation` NVDA and
//! Narrator call, so what this reads is what they are given. Shared by the UI tests
//! (`tests/ui.rs`) and the inspector (`examples/inspect.rs`).
//!
//! Keys are *posted* into the app's own queue rather than pressed: they pass through its
//! accelerators and dialog manager exactly as pressed keys do, but need no window in front
//! and are not taken by a screen reader's keyboard hook. No modifier can be held that way, so
//! a Ctrl shortcut is given as the menu command it stands for.

#![allow(dead_code)]

use std::time::{Duration, Instant};

use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx};
use windows::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
use windows::Win32::System::Variant::VARIANT;
use windows::Win32::UI::Accessibility::*;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::WindowsAndMessaging::{
    BringWindowToTop, EnumWindows, FindWindowW, GA_ROOT, GA_ROOTOWNER, GUITHREADINFO, GetAncestor,
    GetForegroundWindow, GetGUIThreadInfo, GetParent, GetWindowTextW, GetWindowThreadProcessId, IsIconic, IsWindowVisible,
    PostMessageW, SW_RESTORE, ShowWindow,
    SetForegroundWindow, SwitchToThisWindow, WM_CHAR, WM_COMMAND, WM_CONTEXTMENU, WM_KEYDOWN, WM_KEYUP,
    WM_NEXTDLGCTL,
};
use windows::core::{BOOL, w};

/// How long a posted key is given to take effect before what it did is read.
pub const SETTLE: Duration = Duration::from_millis(500);

pub struct Automation {
    uia: IUIAutomation,
}

impl Automation {
    pub fn new() -> Self {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            Self { uia: CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER).expect("UI Automation") }
        }
    }

    /// A visible top-level window with `title` in its title.
    pub fn window_titled(&self, title: &str) -> Option<HWND> {
        find_window(|hwnd, text, _| unsafe { IsWindowVisible(hwnd) }.as_bool() && text.contains(title))
    }

    /// A process's visible window with "Lumenna" in its title, waiting up to `timeout` for it.
    pub fn window_of(&self, process: u32, timeout: Duration) -> Option<HWND> {
        wait(timeout, || {
            find_window(|hwnd, text, owner| {
                owner == process && unsafe { IsWindowVisible(hwnd) }.as_bool() && text.contains("Lumenna")
            })
        })
    }

    /// The window that holds focus in `app`'s thread: a dialog or sheet when one is open,
    /// else the main window.
    pub fn front(&self, app: HWND) -> HWND {
        let focus = thread_focus(app);
        if focus.is_invalid() { app } else { unsafe { GetAncestor(focus, GA_ROOT) } }
    }

    /// A window's tree, one line per element, `depth` levels deep.
    pub fn dump(&self, window: HWND, depth: usize) -> Vec<String> {
        let mut lines = Vec::new();
        if let (Ok(root), Ok(walker)) = unsafe { (self.uia.ElementFromHandle(window), self.uia.ControlViewWalker()) } {
            walk(&walker, &root, 0, depth, &mut lines);
        }
        lines
    }

    /// Where focus is in `app`, as a screen reader would say it: the focused control, and in
    /// a tree or list the selected item, which is what is read there.
    pub fn focus(&self, app: HWND) -> String {
        let focus = thread_focus(app);
        let Ok(element) = (unsafe { self.uia.ElementFromHandle(focus) }) else { return "nothing".to_owned() };
        let mut text = describe(&element);
        unsafe {
            if let Ok(selection) = element.GetCurrentPatternAs::<IUIAutomationSelectionPattern>(UIA_SelectionPatternId)
                && let Ok(selected) = selection.GetCurrentSelection()
                && selected.Length().unwrap_or(0) > 0
                && let Ok(item) = selected.GetElement(0)
            {
                text += &format!(" -> {}", describe(&item));
            }
        }
        text
    }

    /// What `window`'s live region — the app's status line, or a settings page's — says now.
    pub fn status(&self, window: HWND) -> Option<String> {
        self.dump_elements(window).into_iter().find_map(|element| {
            integer(&element, UIA_LiveSettingPropertyId)
                .filter(|l| *l != 0)
                .and_then(|_| unsafe { element.CurrentName() }.ok().map(|n| n.to_string()))
        })
    }

    fn dump_elements(&self, window: HWND) -> Vec<IUIAutomationElement> {
        let mut found = Vec::new();
        let (Ok(root), Ok(walker)) = (unsafe { self.uia.ElementFromHandle(window) }, unsafe { self.uia.ControlViewWalker() }) else {
            return found;
        };
        let mut stack = vec![root];
        while let Some(element) = stack.pop() {
            let mut child = unsafe { walker.GetFirstChildElement(&element) }.ok();
            found.push(element);
            while let Some(current) = child {
                child = unsafe { walker.GetNextSiblingElement(&current) }.ok();
                stack.push(current);
            }
        }
        found
    }

    /// The first element in `window` with this name.
    pub fn named(&self, window: HWND, name: &str) -> Option<IUIAutomationElement> {
        self.dump_elements(window).into_iter().find(|e| unsafe { e.CurrentName() }.is_ok_and(|n| n == name))
    }

    /// The first element in `window` with this name that can take the keyboard focus: a
    /// field, not the label before it, which a dialog names the same.
    pub fn focusable(&self, window: HWND, name: &str) -> Option<IUIAutomationElement> {
        self.dump_elements(window).into_iter().find(|e| unsafe {
            e.CurrentName().is_ok_and(|n| n == name) && e.CurrentIsKeyboardFocusable().is_ok_and(|f| f.as_bool())
        })
    }

    /// Posts one step into `app` (see [`key`] for what a step can be), and waits for it to
    /// take effect.
    pub fn post(&self, app: HWND, step: &str) -> Result<(), String> {
        // An inactive window's thread has no focus for a key to go to.
        if thread_focus(app).is_invalid() {
            self.activate(app);
        }
        let focus = thread_focus(app);
        if step == "context" {
            // What Shift+F10 or the Applications key sends: no point, so -1, -1.
            post(focus, WM_CONTEXTMENU, focus.0 as usize, -1);
        } else if let Some(text) = step.strip_prefix("text:") {
            for unit in text.encode_utf16() {
                post(focus, WM_CHAR, usize::from(unit), 1);
            }
        } else if let Some(id) = step.strip_prefix("cmd:").and_then(|id| id.parse::<usize>().ok()) {
            post(app, WM_COMMAND, id, 0);
        } else if let Some(name) = step.strip_prefix("focus:") {
            // A field by name, as Alt and its letter would reach it.
            let front = self.front(app);
            let found = self.focusable(front, name).ok_or_else(|| format!("nothing focusable called {name} in the window in front"))?;
            // Through the dialog manager, as Alt and a letter or Tab move focus: UI Automation's
            // own SetFocus did not move it in a dialog. The dialog selects an edit's text as it
            // goes, as tabbing in does, so typing replaces it.
            let field = HWND(unsafe { found.CurrentNativeWindowHandle() }.map_err(|e| e.to_string())?.0);
            let dialog = unsafe { GetParent(field) }.map_err(|e| e.to_string())?;
            post(dialog, WM_NEXTDLGCTL, field.0 as usize, 1);
        } else if let Some((verb, name)) = step.split_once(':').filter(|(v, _)| *v == "select" || *v == "invoke") {
            // A tab, list item or tree item to select, or a button to press, by name, in the
            // window that has focus: what Ctrl+Tab or Alt and a letter would reach.
            let front = self.front(app);
            let found = self.named(front, name).ok_or_else(|| format!("nothing called {name} in the window in front"))?;
            unsafe {
                if verb == "select" {
                    let item: IUIAutomationSelectionItemPattern =
                        found.GetCurrentPatternAs(UIA_SelectionItemPatternId).map_err(|e| e.to_string())?;
                    item.Select().map_err(|e| e.to_string())?;
                } else {
                    let button: IUIAutomationInvokePattern =
                        found.GetCurrentPatternAs(UIA_InvokePatternId).map_err(|e| e.to_string())?;
                    button.Invoke().map_err(|e| e.to_string())?;
                }
            }
        } else {
            let vk = key(step).ok_or_else(|| format!("no key called {step}"))?;
            let scan = unsafe { MapVirtualKeyW(u32::from(vk.0), MAPVK_VK_TO_VSC) } as isize;
            post(focus, WM_KEYDOWN, usize::from(vk.0), 1 | (scan << 16));
            post(focus, WM_KEYUP, usize::from(vk.0), 1 | (scan << 16) | (1 << 30) | (1 << 31));
        }
        std::thread::sleep(SETTLE);
        Ok(())
    }

    /// Brings `window` to the front. Joining the foreground thread's input lets a background
    /// process do that without pressing anything; Alt+Tab's own call when that fails.
    pub fn activate(&self, window: HWND) {
        unsafe {
            // A minimized window is in front without anything in it having focus.
            if IsIconic(window).as_bool() {
                let _ = ShowWindow(window, SW_RESTORE);
            }
            let foreground = GetForegroundWindow();
            let theirs = GetWindowThreadProcessId(foreground, None);
            let ours = GetCurrentThreadId();
            let _ = AttachThreadInput(ours, theirs, true);
            let _ = BringWindowToTop(window);
            let _ = SetForegroundWindow(window);
            let _ = AttachThreadInput(ours, theirs, false);
            if GetForegroundWindow() != window {
                SwitchToThisWindow(window, true);
            }
        }
        std::thread::sleep(SETTLE);
    }

    /// Presses keys for real in `window`, refusing unless the window — or a dialog or menu
    /// of its own — is in front, since a key sent anywhere else is typed into whatever is.
    pub fn press(&self, window: HWND, keys: &[String]) -> Result<Vec<String>, String> {
        self.activate(window);
        let mut said = Vec::new();
        for spec in keys {
            if unsafe { GetAncestor(GetForegroundWindow(), GA_ROOTOWNER) } != window {
                return Err(format!("stopped before {spec}: the window is not in front"));
            }
            press(spec)?;
            std::thread::sleep(SETTLE);
            said.push(format!("{spec:>12}  focus: {}", self.focus(window)));
        }
        Ok(said)
    }

    /// The items of the popup menu open now, as described.
    pub fn menu_items(&self) -> Vec<String> {
        let Ok(menu) = (unsafe { FindWindowW(w!("#32768"), None) }) else { return Vec::new() };
        self.dump(menu, 2).into_iter().map(|line| line.trim_start().to_owned()).filter(|line| line.starts_with("MenuItem")).collect()
    }

    /// Whether a popup menu is open anywhere.
    pub fn menu_open(&self) -> bool {
        unsafe { FindWindowW(w!("#32768"), None) }.is_ok_and(|menu| unsafe { IsWindowVisible(menu) }.as_bool())
    }
}

/// Waits up to `timeout` for `found` to find something.
pub fn wait<T>(timeout: Duration, mut found: impl FnMut() -> Option<T>) -> Option<T> {
    let until = Instant::now() + timeout;
    loop {
        if let Some(value) = found() {
            return Some(value);
        }
        if Instant::now() > until {
            return None;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn post(hwnd: HWND, message: u32, wparam: usize, lparam: isize) {
    unsafe {
        let _ = PostMessageW(Some(hwnd), message, WPARAM(wparam), LPARAM(lparam));
    }
}

fn find_window(matches: impl Fn(HWND, &str, u32) -> bool) -> Option<HWND> {
    struct Search<'a> {
        matches: &'a dyn Fn(HWND, &str, u32) -> bool,
        found: Option<HWND>,
    }
    unsafe extern "system" fn visit(hwnd: HWND, lparam: LPARAM) -> BOOL {
        unsafe {
            let search = &mut *(lparam.0 as *mut Search);
            let mut text = [0u16; 512];
            let length = GetWindowTextW(hwnd, &mut text);
            let text = String::from_utf16_lossy(&text[..length.max(0) as usize]);
            let mut process = 0;
            GetWindowThreadProcessId(hwnd, Some(&mut process));
            if (search.matches)(hwnd, &text, process) {
                search.found = Some(hwnd);
                return false.into();
            }
            true.into()
        }
    }
    let mut search = Search { matches: &matches, found: None };
    unsafe {
        let _ = EnumWindows(Some(visit), LPARAM((&raw mut search) as isize));
    }
    search.found
}

fn thread_focus(app: HWND) -> HWND {
    let thread = unsafe { GetWindowThreadProcessId(app, None) };
    let mut info = GUITHREADINFO { cbSize: size_of::<GUITHREADINFO>() as u32, ..Default::default() };
    unsafe {
        let _ = GetGUIThreadInfo(thread, &mut info);
    }
    info.hwndFocus
}

fn walk(walker: &IUIAutomationTreeWalker, element: &IUIAutomationElement, level: usize, depth: usize, lines: &mut Vec<String>) {
    lines.push(format!("{}{}", "  ".repeat(level), describe(element)));
    if level >= depth {
        return;
    }
    let mut child = unsafe { walker.GetFirstChildElement(element) }.ok();
    while let Some(current) = child {
        walk(walker, &current, level + 1, depth, lines);
        child = unsafe { walker.GetNextSiblingElement(&current) }.ok();
    }
}

fn integer(element: &IUIAutomationElement, property: UIA_PROPERTY_ID) -> Option<i32> {
    let value: VARIANT = unsafe { element.GetCurrentPropertyValue(property).ok()? };
    let inner = unsafe { &value.Anonymous.Anonymous };
    (inner.vt.0 == 3).then(|| unsafe { inner.Anonymous.lVal })
}

/// One element as a screen reader is given it: control type, name, value, level and
/// position, states, description and access key.
pub fn describe(element: &IUIAutomationElement) -> String {
    unsafe {
        let kind = element.CurrentControlType().map(|t| control_type(t.0)).unwrap_or("?");
        let name = element.CurrentName().map(|n| n.to_string()).unwrap_or_default();
        let mut line = format!("{kind} '{name}'");
        if let Ok(value) = element.GetCurrentPatternAs::<IUIAutomationValuePattern>(UIA_ValuePatternId)
            && let Ok(value) = value.CurrentValue()
        {
            line += &format!(" value='{value}'");
        }
        let position = integer(element, UIA_PositionInSetPropertyId).unwrap_or(0);
        if position > 0 {
            let size = integer(element, UIA_SizeOfSetPropertyId).unwrap_or(0);
            let level = integer(element, UIA_LevelPropertyId).unwrap_or(0);
            line += &format!(" [{position} of {size}, level {level}]");
        }
        if let Ok(toggle) = element.GetCurrentPatternAs::<IUIAutomationTogglePattern>(UIA_TogglePatternId)
            && let Ok(state) = toggle.CurrentToggleState()
        {
            line += match state.0 {
                0 => " unchecked",
                1 => " checked",
                _ => " mixed",
            };
        }
        if let Ok(expand) = element.GetCurrentPatternAs::<IUIAutomationExpandCollapsePattern>(UIA_ExpandCollapsePatternId)
            && let Ok(state) = expand.CurrentExpandCollapseState()
        {
            line += match state.0 {
                0 => " collapsed",
                1 => " expanded",
                _ => "",
            };
        }
        if let Ok(item) = element.GetCurrentPatternAs::<IUIAutomationSelectionItemPattern>(UIA_SelectionItemPatternId)
            && item.CurrentIsSelected().is_ok_and(|s| s.as_bool())
        {
            line += " selected";
        }
        if let Some(live) = integer(element, UIA_LiveSettingPropertyId).filter(|l| *l != 0) {
            line += if live == 1 { " live=polite" } else { " live=assertive" };
        }
        if let Ok(help) = element.CurrentHelpText()
            && !help.is_empty()
        {
            line += &format!(" help='{help}'");
        }
        if let Ok(key) = element.CurrentAccessKey()
            && !key.is_empty()
        {
            line += &format!(" key={key}");
        }
        if element.CurrentHasKeyboardFocus().is_ok_and(|f| f.as_bool()) {
            line += " FOCUSED";
        }
        if element.CurrentIsOffscreen().is_ok_and(|f| f.as_bool()) {
            line += " offscreen";
        }
        line
    }
}

fn control_type(id: i32) -> &'static str {
    const NAMES: [&str; 39] = [
        "Button", "Calendar", "CheckBox", "ComboBox", "Edit", "Hyperlink", "Image", "ListItem", "List", "Menu",
        "MenuBar", "MenuItem", "ProgressBar", "RadioButton", "ScrollBar", "Slider", "Spinner", "StatusBar", "Tab",
        "TabItem", "Text", "ToolBar", "ToolTip", "Tree", "TreeItem", "Custom", "Group", "Thumb", "DataGrid",
        "DataItem", "Document", "SplitButton", "Window", "Pane", "Header", "HeaderItem", "Table", "TitleBar",
        "Separator",
    ];
    usize::try_from(id - 50000).ok().and_then(|i| NAMES.get(i)).copied().unwrap_or("?")
}

/// A key by name: letters and digits, `f1`–`f12`, `enter`, `esc`, `tab`, `space`, `up`,
/// `down`, `left`, `right`, `home`, `end`, `delete`, `apps`, `pageup`, `pagedown`. A step
/// can also be `text:words`, `cmd:<menu command>`, `context` (a context menu from the
/// keyboard), `focus:Name`, or `select:Name` and `invoke:Name`.
pub fn key(name: &str) -> Option<VIRTUAL_KEY> {
    let lower = name.to_lowercase();
    if let Some(n) = lower.strip_prefix('f').and_then(|n| n.parse::<u16>().ok()) {
        return (1..=12).contains(&n).then(|| VIRTUAL_KEY(VK_F1.0 + n - 1));
    }
    Some(match lower.as_str() {
        "enter" => VK_RETURN,
        "esc" => VK_ESCAPE,
        "tab" => VK_TAB,
        "space" => VK_SPACE,
        "up" => VK_UP,
        "down" => VK_DOWN,
        "left" => VK_LEFT,
        "right" => VK_RIGHT,
        "home" => VK_HOME,
        "end" => VK_END,
        "delete" => VK_DELETE,
        "apps" => VK_APPS,
        "pageup" => VK_PRIOR,
        "pagedown" => VK_NEXT,
        single if single.len() == 1 => VIRTUAL_KEY(u16::from(single.to_ascii_uppercase().as_bytes()[0])),
        _ => return None,
    })
}

/// Presses a key for real: `ctrl+`, `alt+` and `shift+` before a key's name, or `text:`.
fn press(spec: &str) -> Result<(), String> {
    if let Some(text) = spec.strip_prefix("text:") {
        for unit in text.encode_utf16() {
            send(&[unicode(unit, false), unicode(unit, true)]);
        }
        return Ok(());
    }
    let mut parts: Vec<&str> = spec.split('+').collect();
    let name = parts.pop().unwrap_or_default();
    let modifiers: Vec<VIRTUAL_KEY> = parts
        .iter()
        .filter_map(|m| match m.to_lowercase().as_str() {
            "ctrl" => Some(VK_CONTROL),
            "alt" => Some(VK_MENU),
            "shift" => Some(VK_SHIFT),
            _ => None,
        })
        .collect();
    let target = key(name).ok_or_else(|| format!("no key called {name}"))?;
    let mut events: Vec<INPUT> = modifiers.iter().map(|m| keyboard(*m, false)).collect();
    events.push(keyboard(target, false));
    events.push(keyboard(target, true));
    events.extend(modifiers.iter().rev().map(|m| keyboard(*m, true)));
    send(&events);
    Ok(())
}

fn keyboard(key: VIRTUAL_KEY, up: bool) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT { wVk: key, dwFlags: if up { KEYEVENTF_KEYUP } else { KEYBD_EVENT_FLAGS(0) }, ..Default::default() },
        },
    }
}

fn unicode(unit: u16, up: bool) -> INPUT {
    let flags = if up { KEYEVENTF_UNICODE | KEYEVENTF_KEYUP } else { KEYEVENTF_UNICODE };
    INPUT { r#type: INPUT_KEYBOARD, Anonymous: INPUT_0 { ki: KEYBDINPUT { wScan: unit, dwFlags: flags, ..Default::default() } } }
}

fn send(events: &[INPUT]) {
    unsafe {
        SendInput(events, size_of::<INPUT>() as i32);
    }
}
