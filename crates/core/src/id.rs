//! Typed identifiers.
//!
//! Every record in the model is keyed by a UUIDv7 (§3): time-sortable, good index locality,
//! and unique across devices without coordination. The types here are distinct so that a
//! `TaskId` can never be passed where a `ProjectId` is wanted — a real hazard in a model
//! where §3.1 forbids referential integrity across documents and dangling references are
//! expected rather than exceptional.
//!
//! `Ord` on these types is creation order. `Uuid`'s ordering is bytewise over the 16 bytes,
//! and a v7 UUID carries its 48-bit millisecond timestamp big-endian in the leading bytes,
//! so bytewise order and time order coincide. Several repairs in [`crate::repair`] rely on
//! this to pick "the most recently created" deterministically on every replica.

use core::fmt;
use core::str::FromStr;

use uuid::Uuid;

/// A malformed identifier in text form.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("not a valid {kind} identifier: {input}")]
pub struct ParseIdError {
    /// The identifier type that was expected.
    pub kind: &'static str,
    /// The text that failed to parse.
    pub input: String,
}

macro_rules! id_type {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
        pub struct $name(Uuid);

        // No `Default`: `Uuid::default()` is the nil UUID, so a `default()` here that
        // minted a fresh v7 would mean something entirely different from the inner type's,
        // and one that returned nil would hand out a valid-looking identifier that collides
        // with every other caller's.
        #[expect(clippy::new_without_default, reason = "see above")]
        impl $name {
            /// Mints a fresh identifier from the current time.
            #[must_use]
            pub fn new() -> Self {
                Self(Uuid::now_v7())
            }

            /// Wraps an existing UUID, for hydration from a stored document.
            ///
            /// No validation: a document written by a future version, or corrupted in
            /// transit, must still load. §3.1 requires tolerating references that make no
            /// sense rather than refusing to open the store.
            #[must_use]
            pub const fn from_uuid(uuid: Uuid) -> Self {
                Self(uuid)
            }

            /// The underlying UUID.
            #[must_use]
            pub const fn as_uuid(&self) -> &Uuid {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::Display::fmt(&self.0, f)
            }
        }

        impl FromStr for $name {
            type Err = ParseIdError;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Uuid::parse_str(s).map(Self).map_err(|_| ParseIdError {
                    kind: stringify!($name),
                    input: s.to_owned(),
                })
            }
        }
    };
}

id_type!(
    /// Identifies a [`Task`](crate::model::Task).
    TaskId
);
id_type!(
    /// Identifies a [`TaskCompletion`](crate::model::TaskCompletion).
    CompletionId
);
id_type!(
    /// Identifies a [`Project`](crate::model::Project).
    ProjectId
);
id_type!(
    /// Identifies a [`Label`](crate::model::Label).
    LabelId
);
id_type!(
    /// Identifies a [`SavedFilter`](crate::model::SavedFilter).
    FilterId
);
id_type!(
    /// Identifies a [`BlockSeries`](crate::model::BlockSeries).
    SeriesId
);
id_type!(
    /// Identifies a [`BlockAssignment`](crate::model::BlockAssignment).
    AssignmentId
);
id_type!(
    /// Identifies a [`Reminder`](crate::model::Reminder).
    ReminderId
);

impl ProjectId {
    /// The Inbox's identifier, the same on every device.
    ///
    /// Every store has exactly one Inbox (§3.4). If each device minted its own, two devices
    /// that started apart would merge into a store with two, so the Inbox is created by a
    /// deterministic change in `store` under this fixed identifier instead, and identical
    /// changes merge into one record.
    ///
    /// It is a version 8 UUID with every free bit zero but the last, so it sorts before
    /// every UUIDv7 — it is, after all, the oldest record there is.
    pub const INBOX: Self = Self(Uuid::from_u128(0x0000_0000_0000_8000_8000_0000_0000_0001));
}

/// An Iroh node identifier: a device's ed25519 public key (§3.11).
///
/// Core neither generates nor verifies these — that is `sync`'s work. It carries them
/// because reminder delivery (§3.8) and acknowledgement (§3.9) are addressed by device.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct NodeId([u8; 32]);

impl NodeId {
    /// Wraps a raw public key.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// The raw public key.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in &self.0 {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}

impl FromStr for NodeId {
    type Err = ParseIdError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let err = || ParseIdError { kind: "NodeId", input: s.to_owned() };
        if s.len() != 64 {
            return Err(err());
        }
        let mut bytes = [0u8; 32];
        for (i, byte) in bytes.iter_mut().enumerate() {
            *byte = u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).map_err(|_| err())?;
        }
        Ok(Self(bytes))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_sort_by_creation_time() {
        let mut ids: Vec<TaskId> = (0..64).map(|_| TaskId::new()).collect();
        let expected = ids.clone();
        ids.sort();
        assert_eq!(ids, expected, "UUIDv7 ordering must be creation order");
    }

    #[test]
    fn id_text_round_trips() {
        let id = ProjectId::new();
        assert_eq!(id.to_string().parse::<ProjectId>().unwrap(), id);
        assert!("nonsense".parse::<ProjectId>().is_err());
    }

    #[test]
    fn node_id_text_round_trips() {
        let id = NodeId::from_bytes([0xab; 32]);
        assert_eq!(id.to_string(), "ab".repeat(32));
        assert_eq!(id.to_string().parse::<NodeId>().unwrap(), id);
        assert!("ab".parse::<NodeId>().is_err());
        assert!("zz".repeat(32).parse::<NodeId>().is_err());
    }
}
