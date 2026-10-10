//! Pairing and syncing from the browser, on the browser's own event loop.
//!
//! The surface's async halves — [`Lumenna::pair_async`] and [`Lumenna::keep_in_sync`] — are
//! what every other client runs on a runtime of its own; here they are promises. A browser
//! reaches other devices only through a relay (no UDP from a sandbox), so it always uses
//! [`Reach::Internet`], and it has no local network to find a pairing on, so pairing is by
//! code. One tab owns the store (the worker's Web Lock), which is what the lock file is for
//! elsewhere: so this loop is the device's only endpoint.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use lumenna_surface::{DeviceList, Lumenna, PairedWith, Reach, SyncLoop, SyncReport};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::{JsFuture, future_to_promise};

use crate::{Core, Out, error, js, out};

/// What this browser's sync is doing: the loop while it runs, and whether a pairing in
/// progress has been given up.
#[derive(Default)]
pub(crate) struct Syncing {
    running: RefCell<Option<SyncLoop>>,
    cancelled: Cell<bool>,
}

/// What a JavaScript callback answered: the value, or what its promise settled to.
async fn settled(value: JsValue) -> JsValue {
    match value.dyn_into::<js_sys::Promise>() {
        Ok(promise) => JsFuture::from(promise).await.unwrap_or(JsValue::FALSE),
        Err(value) => value,
    }
}

#[wasm_bindgen]
impl Core {
    /// Pairs this browser with another of the person's devices. Without a `code`, it
    /// waits to be dialled, calling `show_code` with the code to give the other device; with
    /// one, it dials that device. `confirm` is called with the three words and answers — or
    /// promises — whether they match on the other device. Resolves to the device paired with.
    ///
    /// `name` is what this browser is called on the other devices, unless it already has a
    /// name from an earlier pairing.
    pub fn pair(
        &self,
        code: Option<String>,
        name: String,
        show_code: js_sys::Function,
        confirm: js_sys::Function,
    ) -> js_sys::Promise {
        let lumenna: Rc<Lumenna> = Rc::clone(&self.lumenna);
        let syncing = Rc::clone(&self.syncing);
        syncing.cancelled.set(false);
        future_to_promise(async move {
            let watching = Rc::clone(&syncing);
            let paired: PairedWith = lumenna
                .pair_async(
                    code,
                    Reach::Internet,
                    name,
                    "web".to_owned(),
                    move |code| {
                        let _ = show_code.call1(&JsValue::NULL, &JsValue::from_str(&code));
                    },
                    async move {
                        while !watching.cancelled.get() {
                            n0_future::time::sleep(Duration::from_millis(250)).await;
                        }
                    },
                    move |words| async move {
                        let words: js_sys::Array = words.iter().map(|w| JsValue::from_str(w)).collect();
                        match confirm.call1(&JsValue::NULL, &words) {
                            Ok(answer) => settled(answer).await.as_bool().unwrap_or(false),
                            Err(_) => false,
                        }
                    },
                )
                .await
                .map_err(|e| JsValue::from(error(e)))?;
            Ok(js(&paired)?.into())
        })
    }

    /// Gives up a pairing that is waiting for the other device.
    #[wasm_bindgen(js_name = cancelPairing)]
    pub fn cancel_pairing(&self) {
        self.syncing.cancelled.set(true);
    }

    /// Starts keeping this browser in sync for as long as the page is open: it answers the
    /// other devices, sends changes on within a second or so, and catches up every few
    /// minutes. `changed` is called when something arrives. Resolves once the endpoint is
    /// open; starting it again while it runs does nothing.
    #[wasm_bindgen(js_name = startSync)]
    pub fn start_sync(&self, changed: js_sys::Function) -> js_sys::Promise {
        let lumenna = Rc::clone(&self.lumenna);
        let syncing = Rc::clone(&self.syncing);
        future_to_promise(async move {
            if syncing.running.borrow().as_ref().is_some_and(SyncLoop::is_running) {
                return Ok(JsValue::UNDEFINED);
            }
            let (handle, looping) = lumenna
                .keep_in_sync(Reach::Internet, move || {
                    let _ = changed.call0(&JsValue::NULL);
                })
                .await
                .map_err(|e| JsValue::from(error(e)))?;
            *syncing.running.borrow_mut() = Some(handle);
            wasm_bindgen_futures::spawn_local(looping);
            Ok(JsValue::UNDEFINED)
        })
    }

    /// Syncs with every paired device now, on the running loop. Resolves to how each went.
    #[wasm_bindgen(js_name = syncNow)]
    pub fn sync_now(&self) -> js_sys::Promise {
        let running = self.syncing.running.borrow().clone();
        future_to_promise(async move {
            let Some(handle) = running.filter(SyncLoop::is_running) else {
                return Err(JsError::new("Sync is not running.").into());
            };
            let report: SyncReport = handle.sync_now().await.map_err(|e| JsValue::from(error(e)))?;
            Ok(js(&report)?.into())
        })
    }

    /// Stops syncing, closing the endpoint.
    #[wasm_bindgen(js_name = stopSync)]
    pub fn stop_sync(&self) {
        if let Some(handle) = self.syncing.running.borrow_mut().take() {
            handle.stop();
        }
    }

    /// Whether this browser is keeping in sync now.
    #[wasm_bindgen(js_name = syncRunning)]
    pub fn sync_running(&self) -> bool {
        self.syncing.running.borrow().as_ref().is_some_and(SyncLoop::is_running)
    }

    /// The paired devices, this one first.
    pub fn devices(&self) -> Out<DeviceList> {
        out(self.lumenna.devices())
    }

}
