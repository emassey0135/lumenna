//! Times and days as this desktop says them.
//!
//! Twelve- or twenty-four-hour is the desktop's setting, not the locale's: GNOME keeps it in
//! `org.gnome.desktop.interface clock-format`, and the top bar's clock follows it, so ours
//! does too. Without GNOME's schema it is the locale's own `%X` with the seconds left off.
//! Day and month names are the locale's, through GLib.

use gtk::{gio, glib};
use gtk::gio::prelude::*;
use lumenna_desktop::speech::Clock;

/// This desktop's way of saying times and dates.
pub struct Locale {
    interface: Option<gio::Settings>,
}

impl Locale {
    pub fn new() -> Self {
        let interface = gio::SettingsSchemaSource::default()
            .and_then(|source| source.lookup("org.gnome.desktop.interface", true))
            .filter(|schema| schema.has_key("clock-format"))
            .map(|_| gio::Settings::new("org.gnome.desktop.interface"));
        Self { interface }
    }

    fn twelve_hour(&self) -> Option<bool> {
        self.interface.as_ref().map(|settings| settings.string("clock-format") == "12h")
    }
}

impl Clock for Locale {
    fn time(&self, clock: &str) -> String {
        let Some((hours, minutes)) = clock.split_once(':') else { return clock.to_owned() };
        let (Ok(hour), Ok(minute)) = (hours.parse::<i32>(), minutes.parse::<i32>()) else {
            return clock.to_owned();
        };
        // Fifty-nine seconds, so the locale's own format's seconds can be found and cut.
        let Ok(time) = glib::DateTime::from_local(2000, 1, 1, hour, minute, 59.0) else {
            return clock.to_owned();
        };
        let format = match self.twelve_hour() {
            Some(true) => "%-l:%M %p",
            Some(false) => "%H:%M",
            None => "%X",
        };
        let Ok(text) = time.format(format) else { return clock.to_owned() };
        let mut text = text.trim().to_owned();
        if self.twelve_hour().is_none() {
            // The locale's own time has seconds, which a block never has.
            if let Some(at) = text.rfind(":59") {
                text.replace_range(at..at + 3, "");
            }
        }
        text
    }

    fn day(&self, iso: &str) -> String {
        let Ok(date) = iso.parse::<jiff::civil::Date>() else { return iso.to_owned() };
        let today = jiff::Zoned::now().date();
        if date == today {
            return "Today".to_owned();
        }
        if today.tomorrow().is_ok_and(|d| d == date) {
            return "Tomorrow".to_owned();
        }
        if today.yesterday().is_ok_and(|d| d == date) {
            return "Yesterday".to_owned();
        }
        let day = glib::DateTime::from_local(
            i32::from(date.year()),
            i32::from(date.month()),
            i32::from(date.day()),
            12,
            0,
            0.0,
        );
        // The year only when it is not this one, as a person would say it.
        let format = if date.year() == today.year() { "%A %-d %B" } else { "%A %-d %B %Y" };
        day.and_then(|day| day.format(format)).map_or_else(|_| iso.to_owned(), |text| text.to_string())
    }
}
