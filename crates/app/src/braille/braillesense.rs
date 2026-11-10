//! What is the BrailleSense's own convention, and no other device's: how its edit boxes say a
//! cell or a character by itself. A BTSpeak's echo is its own Python libraries' translation, and
//! the Orbit Speak edits contracted braille its own way, so neither uses these. The words are the
//! device's English ones, so a person hears in Lumenna what its own apps say.

/// Braille ASCII by dots, dot 1 being bit 0: the cell of each of the 64 six-dot patterns.
const BRAILLE_ASCII: &[u8; 64] = b" A1B'K2L@CIF/MSP\"E3H9O6R^DJG>NTQ,*5<-U8V.%[$+X!&;:4\\0Z7(_?W]#Y)=";

/// What the BrailleSense's edit boxes call each cell by itself, in Braille ASCII's order from the
/// space: its letters inside a word, never a whole word the cell stands for alone ("s t", not
/// "still"), and a cell with no letters by its dots ("dot six").
const CELL_NAMES: [&str; 64] = [
    "space", "the", "dot five", "number sign", "e d", "s h", "and", "apostrophe", "of", "with", "c h", "i n g",
    "dot six", "dash", "four six", "s t", "lower j", "dot two", "lower b", "lower c", "lower d", "e n", "lower f",
    "lower g", "lower h", "i n", "w h", "five six", "g h", "for", "a r", "t h", "dot four", "a", "b", "c", "d", "e",
    "f", "g", "h", "i", "j", "k", "l", "m", "n", "o", "p", "q", "r", "s", "t", "u", "v", "w", "x", "y", "z", "o w",
    "o u", "e r", "four five", "underscore",
];

/// A lower cell at the end of a word, said as the punctuation it is there.
fn word_end_punctuation(cell: u8) -> Option<&'static str> {
    Some(match cell {
        b'1' => "comma",
        b'2' => "semicolon",
        b'3' => "colon",
        b'4' => "period",
        b'6' => "exclamation",
        b'7' => "parenthesis",
        b'8' => "question",
        b'0' => "quotation mark",
        _ => return None,
    })
}

/// What the BrailleSense says for one cell of a line by itself, as its own edit boxes do when it
/// is typed, taken away or read at the caret: `word` is the cells of the word it is in (dot 1
/// being bit 0, dots 7 and 8 bits 6 and 7) and `at` where in it the cell is.
///
/// A cell is said by its name, never back-translated: its letters, or its dots where it has none.
/// After a number sign, until a cell that is not a digit, a letter a to j is its digit. In
/// contracted braille a lower cell ending its word is the punctuation it is there; uncontracted,
/// it is that punctuation wherever it is. A cell with dot 7 or 8 is said by its six dots, or by
/// those two alone.
#[must_use]
pub fn cell_echo(word: &[u8], at: usize, contracted: bool) -> String {
    let Some(&dots) = word.get(at) else { return String::new() };
    let six = dots & 0x3f;
    if six == 0 {
        return match dots & 0xc0 {
            0x40 => "dot seven",
            0x80 => "dot eight",
            0xc0 => "seven eight",
            _ => "space",
        }
        .to_owned();
    }
    let ascii = |cell: u8| BRAILLE_ASCII[usize::from(cell & 0x3f)];
    let cell = ascii(six);
    let digit = |c: u8| matches!(c, b'A'..=b'J');
    let in_number = word[..at]
        .iter()
        .rposition(|&before| ascii(before) == b'#')
        .is_some_and(|sign| word[sign + 1..at].iter().all(|&between| matches!(ascii(between), b'A'..=b'J' | b'1' | b'4')));
    if in_number && digit(cell) {
        return if cell == b'J' { "0".to_owned() } else { char::from(cell - b'A' + b'1').to_string() };
    }
    let ends_word = word[at + 1..].iter().all(|&after| after & 0x3f == 0);
    if (in_number || !contracted || ends_word)
        && let Some(mark) = word_end_punctuation(cell)
    {
        return mark.to_owned();
    }
    CELL_NAMES[usize::from(cell - b' ')].to_owned()
}

