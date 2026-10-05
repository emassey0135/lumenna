//! The icon in the tray, through StatusNotifierItem — GTK 4 has no status icon of its
//! own — by way of `ksni`.
//!
//! Never the only way back to the window, since Orca reaches a tray item poorly: the
//! shortcut from anywhere and starting the app again do the same. GNOME shows such items only
//! with an extension; without one there is nothing to show it, and the item waits quietly
//! for a host to appear rather than failing.

use ksni::TrayMethods;
use ksni::menu::{MenuItem, StandardItem};

use crate::core::{Event, post};

struct Tray;

impl ksni::Tray for Tray {
    fn id(&self) -> String {
        crate::ID.to_owned()
    }

    fn title(&self) -> String {
        "Lumenna".to_owned()
    }

    fn icon_name(&self) -> String {
        "x-office-calendar".to_owned()
    }

    fn category(&self) -> ksni::Category {
        ksni::Category::ApplicationStatus
    }

    /// A click on the icon shows the window.
    fn activate(&mut self, _x: i32, _y: i32) {
        post(Event::ShowWindow);
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        let item = |label: &str, event: fn() -> Event| {
            StandardItem { label: label.to_owned(), activate: Box::new(move |_: &mut Self| post(event())), ..Default::default() }
                .into()
        };
        vec![
            item("_Show Lumenna", || Event::ShowWindow),
            item("_Quick Add a Task…", || Event::QuickAdd),
            item("S_ync Now", || Event::SyncNow),
            MenuItem::Separator,
            item("_Quit", || Event::Quit),
        ]
    }
}

/// Puts the icon in the tray, for as long as the app runs.
pub fn start() {
    gtk::glib::spawn_future_local(async {
        // A tray host that is not there yet is waited for, not an error.
        if let Ok(handle) = Tray.assume_sni_available(true).spawn().await {
            std::mem::forget(handle);
        }
    });
}
