//! Times and days as this browser says them.
//!
//! The core sends `HH:MM` and ISO dates; whether that is "2:30 PM" or "14:30", and what the
//! days and months are called, is the person's locale, which in a browser is `Intl`'s
//! default — the browser's language settings. Twelve- or twenty-four-hour comes with it.

use lumenna_app::speech::{Formats, Spoken};

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

/// The browser's clock, saying days as every app says them.
pub const CLOCK: Spoken<Browser> = Spoken(Browser);

impl Formats for Browser {
    fn time(&self, hour: i8, minute: i8) -> String {
        #[cfg(all(target_family = "wasm", target_os = "unknown"))]
        {
            if let Some(text) = format(&local(2000, 1, 1, hour, minute), &[("hour", "numeric"), ("minute", "2-digit")]) {
                return text;
            }
        }
        format!("{hour:02}:{minute:02}")
    }

    fn date(&self, year: i16, month: i8, day: i8, with_year: bool) -> String {
        #[cfg(all(target_family = "wasm", target_os = "unknown"))]
        {
            let mut options = vec![("weekday", "long"), ("day", "numeric"), ("month", "long")];
            if with_year {
                options.push(("year", "numeric"));
            }
            if let Some(text) = format(&local(year, month, day, 12, 0), &options) {
                return text;
            }
        }
        let Ok(date) = jiff::civil::Date::new(year, month, day) else { return String::new() };
        date.strftime(if with_year { "%A %-d %B %Y" } else { "%A %-d %B" }).to_string()
    }
}
