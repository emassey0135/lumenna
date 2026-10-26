//! Reminders as Windows' own notifications (toasts), so Narrator, NVDA and JAWS announce
//! them as they announce any other, and Do Not Disturb holds them back as it holds any other.
//!
//! A toast needs an app identity (an AUMID). An app without a package gets one by naming it
//! under `HKCU\Software\Classes\AppUserModelId` and claiming it for the process. A toast's
//! buttons and its own activation come back here only while the app runs, which is whenever
//! it gives reminders: one left in the notification centre after it exits does nothing.

use lumenna_app::Due;
use lumenna_records::reminders::DueReminder;
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
    std::env::var("LUMENNA_APP_ID").ok().filter(|id| !id.is_empty()).unwrap_or_else(|| "dev.emassey0135.Lumenna".to_owned())
}

/// The icon a toast shows, as an image of its own on disk: an identity names a file, not a
/// resource. The largest of the app's icon (`lumenna.ico`, a PNG), written where it is missing or
/// older than this build's, under `%LOCALAPPDATA%\Lumenna`.
fn icon_file() -> Option<std::path::PathBuf> {
    const ICO: &[u8] = include_bytes!("../../lumenna.ico");
    let word = |at: usize| u16::from_le_bytes([ICO[at], ICO[at + 1]]);
    let dword = |at: usize| u32::from_le_bytes([ICO[at], ICO[at + 1], ICO[at + 2], ICO[at + 3]]) as usize;
    // 256 a side is written as 0: one less, wrapping, orders it after 64.
    let at = (0..usize::from(word(4))).map(|index| 6 + 16 * index).max_by_key(|at| ICO[*at].wrapping_sub(1))?;
    let image = ICO.get(dword(at + 12)..dword(at + 12) + dword(at + 8))?;
    let path = std::path::PathBuf::from(std::env::var_os("LOCALAPPDATA")?).join("Lumenna").join("Lumenna.png");
    if std::fs::read(&path).ok().as_deref() != Some(image) {
        std::fs::create_dir_all(path.parent()?).ok()?;
        std::fs::write(&path, image).ok()?;
    }
    Some(path)
}

/// What a toast was answered with, posted to the main window.
pub const WM_REMINDER: u32 = windows::Win32::UI::WindowsAndMessaging::WM_APP + 6;

/// What was done with a toast.
pub enum Answered {
    /// Its body: open what it is about.
    Open(Box<DueReminder>),
    /// One of its buttons, by the core's id (`carry`, `replan`, `dismiss`, `snooze`), for the
    /// firings it is about, with what the core says once it is done.
    Button { id: String, said: String, firings: Vec<DueReminder> },
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
        // A package is named for Windows already, by its manifest, and its toasts are its own.
        if super::package::packaged() {
            let notifier = ToastNotificationManager::CreateToastNotifier().ok()?;
            return Some(Self { notifier, shown: std::cell::RefCell::new(Vec::new()) });
        }
        let id = app_id();
        let key = format!("Software\\Classes\\AppUserModelId\\{id}");
        if system::read_string(&key, "DisplayName").as_deref() != Some("Lumenna") {
            let _ = system::write_string(&key, "DisplayName", "Lumenna");
        }
        if let Some(icon) = icon_file() {
            let icon = icon.to_string_lossy();
            if system::read_string(&key, "IconUri").as_deref() != Some(&*icon) {
                let _ = system::write_string(&key, "IconUri", &icon);
            }
        }
        unsafe {
            let _ = SetCurrentProcessExplicitAppUserModelID(&HSTRING::from(&id));
        }
        let notifier = ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(&id)).ok()?;
        Some(Self { notifier, shown: std::cell::RefCell::new(Vec::new()) })
    }

    /// One reminder, or the summary of what was missed (`summary`), as the app lists it
    /// (`lumenna_app::due::arrived`): its title, the whole message, and its buttons in the core's
    /// order and words — Carry Forward or Replan where it has them, whose question the message
    /// ends in, then Dismiss and Snooze. A reminder stays until answered, as a reminder does.
    pub fn remind(&self, due: &Due, summary: bool, window: Poster) {
        let actions: String = due
            .buttons
            .iter()
            .map(|button| format!(r#"<action content="{}" arguments="{}" activationType="foreground"/>"#, escape(&button.title), escape(&button.id)))
            .collect();
        let scenario = if summary { "" } else { r#" scenario="reminder""# };
        let title = if summary { String::new() } else { format!("<text>{}</text>", escape(&due.title)) };
        let xml = format!(
            r#"<toast{scenario} launch="open"><visual><binding template="ToastGeneric">{title}<text>{}</text></binding></visual><actions>{actions}</actions></toast>"#,
            escape(&due.text),
        );
        let due = due.clone();
        self.show(&xml, move |arguments| {
            let answered = match due.buttons.iter().find(|button| button.id == arguments) {
                Some(button) => Answered::Button { id: button.id.clone(), said: button.said.clone(), firings: due.firings.clone() },
                None if summary => Answered::Summary,
                None => match due.firings.first() {
                    Some(reminder) => Answered::Open(Box::new(reminder.clone())),
                    None => Answered::Summary,
                },
            };
            window.send(WM_REMINDER, answered);
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
