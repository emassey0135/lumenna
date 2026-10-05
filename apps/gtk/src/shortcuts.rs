//! The shortcuts that work from anywhere: showing the window, and quick add over
//! whatever is in front.
//!
//! On Wayland an app cannot grab keys for itself; it asks the desktop, through the
//! GlobalShortcuts portal, which shows the person what is asked for and lets them choose the
//! keys — once, the first time — and change them later in the desktop's own settings. So the
//! app proposes Control+Alt+Shift+L and K, as on Windows, and the desktop decides.
//!
//! Through `ashpd`, on a D-Bus connection of its own. A program not run from a sandbox has to
//! say who it is before anything else on that connection asks a portal something, or the
//! portal refuses it ("an app id is required"), and the portal knows an app by its desktop
//! entry, which has to be installed (`data/`). A desktop without the portal, or one that
//! refuses, leaves the window and the tray as the ways back, and says so once.

use std::rc::Rc;

use ashpd::desktop::global_shortcuts::{GlobalShortcuts, NewShortcut};
use futures_util::StreamExt;
use gtk::glib;

/// What a shortcut does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Show,
    QuickAdd,
}

impl Kind {
    const ALL: [Self; 2] = [Self::Show, Self::QuickAdd];

    fn id(self) -> &'static str {
        match self {
            Self::Show => "show",
            Self::QuickAdd => "quick-add",
        }
    }

    /// What the desktop calls it when it asks the person, and in its settings.
    fn description(self) -> &'static str {
        match self {
            Self::Show => "Show Lumenna",
            Self::QuickAdd => "Quick add a task",
        }
    }

    /// The keys proposed, in the portal's own spelling.
    fn preferred(self) -> &'static str {
        match self {
            Self::Show => "CTRL+ALT+SHIFT+l",
            Self::QuickAdd => "CTRL+ALT+SHIFT+k",
        }
    }
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
    glib::spawn_future_local(async move {
        if let Err(error) = run(&application_id, parent, Rc::new(pressed)).await {
            say(format!("The shortcuts from anywhere are not available here. {error}"));
        }
    });
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
        .map(|kind| NewShortcut::new(kind.id(), kind.description()).preferred_trigger(kind.preferred()))
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
