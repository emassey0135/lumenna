//! The shortcuts that work from anywhere: showing the window, and quick add over
//! whatever is in front.
//!
//! On Wayland an app cannot grab keys for itself; it asks the desktop, through the
//! GlobalShortcuts portal, which shows the person what is asked for and lets them choose the
//! keys — once, the first time — and change them later in the desktop's own settings. So the
//! app proposes the core's keys (`HotKind::standard`: Control+Alt+Shift+L and K, as on
//! Windows), and the desktop decides.
//!
//! Through `ashpd`, on a D-Bus connection of its own. A program not run from a sandbox has to
//! say who it is before anything else on that connection asks a portal something, or the
//! portal refuses it ("an app id is required"), and the portal knows an app by its desktop
//! entry, which has to be installed (`data/`).
//!
//! On X11 a desktop without the portal (Xfce, GNOME before 48, most window managers) lets an
//! app grab keys itself: the same keys are grabbed on the root window, on an X connection of
//! the app's own (`grab_on_x11`). Anywhere else without it, or where another program holds
//! the keys, the window and the tray are the ways back, and that is said once.

use std::rc::Rc;

use ashpd::desktop::global_shortcuts::{GlobalShortcuts, NewShortcut};
use futures_util::StreamExt;
use gtk::glib;
use gtk::prelude::*;

/// What a shortcut does, its name and its keys until the person changes them: the core's, the
/// same two every desktop app has.
pub use lumenna_app::hotkeys::HotKind as Kind;

/// The keys proposed for `kind`, in the portal's own spelling: Control+Alt+Shift with its
/// letter, as `HotKind::standard` has them.
fn preferred(kind: Kind) -> String {
    let keys = kind.standard();
    let mut spelled = String::new();
    for (on, name) in [(keys.control, "CTRL+"), (keys.alt, "ALT+"), (keys.shift, "SHIFT+"), (keys.windows, "LOGO+")] {
        if on {
            spelled.push_str(name);
        }
    }
    spelled.push(kind.letter().to_ascii_lowercase());
    spelled
}

/// Asks the desktop for the shortcuts, and calls `pressed` on the main thread whenever one is
/// pressed. Says through `say` if they cannot be had. `parent` is the window to put the
/// desktop's question over, when it is showing.
pub fn start(
    application_id: String,
    parent: Option<gtk::Window>,
    pressed: impl Fn(Kind) + 'static,
    say: impl Fn(String) + 'static,
) {
    let pressed: Rc<dyn Fn(Kind)> = Rc::new(pressed);
    glib::spawn_future_local(async move {
        let Err(error) = run(&application_id, parent, Rc::clone(&pressed)).await else { return };
        let on_x11 = gtk::gdk::Display::default().is_some_and(|display| display.type_().name() == "GdkX11Display");
        let error = match on_x11 {
            true => match grab_on_x11(pressed) {
                Ok(()) => return,
                Err(error) => error,
            },
            false => error.to_string(),
        };
        say(lumenna_records::words::hotkeys_unavailable(&error));
    });
}

/// Grabs the shortcuts' keys on the X root window, with or without Caps Lock and Num Lock on,
/// and hands each press to `pressed` on the main thread. Refused if another program holds any.
fn grab_on_x11(pressed: Rc<dyn Fn(Kind)>) -> Result<(), String> {
    use x11rb::connection::Connection;
    use x11rb::protocol::Event;
    use x11rb::protocol::xproto::{ConnectionExt, GrabMode, ModMask};

    let failed = |error: &dyn std::fmt::Display| format!("X did not answer: {error}");
    let (connection, screen) = x11rb::connect(None).map_err(|e| failed(&e))?;
    let setup = connection.setup();
    let root = setup.roots[screen].root;
    let (first, last) = (setup.min_keycode, setup.max_keycode);
    let mapping = connection.get_keyboard_mapping(first, last - first + 1).map_err(|e| failed(&e))?.reply().map_err(|e| failed(&e))?;
    let per_code = usize::from(mapping.keysyms_per_keycode.max(1));
    // The keysyms of the keys proposed to the portal: each letter in lower case, which is its
    // keysym.
    let keysym = |kind: Kind| u32::from(kind.letter().to_ascii_lowercase());
    let mut codes = Vec::new();
    for kind in Kind::ALL {
        let code = mapping
            .keysyms
            .chunks(per_code)
            .position(|syms| syms.contains(&keysym(kind)))
            .and_then(|at| u8::try_from(at).ok())
            .map(|at| first + at)
            .ok_or_else(|| "This keyboard has no key for them.".to_owned())?;
        codes.push((code, kind));
    }
    let held = ModMask::CONTROL | ModMask::M1 | ModMask::SHIFT;
    for &(code, _) in &codes {
        // Caps Lock is Lock and Num Lock is usually Mod2: either on changes what X matches.
        for locks in [ModMask::from(0u16), ModMask::LOCK, ModMask::M2, ModMask::LOCK | ModMask::M2] {
            connection
                .grab_key(true, root, held | locks, code, GrabMode::ASYNC, GrabMode::ASYNC)
                .map_err(|e| failed(&e))?
                .check()
                .map_err(|_| lumenna_app::hotkeys::taken(&Kind::ALL.map(|kind| kind.standard().describe()), false))?;
        }
    }
    connection.flush().map_err(|e| failed(&e))?;
    // Presses arrive on a thread of their own, which holds the connection and so the grabs, for
    // as long as the app runs; each is handed to the main thread.
    let (sender, mut presses) = futures_channel::mpsc::unbounded();
    std::thread::spawn(move || {
        while let Ok(event) = connection.wait_for_event() {
            if let Event::KeyPress(press) = event
                && let Some(&(_, kind)) = codes.iter().find(|(code, _)| *code == press.detail)
                && sender.unbounded_send(kind).is_err()
            {
                break;
            }
        }
    });
    glib::spawn_future_local(async move {
        while let Some(kind) = presses.next().await {
            pressed(kind);
        }
    });
    Ok(())
}

async fn run(application_id: &str, parent: Option<gtk::Window>, pressed: Rc<dyn Fn(Kind)>) -> ashpd::Result<()> {
    let id = ashpd::AppID::try_from(application_id)?;
    ashpd::register_host_app(id).await?;
    let portal = GlobalShortcuts::new().await?;
    let session = portal.create_session(Default::default()).await?;
    // Heard before binding, so a press straight after the person agrees is not missed.
    let mut activated = portal.receive_activated().await?;
    let shortcuts: Vec<NewShortcut> = Kind::ALL
        .iter()
        .map(|kind| NewShortcut::new(kind.id(), kind.title()).preferred_trigger(preferred(*kind).as_str()))
        .collect();
    let identifier = match &parent {
        Some(window) => ashpd::WindowIdentifier::from_native(window).await,
        None => None,
    };
    portal.bind_shortcuts(&session, &shortcuts, identifier.as_ref(), Default::default()).await?.response()?;
    // The connection is this app's alone, with this one session on it.
    while let Some(press) = activated.next().await {
        if let Some(kind) = Kind::ALL.into_iter().find(|kind| kind.id() == press.shortcut_id()) {
            pressed(kind);
        }
    }
    // The session lives as long as the stream is listened to: for as long as the app runs.
    drop(session);
    Ok(())
}
