//! What the Windows and GTK apps still take from here while they move onto the Crux app, and
//! then this crate goes.
//!
//! How every line is worded (`speech`, `lines`), the shared keyboard table (`keys`), flat rows as
//! a tree (`outline`) and device wording (`devices`) are the app's now (`lumenna_app`); where the
//! store lives (`profile`) is the host's. They are re-exported under their old names here so the
//! two apps build unchanged meanwhile. Only `choices` is still this crate's own, the picks Windows
//! and GTK put together themselves outside the app.

pub mod choices;

pub use lumenna_app::{devices, keys, lines, outline, speech};
pub use lumenna_host::profile;
