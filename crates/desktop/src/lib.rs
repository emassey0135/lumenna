//! What the desktop apps that link the surface directly — Windows and GTK — decide alike,
//! and would otherwise each keep a copy of.
//!
//! None of it is business logic: that is the surface's. This is presentation that does not
//! depend on the toolkit — how a row is worded for a tree item's one name, how the core's flat
//! depth-first rows become a tree, which places a sidebar lists, how a device's sync state is
//! said. Each toolkit's tree reports level, position, set size and expansion itself, so the
//! wording leaves those out for both (`speech`).

pub mod choices;
pub mod devices;
pub mod outline;
pub mod places;
pub mod profile;
pub mod speech;
