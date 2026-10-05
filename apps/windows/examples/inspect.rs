//! Prints a window's UI Automation tree as a screen reader receives it: each element's
//! control type, name, value, description, level and position, and its toggle, expansion and
//! selection state, with the focused element marked.
//!
//! The same native `IUIAutomation` API NVDA and Narrator call, so what it prints is what they
//! are given. For checking the app's accessibility without a screen reader to hand, as the
//! Apple apps' UI tests audit theirs:
//!
//! ```text
//! cargo run -p lumenna-windows --example inspect -- "Lumenna"
//! ```
//!
//! The argument is part of the window's title; the deepest level printed is the second
//! argument, eight by default.
//!
//! With `--keys`, it presses keys in that window instead and says, after each, what has focus
//! and what the status line — the app's live region — last said:
//!
//! ```text
//! cargo run -p lumenna-windows --example inspect -- "Lumenna" --keys ctrl+2 f6 down space
//! ```
//!
//! It brings the window to the front first and refuses to press anything unless the window,
//! or a dialog or menu of its own, is in front: a key sent anywhere else would be typed into
//! whatever is. Keys: letters and digits, `f1`–`f12`, `enter`, `esc`, `tab`, `space`, `up`,
//! `down`, `left`, `right`, `home`, `end`, `delete`, `apps`, `pageup`, `pagedown`, joined to
//! `ctrl+`, `alt+` and `shift+`; `text:words` types the words.

#[cfg(windows)]
fn main() {
    inspect::run();
}

#[cfg(not(windows))]
fn main() {
    eprintln!("UI Automation is Windows'.");
}

