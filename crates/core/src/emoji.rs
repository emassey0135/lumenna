//! The one emoji a task, a block or a project can wear, drawn beside its name as other planners
//! draw icons. Any emoji, chosen with the platform's own picker, so no app keeps a set of icons of
//! its own; the core keeps it and checks it is one.
//!
//! It is chosen to be seen. Its name read aloud before every title is noise, so no line, no
//! braille and no announcement carries it: what is drawn carries it apart from the line, as a
//! colour's swatch is carried beside the name.

use unicode_segmentation::UnicodeSegmentation;

/// What an emoji that is not one is refused with.
pub const REFUSED: &str = "an emoji has to be one emoji alone, such as one from the emoji picker; empty for none";

/// What clears an emoji, beside an empty line.
pub const NONE: &str = "none";

/// Reads what was given for an emoji: one emoji, trimmed; `None` for an empty line or "none".
///
/// # Errors
///
/// [`REFUSED`] when it is more than one grapheme, or one that is no emoji (a letter, a digit,
/// a mark of punctuation).
pub fn read(given: &str) -> Result<Option<String>, &'static str> {
    let given = given.trim();
    if given.is_empty() || given.eq_ignore_ascii_case(NONE) {
        return Ok(None);
    }
    if is_one(given) { Ok(Some(given.to_owned())) } else { Err(REFUSED) }
}

/// Whether `text` is one emoji and nothing else: a single grapheme, which a picker gives
/// whole however many code points it joins (a family, a flag, a skin tone).
#[must_use]
pub fn is_one(text: &str) -> bool {
    let mut graphemes = text.graphemes(true);
    match (graphemes.next(), graphemes.next()) {
        (Some(one), None) => is_emoji(one),
        _ => false,
    }
}

/// What is drawn of a stored emoji: itself, while it is one. Merge or a newer build could leave
/// something else there, which is kept but never drawn in place of an emoji.
#[must_use]
pub fn shown(stored: Option<&str>) -> Option<String> {
    stored.filter(|e| is_one(e)).map(str::to_owned)
}

/// Whether one grapheme is an emoji: a pictograph, a flag's regional indicators, a keycap, or a
/// character asked to be drawn as an emoji (variation selector 16). Unicode's
/// `Extended_Pictographic`, by its blocks rather than its table, which no new emoji leaves.
fn is_emoji(grapheme: &str) -> bool {
    grapheme.chars().any(|c| {
        matches!(
            u32::from(c),
            0xFE0F // the emoji presentation selector
            | 0x20E3 // a keycap
            | 0x00A9 | 0x00AE | 0x203C | 0x2049 | 0x2122 | 0x2139
            | 0x2194..=0x21FF // arrows
            | 0x2300..=0x23FF // technical: the watch, the hourglass
            | 0x24C2
            | 0x25A0..=0x27BF // shapes, symbols, dingbats
            | 0x2900..=0x297F
            | 0x2B00..=0x2BFF
            | 0x3030 | 0x303D | 0x3297 | 0x3299
            | 0x1F000..=0x1FAFF // the pictographs, flags' indicators among them
            | 0x1FC00..=0x1FFFD // kept by Unicode for pictographs to come
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_emoji_is_kept_trimmed() {
        assert_eq!(read(" 🎯 "), Ok(Some("🎯".to_owned())));
    }

    #[test]
    fn an_emoji_of_several_code_points_is_one_emoji() {
        for emoji in ["👨‍👩‍👧", "🇬🇧", "👍🏽", "1️⃣", "❤️", "🏴󠁧󠁢󠁳󠁣󠁴󠁿", "☕"] {
            assert_eq!(read(emoji), Ok(Some(emoji.to_owned())), "{emoji:?}");
        }
    }

    #[test]
    fn nothing_and_none_clear_it() {
        assert_eq!(read(""), Ok(None));
        assert_eq!(read("  "), Ok(None));
        assert_eq!(read("None"), Ok(None));
    }

    #[test]
    fn two_emoji_letters_and_words_are_refused() {
        for given in ["🎯🎯", "a", "1", "?", "work", "🎯 work", "é"] {
            assert_eq!(read(given), Err(REFUSED), "{given:?}");
        }
    }

    #[test]
    fn a_stored_emoji_that_is_not_one_is_not_drawn() {
        assert_eq!(shown(Some("🎯")), Some("🎯".to_owned()));
        assert_eq!(shown(Some("target")), None);
        assert_eq!(shown(None), None);
    }
}
