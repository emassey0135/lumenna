//! "Did you mean...?"
//!
//! §6.1 and §6.3 both promise this exact sentence: *"unknown label 'lapto' — did you mean
//! 'laptop'?"*. It is not a nicety. A sighted user sees a squiggle under the offending
//! token; here the position, the token, **and the suggestion** have to be in the message
//! text, because that message is the only channel there is.
//!
//! Hand-rolled rather than pulled from a crate: this is thirty lines, it is called on lists
//! of a few hundred names, and the alternative is a dependency in the crate that has to
//! compile for a watch.

/// The largest edit distance still worth offering as a suggestion.
///
/// Three is too loose on short words — every three-letter label becomes a suggestion for
/// every other — so the bound also scales with length below.
const MAX_DISTANCE: usize = 3;

/// The closest candidate to `needle`, if any is close enough to be worth saying.
///
/// Comparison is case-insensitive, and an exact match after case folding always wins: a user
/// who typed `@Work` for a label named `work` made no mistake worth reporting.
#[must_use]
pub fn nearest<'a>(needle: &str, candidates: impl IntoIterator<Item = &'a str>) -> Option<&'a str> {
    let needle_folded = needle.to_lowercase();
    // A short word tolerates fewer edits, or every three-letter name suggests every other.
    let budget = match needle_folded.chars().count() {
        0..=2 => 0,
        3..=4 => 1,
        5..=7 => 2,
        _ => MAX_DISTANCE,
    };
    if budget == 0 {
        return None;
    }

    let mut best: Option<(usize, &'a str)> = None;
    for candidate in candidates {
        let folded = candidate.to_lowercase();
        if folded == needle_folded {
            return Some(candidate);
        }
        let distance = distance(&needle_folded, &folded);
        if distance <= budget && best.is_none_or(|(d, _)| distance < d) {
            best = Some((distance, candidate));
        }
    }
    best.map(|(_, candidate)| candidate)
}

/// Edit distance, counting a swap of two adjacent characters as **one** edit.
///
/// Plain Levenshtein scores a transposition as two edits — substitute each character — which
/// puts `wrok` two away from `work` and outside the budget a four-letter word gets. That is
/// the single most common typo there is, so this is the optimal string alignment variant of
/// Damerau-Levenshtein, which counts it as one.
///
/// Three rows rather than a full matrix: names are short, and this runs on every keystroke
/// that completes.
#[must_use]
pub fn distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    if a.is_empty() {
        return b.len();
    }
    if b.is_empty() {
        return a.len();
    }

    // `two_back` is the row for i-2, needed only by the transposition case.
    let mut two_back: Vec<usize> = vec![0; b.len() + 1];
    let mut previous: Vec<usize> = (0..=b.len()).collect();
    let mut current: Vec<usize> = vec![0; b.len() + 1];

    for i in 1..=a.len() {
        current[0] = i;
        for j in 1..=b.len() {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            let mut best = (previous[j - 1] + cost)
                .min(previous[j] + 1)
                .min(current[j - 1] + 1);
            if i > 1 && j > 1 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] {
                best = best.min(two_back[j - 2] + 1);
            }
            current[j] = best;
        }
        std::mem::swap(&mut two_back, &mut previous);
        std::mem::swap(&mut previous, &mut current);
    }
    previous[b.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distance_counts_single_edits() {
        assert_eq!(distance("", ""), 0);
        assert_eq!(distance("laptop", "laptop"), 0);
        assert_eq!(distance("lapto", "laptop"), 1, "a dropped character");
        assert_eq!(distance("laptopp", "laptop"), 1, "a doubled character");
        assert_eq!(distance("laptob", "laptop"), 1, "a wrong character");
        assert_eq!(distance("laptop", ""), 6);
        assert_eq!(distance("kitten", "sitting"), 3);
    }

    #[test]
    fn a_swap_of_two_letters_is_one_edit() {
        // The most common typo there is. Plain Levenshtein calls this two edits and would
        // put it outside a short word's budget.
        assert_eq!(distance("wrok", "work"), 1);
        assert_eq!(distance("laptpo", "laptop"), 1);
        assert_eq!(nearest("wrok", ["work", "personal"]), Some("work"));
    }

    #[test]
    fn the_promised_sentence_is_answerable() {
        assert_eq!(nearest("lapto", ["laptop", "errand", "waiting"]), Some("laptop"));
    }

    #[test]
    fn a_case_difference_is_not_a_typo() {
        assert_eq!(nearest("Work", ["work"]), Some("work"));
    }

    #[test]
    fn something_unrelated_gets_no_suggestion() {
        assert_eq!(nearest("groceries", ["laptop", "errand"]), None);
    }

    #[test]
    fn short_words_do_not_all_suggest_each_other() {
        // Without a length-scaled budget, every three-letter label is one edit from every
        // other and the suggestion becomes noise.
        assert_eq!(nearest("ab", ["cd", "ef"]), None);
        assert_eq!(nearest("cat", ["cot"]), Some("cot"));
        assert_eq!(nearest("cat", ["dog"]), None);
    }

    #[test]
    fn the_closest_candidate_wins() {
        assert_eq!(nearest("worh", ["work", "worthwhile"]), Some("work"));
    }
}
