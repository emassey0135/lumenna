//! Reminders as Windows' own notifications (toasts), so Narrator, NVDA and JAWS announce
//! them as they announce any other, and Do Not Disturb holds them back as it holds any other.
//!
//! A toast needs an app identity (an AUMID). An app without a package gets one by naming it
//! under `HKCU\Software\Classes\AppUserModelId` and claiming it for the process. A toast's
//! buttons and its own activation come back here only while the app runs, which is whenever
//! it gives reminders: one left in the notification centre after it exits does nothing.

use lumenna_surface::DueReminder;
use windows::Data::Xml::Dom::XmlDocument;
use windows::Foundation::TypedEventHandler;
use windows::UI::Notifications::{ToastActivatedEventArgs, ToastNotification, ToastNotificationManager, ToastNotifier};
use windows::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID;
use windows::core::{HSTRING, IInspectable, Interface};

use super::core::Poster;
use super::system;

/// The app's identity; `LUMENNA_APP_ID` gives the UI tests one of their own, so what they
/// show and clear is never the person's.
pub fn app_id() -> String {
    std::env::var("LUMENNA_APP_ID").ok().filter(|id| !id.is_empty()).unwrap_or_else(|| "io.github.emassey0135.Lumenna".to_owned())
}

/// What a toast was answered with, posted to the main window.
pub const WM_REMINDER: u32 = windows::Win32::UI::WindowsAndMessaging::WM_APP + 6;

/// What was done with a toast.
pub enum Answered {
    /// Its body: open what it is about.
    Open(DueReminder),
    Dismiss(DueReminder),
    Snooze(DueReminder),
    /// A block's end: the rest of what it left short, each to its next block.
    Carry(DueReminder),
    /// A block's work outrunning its time: the app, at Replan from Now.
    Replan,
    /// The summary of what was missed: open the window.
    Summary,
}

/// Shows toasts under the app's identity.
pub struct Toasts {
    notifier: ToastNotifier,
    /// Kept while they may still be answered: an event handler lives as long as its toast.
    shown: std::cell::RefCell<Vec<ToastNotification>>,
}

/// How many toasts are kept answerable. Older ones have long since been dealt with or left.
const KEPT: usize = 50;

impl Toasts {
    /// Names the app for Windows and claims the name for this process. `None` where Windows
    /// has no notifications to give.
    pub fn new() -> Option<Self> {
        let id = app_id();
        let key = format!("Software\\Classes\\AppUserModelId\\{id}");
        if system::read_string(&key, "DisplayName").as_deref() != Some("Lumenna") {
            system::write_string(&key, "DisplayName", "Lumenna");
        }
        unsafe {
            let _ = SetCurrentProcessExplicitAppUserModelID(&HSTRING::from(&id));
        }
        let notifier = ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(&id)).ok()?;
        Some(Self { notifier, shown: std::cell::RefCell::new(Vec::new()) })
    }

    /// One reminder: its title, the whole message, and the core's Dismiss and Snooze, with Carry
    /// Forward first at a block's end that left work short, whose question the message ends in,
    /// or Replan where a block's work outruns its time.
    /// It stays until answered, as a reminder does.
    pub fn remind(&self, reminder: &DueReminder, window: Poster) {
        let carry = if reminder.carry.is_empty() {
            String::new()
        } else {
            format!(r#"<action content="{}" arguments="carry" activationType="foreground"/>"#, escape(&reminder.carry))
        };
        let replan = if reminder.replan.is_empty() {
            String::new()
        } else {
            format!(r#"<action content="{}" arguments="replan" activationType="foreground"/>"#, escape(&reminder.replan))
        };
        let xml = format!(
            r#"<toast scenario="reminder" launch="open"><visual><binding template="ToastGeneric"><text>{}</text><text>{}</text></binding></visual><actions>{}{}<action content="{}" arguments="dismiss" activationType="foreground"/><action content="{}" arguments="snooze" activationType="foreground"/></actions></toast>"#,
            escape(&reminder.title),
            escape(&reminder.text),
            carry,
            replan,
            escape(&reminder.dismiss),
            escape(&reminder.snooze),
        );
        let reminder = reminder.clone();
        self.show(&xml, move |arguments| {
            let answered = match arguments {
                "dismiss" => Answered::Dismiss(reminder.clone()),
                "snooze" => Answered::Snooze(reminder.clone()),
                "carry" => Answered::Carry(reminder.clone()),
                "replan" => Answered::Replan,
                _ => Answered::Open(reminder.clone()),
            };
            window.send(WM_REMINDER, answered);
        });
    }

    /// What came while nothing ran, said once.
    pub fn summary(&self, text: &str, window: Poster) {
        let xml = format!(
            r#"<toast launch="open"><visual><binding template="ToastGeneric"><text>{}</text></binding></visual></toast>"#,
            escape(text)
        );
        self.show(&xml, move |_| {
            window.send(WM_REMINDER, Answered::Summary);
        });
    }

    fn show(&self, xml: &str, answered: impl Fn(&str) + Send + 'static) {
        let shown = (|| -> windows::core::Result<ToastNotification> {
            let document = XmlDocument::new()?;
            document.LoadXml(&HSTRING::from(xml))?;
            let toast = ToastNotification::CreateToastNotification(&document)?;
            toast.Activated(&TypedEventHandler::<ToastNotification, IInspectable>::new(move |_, args| {
                let arguments = args
                    .as_ref()
                    .and_then(|args| args.cast::<ToastActivatedEventArgs>().ok())
                    .and_then(|args| args.Arguments().ok())
                    .map(|a| a.to_string())
                    .unwrap_or_default();
                answered(&arguments);
                Ok(())
            }))?;
            self.notifier.Show(&toast)?;
            Ok(toast)
        })();
        if let Ok(toast) = shown {
            let mut kept = self.shown.borrow_mut();
            kept.push(toast);
            if kept.len() > KEPT {
                kept.remove(0);
            }
        }
    }
}

/// Text as XML holds it.
fn escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}
