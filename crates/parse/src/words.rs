//! Splitting input into spanned words.
//!
//! Both parsers work over words rather than characters, because both need to answer the
//! same question: **which span of the input did this consume?** Quick add has to know the date
//! phrase's span so it can be stripped from the title, and no date crate exposes that.
//!
//! Spans are byte offsets into the original input, so a caller can slice it directly and a
//! completion can say exactly what it is replacing.

/// One word, with where it came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Word {
    /// Byte offset of the first character.
    pub start: usize,
    /// Byte offset one past the last character.
    pub end: usize,
    /// The word folded to lowercase, which is what every matcher compares against.
    pub lower: String,
}

impl Word {
    /// The word as the user typed it.
    #[must_use]
    pub fn raw<'a>(&self, input: &'a str) -> &'a str {
        input.get(self.start..self.end).unwrap_or("")
    }

    /// Whether this word is exactly `text`, case-insensitively.
    #[must_use]
    pub fn is(&self, text: &str) -> bool {
        self.lower == text
    }

    /// Whether this word is any of `texts`.
    #[must_use]
    pub fn any_of(&self, texts: &[&str]) -> bool {
        texts.contains(&self.lower.as_str())
    }
}

/// Characters that stand alone as words.
///
/// These are the separators both grammars care about — `every mon, wed and fri`,
/// `#work & (p1 | overdue)`, `!@waiting` — and gluing them to a neighbour would mean every
/// matcher having to strip them, and `!@waiting` failing to parse at all.
///
/// Splitting them out is safe for quick add only because the title is **cut out of the
/// original input** rather than rejoined from words; see [`crate::quickadd`]. Rejoining
/// would turn `buy milk, eggs (not bread)` into `buy milk , eggs ( not bread )`.
const SEPARATORS: [char; 6] = [',', '(', ')', '!', '&', '|'];

/// Splits input into words.
#[must_use]
pub fn words(input: &str) -> Vec<Word> {
    let mut out = Vec::new();
    let mut current: Option<(usize, String)> = None;

    let flush = |current: &mut Option<(usize, String)>, end: usize, out: &mut Vec<Word>| {
        if let Some((start, text)) = current.take() {
            out.push(Word { start, end, lower: text.to_lowercase() });
        }
    };

    for (offset, ch) in input.char_indices() {
        if ch.is_whitespace() {
            flush(&mut current, offset, &mut out);
        } else if SEPARATORS.contains(&ch) {
            flush(&mut current, offset, &mut out);
            out.push(Word {
                start: offset,
                end: offset + ch.len_utf8(),
                lower: ch.to_string(),
            });
        } else {
            match &mut current {
                Some((_, text)) => text.push(ch),
                None => current = Some((offset, ch.to_string())),
            }
        }
    }
    flush(&mut current, input.len(), &mut out);
    out
}

/// The word containing or immediately before `cursor`, and its index.
///
/// What completion needs: the token being typed. A cursor sitting just past the end of a
/// word is still *in* that word — that is where the caret is after typing `@lap` — while a
/// cursor after a space is in no word at all.
#[must_use]
pub fn word_at(words: &[Word], cursor: usize) -> Option<(usize, &Word)> {
    words
        .iter()
        .enumerate()
        .find(|(_, word)| cursor >= word.start && cursor <= word.end)
}

/// The character a byte offset falls at, counted from one, as a person counts along a line.
///
/// Spans stay bytes, for slicing; a message says this instead, since "é" is two bytes and
/// someone arrowing through the line counts it once.
pub(crate) fn position(input: &str, byte: usize) -> usize {
    input.get(..byte).map_or(byte, |before| before.chars().count()) + 1
}

/// `p1` to `p4`, as quick add and the filter language both read a priority.
pub(crate) fn priority_of(text: &str) -> Option<lumenna_core::model::Priority> {
    use lumenna_core::model::Priority;
    match text {
        "p1" => Some(Priority::P1),
        "p2" => Some(Priority::P2),
        "p3" => Some(Priority::P3),
        "p4" => Some(Priority::P4),
        _ => None,
    }
}

/// The name after a `#` or `@` at `at` (`##` too), and how many words it took.
///
/// Quoted, `#"Home Office"`, it runs to the closing quote; one never closed runs to the end,
/// and `#""` names nothing. Unquoted, it is the longest run of words that spells a `known`
/// name, else the first word alone, so `#My Project p1` takes the project and leaves the
/// priority. Quick add and the filter language read it alike: two readers drifted, and one
/// took `#"" now p1` to the end of the line.
pub(crate) fn sigil_name(input: &str, tokens: &[Word], at: usize, known: &[String]) -> (String, usize) {
    let first = tokens[at].raw(input).trim_start_matches(['#', '@']);
    if let Some(opened) = first.strip_prefix('"') {
        if let Some(name) = opened.strip_suffix('"') {
            return (name.to_owned(), 1);
        }
        let mut name = opened.to_owned();
        for (offset, word) in tokens.iter().enumerate().skip(at + 1) {
            let raw = word.raw(input);
            name.push(' ');
            if let Some(closed) = raw.strip_suffix('"') {
                name.push_str(closed);
                return (name, offset - at + 1);
            }
            name.push_str(raw);
        }
        return (name, tokens.len() - at);
    }
    let mut best = (first.to_owned(), 1);
    let mut candidate = first.to_owned();
    for (offset, word) in tokens.iter().enumerate().skip(at + 1) {
        candidate.push(' ');
        candidate.push_str(word.raw(input));
        if known.iter().any(|name| lumenna_core::suggest::same_name(name, &candidate)) {
            best = (candidate.clone(), offset - at + 1);
        }
    }
    best
}

