//! What can go wrong while syncing or pairing, phrased to be read aloud.

/// A sync or pairing failure.
#[derive(Debug, thiserror::Error)]
pub enum SyncError {
    /// The store refused something.
    #[error("{0}")]
    Store(#[from] lumenna_store::StoreError),

    /// The network failed: the peer could not be reached, or the connection dropped.
    #[error("{0}")]
    Network(String),

    /// The peer said something this version does not understand.
    #[error("the other device sent something unexpected: {0}")]
    Protocol(String),

    /// The peer is not one of this person's devices.
    #[error("{0}")]
    Refused(String),

    /// Pairing ended without both sides agreeing.
    #[error("{0}")]
    NotPaired(String),
}

impl From<std::io::Error> for SyncError {
    fn from(error: std::io::Error) -> Self {
        Self::Network(error.to_string())
    }
}

/// Shorthand for this crate's results.
pub type Result<T> = std::result::Result<T, SyncError>;