/// What the BrailleSense's edit boxes call a punctuation mark typed or read by itself, in ASCII's
/// order from the exclamation mark: in computer braille a cell is its character, and a mark is said
/// by its name.
const PUNCTUATION_NAMES: [(char, &str); 32] = [
    ('!', "exclamation"), ('"', "quotation mark"), ('#', "number"), ('$', "dollar"), ('%', "percent"),
    ('&', "ampersand"), ('\'', "apostrophe"), ('(', "left parenthesis"), (')', "right parenthesis"),
    ('*', "asterisk"), ('+', "plus"), (',', "comma"), ('-', "dash"), ('.', "period"), ('/', "slash"),
    (':', "colon"), (';', "semicolon"), ('<', "less than"), ('=', "equals"), ('>', "greater than"),
    ('?', "question"), ('@', "at"), ('[', "left bracket"), ('\\', "back slash"), (']', "right bracket"),
    ('^', "caret"), ('_', "underscore"), ('`', "grave accent"), ('{', "left brace"), ('|', "vertical bar"),
    ('}', "right brace"), ('~', "tilde"),
];

/// What the BrailleSense says for one character by itself, as in computer braille, where a cell is
/// the character it stands for: a punctuation mark by its name ("comma"), a space as "space",
/// anything else as itself.
#[must_use]
pub fn character_echo(character: &str) -> String {
    let mut chars = character.chars();
    match (chars.next(), chars.next()) {
        (Some(' '), None) => "space".to_owned(),
        (Some(one), None) => PUNCTUATION_NAMES
            .iter()
            .find(|(mark, _)| *mark == one)
            .map_or_else(|| one.to_string(), |(_, name)| (*name).to_owned()),
        _ => character.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cells(word: &[&[u8]]) -> Vec<u8> {
        word.iter().map(|dots| dots.iter().fold(0u8, |bits, dot| bits | (1 << (dot - 1)))).collect()
    }

    #[test]
    fn a_cell_is_said_by_its_letters_never_as_the_word_it_stands_for_alone() {
        let still = cells(&[&[3, 4]]);
        assert_eq!(cell_echo(&still, 0, true), "s t", "dots 3-4 alone are \"still\", but are said as their letters");
        let test = cells(&[&[6], &[2, 3, 4, 5], &[1, 5], &[3, 4]]);
        assert_eq!(cell_echo(&test, 0, true), "dot six", "the capital sign is said by its dots");
        assert_eq!(cell_echo(&test, 1, true), "t");
        assert_eq!(cell_echo(&test, 2, true), "e");
        assert_eq!(cell_echo(&test, 3, true), "s t");
        assert_eq!(cell_echo(&cells(&[&[2, 3, 4, 6]]), 0, true), "the");
        assert_eq!(cell_echo(&cells(&[&[4, 5, 6]]), 0, true), "underscore");
        assert_eq!(cell_echo(&cells(&[&[4, 6]]), 0, true), "four six");
    }

    #[test]
    fn a_lower_cell_is_punctuation_only_at_the_end_of_a_contracted_word() {
        let period = cells(&[&[1, 2, 3, 4], &[2, 5, 6]]);
        assert_eq!(cell_echo(&period, 1, true), "period");
        let inside = cells(&[&[2, 5, 6], &[1, 2, 3, 4]]);
        assert_eq!(cell_echo(&inside, 0, true), "lower d", "inside a word it is a lower cell");
        assert_eq!(cell_echo(&inside, 0, false), "period", "uncontracted it is always the punctuation");
        assert_eq!(cell_echo(&cells(&[&[2, 6]]), 0, true), "e n");
    }

    #[test]
    fn after_a_number_sign_a_to_j_are_digits_until_something_else() {
        let number = cells(&[&[3, 4, 5, 6], &[1], &[2, 4, 5], &[1, 2]]);
        assert_eq!(cell_echo(&number, 0, true), "number sign");
        assert_eq!(cell_echo(&number, 1, true), "1");
        assert_eq!(cell_echo(&number, 2, true), "0");
        let letters = cells(&[&[3, 4, 5, 6], &[1], &[1, 3, 4], &[1, 2]]);
        assert_eq!(cell_echo(&letters, 3, true), "b", "a cell that is no digit ends the number");
    }

    #[test]
    fn dots_7_and_8_alone_are_said_as_dots() {
        assert_eq!(cell_echo(&[0x40], 0, true), "dot seven");
        assert_eq!(cell_echo(&[0xc0], 0, true), "seven eight");
        assert_eq!(cell_echo(&[0x41], 0, true), "a");
        assert_eq!(cell_echo(&[], 0, true), "");
    }

    #[test]
    fn computer_braille_punctuation_is_said_by_its_name() {
        assert_eq!(character_echo(","), "comma");
        assert_eq!(character_echo("."), "period");
        assert_eq!(character_echo("\\"), "back slash");
        assert_eq!(character_echo("~"), "tilde");
        assert_eq!(character_echo(" "), "space");
        assert_eq!(character_echo("a"), "a");
        assert_eq!(character_echo("ab"), "ab", "only a character by itself is named");
    }
}
