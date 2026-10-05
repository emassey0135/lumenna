//! DNS-SD through the system's own responder: macOS and iOS.
//!
//! The C API in `<dns_sd.h>`, part of libSystem. It talks to `mDNSResponder`, which is
//! Bonjour, so on iOS it needs no multicast entitlement — only the local network permission
//! and the service types listed under `NSBonjourServices`. On macOS it shares the one
//! responder every other app uses rather than opening port 5353 itself.
//!
//! Each `DNSServiceRef` is used by one thread only, as the API requires: the browse and its
//! resolves on the browsing thread, the registration under a mutex.

use std::collections::{HashMap, HashSet};
use std::ffi::{CStr, CString, c_char, c_int, c_void};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::txt::{Announcement, instance};
use super::{Heard, Hearing};

type ServiceRef = *mut c_void;
type Flags = u32;
type ErrorCode = i32;

/// `kDNSServiceFlagsAdd`: in a browse reply, found rather than lost.
const FLAG_ADD: Flags = 0x2;
/// `kDNSServiceErr_NoError`.
const NO_ERROR: ErrorCode = 0;
/// How long a resolve gets before the instance is left for the next announcement.
const RESOLVE_WAIT: Duration = Duration::from_secs(3);
/// How often the browsing thread looks up to see whether it should stop.
const STOP_CHECK: Duration = Duration::from_millis(250);

type BrowseReply = extern "C" fn(
    ServiceRef,
    Flags,
    u32,
    ErrorCode,
    *const c_char,
    *const c_char,
    *const c_char,
    *mut c_void,
);
type ResolveReply = extern "C" fn(
    ServiceRef,
    Flags,
    u32,
    ErrorCode,
    *const c_char,
    *const c_char,
    u16,
    u16,
    *const u8,
    *mut c_void,
);
type RegisterReply =
    extern "C" fn(ServiceRef, Flags, ErrorCode, *const c_char, *const c_char, *const c_char, *mut c_void);

unsafe extern "C" {
    fn DNSServiceRegister(
        service: *mut ServiceRef,
        flags: Flags,
        interface: u32,
        name: *const c_char,
        regtype: *const c_char,
        domain: *const c_char,
        host: *const c_char,
        port: u16,
        txt_len: u16,
        txt: *const c_void,
        reply: Option<RegisterReply>,
        context: *mut c_void,
    ) -> ErrorCode;
    fn DNSServiceUpdateRecord(
        service: ServiceRef,
        record: *mut c_void,
        flags: Flags,
        len: u16,
        data: *const c_void,
        ttl: u32,
    ) -> ErrorCode;
    fn DNSServiceBrowse(
        service: *mut ServiceRef,
        flags: Flags,
        interface: u32,
        regtype: *const c_char,
        domain: *const c_char,
        reply: BrowseReply,
        context: *mut c_void,
    ) -> ErrorCode;
    fn DNSServiceResolve(
        service: *mut ServiceRef,
        flags: Flags,
        interface: u32,
        name: *const c_char,
        regtype: *const c_char,
        domain: *const c_char,
        reply: ResolveReply,
        context: *mut c_void,
    ) -> ErrorCode;
    fn DNSServiceRefSockFD(service: ServiceRef) -> c_int;
    fn DNSServiceProcessResult(service: ServiceRef) -> ErrorCode;
    fn DNSServiceRefDeallocate(service: ServiceRef);
}

fn failed(what: &str, code: ErrorCode) -> std::io::Error {
    std::io::Error::other(format!("{what} failed with DNS-SD error {code}"))
}

/// Waits until the service's socket has something to read, or `timeout` passes.
fn readable(service: ServiceRef, timeout: Duration) -> bool {
    // SAFETY: `service` is a live reference owned by the caller.
    let fd = unsafe { DNSServiceRefSockFD(service) };
    let mut poll = libc::pollfd { fd, events: libc::POLLIN, revents: 0 };
    let millis = c_int::try_from(timeout.as_millis()).unwrap_or(c_int::MAX);
    // SAFETY: one valid pollfd, for the length given.
    unsafe { libc::poll(&raw mut poll, 1, millis) > 0 }
}

fn text(pointer: *const c_char) -> String {
    if pointer.is_null() {
        return String::new();
    }
    // SAFETY: the API passes NUL-terminated strings valid for the callback's duration.
    unsafe { CStr::from_ptr(pointer) }.to_string_lossy().into_owned()
}

