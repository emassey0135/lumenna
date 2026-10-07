//! The command surface over JSON-RPC: one server, for every process that serves it (the
//! `rpc` feature).
//!
//! `lum rpc` serves it on stdio to one client. Whichever process holds this device's sync
//! endpoint — `lum`'s daemon, or a desktop app — serves it at [`crate::endpoint`]'s
//! address too, a Unix socket or a named pipe, to any number of clients, each with a store
//! connection of its own. So Emacs and the BTSpeak app reach a running app as they reach the
//! daemon, and a write a client makes there is another connection's write to the app, which
//! redraws for it as it does for `lum`.
//!
//! A method calls the surface's operation and returns its record in the [`api`] envelope,
//! the same `lum --json` writes: one implementation of every operation, so no transport
//! drifts from another or from the apps that link the surface.
//!
//! # Framing
//!
//! Newline-delimited JSON, and LSP-style `Content-Length` headers, chosen per message by
//! what arrived: MCP's stdio transport uses the first and `jsonrpc.el` uses the second. A
//! reply is framed the way its request was.

pub mod api;
mod endpoint;
mod server;

pub use endpoint::{Endpoint, relay};
pub use server::{Host, METHODS, SyncHook, serve_streams};

use std::sync::{Arc, Mutex};

use crate::{Lumenna, SyncService};

/// The command surface served at this profile's address, for as long as it lives or until
/// stopped: what a desktop app runs beside its sync, so Emacs and the BTSpeak app reach the
/// app as they would the daemon.
#[cfg_attr(feature = "uniffi", derive(uniffi::Object))]
pub struct CommandServer {
    endpoint: Mutex<Option<Endpoint>>,
}

#[cfg_attr(feature = "uniffi", uniffi::export)]
impl CommandServer {
    /// Stops serving, and gives the address up.
    pub fn stop(&self) {
        let endpoint = self.endpoint.lock().unwrap_or_else(std::sync::PoisonError::into_inner).take();
        if let Some(endpoint) = endpoint {
            endpoint.stop();
        }
    }
}

#[cfg_attr(feature = "uniffi", uniffi::export)]
impl Lumenna {
    /// Serves the command surface at this profile's address while `service` holds the
    /// device's endpoint, answering a `sync` with a round on it. `app` is what a client is
    /// told is answering, such as "Lumenna for Mac"; `device_name` and `platform` are what a
    /// pairing asked for over it says of this device.
    ///
    /// A client gets a store connection of its own, so what it writes reaches the app as
    /// another process's write, which the app redraws for already.
    #[must_use]
    pub fn serve_commands(
        &self,
        service: Arc<SyncService>,
        app: String,
        device_name: String,
        platform: String,
    ) -> Arc<CommandServer> {
        let host = Host {
            process: app,
            device_name,
            platform,
            sync: Some({
                let service = Arc::clone(&service);
                Arc::new(move || service.sync_now())
            }),
            // The app takes them itself.
            backups: false,
        };
        let endpoint = Endpoint::serve(self.directory.clone(), host, move || service.is_running());
        Arc::new(CommandServer { endpoint: Mutex::new(Some(endpoint)) })
    }
}
