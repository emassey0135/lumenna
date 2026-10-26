//! The colours a project, label, saved filter or block can have: a fixed palette, each named,
//! each with a value for a light background and one for a dark. A colour is always said and
//! shown by its name too, so it is never the only way to tell two things apart; drawn, it is a
//! mark beside the name, and each value has at least 3:1 contrast with its background, as
//! WCAG asks of a mark that means something (white for light, and #3A3A3C, lighter than any
//! app's dark background, for dark).
//!
//! A colour is stored by its name. One a person gave before there was a palette, or a newer
//! build's, still loads and is still said; it is only not drawn.

/// One of the palette's colours.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Colour {
    /// What it is called, and stored as.
    pub name: &'static str,
    /// Its value on a light background, `#RRGGBB`.
    pub light: &'static str,
    /// Its value on a dark background, `#RRGGBB`.
    pub dark: &'static str,
}

/// Every colour, in the order offered.
pub const PALETTE: [Colour; 11] = [
    Colour { name: "red", light: "#D32F2F", dark: "#EF5350" },
    Colour { name: "orange", light: "#D84315", dark: "#FF8A65" },
    Colour { name: "yellow", light: "#9E7700", dark: "#FFD54F" },
    Colour { name: "green", light: "#2E7D32", dark: "#66BB6A" },
    Colour { name: "teal", light: "#00796B", dark: "#4DB6AC" },
    Colour { name: "blue", light: "#1565C0", dark: "#64B5F6" },
    Colour { name: "indigo", light: "#3949AB", dark: "#8C9EFF" },
    Colour { name: "purple", light: "#7B1FA2", dark: "#CE93D8" },
    Colour { name: "pink", light: "#C2185B", dark: "#F48FB1" },
    Colour { name: "brown", light: "#795548", dark: "#BCAAA4" },
    Colour { name: "grey", light: "#6B6B6B", dark: "#BDBDBD" },
];

impl Colour {
    /// The palette's colour called `name`, whatever its capitals; "gray" is grey.
    #[must_use]
    pub fn named(name: &str) -> Option<Self> {
        let name = name.trim();
        let name = if name.eq_ignore_ascii_case("gray") { "grey" } else { name };
        PALETTE.into_iter().find(|c| c.name.eq_ignore_ascii_case(name))
    }

    /// Every colour's name, for saying which there are: "red, orange, …, and grey".
    #[must_use]
    pub fn names() -> String {
        let names: Vec<&str> = PALETTE.iter().map(|c| c.name).collect();
        crate::words::listed(&names)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn luminance(hex: &str) -> f64 {
        let channel = |at: usize| {
            let c = f64::from(u8::from_str_radix(&hex[at..at + 2], 16).unwrap()) / 255.0;
            if c <= 0.040_45 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
        };
        0.2126 * channel(1) + 0.7152 * channel(3) + 0.0722 * channel(5)
    }

    fn contrast(a: &str, b: &str) -> f64 {
        let (a, b) = (luminance(a), luminance(b));
        (a.max(b) + 0.05) / (a.min(b) + 0.05)
    }

    #[test]
    fn every_colour_stands_out_from_a_light_and_a_dark_background() {
        for colour in PALETTE {
            assert!(contrast(colour.light, "#FFFFFF") >= 3.0, "{} on white", colour.name);
            assert!(contrast(colour.dark, "#3A3A3C") >= 3.0, "{} on dark grey", colour.name);
        }
    }

    #[test]
    fn a_colour_is_found_by_its_name_whatever_its_capitals() {
        assert_eq!(Colour::named("Teal").map(|c| c.name), Some("teal"));
        assert_eq!(Colour::named(" gray ").map(|c| c.name), Some("grey"));
        assert_eq!(Colour::named("chartreuse"), None);
    }

    #[test]
    fn every_colour_has_a_name_of_its_own() {
        let names: std::collections::BTreeSet<_> = PALETTE.iter().map(|c| c.name).collect();
        assert_eq!(names.len(), PALETTE.len());
    }
}