/// A length glued into one word: `45m`, `90min`, `45minutes`, `2h`, `1hour`, `1h30m`, `1h30`.
///
/// The one reading of a length, for a task's estimate and a block line's `for 2h` alike;
/// `None` for anything else, for nothing, and for more minutes than a `u32` holds.
#[must_use]
pub fn minutes_of(text: &str) -> Option<u32> {
    let mut total: u32 = 0;
    let mut rest = text;
    let mut after_hours = false;
    while !rest.is_empty() {
        let split = rest.find(|c: char| !c.is_ascii_digit()).unwrap_or(rest.len());
        if split == 0 {
            return None;
        }
        let value: u32 = rest[..split].parse().ok()?;
        rest = &rest[split..];
        if let Some(after) = ["hours", "hour", "hrs", "hr", "h"].iter().find_map(|unit| rest.strip_prefix(unit)) {
            total = total.checked_add(value.checked_mul(60)?)?;
            rest = after;
            after_hours = true;
        } else if rest.is_empty() && after_hours {
            // "1h30": the minutes after the hours need no unit.
            total = total.checked_add(value)?;
        } else {
            // Anything but a minute unit here means this word is not a length at all.
            let after = ["minutes", "minute", "mins", "min", "m"].iter().find_map(|unit| rest.strip_prefix(unit))?;
            total = total.checked_add(value)?;
            rest = after;
        }
    }
    (total > 0).then_some(total)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lowers(input: &str) -> Vec<String> {
        words(input).into_iter().map(|w| w.lower).collect()
    }

    #[test]
    fn words_split_on_whitespace() {
        assert_eq!(lowers("review PR tomorrow"), ["review", "pr", "tomorrow"]);
        assert_eq!(lowers("   spaced   out  "), ["spaced", "out"]);
        assert!(lowers("").is_empty());
    }

    #[test]
    fn separators_stand_alone() {
        assert_eq!(lowers("mon, wed and fri"), ["mon", ",", "wed", "and", "fri"]);
        assert_eq!(
            lowers("#work & (p1 | overdue)"),
            ["#work", "&", "(", "p1", "|", "overdue", ")"]
        );
        // Negation glued to its operand is the common way to write it.
        assert_eq!(lowers("!@waiting"), ["!", "@waiting"]);
    }

    #[test]
    fn spans_point_back_at_the_input() {
        let input = "review PR tomorrow";
        let found = words(input);
        assert_eq!(found[1].raw(input), "PR");
        assert_eq!(&input[found[2].start..found[2].end], "tomorrow");
    }

    #[test]
    fn spans_survive_multi_byte_characters() {
        let input = "café tomorrow";
        let found = words(input);
        assert_eq!(found[0].raw(input), "café");
        assert_eq!(found[1].raw(input), "tomorrow");
    }

    #[test]
    fn the_cursor_finds_the_word_being_typed() {
        let input = "add @lap";
        let found = words(input);
        // Caret at the very end, mid-word.
        assert_eq!(word_at(&found, 8).map(|(i, _)| i), Some(1));
        // Caret inside the first word.
        assert_eq!(word_at(&found, 1).map(|(i, _)| i), Some(0));
        // Caret after a trailing space is in no word.
        let spaced = words("add ");
        assert_eq!(word_at(&spaced, 4), None);
    }

    #[test]
    fn a_position_counts_characters_from_one() {
        assert_eq!(position("café #x", 6), 6);
        assert_eq!(position("#x", 0), 1);
    }

    #[test]
    fn a_length_reads_every_spelling_once_and_never_overflows() {
        for (text, minutes) in [("45m", 45), ("90min", 90), ("45minutes", 45), ("2h", 120), ("1hour", 60), ("1h30m", 90), ("1h30", 90), ("2hours15mins", 135)] {
            assert_eq!(minutes_of(text), Some(minutes), "{text}");
        }
        for text in ["", "0m", "h", "45", "45x", "99999999999h", "71582789h"] {
            assert_eq!(minutes_of(text), None, "{text}");
        }
    }

    #[test]
    fn an_empty_quoted_name_takes_only_itself() {
        let input = r#"call #"" now p1"#;
        let found = words(input);
        assert_eq!(sigil_name(input, &found, 1, &[]), (String::new(), 1));
        let input = r#"#"Home Office" p1"#;
        assert_eq!(sigil_name(input, &words(input), 0, &[]), ("Home Office".to_owned(), 2));
    }
}
