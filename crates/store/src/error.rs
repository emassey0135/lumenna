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
    /// type does not put `rusqlite` in the signature of every caller — including the WASM
    /// build, where §16.12 replaces SQLite with IndexedDB and there is no `rusqlite` to
    /// name.
    #[error("sqlite: {0}")]
    Sqlite(String),

    /// A document's root did not have the shape this version expects — a collection key
    /// holding a scalar where a map belongs, say.
    ///
    /// Distinct from a *record* that fails to hydrate, which is skipped and reported
    /// (§3.1) rather than raised: one unreadable task must not cost you the other four
    /// hundred.
    #[error("document {doc} is not shaped like a Lumenna document: {detail}")]
    MalformedDocument {
        /// Which document.
        doc: String,
        /// What was wrong.
        detail: String,
    },
}

/// Shorthand for this crate's results.
pub type Result<T> = std::result::Result<T, StoreError>;
