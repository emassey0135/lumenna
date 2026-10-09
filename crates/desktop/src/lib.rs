//! What the desktop apps that link the surface directly — Windows and GTK — decide alike,
//! and would otherwise each keep a copy of.
//!
//! None of it is business logic: that is the surface's. This is presentation that does not
//! depend on the toolkit — how a row is worded for a tree item's one name, how the core's flat
//! depth-first rows become a tree, how a device's sync state is said. Each toolkit's tree
//! reports level, position, set size and expansion itself, so the wording leaves those out
//! for both (`speech`). The sidebar's places are the surface's (`lumenna_surface::places`),
//! since the iPad and Android list them too.

pub mod choices;
pub mod devices;
pub mod outline;
pub mod profile;
pub mod speech;
