//! Quick add for blocks: `block "Deep work" 9-11am weekdays`, in the task line's grammar.
//!
//! A line that starts with the word `block` makes a block, not a task: a name, quoted or the
//! words before what follows, then when it runs (`9-11am`, `9am to 11am`, `9am for 2h`), and in
//! any order a day (`tomorrow`) and a repetition (`every weekday`, `weekdays`). As in quick add,
//! nothing is swallowed: a word that is none of these is refused, named.

use jiff::civil;

use crate::date::{clock, parse_date, parse_recurrence, parse_time};
use crate::words::{Word, minutes_of, words};

/// A block line, as understood. The day and the repetition are kept as typed, for whatever
/// reads a block's day and repetition to read them as it reads the block form's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockLine {
    /// Its name, cut from the line.
    pub title: String,
    /// When it starts.
    pub start: civil::Time,
    /// How long it runs.
    pub minutes: u32,
    /// The day, as typed: `tomorrow`. Absent for today.
    pub date: Option<String>,
    /// The repetition, as typed: `every weekday`. Absent for once.
    pub repeat: Option<String>,
}

/// Whether `text` asks for a block: its first word is `block`, with more after it.
#[must_use]
pub fn is_block_line(text: &str) -> bool {
    let words = words(text);
    words.first().is_some_and(|w| w.is("block")) && words.len() > 1
}

/// Reads a block line; `None` if `text` is not one (see [`is_block_line`]), else what it says
/// or why it cannot be read.
///
/// # Errors
///
/// In the inner result: no name, no time it runs, or a word that is none of the parts.
#[must_use]
pub fn parse_block_line(text: &str) -> Option<Result<BlockLine, String>> {
    if !is_block_line(text) {
        return None;
    }
    let tokens = words(text);
    Some(read(text, &tokens))
}

fn read(text: &str, tokens: &[Word]) -> Result<BlockLine, String> {
    // The name: quoted, or the words before the first that says when.
    let (title, mut index) = match text[tokens[1].start..].strip_prefix('"') {
        Some(rest) => {
            let close = rest.find('"').ok_or("the block's name has no closing quote")?;
            let end = tokens[1].start + 1 + close;
            let next = tokens.iter().position(|w| w.start > end).unwrap_or(tokens.len());
            (text[tokens[1].start + 1..end].trim().to_owned(), next)
        }
        None => {
            // At least its first word, so "Weekly review" is a name, not a repetition; a line
            // whose first word says when has no name.
            if starts_a_part(tokens, 1) && range(tokens, 1).is_some() {
                return Err("a block needs a name, such as block Deep work 9-11am".to_owned());
            }
            let first = (2..tokens.len()).find(|&i| starts_a_part(tokens, i)).unwrap_or(tokens.len());
            let title = text[tokens[1].start..tokens[first - 1].end].trim().to_owned();
            (title, first)
        }
    };
    if title.is_empty() {
        return Err("a block needs a name, such as block Deep work 9-11am".to_owned());
    }
    let mut runs = None;
    let mut date = None;
    let mut repeat = None;
    while index < tokens.len() {
        let phrase = |from: usize, used: usize| text[tokens[from].start..tokens[from + used - 1].end].to_owned();
        if runs.is_none()
            && let Some((start, minutes, used)) = range(tokens, index)
        {
            runs = Some((start, minutes));
            index += used;
        } else if repeat.is_none() && tokens[index].any_of(&["weekdays", "weekends"]) {
            repeat = Some(if tokens[index].is("weekdays") { "every weekday" } else { "every saturday and sunday" }.to_owned());
            index += 1;
        } else if repeat.is_none()
            && let Some((_, _, used)) = parse_recurrence(tokens, index)
        {
            repeat = Some(phrase(index, used));
            index += used;
        } else if date.is_none()
            && let Some((_, used)) = parse_date(tokens, index)
        {
            date = Some(phrase(index, used));
            index += used;
        } else {
            return Err(format!("could not read '{}' in a block line", text[tokens[index].start..].trim()));
        }
    }
    let (start, minutes) = runs.ok_or("say when it runs, such as 9-11am, 9am to 11am, or 9am for 2h")?;
    Ok(BlockLine { title, start, minutes, date, repeat })
}

