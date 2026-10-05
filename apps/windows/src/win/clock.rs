//! Times and days in the person's own Windows locale.

use windows::Win32::Foundation::SYSTEMTIME;
use windows::Win32::Globalization::{DATE_LONGDATE, GetDateFormatEx, GetTimeFormatEx, TIME_NOSECONDS};
use windows::core::PCWSTR;

use crate::speech::Clock;

/// The Windows locale's way of saying times and dates.
pub struct Locale;

impl Clock for Locale {
    fn time(&self, clock: &str) -> String {
        let Some((hours, minutes)) = clock.split_once(':') else { return clock.to_owned() };
        let (Ok(hour), Ok(minute)) = (hours.parse::<u16>(), minutes.parse::<u16>()) else {
            return clock.to_owned();
        };
        let time = SYSTEMTIME { wYear: 2000, wMonth: 1, wDay: 1, wHour: hour, wMinute: minute, ..Default::default() };
        let mut buffer = [0u16; 64];
        let written = unsafe {
            GetTimeFormatEx(PCWSTR::null(), TIME_NOSECONDS, Some(&time), PCWSTR::null(), Some(&mut buffer))
        };
        decode(&buffer, written).unwrap_or_else(|| clock.to_owned())
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
        let day = SYSTEMTIME {
            wYear: date.year() as u16,
            wMonth: date.month() as u16,
            wDay: date.day() as u16,
            ..Default::default()
        };
        let mut buffer = [0u16; 128];
        let written = unsafe {
            GetDateFormatEx(PCWSTR::null(), DATE_LONGDATE, Some(&day), PCWSTR::null(), Some(&mut buffer), PCWSTR::null())
        };
        decode(&buffer, written).unwrap_or_else(|| iso.to_owned())
    }
}

/// What a formatting call wrote, without its terminating nul.
fn decode(buffer: &[u16], written: i32) -> Option<String> {
    let length = usize::try_from(written).ok().filter(|n| *n > 0)? - 1;
    Some(String::from_utf16_lossy(&buffer[..length]))
}
