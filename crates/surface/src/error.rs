//! What can go wrong, as a sentence a person can act on.

/// An operation that could not be done.
///
/// One variant, carrying a sentence. Every message in this crate is already written to be read
/// aloud — *"no project called 'Wrok' — did you mean 'Work'?"* — and a client's job is to say
/// it, not to compose its own from a code. A typed variant earns its place only when a client
/// would act differently on it; when one does, it is added here.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Error))]
pub enum LumennaError {
    /// The operation failed; the reason says why.
    ///
    /// Not called `message`: Kotlin's exceptions already have a `message`, and UniFFI's
    /// generated class would declare it twice. Kotlin reads the sentence as `message` anyway.
    #[error("{reason}")]
    Failed {
        /// What went wrong.
        reason: String,
    },
    /// Another process holds this device's sync endpoint, so this one cannot open it. A client
    /// that can reach that process — the CLI, over the daemon's socket — asks it instead.
    #[error("{reason}")]
    SyncElsewhere {
        /// Said as it is.
        reason: String,
    },
}

impl LumennaError {
    /// A failure with this message.
    pub fn new(message: impl Into<String>) -> Self {
        Self::Failed { reason: message.into() }
    }

    /// The sentence.
    #[must_use]
    pub fn message(&self) -> &str {
        match self {
            Self::Failed { reason } | Self::SyncElsewhere { reason } => reason,
        }
    }
}

macro_rules! from {
    ($($type:ty),*) => {
        $(
            impl From<$type> for LumennaError {
                fn from(error: $type) -> Self {
                    Self::new(error.to_string())
                }
            }
        )*
    };
}

from!(
    lumenna_store::StoreError,
    lumenna_core::edit::EditError,
    lumenna_core::recur::RecurError,
    lumenna_parse::ParseError,
    std::io::Error
);

/// Shorthand for this crate's results.
pub type Result<T> = std::result::Result<T, LumennaError>;