/// What browse replies leave for the browsing thread to act on after each
/// `DNSServiceProcessResult`.
#[derive(Default)]
struct Browsing {
    replies: Vec<(bool, String, String, String, u32)>,
}

extern "C" fn on_browse(
    _service: ServiceRef,
    flags: Flags,
    interface: u32,
    error: ErrorCode,
    name: *const c_char,
    regtype: *const c_char,
    domain: *const c_char,
    context: *mut c_void,
) {
    if error != NO_ERROR {
        return;
    }
    // SAFETY: `context` is the `Browsing` the browsing thread owns, and only that thread
    // runs this callback, inside `DNSServiceProcessResult`.
    let state = unsafe { &mut *context.cast::<Browsing>() };
    state.replies.push((flags & FLAG_ADD != 0, text(name), text(regtype), text(domain), interface));
}

/// What one resolve found.
#[derive(Default)]
struct Resolving {
    txt: Option<Vec<u8>>,
}

extern "C" fn on_resolve(
    _service: ServiceRef,
    _flags: Flags,
    _interface: u32,
    error: ErrorCode,
    _fullname: *const c_char,
    _host: *const c_char,
    _port: u16,
    txt_len: u16,
    txt: *const u8,
    context: *mut c_void,
) {
    if error != NO_ERROR || txt.is_null() {
        return;
    }
    // SAFETY: `context` is the resolving thread's own `Resolving`, as for `on_browse`, and
    // `txt` is `txt_len` bytes valid for the callback's duration.
    let (state, record) = unsafe {
        (&mut *context.cast::<Resolving>(), std::slice::from_raw_parts(txt, usize::from(txt_len)))
    };
    state.txt = Some(record.to_vec());
}

/// Resolves one instance to its TXT record, waiting a few seconds at most.
fn resolve(name: &str, regtype: &str, domain: &str, interface: u32) -> Option<Announcement> {
    let (name, regtype, domain) =
        (CString::new(name).ok()?, CString::new(regtype).ok()?, CString::new(domain).ok()?);
    let state = Box::into_raw(Box::new(Resolving::default()));
    let mut service: ServiceRef = std::ptr::null_mut();
    // SAFETY: valid strings and an out-pointer; `state` outlives the reference, freed below.
    let code = unsafe {
        DNSServiceResolve(
            &raw mut service,
            0,
            interface,
            name.as_ptr(),
            regtype.as_ptr(),
            domain.as_ptr(),
            on_resolve,
            state.cast(),
        )
    };
    if code == NO_ERROR {
        let deadline = Instant::now() + RESOLVE_WAIT;
        // SAFETY: `state` is ours until freed below; the callback only runs inside
        // `DNSServiceProcessResult`, on this thread.
        while unsafe { (*state).txt.is_none() } {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() || !readable(service, left) {
                break;
            }
            // SAFETY: a live reference, used on its own thread.
            if unsafe { DNSServiceProcessResult(service) } != NO_ERROR {
                break;
            }
        }
        // SAFETY: allocated by the successful call above, released once.
        unsafe { DNSServiceRefDeallocate(service) };
    }
    // SAFETY: from `Box::into_raw` above; the reference that used it is gone.
    let state = unsafe { Box::from_raw(state) };
    state.txt.as_deref().and_then(Announcement::from_record)
}

/// A registration, owned under the responder's mutex.
struct Registration {
    service: ServiceRef,
    port: u16,
}

// SAFETY: a `DNSServiceRef` may move between threads; it is used by one at a time, under
// the mutex that holds it.
unsafe impl Send for Registration {}

impl Drop for Registration {
    fn drop(&mut self) {
        // SAFETY: allocated by `DNSServiceRegister`, released once. Releasing it withdraws
        // the announcement.
        unsafe { DNSServiceRefDeallocate(self.service) };
    }
}

