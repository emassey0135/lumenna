//! The paired device roster.

use jiff::Timestamp;

use crate::id::NodeId;

/// One of your devices.
///
/// This lives in its own Automerge document because it is small, changes rarely, and every
/// device needs it. The device's *private* key never appears here or anywhere else that
/// syncs.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Device {
    /// The device's ed25519 public key, which is also its Iroh address.
    pub node_id: NodeId,
    /// What the user calls it.
    pub name: String,
    /// What it runs, for display.
    pub platform: String,
    /// When it joined.
    pub paired_at: Timestamp,
    /// When it was last heard from. Best-effort, like everything else that crosses devices.
    pub last_seen: Timestamp,
    /// The [`SCHEMA_VERSION`](super::SCHEMA_VERSION) it runs, which only it writes. Zero for
    /// a device that has not said, which a build from before versions never does.
    #[cfg_attr(feature = "serde", serde(default))]
    pub schema: u32,
}
