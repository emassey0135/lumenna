//! Times and days as this browser says them.
//!
//! The core sends `HH:MM` and ISO dates; whether that is "2:30 PM" or "14:30", and what the
//! days and months are called, is the person's locale, which in a browser is `Intl`'s
//! default — the browser's language settings. Twelve- or twenty-four-hour comes with it.

use lumenna_desktop::speech::Clock;

/// The browser's own way of saying times and dates.
pub struct Browser;

#[cfg(all(target_family = "wasm", target_os = "unknown"))]
fn format(date: &js_sys::Date, options: &[(&str, &str)]) -> Option<String> {
    use wasm_bindgen::JsValue;
    let settings = js_sys::Object::new();
    for (key, value) in options {
        js_sys::Reflect::set(&settings, &JsValue::from_str(key), &JsValue::from_str(value)).ok()?;
    }
    // No locales: the browser's own.
    let format = js_sys::Intl::DateTimeFormat::new(&js_sys::Array::new(), &settings).format();
    format.call1(&JsValue::NULL, date).ok()?.as_string()
}

#[cfg(all(target_family = "wasm", target_os = "unknown"))]
fn local(year: i16, month: i8, day: i8, hour: i8, minute: i8) -> js_sys::Date {
    js_sys::Date::new_with_year_month_day_hr_min(
        u32::try_from(year).unwrap_or(2000),
        i32::from(month) - 1,
        i32::from(day),
        i32::from(hour),
        i32::from(minute),
    )
}

impl Clock for Browser {
    fn time(&self, clock: &str) -> String {
        let Ok(time) = clock.parse::<jiff::civil::Time>() else { return clock.to_owned() };
        #[cfg(all(target_family = "wasm", target_os = "unknown"))]
        {
            let date = local(2000, 1, 1, time.hour(), time.minute());
            if let Some(text) = format(&date, &[("hour", "numeric"), ("minute", "2-digit")]) {
                return text;
            }
        }
        format!("{:02}:{:02}", time.hour(), time.minute())
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
        #[cfg(all(target_family = "wasm", target_os = "unknown"))]
        {
            // The year only when it is not this one, as a person would say it.
            let mut options = vec![("weekday", "long"), ("day", "numeric"), ("month", "long")];
            if date.year() != today.year() {
                options.push(("year", "numeric"));
            }
            if let Some(text) = format(&local(date.year(), date.month(), date.day(), 12, 0), &options) {
                return text;
            }
        }
        date.strftime("%A %-d %B %Y").to_string()
    }
}
