//! The Windows 11 Widgets board's provider: a COM server the board starts, drawing the glance
//! (`crate::widget`) as Adaptive Cards, small and medium.
//!
//! It is `lumenna.exe` started with [`ARGUMENT`] by the package's manifest
//! (`installer/msix/AppxManifest.xml`: a `windows.comServer` and the
//! `com.microsoft.windows.widgets` app extension naming [`CLASS`]), not a program of its own: the
//! store, its profile and the core are statically linked into the one executable, which a second
//! would carry again whole. Started so, it opens no window, takes no instance lock and gives no
//! reminders, so it runs beside the window and `lum` as another connection to the store does.
//! Only a package has the identity the board needs, so the Inno install offers no widget.
//!
//! The board tells it which widgets exist and which are on screen (`Activate`, `Deactivate`).
//! While one is, it looks once every [`POLL`] whether the store moved, as the window does once a
//! second, and draws again then, at each moment the glance's timeline names, and each minute a
//! timer runs. It exits once the last widget is taken off the board.

mod bindings;

use std::collections::BTreeMap;
use std::sync::{Arc, Condvar, Mutex, PoisonError};
use std::time::Duration;

use jiff::Timestamp;
use lumenna_app::glance;
use lumenna_records::assistant::{Argument, ArgumentValue};
use windows::Win32::Foundation::{CLASS_E_NOAGGREGATION, E_POINTER};
use windows::Win32::System::Com::{
    CLSCTX_LOCAL_SERVER, COINIT_MULTITHREADED, CoInitializeEx, CoRegisterClassObject, CoRevokeClassObject, IClassFactory,
    IClassFactory_Impl, REGCLS_MULTIPLEUSE,
};
use windows::core::{BOOL, GUID, HSTRING, IUnknown, Interface, Ref, implement};

use bindings::{
    IWidgetProvider, IWidgetProvider_Impl, WidgetActionInvokedArgs, WidgetContext, WidgetContextChangedArgs, WidgetManager,
    WidgetSize, WidgetUpdateRequestOptions,
};

use super::clock;
use crate::{profile, widget};

/// The provider's COM class, as the manifest names it.
pub const CLASS: GUID = GUID::from_u128(0x18d24380_6221_4eaf_975d_40086936d300);

/// What the manifest starts `lumenna.exe` with to be the provider.
pub const ARGUMENT: &str = "--widgets";

/// How often an active widget's store is looked at for another process's writes.
const POLL: Duration = Duration::from_secs(2);

/// How far ahead the glance is worked out; the timeline is asked for again long before.
const HOURS: u32 = 24;

/// Whether this process was started to be the provider.
pub fn requested() -> bool {
    std::env::args().skip(1).any(|argument| argument == ARGUMENT)
}

/// A widget on the board, as the provider knows it.
struct Widget {
    size: WidgetSize,
    /// On screen, so kept up to date.
    active: bool,
    /// Drawn since the store or the moment last changed.
    drawn: bool,
}

#[derive(Default)]
struct State {
    widgets: BTreeMap<String, Widget>,
    /// Something asks for every widget to be drawn now: one made, shown, resized, or a task
    /// marked done from one.
    dirty: bool,
    /// The last widget was taken off the board.
    ended: bool,
}

/// What the board's calls and the drawing loop share.
struct Shared {
    device: lumenna_ffi::Device,
    state: Mutex<State>,
    wake: Condvar,
}

impl Shared {
    fn change(&self, change: impl FnOnce(&mut State)) {
        change(&mut self.state.lock().unwrap_or_else(PoisonError::into_inner));
        self.wake.notify_all();
    }

    /// A widget the board names, made or shown: drawn at once.
    fn hold(&self, context: &WidgetContext, active: bool) -> windows::core::Result<()> {
        let id = context.Id()?.to_string();
        let size = context.Size()?;
        self.change(|state| {
            state.widgets.insert(id, Widget { size, active, drawn: false });
            state.dirty = true;
        });
        Ok(())
    }

    /// Draws every widget on screen and every one not yet drawn, and returns when they must be
    /// drawn again, the store unchanged.
    fn draw(&self, widgets: &[(String, WidgetSize)]) -> Option<Timestamp> {
        // A glance that cannot be worked out, or has no moment, is the core's empty day, never a
        // card left blank.
        let entries = self.device.glance(HOURS, Arc::new(clock::Formats)).unwrap_or_default();
        let now = Timestamp::now();
        let empty;
        let entry = match glance::current(&entries, now) {
            Some(entry) => entry,
            None => {
                empty = lumenna_ffi::glance_empty(Arc::new(clock::Formats));
                &empty
            }
        };
        let data = HSTRING::from(widget::data(entry, now).to_string());
        if let Ok(manager) = WidgetManager::GetDefault() {
            for (id, size) in widgets {
                let template = if *size == WidgetSize::Small { widget::SMALL } else { widget::MEDIUM };
                let _ = WidgetUpdateRequestOptions::CreateInstance(&HSTRING::from(id)).and_then(|options| {
                    options.SetTemplate(&HSTRING::from(template))?;
                    options.SetData(&data)?;
                    manager.UpdateWidget(&options)
                });
            }
        }
        glance::redraw_at(&entries, now)
    }

