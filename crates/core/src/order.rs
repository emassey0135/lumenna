//! Fractional indexing: the ordering scheme for tasks, projects, labels, and assignments.
//!
//! §3.13 rules out an Automerge list of children on the parent. List CRDTs handle insertion
//! within one list well, but reparenting between lists produces duplicates or losses. So
//! `order` is a single last-write-wins string, sorted between its neighbours, and moving an
//! item is one field write regardless of how far it travels or which parent it lands under.
//!
//! An [`OrderKey`] is read as a base-62 fraction with an implied leading `0.`, over the
//! alphabet `0-9A-Za-z`. That alphabet is chosen so byte order and digit order coincide,
//! which is what lets any consumer — SQLite's `ORDER BY`, a plain `sort()`, a tree widget's
//! comparator — sort correctly without knowing this module exists.
//!
//! Keys never end in the lowest digit. `V` and `V0` are the same fraction, so allowing both
//! would make two distinct keys compare equal and destroy the strictness the whole scheme
//! rests on. Every constructor here preserves that invariant.
//!
//! Two devices inserting between the same neighbours concurrently produce the *same* key.
//! That is a tie, not a conflict: §3.13 breaks it by identifier, which [`OrderKey::cmp_with`]
//! implements.

use core::cmp::Ordering;
use core::fmt;
use core::str::FromStr;

const ALPHABET: &[u8; 62] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
const BASE: usize = 62;

/// A key was asked for that cannot exist.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum OrderError {
    /// The bounds were given in the wrong order, or are equal. There is no key strictly
    /// between `x` and `x`.
    #[error("cannot order between {before} and {after}: bounds are not ascending")]
    NotAscending {
        /// The lower bound as given.
        before: OrderKey,
        /// The upper bound as given.
        after: OrderKey,
    },
    /// The text is not a valid key: empty, a character outside the alphabet, or a trailing
    /// lowest digit.
    #[error("not a valid order key: {0:?}")]
    Malformed(String),
}

/// A position in a sibling list.
///
/// Ordering is bytewise over the string, which for this alphabet is numeric order over the
/// fraction it denotes.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct OrderKey(String);

impl OrderKey {
    /// The key for the only item in a list — the midpoint of the whole range.
    #[must_use]
    pub fn middle() -> Self {
        Self::between(None, None).expect("unbounded midpoint always exists")
    }

    /// A key strictly between two neighbours.
    ///
    /// `None` for `before` means the head of the list, `None` for `after` means the tail;
    /// both `None` means the list is empty. The result is strictly greater than `before` and
    /// strictly less than `after`.
    ///
    /// # Errors
    ///
    /// [`OrderError::NotAscending`] if the bounds are equal or reversed. That is a caller
    /// bug in normal use, but it is reachable from data: §3.13 permits two concurrent
    /// inserts to mint identical keys, and a UI that then asks to insert between those two
    /// twins lands here. Resolve it by re-spacing the siblings rather than retrying.
    pub fn between(before: Option<&Self>, after: Option<&Self>) -> Result<Self, OrderError> {
        if let (Some(a), Some(b)) = (before, after)
            && a >= b
        {
            return Err(OrderError::NotAscending { before: a.clone(), after: b.clone() });
        }
        let lower = before.map_or("", |k| k.0.as_str());
        let upper = after.map(|k| k.0.as_str());
        Ok(Self(midpoint(lower, upper)))
    }

    /// A key that sorts before every existing sibling.
    #[must_use]
    pub fn before(first: &Self) -> Self {
        Self::between(None, Some(first)).expect("zero is below every valid key")
    }

    /// A key that sorts after every existing sibling.
    #[must_use]
    pub fn after(last: &Self) -> Self {
        Self::between(Some(last), None).expect("one is above every valid key")
    }

    /// The key as stored.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Total order over `(order, id)`, the comparator §3.13 specifies.
    ///
    /// Concurrent inserts between the same neighbours converge on the same key. Falling back
    /// to the identifier keeps the resulting list stable and identical on every replica,
    /// which sorting on `order` alone does not guarantee.
    pub fn cmp_with<I: Ord>(&self, self_id: &I, other: &Self, other_id: &I) -> Ordering {
        self.cmp(other).then_with(|| self_id.cmp(other_id))
    }
}

impl fmt::Display for OrderKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for OrderKey {
    type Err = OrderError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let valid = !s.is_empty()
            && s.bytes().all(|b| ALPHABET.contains(&b))
            && !s.ends_with(ALPHABET[0] as char);
        if valid { Ok(Self(s.to_owned())) } else { Err(OrderError::Malformed(s.to_owned())) }
    }
}

