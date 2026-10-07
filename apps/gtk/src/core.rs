//! The open store, and what runs beside the window: syncing, backups, and noticing what other
//! processes wrote.
//!
//! Operations are milliseconds against local SQLite, so they run on GTK's main thread and a
//! view never shows a state the store has moved past — as on the Mac and Windows. What takes
//! longer — starting the sync endpoint, a sync round, a backup — runs on a thread of its own
//! and hands what it has to say back to the main thread as an [`Event`].

use std::sync::{Arc, Mutex, PoisonError};

use gtk::glib;
use lumenna_desktop::speech;
use lumenna_surface::rpc::CommandServer;
use lumenna_surface::{Lumenna, LumennaError, Reach, SyncListener, SyncService};

/// Something from another thread for the window.
#[derive(Debug)]
pub enum Event {
    /// Something arrived from another device: every view reads the store again.
    Changed,
    /// A sentence to say.
    Say(String),
    /// From the tray: show the window.
    ShowWindow,
    /// From the tray: quick add, over whatever is in front.
    QuickAdd,
    /// From the tray: sync now.
    SyncNow,
    /// From the tray: quit.
    Quit,
}

/// Hands an event to the main thread, which gives it to the window.
pub fn post(event: Event) {
    glib::MainContext::default().invoke(move || crate::window::receive(event));
}

/// The open store.
pub struct Core {
    pub lumenna: Arc<Lumenna>,
    sync: Arc<Mutex<Option<Arc<SyncService>>>>,
    /// The command surface, served to Emacs, the BTSpeak app and `lum` while this app holds
    /// the device's endpoint.
    commands: Arc<Mutex<Option<Arc<CommandServer>>>>,
}

impl Core {
    pub fn open(directory: &std::path::Path) -> Result<Self, LumennaError> {
        let lumenna = Lumenna::open(&directory.display().to_string())?;
        Ok(Self { lumenna, sync: Arc::new(Mutex::new(None)), commands: Arc::new(Mutex::new(None)) })
    }

    /// Starts keeping this device in sync, for as long as the app runs: the resident
    /// app is the device's sync process, with no service to set up.
    ///
    /// If another process already holds the endpoint — `lum daemon` — the service waits its
    /// turn and takes over when that process stops, so it is kept either way; meanwhile Sync
    /// Now asks that process for a round.
    ///
    /// Whichever process holds the endpoint also serves the command surface at the profile's
    /// socket, so the app does too, from when it takes the endpoint until it lets it go.
    pub fn start_syncing(&self) {
        let lumenna = Arc::clone(&self.lumenna);
        let sync = Arc::clone(&self.sync);
        let commands = Arc::clone(&self.commands);
        std::thread::spawn(move || {
            let listener: Arc<dyn SyncListener> = Arc::new(Arrivals);
            match lumenna.start_sync(Reach::Internet, listener) {
                Ok(service) => {
                    let name = gtk::glib::host_name().to_string();
                    let server =
                        lumenna.serve_commands(Arc::clone(&service), "Lumenna for Linux".to_owned(), name, "linux".to_owned());
                    *commands.lock().unwrap_or_else(PoisonError::into_inner) = Some(server);
                    *sync.lock().unwrap_or_else(PoisonError::into_inner) = Some(service);
                }
                Err(error) => post(Event::Say(format!("Syncing could not start. {}", sentence(&error)))),
            }
        });
    }

    /// Stops serving and syncing, and lets go of the endpoint, before returning: the socket
    /// first, so no client is answered by a store that has stopped syncing.
    pub fn stop_syncing(&self) {
        let commands = self.commands.lock().unwrap_or_else(PoisonError::into_inner).take();
        if let Some(commands) = commands {
            commands.stop();
        }
        let service = self.sync.lock().unwrap_or_else(PoisonError::into_inner).take();
        if let Some(service) = service {
            service.stop();
        }
    }

    /// Syncs with every paired device now, on this app's endpoint or one opened for the
    /// round, and says how it went.
    pub fn sync_now(&self) {
        let lumenna = Arc::clone(&self.lumenna);
        let sync = Arc::clone(&self.sync);
        std::thread::spawn(move || {
            let service = sync.lock().unwrap_or_else(PoisonError::into_inner).clone();
            let result = match service {
                Some(service) => service.sync_now(),
                None => lumenna.sync_now(Reach::Internet),
            };
            match result {
                Ok(report) => {
                    let failures: Vec<String> = report
                        .peers
                        .iter()
                        .filter_map(|peer| peer.error.as_ref().map(|e| format!("{}: {e}", peer.name)))
                        .collect();
                    post(Event::Say(speech::announcement(&report.announcement, &failures)));
                    post(Event::Changed);
                }
                Err(error) => post(Event::Say(sentence(&error))),
            }
        });
    }

    /// Takes a backup if one is due, and says so only if it failed.
    pub fn back_up_if_due(&self) {
        let lumenna = Arc::clone(&self.lumenna);
        std::thread::spawn(move || {
            if let Err(error) = lumenna.back_up_if_due() {
                post(Event::Say(format!("The automatic backup failed. {}", sentence(&error))));
            }
        });
    }
}

/// What a sync brought in: every view reads the store again.
struct Arrivals;

impl SyncListener for Arrivals {
    fn changed(&self) {
        post(Event::Changed);
    }
}

/// The sentence the core wrote for an error, which is already phrased to be read aloud.
pub fn sentence(error: &LumennaError) -> String {
    speech::sentence(error.message())
}
