//! Failures, phrased to be read aloud.

/// Anything that stops a command.
#[derive(Debug)]
pub enum CliError {
    /// Something specific went wrong, already phrased for the user.
    Message(String),
    /// The store refused.
    Store(lumenna_store::StoreError),
    /// An edit could not be computed.
    Edit(lumenna_core::edit::EditError),
    /// A filter query could not be read.
    Parse(lumenna_parse::ParseError),
    /// A recurrence rule could not be used.
    Recurrence(lumenna_core::recur::RecurError),
    /// The filesystem refused.
    Io(std::io::Error),
}

impl std::fmt::Display for CliError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Message(text) => f.write_str(text),
            Self::Store(e) => write!(f, "{e}"),
            Self::Edit(e) => write!(f, "{e}"),
            Self::Parse(e) => write!(f, "{e}"),
            Self::Recurrence(e) => write!(f, "{e}"),
            Self::Io(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for CliError {}

macro_rules! from {
    ($variant:ident, $type:ty) => {
        impl From<$type> for CliError {
            fn from(error: $type) -> Self {
                Self::$variant(error)
            }
        }
    };
}

from!(Store, lumenna_store::StoreError);
from!(Edit, lumenna_core::edit::EditError);
from!(Parse, lumenna_parse::ParseError);
from!(Recurrence, lumenna_core::recur::RecurError);
from!(Io, std::io::Error);

/// Shorthand for this crate's results.
pub type Result<T> = std::result::Result<T, CliError>;
