//! The palette ([`lumenna_core::colour`]) as a person meets it: the choices a Colour pick and
//! the block form offer, each with its swatch, and a colour named in words read back.

use lumenna_core::colour::{Colour, PALETTE};

use crate::actions::Choice;
use crate::error::{LumennaError, Result};
use crate::types::Swatch;

/// What the choice that takes a colour away is called.
pub const NO_COLOUR: &str = "No colour";

/// The palette as choices, "No colour" first (its id empty), `current` saying so.
#[must_use]
pub fn colour_choices(current: Option<&str>) -> Vec<Choice> {
    let current = current.and_then(Colour::named).map(|c| c.name);
    let now = |is: bool| is.then(|| "its colour now".to_owned());
    std::iter::once(Choice { id: String::new(), title: NO_COLOUR.to_owned(), detail: now(current.is_none()), ..Choice::default() })
        .chain(PALETTE.iter().map(|colour| Choice {
            id: colour.name.to_owned(),
            title: colour.name.to_owned(),
            detail: now(current == Some(colour.name)),
            swatch: Swatch::of(Some(colour.name)),
            ..Choice::default()
        }))
        .collect()
}

/// The colour a person named, as it is stored: `None` for none ("", "none", "No colour").
///
/// # Errors
///
/// If it is no colour of the palette's, saying which there are.
pub fn colour_named(text: &str) -> Result<Option<String>> {
    let text = text.trim();
    if text.is_empty() || text.eq_ignore_ascii_case("none") || text.eq_ignore_ascii_case(NO_COLOUR) {
        return Ok(None);
    }
    Colour::named(text)
        .map(|colour| Some(colour.name.to_owned()))
        .ok_or_else(|| LumennaError::new(format!("there is no colour called {text}: the colours are {}", Colour::names())))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_colour_is_read_by_its_name_and_none_takes_it_away() {
        assert_eq!(colour_named(" Teal ").unwrap(), Some("teal".to_owned()));
        assert_eq!(colour_named("none").unwrap(), None);
        assert_eq!(colour_named("").unwrap(), None);
        let refused = colour_named("chartreuse").unwrap_err().message().to_owned();
        assert!(refused.contains("red, orange"), "{refused}");
    }

    #[test]
    fn the_choices_start_with_none_and_say_which_is_its_colour_now() {
        let choices = colour_choices(Some("teal"));
        assert_eq!(choices[0].title, NO_COLOUR);
        assert_eq!(choices[0].detail, None);
        let teal = choices.iter().find(|c| c.id == "teal").unwrap();
        assert_eq!(teal.detail.as_deref(), Some("its colour now"));
        assert_eq!(teal.swatch.as_ref().map(|s| s.light.as_str()), Some("#00796B"));
        assert_eq!(colour_choices(None)[0].detail.as_deref(), Some("its colour now"));
    }
}
