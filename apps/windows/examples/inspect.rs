//! Prints a window's UI Automation tree as a screen reader receives it: each element's
//! control type, name, value, description, level and position, and its toggle, expansion and
//! selection state, with the focused element marked.
//!
//! The same native `IUIAutomation` API NVDA and Narrator call, so what it prints is what they
//! are given — for checking the app's accessibility without a screen reader to hand:
//!
//! ```text
//! cargo run -p lumenna-windows --example inspect -- "- Lumenna"
//! ```
//!
//! The argument is part of the window's title — every main window's ends "- Lumenna", which
//! a folder or terminal named after the checkout does not; the deepest level printed is the second
//! argument, eight by default.
//!
//! `--post` drives the window through its own queue, and says after each step what has
//! focus, then what the status line — the app's live region — last said:
//!
//! ```text
//! cargo run -p lumenna-windows --example inspect -- "- Lumenna" --post cmd:141 down space f6
//! ```
//!
//! A step is a key's name (`enter`, `f6`, `down`, `a`…), `text:words`, `cmd:<menu command>`
//! for a Ctrl shortcut (no modifier can be held in a posted key), `context` for a keyboard
//! context menu, `select:Name` or `invoke:Name` for a tab, item or button in the window in
//! front, or `dump` to print that window. The steps are `tests/automation`'s, which the UI
//! tests use too.
//!
//! `--keys` presses keys for real instead — `ctrl+`, `alt+` and `shift+` work there — after
//! bringing the window to the front, and refuses to press anything if it is not in front: a
//! key sent anywhere else would be typed into whatever is.

#[cfg(windows)]
#[path = "../tests/automation/mod.rs"]
mod automation;

#[cfg(windows)]
fn main() {
    let mut args = std::env::args().skip(1);
    let title = args.next().unwrap_or_else(|| "- Lumenna".to_owned());
    let mode = args.next();
    let rest: Vec<String> = args.collect();
    let automation = automation::Automation::new();
    let Some(window) = automation.window_titled(&title) else {
        eprintln!("no visible window has '{title}' in its title");
        std::process::exit(1);
    };
    match mode.as_deref() {
        Some("--post") => {
            for step in &rest {
                if step == "dump" {
                    for line in automation.dump(automation.front(window), 8) {
                        println!("  {line}");
                    }
                    continue;
                }
                if let Err(why) = automation.post(window, step) {
                    eprintln!("{why}");
                    continue;
                }
                println!("{step:>12}  focus: {}", automation.focus(window));
            }
            if let Some(status) = automation.status(window) {
                println!("{:>12}  status: {status}", "");
            }
        }
        Some("--keys") => match automation.press(window, &rest) {
            Ok(lines) => lines.iter().for_each(|line| println!("{line}")),
            Err(why) => {
                eprintln!("{why}");
                std::process::exit(2);
            }
        },
        depth => {
            let depth = depth.and_then(|d| d.parse().ok()).unwrap_or(8);
            for line in automation.dump(window, depth) {
                println!("{line}");
            }
            println!("\nfocus: {}", automation.focus(window));
        }
    }
}

#[cfg(not(windows))]
fn main() {
    eprintln!("UI Automation is Windows'.");
}