/// Whether a part — when it runs, a day, a repetition — starts at `at`.
fn starts_a_part(tokens: &[Word], at: usize) -> bool {
    range(tokens, at).is_some()
        || tokens[at].any_of(&["weekdays", "weekends"])
        || parse_recurrence(tokens, at).is_some()
        || parse_date(tokens, at).is_some()
}

/// When a block runs, starting at `at`: `9-11am`, `9:30-10:15`, `9am to 11am`, `9 to 11am`,
/// `9am for 2h`. The start and its length in minutes, and the words used.
fn range(tokens: &[Word], at: usize) -> Option<(civil::Time, u32, usize)> {
    let word = tokens.get(at)?;
    // Joined by a dash in one word: "9-11am".
    if let Some((from, to)) = word.lower.split_once('-') {
        let end = clock(to)?;
        let start = clock(from).or_else(|| clock(&format!("{from}{}", meridiem(to)?)))?;
        return length(start, end).map(|minutes| (start, minutes, 1));
    }
    // "9am to 11am", "9 to 11am", "9am - 11am".
    if tokens.get(at + 1).is_some_and(|w| w.any_of(&["to", "-", "until", "till"])) {
        let (end, used) = parse_time(tokens, at + 2)?;
        let start = clock(&word.lower).or_else(|| clock(&format!("{}{}", word.lower, meridiem(&tokens[at + 2].lower)?)))?;
        return length(start, end).map(|minutes| (start, minutes, 2 + used));
    }
    // "9am for 2h", "at 9am for 90m".
    let (start, used) = parse_time(tokens, at)?;
    if tokens.get(at + used).is_some_and(|w| w.is("for")) {
        let minutes = minutes_of(&tokens.get(at + used + 1)?.lower)?;
        return Some((start, minutes, used + 2));
    }
    None
}

fn meridiem(text: &str) -> Option<&'static str> {
    if text.ends_with("am") {
        Some("am")
    } else if text.ends_with("pm") {
        Some("pm")
    } else {
        None
    }
}

/// Minutes from `start` to `end` the same day; none if it does not come after.
fn length(start: civil::Time, end: civil::Time) -> Option<u32> {
    let minutes = lumenna_core::time::minutes_of_day(end) - lumenna_core::time::minutes_of_day(start);
    u32::try_from(minutes).ok().filter(|m| *m > 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(text: &str) -> BlockLine {
        parse_block_line(text).expect("a block line").expect("read")
    }

    #[test]
    fn a_block_line_reads_its_name_when_it_runs_a_day_and_a_repetition() {
        let deep = read(r#"block "Deep work" 9-11am weekdays"#);
        assert_eq!(deep.title, "Deep work");
        assert_eq!(deep.start, civil::time(9, 0, 0, 0));
        assert_eq!(deep.minutes, 120);
        assert_eq!(deep.repeat.as_deref(), Some("every weekday"));
        let lunch = read("block Lunch 12:30-13:15 tomorrow");
        assert_eq!((lunch.title.as_str(), lunch.minutes, lunch.date.as_deref()), ("Lunch", 45, Some("tomorrow")));
        let review = read("block Weekly review friday 4pm for 1h30m every friday");
        assert_eq!(review.title, "Weekly review");
        assert_eq!((review.start, review.minutes), (civil::time(16, 0, 0, 0), 90));
        assert_eq!(review.date.as_deref(), Some("friday"));
        assert_eq!(review.repeat.as_deref(), Some("every friday"));
        assert_eq!(read("block Standup 9 to 9:15am").minutes, 15);
        assert_eq!(read("block Reading 8pm for 45minutes").minutes, 45, "a length reads as a task's estimate does");
        assert_eq!(read("block Walk 7am for 1hour").minutes, 60);
    }

    #[test]
    fn a_task_line_is_no_block_and_a_block_line_that_cannot_be_read_says_what_is_wrong() {
        assert!(parse_block_line("buy milk").is_none());
        assert!(parse_block_line("block").is_none(), "the word alone is a task's title");
        let refused = |text: &str| parse_block_line(text).unwrap().unwrap_err();
        assert!(refused("block Deep work").contains("say when it runs"));
        assert!(refused("block Deep work 9-11am purple").contains("'purple'"));
        assert!(refused("block 9-11am").contains("needs a name"));
        assert!(refused("block Late 11am-9am").contains("say when it runs"), "an end before its start is no range");
    }
}
