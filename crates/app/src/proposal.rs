//! Replan from Now and Plan the Day: the core's proposal for the day, said in full, then made
//! as the person checks it.
//!
//! Each change starts checked. A change made alone can leave the day crowded, so after a
//! partial apply the host proposes again around what was left, and that is what is shown next;
//! the screen closes once nothing is left to propose. A refusal (the day moved since) is kept
//! on the screen, which asks for a new proposal with the same pins. Replan from Now's other
//! options each ask again with their pins, shown in place.

use crux_core::Command;
use crux_core::render::render;
use lumenna_records::LumennaError;
use lumenna_records::proposals::{DayPins, DayPlan, Replan, ReplanPins, TimeStyle};
use lumenna_records::{ActionKind, ProposalWords, proposal_words};
use serde::{Deserialize, Serialize};

use crate::host::{Arrival, HostRequest};
use crate::model::Model;
use crate::{Effect, Event, Lumenna, host, say};

/// What the person does on a proposal's screen.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub enum ProposalEvent {
    /// Check or uncheck the change at this position.
    Check {
        /// Which change.
        index: u32,
        /// Whether it is to be made.
        on: bool,
    },
    /// Make the changes checked.
    Apply,
    /// Take the other option at this position instead: Replan from Now's alone.
    Take(u32),
    /// Close the screen.
    Close,
    /// Open Replan from Now, from wherever it is asked for: an overrun warning's button.
    Replan,
}

/// Which proposal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Kind {
    /// Replan from Now.
    Replan,
    /// Plan the Day, for a day (ISO).
    PlanDay(String),
}

/// A proposal, either kind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Proposal {
    /// Replan from Now's.
    Replan(Replan),
    /// Plan the Day's.
    Day(DayPlan),
}

/// The pins a proposal is asked for with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pins {
    /// Replan from Now's.
    Replan(ReplanPins),
    /// Plan the Day's.
    Day(DayPins),
}

/// What applying made, and the proposal for the day as it now is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Applied {
    /// What was made, as said.
    pub announcement: String,
    /// Anything else worth saying.
    pub notices: Vec<String>,
    /// What is proposed next.
    pub next: Proposal,
}

impl Proposal {
    /// What it says on showing: the whole proposal in a sentence or two.
    #[must_use]
    pub fn spoken(&self) -> String {
        let (announcement, notices) = match self {
            Self::Replan(r) => (&r.announcement, &r.notices),
            Self::Day(d) => (&d.announcement, &d.notices),
        };
        std::iter::once(announcement.as_str())
            .chain(notices.iter().map(String::as_str))
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// How many changes it proposes.
    #[must_use]
    pub fn len(&self) -> usize {
        match self {
            Self::Replan(r) => r.changes.len(),
            Self::Day(d) => d.sittings.len(),
        }
    }

    /// Whether it proposes nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The pins it was proposed with.
    #[must_use]
    pub fn pins(&self) -> Pins {
        match self {
            Self::Replan(r) => Pins::Replan(r.pins.clone()),
            Self::Day(d) => Pins::Day(d.pins.clone()),
        }
    }
}

/// A proposal's screen, open.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Screen {
    /// Which.
    pub kind: Kind,
    /// What is proposed, once the host has said.
    pub proposal: Option<Proposal>,
    /// Which changes are checked.
    pub checked: Vec<bool>,
    /// Why the last apply was refused.
    pub refusal: Option<String>,
    /// Counted: a new proposal is a new list, whose first check box takes focus.
    pub shown: u32,
    /// Something on its way to the host.
    pub busy: bool,
    /// Whether anything was made since the screen opened.
    pub made: bool,
}

impl Screen {
    /// The screen for `kind`, waiting for its first proposal.
    #[must_use]
    pub fn new(kind: Kind) -> Self {
        Self { kind, proposal: None, checked: Vec::new(), refusal: None, shown: 0, busy: true, made: false }
    }

    /// Shows `proposal`: every change checked.
    pub fn show(&mut self, proposal: Proposal) {
        self.checked = vec![true; proposal.len()];
        self.proposal = Some(proposal);
        self.shown += 1;
        self.busy = false;
    }
}

/// One change, as its line says it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub struct ChangeView {
    /// The change, as a sentence in the clock asked for.
    pub sentence: String,
    /// Why, for a sitting: its suggestion's sentence.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// Whether it is to be made.
    pub checked: bool,
}

/// A proposal's screen as shown.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub struct ProposalView {
    /// Which: `replan` or `plan_day`.
    pub kind: ActionKind,
    /// Every word of the screen, the core's.
    pub words: ProposalWords,
    /// Whether a proposal has come yet.
    pub ready: bool,
    /// The whole proposal in a sentence or two.
    pub announcement: String,
    /// Each change.
    pub changes: Vec<ChangeView>,
    /// What does not fit anywhere.
    pub unfit: Vec<String>,
    /// Other ways to replan, each its sentence.
    pub others: Vec<String>,
    /// Why the last apply was refused.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal: Option<String>,
    /// Counted: a new proposal is a new list.
    pub shown: u32,
    /// Something on its way: the buttons wait.
    pub busy: bool,
}

