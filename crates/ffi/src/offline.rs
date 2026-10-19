//! The network, for a build without one: the Apple Watch's, which syncs through its phone over
//! the link alone (watchOS allows no sockets). Each request is answered at once, saying so; the
//! shape is the bridge's `Network`'s, so the app is built over either.

use std::sync::Arc;

use crux_core::middleware::{EffectMiddleware, EffectResolver};
use lumenna_app::{NetworkAnswer, NetworkRequest};
use lumenna_host::{Lumenna, Reach};
use lumenna_bridge::Store;

/// What a device without a network of its own says it cannot do: reach another device but the
/// phone it is linked to.
const ELSEWHERE: &str = "This device syncs through its phone";

/// No network: every request answered at once.
pub struct Network;

impl Network {
    /// The same arguments as the bridge's, none of them needed.
    pub fn new(_lumenna: Arc<Lumenna>, _store: Store, _reach: Reach, _platform: String) -> Self {
        Self
    }

    /// Nothing to tell.
    pub fn handle(&self) -> NetworkHandle {
        NetworkHandle
    }
}

/// The lifecycle the shell tells a network, which this one has no use for.
#[derive(Clone)]
pub struct NetworkHandle;

impl NetworkHandle {
    /// Nothing runs, in front or away.
    pub fn in_front(&self, _in_front: bool) {}
}

impl EffectMiddleware for Network {
    type Op = NetworkRequest;

    fn try_process_effect(&self, request: NetworkRequest, mut resolver: EffectResolver<NetworkAnswer>) {
        let answer = match request {
            // Syncing is the link's, which the shell runs; nothing to start here.
            NetworkRequest::StartSync | NetworkRequest::Confirm(_) | NetworkRequest::CancelPairing => NetworkAnswer::Done,
            NetworkRequest::SyncNow | NetworkRequest::Status => NetworkAnswer::Said(ELSEWHERE.to_owned()),
            NetworkRequest::Pair { .. } => NetworkAnswer::Paired(Err(ELSEWHERE.to_owned())),
        };
        // Answered on a thread, as every middleware hands its work on: a resolver must never be
        // called inside `try_process_effect`.
        std::thread::spawn(move || resolver.resolve(answer));
    }
}
