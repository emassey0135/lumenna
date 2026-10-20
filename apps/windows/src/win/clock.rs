//! Times and dates in the person's own Windows locale. Saying a day — "Today", "Tomorrow", the
//! year only when it is not this one — is the app's (`speech::Spoken`), so the Crux app's lines
//! and the ones still worded here read alike.

use windows::Win32::Foundation::SYSTEMTIME;
use windows::Win32::Globalization::{GetDateFormatEx, GetLocaleInfoEx, GetTimeFormatEx, LOCALE_SLONGDATE, TIME_NOSECONDS};
use windows::core::{HSTRING, PCWSTR};

use crate::speech::{Clock, Formats as Formatting, Spoken};

/// The Windows locale's way of writing a time of day and a date.
#[derive(Clone, Copy)]
pub struct Formats;

impl Formatting for Formats {
    fn time(&self, hour: i8, minute: i8) -> String {
        let time = SYSTEMTIME { wYear: 2000, wMonth: 1, wDay: 1, wHour: hour as u16, wMinute: minute as u16, ..Default::default() };
        let mut buffer = [0u16; 64];
        let written = unsafe { GetTimeFormatEx(PCWSTR::null(), TIME_NOSECONDS, Some(&time), PCWSTR::null(), Some(&mut buffer)) };
        decode(&buffer, written).unwrap_or_else(|| format!("{hour:02}:{minute:02}"))
    }

    fn date(&self, year: i16, month: i8, day: i8, with_year: bool) -> String {
        let date = SYSTEMTIME { wYear: year as u16, wMonth: month as u16, wDay: day as u16, ..Default::default() };
        // The locale's long date, the weekday and month named, without its year unless asked.
        let pattern = long_date();
        let pattern = HSTRING::from(if with_year { pattern } else { without_year(&pattern) });
        let mut buffer = [0u16; 128];
        let written = unsafe {
            GetDateFormatEx(PCWSTR::null(), Default::default(), Some(&date), &pattern, Some(&mut buffer), PCWSTR::null())
        };
        decode(&buffer, written).unwrap_or_else(|| format!("{year}-{month:02}-{day:02}"))
    }
}

impl lumenna_ffi::Locale for Formats {
    fn time(&self, hour: i8, minute: i8) -> String {
        Formatting::time(self, hour, minute)
    }

    fn date(&self, year: i16, month: i8, day: i8, with_year: bool) -> String {
        Formatting::date(self, year, month, day, with_year)
    }
}

/// Times and days as this PC says them, for what the screens not yet drawn from the app word.
pub struct Locale;

impl Clock for Locale {
    fn time(&self, clock: &str) -> String {
        Spoken(Formats).time(clock)
    }

    fn day(&self, iso: &str) -> String {
        Spoken(Formats).day(iso)
    }
}

/// The locale's long date pattern: "dddd, MMMM d, yyyy".
fn long_date() -> String {
    let mut buffer = [0u16; 96];
    let written = unsafe { GetLocaleInfoEx(PCWSTR::null(), LOCALE_SLONGDATE, Some(&mut buffer)) };
    decode(&buffer, written).unwrap_or_else(|| "dddd, MMMM d, yyyy".to_owned())
}

/// A date pattern with its year left out, and whatever only separated the year from the rest:
/// "dddd, MMMM d, yyyy" is "dddd, MMMM d", "yyyy'年'M'月'd'日'" is "M'月'd'日'".
fn without_year(pattern: &str) -> String {
    #[derive(PartialEq)]
    enum Kind {
        Field(char),
        Quoted,
        Other,
    }
    let mut tokens: Vec<(Kind, String)> = Vec::new();
    let mut chars = pattern.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\'' {
            let mut quoted = String::from(c);
            for next in chars.by_ref() {
                quoted.push(next);
                if next == '\'' {
                    break;
                }
            }
            tokens.push((Kind::Quoted, quoted));
        } else if c.is_ascii_alphabetic() {
            let mut run = String::from(c);
            while chars.peek() == Some(&c) {
                run.push(c);
                chars.next();
            }
            tokens.push((Kind::Field(c), run));
        } else {
            match tokens.last_mut() {
                Some((Kind::Other, text)) => text.push(c),
                _ => tokens.push((Kind::Other, c.to_string())),
            }
        }
    }
    let Some(at) = tokens.iter().position(|(kind, _)| *kind == Kind::Field('y')) else { return pattern.to_owned() };
    tokens.remove(at);
    // A year's own word, as in East Asian dates, goes with it.
    if tokens.get(at).is_some_and(|(kind, _)| *kind == Kind::Quoted) {
        tokens.remove(at);
    }
    let field = |kind: &Kind| matches!(kind, Kind::Field(_));
    if !tokens[at..].iter().any(|(kind, _)| field(kind)) {
        // Last: what came before it only led up to it ("de", a comma).
        while tokens.last().is_some_and(|(kind, _)| !field(kind)) {
            tokens.pop();
        }
    } else {
        while tokens.get(at).is_some_and(|(kind, _)| *kind == Kind::Other) {
            tokens.remove(at);
        }
    }
    tokens.into_iter().map(|(_, text)| text).collect::<String>().trim().to_owned()
}

/// What a formatting call wrote, without its terminating nul.
fn decode(buffer: &[u16], written: i32) -> Option<String> {
    let length = usize::try_from(written).ok().filter(|n| *n > 0)? - 1;
    Some(String::from_utf16_lossy(&buffer[..length]))
}

#[cfg(test)]
mod tests {
    use super::without_year;

    #[test]
    fn a_long_date_without_its_year_keeps_the_weekday_month_and_day() {
        assert_eq!(without_year("dddd, MMMM d, yyyy"), "dddd, MMMM d");
        assert_eq!(without_year("dddd, d MMMM yyyy"), "dddd, d MMMM");
        assert_eq!(without_year("dddd d. MMMM yyyy"), "dddd d. MMMM");
        assert_eq!(without_year("yyyy'年'M'月'd'日'"), "M'月'd'日'");
        assert_eq!(without_year("d 'de' MMMM 'de' yyyy"), "d 'de' MMMM");
        assert_eq!(without_year("d MMMM yyyy 'г.'"), "d MMMM");
        assert_eq!(without_year("dddd, MMMM d"), "dddd, MMMM d");
    }
}
