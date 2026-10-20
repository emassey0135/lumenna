//! Times and days as this desktop says them.
//!
//! Twelve- or twenty-four-hour is the desktop's setting, not the locale's: GNOME keeps it in
//! `org.gnome.desktop.interface clock-format`, and the top bar's clock follows it, so ours
//! does too. Without GNOME's schema it is the locale's own `%X` with the seconds left off.
//! Day and month names are the locale's, through GLib.

use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};

use gtk::{gio, glib};
use gtk::gio::prelude::*;
use lumenna_app::speech::Clock;

/// This desktop's way of saying times and dates.
pub struct Locale {
    interface: Option<gio::Settings>,
    /// The desktop's clock setting, kept current from its settings for the app's own thread,
    /// which cannot hold them: 0 for none, 1 for twelve-hour, 2 for twenty-four (`twelve_from`).
    pub twelve: Arc<AtomicU8>,
}

impl Locale {
    pub fn new() -> Self {
        let interface = gio::SettingsSchemaSource::default()
            .and_then(|source| source.lookup("org.gnome.desktop.interface", true))
            .filter(|schema| schema.has_key("clock-format"))
            .map(|_| gio::Settings::new("org.gnome.desktop.interface"));
        let twelve = Arc::new(AtomicU8::new(0));
        if let Some(settings) = &interface {
            let held = Arc::clone(&twelve);
            let keep = move |settings: &gio::Settings| {
                held.store(if settings.string("clock-format") == "12h" { 1 } else { 2 }, Ordering::Relaxed);
            };
            keep(settings);
            settings.connect_changed(Some("clock-format"), move |settings, _| keep(settings));
        }
        Self { interface, twelve }
    }

    fn twelve_hour(&self) -> Option<bool> {
        self.interface.as_ref().map(|settings| settings.string("clock-format") == "12h")
    }

    /// The clock the core is to say times in, for what it words whole: a replan's changes.
    /// The desktop's setting, else whether the locale's own time says 13:00 as 13.
    pub fn style(&self) -> lumenna_records::proposals::TimeStyle {
        let twelve = self.twelve_hour().unwrap_or_else(|| {
            glib::DateTime::from_local(2000, 1, 1, 13, 0, 0.0)
                .and_then(|time| time.format("%X"))
                .is_ok_and(|text| !text.contains("13"))
        });
        if twelve { lumenna_records::proposals::TimeStyle::TwelveHour } else { lumenna_records::proposals::TimeStyle::TwentyFourHour }
    }
}

impl Clock for Locale {
    fn time(&self, clock: &str) -> String {
        let Some((hours, minutes)) = clock.split_once(':') else { return clock.to_owned() };
        let (Ok(hour), Ok(minute)) = (hours.parse::<i32>(), minutes.parse::<i32>()) else {
            return clock.to_owned();
        };
        time(hour, minute, self.twelve_hour())
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

/// Now, as the core is to take it, a local date and time: the clock's (`None`), unless
/// `LUMENNA_NOW` (`2026-05-06T10:00`) pins it, so a test of the rest of a day, or of a
/// reminder, runs the same at any hour.
pub fn pinned_now() -> Option<String> {
    std::env::var("LUMENNA_NOW").ok().filter(|now| !now.is_empty())
}

/// The pinned now as an instant (RFC 3339), for what takes one: reminders.
pub fn pinned_instant() -> Option<String> {
    let civil: jiff::civil::DateTime = pinned_now()?.parse().ok()?;
    Some(civil.to_zoned(jiff::tz::TimeZone::system()).ok()?.timestamp().to_string())
}

/// The clock setting as `Locale::twelve` holds it.
pub fn twelve_from(held: u8) -> Option<bool> {
    match held {
        1 => Some(true),
        2 => Some(false),
        _ => None,
    }
}

/// A time of day as this desktop writes it: twelve- or twenty-four-hour as `twelve` says, else
/// the locale's own `%X` with the seconds left off.
pub fn time(hour: i32, minute: i32, twelve: Option<bool>) -> String {
    let plain = format!("{hour:02}:{minute:02}");
    // Fifty-nine seconds, so the locale's own format's seconds can be found and cut.
    let Ok(time) = glib::DateTime::from_local(2000, 1, 1, hour, minute, 59.0) else {
        return plain;
    };
    let format = match twelve {
        Some(true) => "%-l:%M %p",
        Some(false) => "%H:%M",
        None => "%X",
    };
    let Ok(text) = time.format(format) else { return plain };
    let mut text = text.trim().to_owned();
    if twelve.is_none() {
        // The locale's own time has seconds, which a block never has.
        if let Some(at) = text.rfind(":59") {
            text.replace_range(at..at + 3, "");
        }
    }
    text
}
