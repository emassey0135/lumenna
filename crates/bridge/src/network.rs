//! What reaches other devices, natively: the network middleware for a shell that runs the app in
//! a process of its own (Android, and the Apple and desktop apps at their turn).
//!
//! Each request runs on a thread of its own over the host's blocking sync, since a pairing
//! waits minutes on another device and on the person, and the answer about its words comes in
//! meanwhile as a request of its own. A browser runs the same requests on its event loop
//! instead (the web core's runner).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex, PoisonError};

use crux_core::middleware::{EffectMiddleware, EffectResolver};
use lumenna_app::{NetworkAnswer, NetworkRequest};
use lumenna_host::{Lumenna, PairingPrompt, Reach, SyncListener, SyncService};
use lumenna_records::words::spoken;

use crate::Store;

/// The network, answered on threads over the host's blocking sync.
pub struct Network(Arc<Inner>);

struct Inner {
    lumenna: Arc<Lumenna>,
    store: Store,
    reach: Reach,
    /// What this device says it runs, in pairing.
    platform: String,
    /// The sync service while it runs.
    service: Mutex<Option<Arc<SyncService>>>,
    /// Whether the app asked to keep in sync, which holds while it is away from the front.
    wanted: AtomicBool,
    /// Whether the app is in front: syncing runs only then.
    in_front: AtomicBool,
    /// The pairing's question about the words, while it waits for the person.
    confirm: Mutex<Option<mpsc::Sender<bool>>>,
    /// Whether the pairing under way has been given up.
    cancelled: Arc<AtomicBool>,
}

impl Network {
    /// The network for this store, reaching devices by `reach`, saying it runs `platform`.
    #[must_use]
    pub fn new(lumenna: Arc<Lumenna>, store: Store, reach: Reach, platform: String) -> Self {
        Self(Arc::new(Inner {
            lumenna,
            store,
            reach,
            platform,
            service: Mutex::new(None),
            wanted: AtomicBool::new(false),
            in_front: AtomicBool::new(true),
            confirm: Mutex::new(None),
            cancelled: Arc::new(AtomicBool::new(false)),
        }))
    }

    /// The same network, for the shell to tell when the app comes to the front or leaves it.
    #[must_use]
    pub fn handle(&self) -> NetworkHandle {
        NetworkHandle(Arc::clone(&self.0))
    }
}

/// What the shell tells the network itself: the platform's lifecycle, which the app does not know.
#[derive(Clone)]
pub struct NetworkHandle(Arc<Inner>);

impl NetworkHandle {
    /// The app came to the front, or left it: syncing stops while it is away, and starts again
    /// when it comes back if the app had asked for it.
    pub fn in_front(&self, in_front: bool) {
        self.0.in_front.store(in_front, Ordering::Relaxed);
        let inner = Arc::clone(&self.0);
        std::thread::spawn(move || {
            if in_front {
                if inner.wanted.load(Ordering::Relaxed) {
                    let _ = inner.start();
                }
            } else if let Some(service) = inner.lock_service().take() {
                service.stop();
            }
        });
    }
}

impl EffectMiddleware for Network {
    type Op = NetworkRequest;

    fn try_process_effect(&self, request: NetworkRequest, resolver: EffectResolver<NetworkAnswer>) {
        let inner = Arc::clone(&self.0);
        std::thread::spawn(move || inner.answer(request, resolver));
    }
}

/// What arrives from other devices is in the store already: the app takes it in.
struct Arrivals(Store);

impl SyncListener for Arrivals {
    fn changed(&self) {
        self.0.tick();
    }
}

/// A pairing's prompt, answering the app's request as the pairing goes: its code, its words.
struct Prompt {
    resolver: Arc<Mutex<EffectResolver<NetworkAnswer>>>,
    inner: Arc<Inner>,
}

impl PairingPrompt for Prompt {
    fn show_code(&self, code: String) {
        self.resolver.lock().unwrap_or_else(PoisonError::into_inner).resolve(NetworkAnswer::Code(code));
    }

