//! The app beside its store: the app's host effects answered by the host, in one loop.
//!
//! A [`Session`] holds the Crux core and the open store. An event goes in; every effect the
//! app asks of the host is answered here, at once, on the caller's thread, and what is left is
//! the shell's: draw, and say. That is all a shell is handed ([`Shell`]).
//!
//! One loop for every shell that runs the app in the same process as its store: the web
//! worker, which has no threads for Crux's middleware, and the desktop apps, which link it.
//! Operations are milliseconds against local SQLite, so answering inline never holds a frame.
//! [`answer`] is the whole of what the host does for the app, and middleware for a shell that
//! wants the host off its thread wraps the same function.

use std::sync::Arc;

use crux_core::Core;
use crux_core::Request;
use jiff::Zoned;
use lumenna_app::{Arrival, Effect, Event, HostAnswer, HostRequest, ViewModel, Watch};
use lumenna_host::Lumenna;
use lumenna_store::ChangeHash;

/// What the shell does, once an event has run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Shell {
    /// Draw [`Session::view`] again.
    Render,
    /// Say this, once focus is where the view model puts it.
    Announce(String),
}

/// The store as it stands now, and the heads it was read at.
///
/// # Errors
///
/// If the store cannot be read.
pub fn arrival(lumenna: &Lumenna) -> lumenna_host::Result<(Arrival, Vec<ChangeHash>)> {
    let (snapshot, heads) = lumenna.snapshot()?;
    Ok((Arrival { snapshot: Arc::new(snapshot), now: Zoned::now() }, heads))
}

/// What the host answers to one of the app's requests: the whole of the host's part.
pub fn answer(lumenna: &Lumenna, request: HostRequest) -> HostAnswer {
    let wrote = |result: lumenna_host::Result<lumenna_records::Change>| {
        let after = arrival(lumenna).ok().map(|(arrival, _)| arrival);
        HostAnswer::Wrote(Box::new(result), after)
    };
    match request {
        HostRequest::Act { action, answer } => wrote(lumenna.act(action, answer)),
        HostRequest::AddTask { text } => wrote(lumenna.add_task(&text)),
        HostRequest::Undo => wrote(lumenna.undo()),
        HostRequest::Redo => wrote(lumenna.redo()),
        HostRequest::Choices { action } => HostAnswer::Offered(lumenna.choices(action)),
    }
}

/// The app and its store, together.
pub struct Session {
    core: Core<lumenna_app::Lumenna>,
    lumenna: Arc<Lumenna>,
    /// Every watch the app holds open, answered whenever the store moves.
    watching: Vec<Request<Watch>>,
    /// The heads last sent, and the minute: a tick sends again only when either moved.
    sent: Option<(Vec<ChangeHash>, i8)>,
}

impl Session {
    /// The app over this store, started: it is watching the store, and has it.
    #[must_use]
    pub fn new(lumenna: Arc<Lumenna>) -> (Self, Vec<Shell>) {
        let mut session = Self { core: Core::new(), lumenna, watching: Vec::new(), sent: None };
        let shell = session.send(Event::Start);
        (session, shell)
    }

    /// Runs an event, answering everything it asks of the host, and says what is left for the
    /// shell.
    pub fn send(&mut self, event: Event) -> Vec<Shell> {
        let effects = self.core.process_event(event);
        self.run(effects)
    }

    /// Looks for a change made elsewhere — `lum`, another app, a sync — or a new minute, and
    /// tells the app if there is one. A shell calls this on a timer, or when told the store
    /// changed.
    pub fn tick(&mut self) -> Vec<Shell> {
        let Ok((arrival, heads)) = arrival(&self.lumenna) else { return Vec::new() };
        let minute = arrival.now.minute();
        if self.sent.as_ref().is_some_and(|(sent, at)| *sent == heads && *at == minute) {
            return Vec::new();
        }
        self.sent = Some((heads, minute));
        let mut effects = Vec::new();
        let mut watching = std::mem::take(&mut self.watching);
        for handle in &mut watching {
            if let Ok(more) = self.core.resolve(handle, arrival.clone()) {
                effects.extend(more);
            }
        }
        self.watching = watching;
        self.run(effects)
    }

    /// What to draw.
    #[must_use]
    pub fn view(&self) -> ViewModel {
        self.core.view()
    }

    fn run(&mut self, mut effects: Vec<Effect>) -> Vec<Shell> {
        let mut shell = Vec::new();
        while !effects.is_empty() {
            let mut next = Vec::new();
            for effect in effects {
                match effect {
                    Effect::Render(_) => {
                        if shell.last() != Some(&Shell::Render) {
                            shell.push(Shell::Render);
                        }
                    }
                    Effect::Announce(request) => shell.push(Shell::Announce(request.operation.text)),
                    Effect::Watch(mut request) => {
                        if let Ok((arrival, heads)) = arrival(&self.lumenna) {
                            self.sent = Some((heads, arrival.now.minute()));
                            if let Ok(more) = self.core.resolve(&mut request, arrival) {
                                next.extend(more);
                            }
                        }
                        self.watching.push(request);
                    }
                    Effect::Host(request) => {
                        let (operation, mut handle) = request.split();
                        let found = answer(&self.lumenna, operation);
                        if let HostAnswer::Wrote(_, Some(_)) = &found
                            && let Ok((_, heads)) = self.lumenna.snapshot()
                        {
                            // The app has the store as this write left it.
                            self.sent = Some((heads, Zoned::now().minute()));
                        }
                        if let Ok(more) = self.core.resolve(&mut handle, found) {
                            next.extend(more);
                        }
                    }
                }
            }
            effects = next;
        }
        // Drawing once, after everything else, is enough.
        if let Some(at) = shell.iter().position(|s| *s == Shell::Render) {
            shell.remove(at);
            shell.insert(0, Shell::Render);
        }
        shell
    }
}
