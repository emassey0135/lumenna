//! What screen readers are told beyond what the stock controls report, through Dynamic
//! Annotation (`IAccPropServices`) — Microsoft's way of adding to a standard control's
//! accessibility without writing a provider for it, which is the whole point of using stock
//! controls.
//!
//! Two things only: a control's name, where no label beside it gives one; and the status
//! line as a live region, which is how a change is announced: a screen reader does not
//! notice a change it did not cause by moving focus. The live region is the documented Win32
//! route — `LiveSetting` on the control, then `EVENT_OBJECT_LIVEREGIONCHANGED` — which NVDA,
//! JAWS and Narrator all read.

use std::cell::OnceCell;

use windows::Win32::Foundation::HWND;
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance};
use windows::Win32::System::Variant::{VARIANT, VARIANT_0, VARIANT_0_0, VARIANT_0_0_0, VT_I4};
use windows::Win32::UI::Accessibility::{
    CLSID_AccPropServices, IAccPropServices, LiveSetting_Property_GUID, NotifyWinEvent,
    PROPID_ACC_DESCRIPTION, PROPID_ACC_NAME,
};
use windows::Win32::UI::WindowsAndMessaging::{CHILDID_SELF, EVENT_OBJECT_LIVEREGIONCHANGED, OBJID_CLIENT};
use windows::core::HSTRING;

/// UI Automation's `Polite`: said after what the screen reader is saying now — the row focus
/// has just moved to — rather than cutting it off.
const POLITE: i32 = 1;

thread_local! {
    static SERVICES: OnceCell<Option<IAccPropServices>> = const { OnceCell::new() };
}

fn with_services(act: impl FnOnce(&IAccPropServices)) {
    SERVICES.with(|cell| {
        let services = cell.get_or_init(|| unsafe {
            CoCreateInstance(&CLSID_AccPropServices, None, CLSCTX_INPROC_SERVER).ok()
        });
        if let Some(services) = services {
            act(services);
        }
    });
}

/// Names a control: what a screen reader calls it.
pub fn set_name(hwnd: HWND, name: &str) {
    with_services(|services| unsafe {
        let _ = services.SetHwndPropStr(
            hwnd,
            OBJID_CLIENT.0 as u32,
            CHILDID_SELF,
            PROPID_ACC_NAME,
            &HSTRING::from(name),
        );
    });
}

/// Describes a control: what a screen reader says after its name and role, such as what a
/// field takes. The Mac's hints, in Windows' terms.
pub fn set_description(hwnd: HWND, description: &str) {
    with_services(|services| unsafe {
        let _ = services.SetHwndPropStr(
            hwnd,
            OBJID_CLIENT.0 as u32,
            CHILDID_SELF,
            PROPID_ACC_DESCRIPTION,
            &HSTRING::from(description),
        );
    });
}

/// Makes a static control a polite live region: whenever [`changed`] is called, its text is
/// read out.
pub fn make_live(hwnd: HWND) {
    let value = VARIANT {
        Anonymous: VARIANT_0 {
            Anonymous: std::mem::ManuallyDrop::new(VARIANT_0_0 {
                vt: VT_I4,
                Anonymous: VARIANT_0_0_0 { lVal: POLITE },
                ..Default::default()
            }),
        },
    };
    with_services(|services| unsafe {
        let _ = services.SetHwndProp(hwnd, OBJID_CLIENT.0 as u32, CHILDID_SELF, LiveSetting_Property_GUID, &value);
    });
}

/// Says that a live region's text changed, so it is read.
pub fn changed(hwnd: HWND) {
    unsafe { NotifyWinEvent(EVENT_OBJECT_LIVEREGIONCHANGED, hwnd, OBJID_CLIENT.0, CHILDID_SELF as i32) };
}

/// Lets go of what was set on a control, before it is destroyed.
pub fn clear(hwnd: HWND) {
    with_services(|services| unsafe {
        let _ = services.ClearHwndProps(
            hwnd,
            OBJID_CLIENT.0 as u32,
            CHILDID_SELF,
            &[PROPID_ACC_NAME, PROPID_ACC_DESCRIPTION, LiveSetting_Property_GUID],
        );
    });
}