    /// Draws as the board and the store ask, until the last widget goes.
    fn run(&self) {
        let mut heads = String::new();
        let mut redraw: Option<Timestamp> = None;
        loop {
            let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
            let active = state.widgets.values().any(|widget| widget.active);
            let until_redraw = redraw.map(|at| Duration::try_from(at.duration_since(Timestamp::now())).unwrap_or_default());
            let wait = match (active, until_redraw) {
                (true, Some(until)) => Some(until.min(POLL)),
                (true, None) => Some(POLL),
                (false, until) => until,
            };
            if !state.dirty && !state.ended {
                state = match wait {
                    Some(wait) => self.wake.wait_timeout(state, wait).unwrap_or_else(PoisonError::into_inner).0,
                    None => self.wake.wait(state).unwrap_or_else(PoisonError::into_inner),
                };
            }
            if state.ended {
                return;
            }
            let dirty = std::mem::take(&mut state.dirty);
            drop(state);
            let now_heads = self.device.heads().unwrap_or_default();
            let moved = now_heads != heads;
            let due = redraw.is_some_and(|at| at <= Timestamp::now());
            if !(dirty || moved || due) {
                continue;
            }
            heads = now_heads;
            let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
            let widgets: Vec<(String, WidgetSize)> = state
                .widgets
                .iter_mut()
                .filter(|(_, widget)| widget.active || !widget.drawn || dirty)
                .map(|(id, widget)| {
                    widget.drawn = true;
                    (id.clone(), widget.size)
                })
                .collect();
            // One off screen is drawn again when it is shown.
            for widget in state.widgets.values_mut().filter(|widget| !widget.active && (moved || due)) {
                widget.drawn = false;
            }
            drop(state);
            redraw = self.draw(&widgets);
        }
    }
}

#[implement(IWidgetProvider)]
struct Provider(Arc<Shared>);

impl IWidgetProvider_Impl for Provider_Impl {
    fn CreateWidget(&self, context: Ref<WidgetContext>) -> windows::core::Result<()> {
        let context = context.ok()?;
        self.0.hold(context, context.IsActive().unwrap_or(false))
    }

    fn DeleteWidget(&self, id: &HSTRING, _state: &HSTRING) -> windows::core::Result<()> {
        let id = id.to_string();
        self.0.change(|state| {
            state.widgets.remove(&id);
            state.ended = state.widgets.is_empty();
        });
        Ok(())
    }

    fn OnActionInvoked(&self, invoked: Ref<WidgetActionInvokedArgs>) -> windows::core::Result<()> {
        let invoked = invoked.ok()?;
        if let Some(task) = widget::completing(&invoked.Verb()?.to_string(), &invoked.Data()?.to_string()) {
            // The board has nowhere to say a refusal: the card drawn again shows what is so.
            let id = Argument { name: "id".to_owned(), value: ArgumentValue::Text(task) };
            let _ = self.0.device.assist("complete_task".to_owned(), vec![id]);
        } else if let Some(id) = widget::launching(&invoked.Verb()?.to_string(), &invoked.Data()?.to_string()) {
            // As the Jump List starts it: a copy already running takes the id (`tray::claim`).
            // Within the package, so it opens the package's own store.
            if let Ok(exe) = std::env::current_exe() {
                let _ = std::process::Command::new(exe).args(["--launch", &id]).spawn();
            }
        }
        self.0.change(|state| state.dirty = true);
        Ok(())
    }

    fn OnWidgetContextChanged(&self, changed: Ref<WidgetContextChangedArgs>) -> windows::core::Result<()> {
        let context = changed.ok()?.WidgetContext()?;
        self.0.hold(&context, context.IsActive().unwrap_or(true))
    }

    fn Activate(&self, context: Ref<WidgetContext>) -> windows::core::Result<()> {
        self.0.hold(context.ok()?, true)
    }

    fn Deactivate(&self, id: &HSTRING) -> windows::core::Result<()> {
        let id = id.to_string();
        self.0.change(|state| {
            if let Some(widget) = state.widgets.get_mut(&id) {
                widget.active = false;
            }
        });
        Ok(())
    }
}

/// What COM asks for a provider.
#[implement(IClassFactory)]
struct Factory(Arc<Shared>);

impl IClassFactory_Impl for Factory_Impl {
    fn CreateInstance(
        &self,
        outer: Ref<IUnknown>,
        iid: *const GUID,
        object: *mut *mut core::ffi::c_void,
    ) -> windows::core::Result<()> {
        if object.is_null() || iid.is_null() {
            return Err(E_POINTER.into());
        }
        unsafe { *object = std::ptr::null_mut() };
        if !outer.is_null() {
            return Err(CLASS_E_NOAGGREGATION.into());
        }
        let provider: IWidgetProvider = Provider(Arc::clone(&self.0)).into();
        unsafe { provider.query(iid, object).ok() }
    }

    fn LockServer(&self, _lock: BOOL) -> windows::core::Result<()> {
        Ok(())
    }
}

/// Serves the board until the last widget is taken off it.
pub fn serve() {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
    }
    let arguments = profile::arguments(std::env::args().skip(1));
    let Some(directory) = profile::directory(arguments.profile) else { return };
    let Ok(device) = lumenna_ffi::Device::new(directory.display().to_string()) else { return };
    let shared = Arc::new(Shared { device, state: Mutex::new(State::default()), wake: Condvar::new() });
    // Registered first: whoever started this process waits for the class, not for the board.
    let factory: IClassFactory = Factory(Arc::clone(&shared)).into();
    let Ok(cookie) = (unsafe { CoRegisterClassObject(&CLASS, &factory, CLSCTX_LOCAL_SERVER, REGCLS_MULTIPLEUSE) }) else {
        return;
    };
    // The widgets already on the board, from before this process started.
    if let Ok(infos) = WidgetManager::GetDefault().and_then(|manager| manager.GetWidgetInfos()) {
        for context in infos.iter().flatten().filter_map(|info| info.WidgetContext().ok()) {
            let _ = shared.hold(&context, context.IsActive().unwrap_or(false));
        }
    }
    shared.run();
    unsafe {
        let _ = CoRevokeClassObject(cookie);
    }
}
