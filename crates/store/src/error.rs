//! What can go wrong between a document and the model.

/// A failure reading or writing an Automerge document.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    /// Automerge itself refused an operation.
    #[error("automerge: {0}")]
    Automerge(#[from] automerge::AutomergeError),

    /// SQLite refused something.
    ///
    /// Carried as text rather than the original error so that this crate's public error
    /// type does not put `rusqlite` in the signature of every caller.
    #[error("sqlite: {0}")]
    Sqlite(String),

    /// A document's root did not have the shape this version expects — a collection key
    /// holding a scalar where a map belongs, say.
    ///
    /// Distinct from a *record* that fails to hydrate, which is skipped and reported
    /// rather than raised: one unreadable task must not cost you the other four
    /// hundred.
    #[error("document {doc} is not shaped like a Lumenna document: {detail}")]
    MalformedDocument {
        /// Which document.
        doc: String,
        /// What was wrong.
        detail: String,
    },

    /// A file offered as a backup or an export is not one this version can read.
    #[error("{0}")]
    Unreadable(String),

    /// The filesystem refused something — writing a backup, say.
    #[error("{0}")]
    Io(String),

    /// Another process rebuilt the store for a real erasure while this one held the history
    /// before it, so this one's write was not made: it is to be made again over the new one.
    #[error("the store was just rebuilt, to erase what was deleted for good; try that again")]
    Rebuilt,

    /// Another process went on writing while this one rebuilt the store for a real erasure,
    /// every time it tried: nothing was rebuilt.
    #[error("the store kept changing while deleted things were being erased for good; try that again")]
    Busy,
}

impl From<std::io::Error> for StoreError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error.to_string())
    }
}

/// Shorthand for this crate's results.
pub type Result<T> = std::result::Result<T, StoreError>;