#[cfg(windows)]
mod inspect {
    use windows::Win32::Foundation::{HWND, LPARAM};
    use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx};
    use windows::Win32::System::Variant::VARIANT;
    use windows::Win32::UI::Accessibility::*;
    use windows::Win32::UI::WindowsAndMessaging::{EnumWindows, GetWindowTextW, IsWindowVisible};
    use windows::core::BOOL;

    pub fn run() {
        let mut args = std::env::args().skip(1);
        let title = args.next().unwrap_or_else(|| "Lumenna".to_owned());
        let next = args.next();
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            let automation: IUIAutomation =
                CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER).expect("UI Automation");
            let Some(window) = find(&title) else {
                eprintln!("no visible window has '{title}' in its title");
                std::process::exit(1);
            };
            if next.as_deref() == Some("--keys") {
                keys::press(&automation, window, &args.collect::<Vec<_>>());
                return;
            }
            if next.as_deref() == Some("--post") {
                keys::post(&automation, window, &args.collect::<Vec<_>>());
                return;
            }
            let depth: usize = next.and_then(|d| d.parse().ok()).unwrap_or(8);
            let root = automation.ElementFromHandle(window).expect("the window's element");
            let walker = automation.ControlViewWalker().expect("a tree walker");
            print(&walker, &root, 0, depth);
            if let Ok(focused) = automation.GetFocusedElement() {
                println!("\nfocus: {}", describe(&focused));
            }
        }
    }

    mod keys {
        use std::time::Duration;

        use windows::Win32::Foundation::HWND;
        use windows::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
        use windows::Win32::Foundation::{LPARAM, WPARAM};
        use windows::Win32::UI::Accessibility::{
            IUIAutomation, IUIAutomationElement, IUIAutomationSelectionPattern, UIA_LiveSettingPropertyId,
            UIA_SelectionPatternId,
        };
        use windows::Win32::UI::Input::KeyboardAndMouse::*;
        use windows::Win32::UI::WindowsAndMessaging::{
            BringWindowToTop, GA_ROOTOWNER, GUITHREADINFO, GetAncestor, GetForegroundWindow, GetGUIThreadInfo,
            GetWindowThreadProcessId, PostMessageW, WM_CHAR, WM_COMMAND, WM_CONTEXTMENU, WM_KEYDOWN, WM_KEYUP,
            SetForegroundWindow, SwitchToThisWindow,
        };

        use super::{describe, integer};

        pub fn press(automation: &IUIAutomation, window: HWND, keys: &[String]) {
            unsafe {
                // Joining the foreground thread's input is what lets a background process
                // bring a window forward without pressing anything to do it.
                let foreground = GetForegroundWindow();
                let theirs = GetWindowThreadProcessId(foreground, None);
                let ours = GetCurrentThreadId();
                let _ = AttachThreadInput(ours, theirs, true);
                let _ = BringWindowToTop(window);
                let _ = SetForegroundWindow(window);
                // As Alt+Tab does, when the foreground lock refuses the above.
                if GetForegroundWindow() != window {
                    SwitchToThisWindow(window, true);
                }
                let _ = AttachThreadInput(ours, theirs, false);
            }
            std::thread::sleep(Duration::from_millis(400));
            for key in keys {
                let front = unsafe { GetAncestor(GetForegroundWindow(), GA_ROOTOWNER) };
                if front != window {
                    eprintln!("stopped before {key}: the window is not in front");
                    std::process::exit(2);
                }
                send(key);
                std::thread::sleep(Duration::from_millis(500));
                let focus = unsafe { automation.GetFocusedElement() }.map(|e| describe(&e)).unwrap_or_default();
                println!("{key:>12}  focus: {focus}");
            }
            if let Some(status) = status(automation, window) {
                println!("{:>12}  status: {status}", "");
            }
        }

        /// Posts keys into the window's own queue rather than pressing them, for when it cannot
        /// come to the front. They pass through the app's accelerators and dialog manager as
        /// pressed keys do, but no modifier is held, so a Ctrl shortcut is given as the menu
        /// command it stands for: `cmd:141`.
        pub fn post(automation: &IUIAutomation, window: HWND, keys: &[String]) {
            let thread = unsafe { GetWindowThreadProcessId(window, None) };
            for key in keys {
                let focus = thread_focus(thread);
                if key == "context" {
                    // What Shift+F10 or the Applications key sends: no point, so -1, -1.
                    unsafe {
                        let _ = PostMessageW(Some(focus), WM_CONTEXTMENU, WPARAM(focus.0 as usize), LPARAM(-1));
                    }
                } else if let Some(text) = key.strip_prefix("text:") {
                    for unit in text.encode_utf16() {
                        unsafe {
                            let _ = PostMessageW(Some(focus), WM_CHAR, WPARAM(usize::from(unit)), LPARAM(1));
                        }
                    }
                } else if let Some(id) = key.strip_prefix("cmd:").and_then(|id| id.parse::<usize>().ok()) {
                    unsafe {
                        let _ = PostMessageW(Some(window), WM_COMMAND, WPARAM(id), LPARAM(0));
                    }
                } else if let Some(vk) = virtual_key(key) {
                    let scan = unsafe { MapVirtualKeyW(u32::from(vk.0), MAPVK_VK_TO_VSC) } as isize;
                    unsafe {
                        let _ = PostMessageW(Some(focus), WM_KEYDOWN, WPARAM(usize::from(vk.0)), LPARAM(1 | (scan << 16)));
                        let _ = PostMessageW(
                            Some(focus),
                            WM_KEYUP,
                            WPARAM(usize::from(vk.0)),
                            LPARAM(1 | (scan << 16) | (1 << 30) | (1 << 31)),
                        );
                    }
                } else {
                    eprintln!("no key called {key}");
                    continue;
                }
                std::thread::sleep(Duration::from_millis(600));
                println!("{key:>12}  focus: {}", where_focus_is(automation, thread_focus(thread)));
            }
            if let Some(status) = status(automation, window) {
                println!("{:>12}  status: {status}", "");
            }
        }

        fn thread_focus(thread: u32) -> HWND {
            let mut info = GUITHREADINFO { cbSize: size_of::<GUITHREADINFO>() as u32, ..Default::default() };
            unsafe {
                let _ = GetGUIThreadInfo(thread, &mut info);
            }
            info.hwndFocus
        }

        /// The focused control, and in a tree, the selected item — which is what a screen reader
        /// reads as focus there.
        fn where_focus_is(automation: &IUIAutomation, focus: HWND) -> String {
            unsafe {
                let Ok(element) = automation.ElementFromHandle(focus) else { return "nothing".to_owned() };
                let mut text = describe(&element);
                if let Ok(selection) = element.GetCurrentPatternAs::<IUIAutomationSelectionPattern>(UIA_SelectionPatternId)
                    && let Ok(selected) = selection.GetCurrentSelection()
                        && selected.Length().unwrap_or(0) > 0
                            && let Ok(item) = selected.GetElement(0) {
                                text += &format!("\n{:>14}-> {}", "", describe(&item));
                            }
                text
            }
        }

        fn status(automation: &IUIAutomation, window: HWND) -> Option<String> {
            unsafe {
                let root = automation.ElementFromHandle(window).ok()?;
                let walker = automation.ControlViewWalker().ok()?;
                let mut child: Option<IUIAutomationElement> = walker.GetFirstChildElement(&root).ok();
                while let Some(current) = child {
                    if integer(&current, UIA_LiveSettingPropertyId).is_some_and(|l| l != 0) {
                        return current.CurrentName().ok().map(|n| n.to_string());
                    }
                    child = walker.GetNextSiblingElement(&current).ok();
                }
                None
            }
        }

        fn send(spec: &str) {
            if let Some(text) = spec.strip_prefix("text:") {
                for unit in text.encode_utf16() {
                    input(&[unicode(unit, false), unicode(unit, true)]);
                }
                return;
            }
            let mut parts: Vec<&str> = spec.split('+').collect();
            let Some(name) = parts.pop() else { return };
            let modifiers: Vec<VIRTUAL_KEY> = parts
                .iter()
                .filter_map(|m| match m.to_lowercase().as_str() {
                    "ctrl" => Some(VK_CONTROL),
                    "alt" => Some(VK_MENU),
                    "shift" => Some(VK_SHIFT),
                    _ => None,
                })
                .collect();
            let Some(key) = virtual_key(name) else {
                eprintln!("no key called {name}");
                return;
            };
            let mut events: Vec<INPUT> = modifiers.iter().map(|m| keyboard(*m, false)).collect();
            events.push(keyboard(key, false));
            events.push(keyboard(key, true));
            events.extend(modifiers.iter().rev().map(|m| keyboard(*m, true)));
            input(&events);
        }

        fn virtual_key(name: &str) -> Option<VIRTUAL_KEY> {
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

        fn input(events: &[INPUT]) {
            unsafe {
                SendInput(events, size_of::<INPUT>() as i32);
            }
        }
    }

    fn find(title: &str) -> Option<HWND> {
        struct Search<'a> {
            title: &'a str,
            found: Option<HWND>,
        }
        unsafe extern "system" fn visit(hwnd: HWND, lparam: LPARAM) -> BOOL {
            unsafe {
                let search = &mut *(lparam.0 as *mut Search);
                let mut text = [0u16; 512];
                let length = GetWindowTextW(hwnd, &mut text);
                let text = String::from_utf16_lossy(&text[..length.max(0) as usize]);
                if IsWindowVisible(hwnd).as_bool() && text.contains(search.title) {
                    search.found = Some(hwnd);
                    return false.into();
                }
                true.into()
            }
        }
        let mut search = Search { title, found: None };
        unsafe {
            let _ = EnumWindows(Some(visit), LPARAM((&raw mut search) as isize));
        }
        search.found
    }

    unsafe fn print(walker: &IUIAutomationTreeWalker, element: &IUIAutomationElement, level: usize, depth: usize) {
        println!("{}{}", "  ".repeat(level), describe(element));
        if level >= depth {
            return;
        }
        unsafe {
            let mut child = walker.GetFirstChildElement(element).ok();
            while let Some(current) = child {
                print(walker, &current, level + 1, depth);
                child = walker.GetNextSiblingElement(&current).ok();
            }
        }
    }

    fn integer(element: &IUIAutomationElement, property: UIA_PROPERTY_ID) -> Option<i32> {
        let value: VARIANT = unsafe { element.GetCurrentPropertyValue(property).ok()? };
        let inner = unsafe { &value.Anonymous.Anonymous };
        (inner.vt.0 == 3).then(|| unsafe { inner.Anonymous.lVal })
    }

    fn describe(element: &IUIAutomationElement) -> String {
        unsafe {
            let kind = element.CurrentControlType().map(|t| control_type(t.0)).unwrap_or("?");
            let name = element.CurrentName().map(|n| n.to_string()).unwrap_or_default();
            let mut line = format!("{kind} '{name}'");
            if let Ok(value) = element.GetCurrentPatternAs::<IUIAutomationValuePattern>(UIA_ValuePatternId)
                && let Ok(value) = value.CurrentValue() {
                    line += &format!(" value='{value}'");
                }
            let position = integer(element, UIA_PositionInSetPropertyId).unwrap_or(0);
            if position > 0 {
                let size = integer(element, UIA_SizeOfSetPropertyId).unwrap_or(0);
                let level = integer(element, UIA_LevelPropertyId).unwrap_or(0);
                line += &format!(" [{position} of {size}, level {level}]");
            }
            if let Ok(toggle) = element.GetCurrentPatternAs::<IUIAutomationTogglePattern>(UIA_TogglePatternId)
                && let Ok(state) = toggle.CurrentToggleState() {
                    line += match state.0 {
                        0 => " unchecked",
                        1 => " checked",
                        _ => " mixed",
                    };
                }
            if let Ok(expand) = element.GetCurrentPatternAs::<IUIAutomationExpandCollapsePattern>(UIA_ExpandCollapsePatternId)
                && let Ok(state) = expand.CurrentExpandCollapseState() {
                    line += match state.0 {
                        0 => " collapsed",
                        1 => " expanded",
                        _ => "",
                    };
                }
            if let Ok(item) = element.GetCurrentPatternAs::<IUIAutomationSelectionItemPattern>(UIA_SelectionItemPatternId)
                && item.CurrentIsSelected().is_ok_and(|s| s.as_bool()) {
                    line += " selected";
                }
            if let Some(live) = integer(element, UIA_LiveSettingPropertyId).filter(|l| *l != 0) {
                line += if live == 1 { " live=polite" } else { " live=assertive" };
            }
            if let Ok(help) = element.CurrentHelpText()
                && !help.is_empty() {
                    line += &format!(" help='{help}'");
                }
            if let Ok(key) = element.CurrentAccessKey()
                && !key.is_empty() {
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
            "Button", "Calendar", "CheckBox", "ComboBox", "Edit", "Hyperlink", "Image", "ListItem", "List",
            "Menu", "MenuBar", "MenuItem", "ProgressBar", "RadioButton", "ScrollBar", "Slider", "Spinner",
            "StatusBar", "Tab", "TabItem", "Text", "ToolBar", "ToolTip", "Tree", "TreeItem", "Custom", "Group",
            "Thumb", "DataGrid", "DataItem", "Document", "SplitButton", "Window", "Pane", "Header",
            "HeaderItem", "Table", "TitleBar", "Separator",
        ];
        usize::try_from(id - 50000).ok().and_then(|i| NAMES.get(i)).copied().unwrap_or("?")
    }
}