    fn confirm(&self, words: Vec<String>) -> bool {
        let (answer, answered) = mpsc::channel();
        *self.inner.confirm.lock().unwrap_or_else(PoisonError::into_inner) = Some(answer);
        self.resolver.lock().unwrap_or_else(PoisonError::into_inner).resolve(NetworkAnswer::Words(words));
        answered.recv().unwrap_or(false)
    }

    fn is_cancelled(&self) -> bool {
        self.inner.cancelled.load(Ordering::Relaxed)
    }
}

impl Inner {
    fn lock_service(&self) -> std::sync::MutexGuard<'_, Option<Arc<SyncService>>> {
        self.service.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Starts the sync service, unless it runs already or the app is away from the front.
    fn start(&self) -> Result<(), String> {
        if !self.in_front.load(Ordering::Relaxed) || self.lock_service().is_some() {
            return Ok(());
        }
        let service = self
            .lumenna
            .start_sync(self.reach, Arc::new(Arrivals(self.store.clone())))
            .map_err(|error| error.message().to_owned())?;
        *self.lock_service() = Some(service);
        Ok(())
    }

    fn answer(self: Arc<Self>, request: NetworkRequest, mut resolver: EffectResolver<NetworkAnswer>) {
        let answer = match request {
            NetworkRequest::StartSync => {
                self.wanted.store(true, Ordering::Relaxed);
                match self.start() {
                    Ok(()) => NetworkAnswer::Done,
                    Err(why) => NetworkAnswer::Said(why),
                }
            }
            NetworkRequest::SyncNow => {
                let service = self.lock_service().clone();
                let report = match service {
                    Some(service) => service.sync_now(),
                    None => self.lumenna.sync_now(self.reach),
                };
                match report {
                    Ok(report) => {
                        let failures: Vec<String> = report
                            .peers
                            .iter()
                            .filter_map(|peer| peer.error.as_ref().map(|error| format!("{}: {error}", peer.name)))
                            .collect();
                        NetworkAnswer::Said(spoken(&report.announcement, &failures))
                    }
                    Err(error) => NetworkAnswer::Said(error.message().to_owned()),
                }
            }
            NetworkRequest::Status => match self.lumenna.sync_status() {
                Ok(status) => NetworkAnswer::Said(spoken(&status.announcement, &status.notices)),
                Err(error) => NetworkAnswer::Said(error.message().to_owned()),
            },
            NetworkRequest::Pair { code, name } => {
                // A pairing answers more than once: its code, its words, how it ended.
                self.cancelled.store(false, Ordering::Relaxed);
                let resolver = Arc::new(Mutex::new(resolver));
                let prompt = Arc::new(Prompt { resolver: Arc::clone(&resolver), inner: Arc::clone(&self) });
                let paired = self.lumenna.pair(code, self.reach, name, self.platform.clone(), prompt);
                let ended = NetworkAnswer::Paired(match paired {
                    Ok(paired) => Ok(spoken(&paired.announcement, &paired.notices)),
                    Err(error) => Err(error.message().to_owned()),
                });
                resolver.lock().unwrap_or_else(PoisonError::into_inner).resolve(ended);
                return;
            }
            NetworkRequest::Confirm(matched) => {
                if let Some(answer) = self.confirm.lock().unwrap_or_else(PoisonError::into_inner).take() {
                    let _ = answer.send(matched);
                }
                NetworkAnswer::Done
            }
            NetworkRequest::CancelPairing => {
                self.cancelled.store(true, Ordering::Relaxed);
                // A pairing waiting on the words gives up too.
                if let Some(answer) = self.confirm.lock().unwrap_or_else(PoisonError::into_inner).take() {
                    let _ = answer.send(false);
                }
                NetworkAnswer::Done
            }
        };
        resolver.resolve(answer);
    }
}