/// Browses one service type and announces this endpoint under it.
pub(crate) struct Responder {
    regtype: CString,
    registration: Mutex<Option<Registration>>,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Responder {
    /// Starts browsing `service_type`, telling `heard` about everything found and lost.
    pub(crate) fn start(service_type: &str, heard: Hearing) -> std::io::Result<Self> {
        let regtype = CString::new(service_type).map_err(std::io::Error::other)?;
        let browsing = Box::into_raw(Box::<Browsing>::default());
        let mut service: ServiceRef = std::ptr::null_mut();
        // SAFETY: valid strings and an out-pointer; `browsing` outlives the reference, which
        // the browsing thread releases before freeing it.
        let code = unsafe {
            DNSServiceBrowse(
                &raw mut service,
                0,
                0,
                regtype.as_ptr(),
                std::ptr::null(),
                on_browse,
                browsing.cast(),
            )
        };
        if code != NO_ERROR {
            // SAFETY: never handed to a live reference.
            drop(unsafe { Box::from_raw(browsing) });
            return Err(failed("browsing", code));
        }
        let stop = Arc::new(AtomicBool::new(false));
        let stopping = Arc::clone(&stop);
        // Raw pointers are not `Send`; the thread is their only user from here on.
        let (service, browsing) = (service as usize, browsing as usize);
        let thread = std::thread::Builder::new().name("lumenna-dnssd".to_owned()).spawn(move || {
            let (service, browsing) = (service as ServiceRef, browsing as *mut Browsing);
            // The interfaces each instance was heard on. Bonjour reports an instance once per
            // interface — Wi-Fi, a VPN, a virtual machine's network — so it is resolved once,
            // when first heard, and lost only when gone from every interface.
            let mut on: HashMap<String, HashSet<u32>> = HashMap::new();
            while !stopping.load(Ordering::Relaxed) {
                if readable(service, STOP_CHECK) {
                    // SAFETY: a live reference, used only by this thread.
                    if unsafe { DNSServiceProcessResult(service) } != NO_ERROR {
                        break;
                    }
                }
                // SAFETY: written only by `on_browse`, which runs inside the call above.
                let replies = std::mem::take(unsafe { &mut (*browsing).replies });
                for (added, name, regtype, domain, interface) in replies {
                    if added {
                        let interfaces = on.entry(name.clone()).or_default();
                        if interfaces.insert(interface) && interfaces.len() == 1 {
                            // Resolved on a thread of its own: an announcement whose device
                            // has gone times out there, without holding up the rest.
                            let heard = Arc::clone(&heard);
                            let _ = std::thread::Builder::new()
                                .name("lumenna-dnssd-resolve".to_owned())
                                .spawn(move || {
                                    if let Some(announced) = resolve(&name, &regtype, &domain, 0) {
                                        heard(Heard::Found(name, announced));
                                    }
                                });
                        }
                    } else if let Some(interfaces) = on.get_mut(&name) {
                        interfaces.remove(&interface);
                        if interfaces.is_empty() {
                            on.remove(&name);
                            heard(Heard::Lost(name));
                        }
                    }
                }
            }
            // SAFETY: the reference first, then the state its callbacks wrote to.
            unsafe {
                DNSServiceRefDeallocate(service);
                drop(Box::from_raw(browsing));
            }
        })?;
        Ok(Self { regtype, registration: Mutex::new(None), stop, thread: Some(thread) })
    }

    /// Announces `announced`, replacing any earlier announcement.
    pub(crate) fn announce(&self, announced: &Announcement) -> std::io::Result<()> {
        let Some(port) = announced.addrs.first().map(std::net::SocketAddr::port) else {
            return Ok(());
        };
        let record = announced.to_record();
        let len = u16::try_from(record.len()).map_err(std::io::Error::other)?;
        let mut registration =
            self.registration.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(current) = registration.as_ref().filter(|r| r.port == port) {
            // Same port, new addresses: only the TXT record changes.
            // SAFETY: a live registration, under its mutex; `record` is `len` bytes.
            let code = unsafe {
                DNSServiceUpdateRecord(current.service, std::ptr::null_mut(), 0, len, record.as_ptr().cast(), 0)
            };
            return if code == NO_ERROR { Ok(()) } else { Err(failed("updating the announcement", code)) };
        }
        *registration = None;
        let name = CString::new(instance(&announced.id)).map_err(std::io::Error::other)?;
        let mut service: ServiceRef = std::ptr::null_mut();
        // SAFETY: valid strings, an out-pointer, and `len` bytes of record. No reply is
        // asked for, which the API allows; the responder picks the host's own name.
        let code = unsafe {
            DNSServiceRegister(
                &raw mut service,
                0,
                0,
                name.as_ptr(),
                self.regtype.as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                port.to_be(),
                len,
                record.as_ptr().cast(),
                None,
                std::ptr::null_mut(),
            )
        };
        if code != NO_ERROR {
            return Err(failed("announcing", code));
        }
        *registration = Some(Registration { service, port });
        Ok(())
    }
}

impl Drop for Responder {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        // The registration goes with the mutex, withdrawing the announcement.
    }
}
