//! The system's date and time picker (`SysDateTimePick32`), beside a field that takes a phrase:
//! the field is the way in, and the picker fills it, a day as `YYYY-MM-DD` and a time as
//! `HH:MM`, which the core reads like anything typed. Reading a phrase is the core's, so the
//! picker takes the field's value only when it is already one of those.
//!
//! A stock control, so its fields, their values and the arrows that change them are the
//! control's to report; it is named after the field it fills.

use jiff::civil::{Date, Time};
use windows::Win32::Foundation::{HWND, LPARAM, SYSTEMTIME};
use windows::Win32::Globalization::{GetLocaleInfoEx, LOCALE_SSHORTTIME};
use windows::Win32::UI::Controls::{
    DTM_GETSYSTEMTIME, DTM_SETFORMATW, DTM_SETSYSTEMTIME, DTN_DATETIMECHANGE, DTS_SHORTDATEFORMAT, DTS_TIMEFORMAT, GDT_VALID,
    NMHDR,
};
use windows::Win32::UI::WindowsAndMessaging::{FindWindowExW, WM_NOTIFY, WS_TABSTOP};
use windows::core::{HSTRING, PCWSTR, w};

use super::{a11y, controls};

/// The picker's window class, for a dialog template.
pub const CLASS: &str = "SysDateTimePick32";

/// What a picker picks.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    /// A day.
    Date,
    /// A time of day, to the minute.
    Time,
}

impl Kind {
    /// What choosing one is called, in the app's words.
    pub fn choose(self) -> String {
        // PLACEHOLDER until `lumenna_records::picker_words` is pushed.
        match self {
            Kind::Date => "Choose a Day".to_owned(),
            Kind::Time => "Choose a Time".to_owned(),
        }
    }

    /// The picker's style in a template.
    pub fn style(self) -> u32 {
        WS_TABSTOP.0
            | match self {
                Kind::Date => DTS_SHORTDATEFORMAT,
                Kind::Time => DTS_TIMEFORMAT,
            }
    }
}

/// Readies a picker made from a template: named after the field it fills, then what it does in
/// the app's words (`choose`: "Choose a Day"), so a page's three clocks are told apart; a time
/// shown as the locale's short time, without the seconds the control's own format adds.
pub fn ready(picker: HWND, kind: Kind, field: &str, choose: &str) {
    let name = format!("{field}, {choose}");
    a11y::set_name(picker, &name);
    match kind {
        // The button that opens the calendar, which the control leaves unnamed.
        Kind::Date => a11y::set_item_name(picker, CALENDAR_BUTTON, choose),
        Kind::Time => {
            let format = HSTRING::from(short_time());
            controls::send(picker, DTM_SETFORMATW, 0, format.as_ptr() as isize);
            // The arrows beside the time are a control of their own, named as the picker.
            if let Ok(arrows) = unsafe { FindWindowExW(Some(picker), None, w!("msctls_updown32"), None) } {
                a11y::set_name(arrows, &name);
            }
        }
    }
}

/// A date picker's button, as its MSAA child: the first is its date.
const CALENDAR_BUTTON: u32 = 2;

/// The locale's short time pattern: "h:mm tt", "HH:mm".
fn short_time() -> String {
    let mut buffer = [0u16; 80];
    let written = unsafe { GetLocaleInfoEx(PCWSTR::null(), LOCALE_SSHORTTIME, Some(&mut buffer)) };
    usize::try_from(written)
        .ok()
        .filter(|n| *n > 0)
        .map_or_else(|| "h:mm tt".to_owned(), |n| String::from_utf16_lossy(&buffer[..n - 1]))
}

/// Shows `text` in the picker, if it is a day or a time as the picker writes one; anything else
/// is a phrase only the core reads, and the picker keeps what it shows. Never while the picker
/// has focus, which would move what the person is changing.
pub fn show(picker: HWND, kind: Kind, text: &str) {
    if picker.is_invalid() || controls::focused() == picker {
        return;
    }
    let Some(mut wanted) = parsed(kind, text.trim()) else { return };
    let mut shown = SYSTEMTIME::default();
    let got = controls::send(picker, DTM_GETSYSTEMTIME, 0, (&raw mut shown) as isize);
    if kind == Kind::Time {
        // A time picker holds a whole moment: keep its day.
        if got == GDT_VALID.0 as isize {
            (wanted.wYear, wanted.wMonth, wanted.wDay) = (shown.wYear, shown.wMonth, shown.wDay);
        }
    }
    if got == GDT_VALID.0 as isize && written(kind, &shown) == written(kind, &wanted) {
        return;
    }
    controls::send(picker, DTM_SETSYSTEMTIME, GDT_VALID.0 as usize, (&raw const wanted) as isize);
}

/// What the picker shows, as it fills its field.
pub fn read(picker: HWND, kind: Kind) -> Option<String> {
    let mut shown = SYSTEMTIME::default();
    let got = controls::send(picker, DTM_GETSYSTEMTIME, 0, (&raw mut shown) as isize);
    (got == GDT_VALID.0 as isize).then(|| written(kind, &shown))
}

/// The control whose value the person changed, if `message` says one did.
pub fn changed(message: u32, lparam: LPARAM) -> Option<u16> {
    if message != WM_NOTIFY {
        return None;
    }
    let header = unsafe { &*(lparam.0 as *const NMHDR) };
    (header.code == DTN_DATETIMECHANGE).then_some(header.idFrom as u16)
}

fn written(kind: Kind, time: &SYSTEMTIME) -> String {
    match kind {
        Kind::Date => format!("{:04}-{:02}-{:02}", time.wYear, time.wMonth, time.wDay),
        Kind::Time => format!("{:02}:{:02}", time.wHour, time.wMinute),
    }
}

fn parsed(kind: Kind, text: &str) -> Option<SYSTEMTIME> {
    let today = jiff::Zoned::now().date();
    let (date, time) = match kind {
        Kind::Date => (text.parse::<Date>().ok()?, Time::midnight()),
        Kind::Time => (today, text.parse::<Time>().ok()?),
    };
    Some(SYSTEMTIME {
        wYear: date.year() as u16,
        wMonth: date.month() as u16,
        wDayOfWeek: date.weekday().to_sunday_zero_offset() as u16,
        wDay: date.day() as u16,
        wHour: time.hour() as u16,
        wMinute: time.minute() as u16,
        wSecond: 0,
        wMilliseconds: 0,
    })
}
