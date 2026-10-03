//! Splitting input into spanned words.
//!
//! Both parsers work over words rather than characters, because both need to answer the
//! same question: **which span of the input did this consume?** §6.1 is explicit that quick
//! add has to know the date phrase's span so it can be stripped from the title, and that
//! this is the actual requirement no date crate exposes.
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
}