fn digit(byte: u8) -> usize {
    ALPHABET.iter().position(|&d| d == byte).unwrap_or(0)
}

/// Shortest key strictly between the fractions `lower` and `upper`.
///
/// `lower` empty means zero, `upper` `None` means one. Both bounds are assumed valid and
/// ascending; [`OrderKey::between`] checks that.
fn midpoint(lower: &str, upper: Option<&str>) -> String {
    if let Some(upper) = upper {
        // Descend through any shared prefix: the answer starts with it too, and what is left
        // is the same problem one digit further in.
        let shared = lower
            .bytes()
            .chain(core::iter::repeat(ALPHABET[0]))
            .zip(upper.bytes())
            .take_while(|(a, b)| a == b)
            .count();
        if shared > 0 {
            return format!("{}{}", &upper[..shared], midpoint(slice_from(lower, shared), Some(&upper[shared..])));
        }
    }

    let low_digit = lower.as_bytes().first().map_or(0, |&b| digit(b));
    let high_digit = upper.and_then(|u| u.as_bytes().first()).map_or(BASE, |&b| digit(b));

    if high_digit - low_digit > 1 {
        // Room for a digit between them: one character is enough.
        return String::from(ALPHABET[low_digit.midpoint(high_digit)] as char);
    }
    if let Some(upper) = upper
        && upper.len() > 1
    {
        // The bounds are adjacent digits, but the upper bound has more to it. Truncating it
        // to its first digit lands strictly between the two.
        return upper[..1].to_owned();
    }
    // Nothing fits at this digit: keep the lower bound's digit and go deeper, where the
    // upper bound is now unbounded.
    format!("{}{}", ALPHABET[low_digit] as char, midpoint(slice_from(lower, 1), None))
}

fn slice_from(s: &str, n: usize) -> &str {
    s.get(n..).unwrap_or("")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(s: &str) -> OrderKey {
        s.parse().unwrap()
    }

    #[test]
    fn middle_is_the_middle() {
        assert_eq!(OrderKey::middle().as_str(), "V");
    }

    #[test]
    fn neighbours_bracket_the_result() {
        let a = OrderKey::middle();
        let b = OrderKey::after(&a);
        let c = OrderKey::before(&a);
        assert!(c < a && a < b);
        let mid = OrderKey::between(Some(&a), Some(&b)).unwrap();
        assert!(a < mid && mid < b);
    }

    #[test]
    fn repeated_insertion_stays_strict() {
        // Inserting always at the same place is the worst case: keys grow one digit at a
        // time and must never collide or invert.
        let mut lo = OrderKey::middle();
        let hi = OrderKey::after(&lo);
        for _ in 0..500 {
            let mid = OrderKey::between(Some(&lo), Some(&hi)).unwrap();
            assert!(lo < mid && mid < hi, "{lo} < {mid} < {hi}");
            lo = mid;
        }
    }

    #[test]
    fn keys_never_end_in_the_lowest_digit() {
        let mut k = OrderKey::middle();
        for _ in 0..200 {
            k = OrderKey::before(&k);
            assert!(!k.as_str().ends_with('0'), "{k}");
        }
    }

    #[test]
    fn equal_or_reversed_bounds_are_rejected() {
        let a = key("V");
        let b = key("W");
        assert!(matches!(
            OrderKey::between(Some(&a), Some(&a)),
            Err(OrderError::NotAscending { .. })
        ));
        assert!(matches!(
            OrderKey::between(Some(&b), Some(&a)),
            Err(OrderError::NotAscending { .. })
        ));
    }

    #[test]
    fn malformed_keys_are_rejected() {
        assert!("".parse::<OrderKey>().is_err());
        assert!("V0".parse::<OrderKey>().is_err(), "trailing lowest digit aliases V");
        assert!("V-".parse::<OrderKey>().is_err());
        assert!("Vz".parse::<OrderKey>().is_ok());
    }

    #[test]
    fn ties_break_by_id() {
        // Two devices inserting at the same place converge on the same key (§3.13).
        let a = OrderKey::middle();
        let b = OrderKey::middle();
        assert_eq!(a, b);
        assert_eq!(a.cmp_with(&1, &b, &2), Ordering::Less);
        assert_eq!(a.cmp_with(&3, &b, &2), Ordering::Greater);
    }
}
