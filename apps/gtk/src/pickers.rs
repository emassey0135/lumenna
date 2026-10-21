//! GTK's own day and time pickers, beside the line they fill. The line stays first and the way
//! in ("next friday", "9am"), which the core reads; a day or time chosen here is put in it, ISO
//! or `HH:MM`, as if typed, for the core to read the same way. A day is GTK's calendar, for a
//! pointer alone; GTK has no time picker, so a time is two spin buttons, hours and minutes, each
//! saying its number. What they are called is the core's (`picker_words`).

use std::cell::Cell;
use std::rc::Rc;

use gtk::glib;
use gtk::prelude::*;

/// The day `text` names, if it is an ISO date.
fn iso_day(text: &str) -> Option<(i32, i32, i32)> {
    let mut parts = text.trim().splitn(3, '-');
    let (year, month, day) = (parts.next()?, parts.next()?, parts.next()?);
    if year.len() != 4 || month.len() != 2 || day.len() != 2 {
        return None;
    }
    Some((year.parse().ok()?, month.parse().ok()?, day.parse().ok()?))
}

/// The time `text` holds, if it is one in `HH:MM`.
fn clock_time(text: &str) -> Option<(f64, f64)> {
    let (hours, minutes) = text.trim().split_once(':')?;
    let (hours, minutes): (u32, u32) = (hours.parse().ok()?, minutes.parse().ok()?);
    (hours < 24 && minutes < 60 && minutes.to_string().len() <= 2).then_some((f64::from(hours), f64::from(minutes)))
}

/// GTK's calendar for `entry`, showing the day the entry names while it is a date, else `from`
/// (ISO), else today; a day chosen goes into the entry.
pub fn calendar(entry: &gtk::Entry, from: Option<&str>) -> gtk::Calendar {
    // For a pointer alone: GTK 4's calendar gives AT-SPI its month and year buttons and none of
    // its days, so arrowing through it says nothing. The line beside it is the way in for the
    // keyboard and a screen reader; the calendar is a named group out of the focus chain.
    let calendar = gtk::Calendar::builder().accessible_role(gtk::AccessibleRole::Group).can_focus(false).build();
    calendar.update_property(&[gtk::accessible::Property::Label(&lumenna_records::picker_words().day)]);
    // Whether the calendar is being moved to the entry's day, not chosen from.
    let following = Rc::new(Cell::new(false));
    let show = {
        let (calendar, following) = (calendar.clone(), Rc::clone(&following));
        move |day: (i32, i32, i32)| {
            if let Ok(date) = glib::DateTime::from_local(day.0, day.1, day.2, 0, 0, 0.0) {
                following.set(true);
                calendar.select_day(&date);
                following.set(false);
            }
        }
    };
    if let Some(day) = iso_day(&entry.text()).or_else(|| from.and_then(iso_day)) {
        show(day);
    }
    entry.connect_changed(move |entry| {
        if let Some(day) = iso_day(&entry.text()) {
            show(day);
        }
    });
    let entry = entry.downgrade();
    calendar.connect_day_selected(move |calendar| {
        if following.get() {
            return;
        }
        let date = calendar.date();
        let day = format!("{:04}-{:02}-{:02}", date.year(), date.month(), date.day_of_month());
        if let Some(entry) = entry.upgrade()
            && entry.text() != day
        {
            entry.set_text(&day);
        }
    });
    calendar
}

/// Hours and minutes for `entry`, a group named by the core's word for a time, after `of`, the
/// line's name, where several clocks are on one page; showing the time the entry holds while it
/// is one in `HH:MM`, else `from` (`HH:MM`); a time chosen goes into the entry.
pub fn clock(entry: &gtk::Entry, from: Option<&str>, of: Option<&str>) -> gtk::Box {
    let words = lumenna_records::picker_words();
    let group = gtk::Box::builder().orientation(gtk::Orientation::Horizontal).spacing(6).accessible_role(gtk::AccessibleRole::Group).build();
    let name = of.map_or_else(|| words.time.clone(), |of| format!("{of}, {}", words.time));
    group.update_property(&[gtk::accessible::Property::Label(&name)]);
    let spin = |most: f64, name: &str| {
        let spin = gtk::SpinButton::with_range(0.0, most, 1.0);
        spin.set_wrap(true);
        spin.set_numeric(true);
        spin.update_property(&[gtk::accessible::Property::Label(name)]);
        spin
    };
    let (hours, minutes) = (spin(23.0, &words.hours), spin(59.0, &words.minutes));
    // Minutes as two digits, as the line has them.
    minutes.connect_output(|spin| {
        spin.set_text(&format!("{:02}", spin.value() as u32));
        glib::Propagation::Stop
    });
    for (spin, name) in [(&hours, &words.hours), (&minutes, &words.minutes)] {
        group.append(&gtk::Label::new(Some(name)));
        group.append(spin);
    }
    let following = Rc::new(Cell::new(false));
    let show = {
        let (hours, minutes, following) = (hours.clone(), minutes.clone(), Rc::clone(&following));
        move |(h, m): (f64, f64)| {
            following.set(true);
            hours.set_value(h);
            minutes.set_value(m);
            following.set(false);
        }
    };
    if let Some(time) = clock_time(&entry.text()).or_else(|| from.and_then(clock_time)) {
        show(time);
    }
    entry.connect_changed(move |entry| {
        if let Some(time) = clock_time(&entry.text()) {
            show(time);
        }
    });
    for spin in [&hours, &minutes] {
        let (entry, hours, minutes, following) = (entry.downgrade(), hours.downgrade(), minutes.downgrade(), Rc::clone(&following));
        spin.connect_value_changed(move |_| {
            let (Some(entry), Some(hours), Some(minutes)) = (entry.upgrade(), hours.upgrade(), minutes.upgrade()) else { return };
            if following.get() {
                return;
            }
            let time = format!("{:02}:{:02}", hours.value() as u32, minutes.value() as u32);
            if entry.text() != time {
                entry.set_text(&time);
            }
        });
    }
    group
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_date_and_a_time_are_read_only_in_the_shapes_a_picker_writes() {
        assert_eq!(iso_day("2030-01-15"), Some((2030, 1, 15)));
        assert_eq!(iso_day("next friday"), None);
        assert_eq!(iso_day("2030-1-15"), None);
        assert_eq!(clock_time("09:05"), Some((9.0, 5.0)));
        assert_eq!(clock_time("9am"), None);
        assert_eq!(clock_time("24:00"), None);
    }
}
