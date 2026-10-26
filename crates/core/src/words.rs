//! Small pieces of English the core's own sentences share, so each is joined one way.

/// Parts joined as a sentence lists them: "a", "a and b", "a, b, and c", the comma before the
/// last of three or more as the app's lists have it.
#[must_use]
pub fn listed<S: AsRef<str>>(parts: &[S]) -> String {
    match parts {
        [] => String::new(),
        [one] => one.as_ref().to_owned(),
        [first, second] => format!("{} and {}", first.as_ref(), second.as_ref()),
        [rest @ .., last] => {
            let rest: Vec<&str> = rest.iter().map(AsRef::as_ref).collect();
            format!("{}, and {}", rest.join(", "), last.as_ref())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_list_is_joined_with_and_and_a_comma_before_the_last_of_three() {
        assert_eq!(listed::<&str>(&[]), "");
        assert_eq!(listed(&["a"]), "a");
        assert_eq!(listed(&["a", "b"]), "a and b");
        assert_eq!(listed(&["a", "b", "c"]), "a, b, and c");
    }
}