/// The screen as shown.
#[must_use]
pub fn view(screen: &Screen) -> ProposalView {
    let kind = match screen.kind {
        Kind::Replan => ActionKind::Replan,
        Kind::PlanDay(_) => ActionKind::PlanDay,
    };
    let checked = |at: usize| screen.checked.get(at).copied().unwrap_or(true);
    let (announcement, changes, unfit, others) = match &screen.proposal {
        None => (String::new(), Vec::new(), Vec::new(), Vec::new()),
        Some(p @ Proposal::Replan(r)) => (
            p.spoken(),
            r.changes.iter().enumerate().map(|(at, c)| ChangeView { sentence: c.sentence.clone(), reason: None, checked: checked(at) }).collect(),
            r.unplaceable.clone(),
            r.alternatives.iter().map(|a| a.sentence.clone()).collect(),
        ),
        Some(p @ Proposal::Day(d)) => (
            p.spoken(),
            d.sittings
                .iter()
                .enumerate()
                .map(|(at, s)| ChangeView { sentence: s.sentence.clone(), reason: s.reason.clone(), checked: checked(at) })
                .collect(),
            d.unfit.clone(),
            Vec::new(),
        ),
    };
    ProposalView {
        kind,
        words: proposal_words(kind),
        ready: screen.proposal.is_some(),
        announcement,
        changes,
        unfit,
        others,
        refusal: screen.refusal.clone(),
        shown: screen.shown,
        busy: screen.busy,
    }
}

/// The clock a proposal is asked for in, until a shell says.
#[must_use]
pub const fn default_style() -> TimeStyle {
    TimeStyle::TwentyFourHour
}

impl Lumenna {
    /// Opens a proposal's screen and asks for its first proposal.
    pub(crate) fn propose(&self, kind: Kind, model: &mut Model) -> Command<Effect, Event> {
        model.proposal = Some(Screen::new(kind));
        model.proposal_made = false;
        self.repropose(None, model)
    }

    /// Proposes the open screen's day again with `pins`, from the store as it is: shown and
    /// said in place, or why not.
    pub(crate) fn repropose(&self, pins: Option<Pins>, model: &mut Model) -> Command<Effect, Event> {
        let style = model.style.unwrap_or(default_style());
        let (Some(store), Some(screen)) = (&model.store, &mut model.proposal) else { return Command::done() };
        let proposed = match (&screen.kind, pins) {
            (Kind::Replan, pins) => {
                let pins = match pins {
                    Some(Pins::Replan(pins)) => pins,
                    _ => lumenna_records::proposals::ReplanPins::default(),
                };
                lumenna_records::replan::propose(&store.snapshot, style, pins, &store.now, true).map(Proposal::Replan)
            }
            (Kind::PlanDay(date), pins) => {
                let pins = match pins {
                    Some(Pins::Day(pins)) => pins,
                    _ => lumenna_records::proposals::DayPins::default(),
                };
                lumenna_records::dayplan::propose(&store.snapshot, Some(date), style, &pins, &store.now).map(Proposal::Day)
            }
        };
        match proposed {
            Ok(proposal) => {
                let said = proposal.spoken();
                screen.show(proposal);
                render().and(say(said))
            }
            Err(error) => {
                screen.busy = false;
                screen.refusal = Some(error.message().to_owned());
                render().and(say(error.message()))
            }
        }
    }

    pub(crate) fn proposal(&self, event: ProposalEvent, model: &mut Model) -> Command<Effect, Event> {
        if event == ProposalEvent::Replan {
            return self.propose(Kind::Replan, model);
        }
        let Some(screen) = &mut model.proposal else { return Command::done() };
        match event {
            ProposalEvent::Check { index, on } => {
                if let Some(checked) = screen.checked.get_mut(index as usize) {
                    *checked = on;
                }
                render()
            }
            ProposalEvent::Apply => {
                let Some(proposal) = screen.proposal.clone() else { return Command::done() };
                if screen.busy {
                    return Command::done();
                }
                screen.busy = true;
                let accepted = screen.checked.iter().enumerate().filter(|(_, on)| **on).map(|(at, _)| at).collect();
                render().and(host(HostRequest::Apply { proposal, accepted }))
            }
            ProposalEvent::Take(index) => {
                let Some(Proposal::Replan(replan)) = &screen.proposal else { return Command::done() };
                let Some(other) = replan.alternatives.get(index as usize) else { return Command::done() };
                let pins = Some(Pins::Replan(other.pins.clone()));
                screen.refusal = None;
                self.repropose(pins, model)
            }
            ProposalEvent::Close => {
                model.proposal_made = screen.made;
                model.proposal = None;
                render().and(self.replanned(model))
            }
            ProposalEvent::Replan => Command::done(),
        }
    }

    pub(crate) fn applied(&self, result: Result<Applied, LumennaError>, store: Option<Arrival>, model: &mut Model) -> Command<Effect, Event> {
        if let Some(store) = store {
            model.store = Some(store);
            model.settle();
        }
        let Some(screen) = &mut model.proposal else { return render() };
        match result {
            Ok(applied) => {
                screen.made = true;
                screen.refusal = None;
                let said = std::iter::once(applied.announcement.as_str())
                    .chain(applied.notices.iter().map(String::as_str))
                    .filter(|s| !s.is_empty())
                    .collect::<Vec<_>>()
                    .join(" ");
                // Nothing left to propose: the screen closes, saying what was made.
                if applied.next.is_empty() {
                    model.proposal_made = true;
                    model.proposal = None;
                    return render().and(say(said)).and(self.replanned(model));
                }
                screen.show(applied.next);
                render().and(say(said))
            }
            // The day moved since: why is kept here, and the day as it now is proposed.
            Err(error) => {
                screen.refusal = Some(error.message().to_owned());
                let pins = screen.proposal.as_ref().map(Proposal::pins);
                let said = say(error.message());
                render().and(said).and(self.repropose(pins, model))
            }
        }
    }
}
