//! The app over JSON-RPC, for a client that cannot link it: Emacs.
//!
//! The host's server opens a session at a client's first `app.*` call ([`Apps`]), over the
//! client's own store connection, and drops it with the connection. The session is an [`App`],
//! as every native shell has, each of the client's windows or buffers a screen of it: the client
//! sends what the person did (`app.send`, ShellEvent as its serde shape) and every turn comes
//! back as a `lumenna/turn` notification, the view and its lines worded in English in the
//! client's clock (`app.clock`).

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use base64::Engine;
use lumenna_host::Lumenna;
use lumenna_host::rpc::{AppSession, Notify};
use lumenna_records::{LumennaError, Syntax};
use lumenna_records::proposals::TimeStyle;
use serde_json::{Value, json};

use crate::{App, Device, Locale, Shell, ShellEvent, Turn};

/// The app for each client that asks for it. A client's app never keeps the device in sync
/// itself, as Emacs never did: the daemon or a desktop app does, and serving the profile's
/// address is the holder's, which a `lum rpc` of the client's own is not.
pub struct Apps {
    /// What this device says it runs, in pairing.
    pub platform: String,
}

impl lumenna_host::rpc::Apps for Apps {
    fn open(&self, lumenna: Arc<Lumenna>, notify: Notify) -> lumenna_host::Result<Box<dyn AppSession>> {
        let device = Device::over(lumenna);
        let clock = Arc::new(English::default());
        let app = App::new(&device, self.platform.clone(), Arc::clone(&clock) as Arc<dyn Locale>, Arc::new(Notifying(notify)))?;
        app.in_front(false);
        Ok(Box::new(Session { app, clock, _device: device }))
    }
}

/// One client's app.
struct Session {
    app: App,
    clock: Arc<English>,
    _device: Device,
}

impl AppSession for Session {
    fn call(&self, method: &str, params: &Value) -> Result<Value, String> {
        let screen = || params.get("screen").and_then(Value::as_u64).map_or(Ok(0), |n| u32::try_from(n).map_err(|e| e.to_string()));
        Ok(match method {
            "send" => {
                let event = params.get("event").cloned().ok_or("no event")?;
                let event: ShellEvent = serde_json::from_value(event).map_err(|e| format!("not an event: {e}"))?;
                self.app.send_on(screen()?, event);
                Value::Null
            }
            "close" => {
                self.app.close_screen(screen()?);
                Value::Null
            }
            "view" => {
                let screen = screen()?;
                json!({ "view": self.app.view_on(screen), "lines": self.app.lines_on(screen) })
            }
            "still" => Value::Bool(self.app.is_still()),
            "tick" => {
                self.app.tick();
                Value::Null
            }
            "in_front" => {
                self.app.in_front(params.get("in_front").and_then(Value::as_bool).unwrap_or(true));
                Value::Null
            }
            // The clock the client says times in: its words, and the sentences the core composes
            // whole.
            "clock" => {
                let twenty_four = params.get("twenty_four").and_then(Value::as_bool).unwrap_or(false);
                self.clock.twenty_four.store(twenty_four, Ordering::Relaxed);
                let style = if twenty_four { TimeStyle::TwentyFourHour } else { TimeStyle::TwelveHour };
                self.app.send(ShellEvent::Clock(style));
                Value::Null
            }
            // What could go at the cursor of a line the client has open outside quick add: a
            // filter. At keystroke rate, so straight from the store.
            "complete" => {
                let text = params.get("text").and_then(Value::as_str).unwrap_or_default().to_owned();
                let cursor = params.get("cursor").and_then(Value::as_u64).and_then(|c| u32::try_from(c).ok()).unwrap_or(0);
                let syntax: Syntax = params
                    .get("syntax")
                    .cloned()
                    .map(serde_json::from_value)
                    .transpose()
                    .map_err(|e| e.to_string())?
                    .unwrap_or(Syntax::Filter);
                let completions = self.app.complete(text, cursor, syntax).map_err(|e: LumennaError| e.message().to_owned())?;
                serde_json::to_value(completions).map_err(|e| e.to_string())?
            }
            other => return Err(format!("no app method called '{other}'")),
        })
    }
}

/// Each turn, as the client's notification.
struct Notifying(Notify);

impl Shell for Notifying {
    fn turn(&self, turn: Turn) {
        let files: Vec<Value> = turn
            .files
            .iter()
            .map(|file| json!({ "name": file.name, "kind": file.kind, "data": base64::engine::general_purpose::STANDARD.encode(&file.bytes) }))
            .collect();
        (self.0)(json!({
            "screen": turn.screen,
            "seq": turn.seq,
            "view": turn.view,
            "lines": turn.lines,
            "said": turn.said,
            "files": files,
            "notices": turn.notices,
        }));
    }
}

/// Times and dates in English, as `lum` writes them: "2:30 PM" or "14:30", "Tuesday 20
/// October".
#[derive(Default)]
struct English {
    twenty_four: AtomicBool,
}

const MONTHS: [&str; 12] =
    ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];

impl Locale for English {
    fn time(&self, hour: i8, minute: i8) -> String {
        if self.twenty_four.load(Ordering::Relaxed) {
            return format!("{hour:02}:{minute:02}");
        }
        let twelve = match hour % 12 {
            0 => 12,
            h => h,
        };
        format!("{twelve}:{minute:02} {}", if hour < 12 { "AM" } else { "PM" })
    }

    fn date(&self, year: i16, month: i8, day: i8, with_year: bool) -> String {
        let weekday = jiff::civil::Date::new(year, month, day).map(|d| format!("{:?} ", d.weekday())).unwrap_or_default();
        let name = usize::try_from(month - 1).ok().and_then(|m| MONTHS.get(m)).copied().unwrap_or_default();
        if with_year { format!("{weekday}{day} {name} {year}") } else { format!("{weekday}{day} {name}") }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn english_says_a_time_in_either_clock_and_a_date_with_its_weekday() {
        let english = English::default();
        assert_eq!(english.time(14, 30), "2:30 PM");
        assert_eq!(english.time(0, 5), "12:05 AM");
        english.twenty_four.store(true, Ordering::Relaxed);
        assert_eq!(english.time(9, 5), "09:05");
        assert_eq!(english.date(2026, 10, 20, false), "Tuesday 20 October");
        assert_eq!(english.date(2027, 1, 1, true), "Friday 1 January 2027");
    }
}
