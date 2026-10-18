//! The planner, for the browser: Replan from Now and Plan the Day, each a proposal said in full
//! and applied as the person accepts it; the words of their screens; What Should I Work On?;
//! and carrying a block's short work forward from its end. Each export is one surface call,
//! with times in this browser's clock.

use lumenna_host::{
    ActionKind, Change, Choices, DayPins, DayPlan, DayPlanned, ProposalWords, ProposedSitting, Replan, ReplanChange,
    ReplanPins, Replanned, TimeStyle,
};
use tsify::Ts;
use wasm_bindgen::prelude::*;

use crate::{Core, Out, js, out};

/// Whether this browser says times on a twelve-hour clock: what its locale's `Intl` does with
/// an hour. Elsewhere (the workspace's native check), twenty-four.
fn style() -> TimeStyle {
    #[cfg(all(target_family = "wasm", target_os = "unknown"))]
    {
        use wasm_bindgen::JsValue;
        let settings = js_sys::Object::new();
        let _ = js_sys::Reflect::set(&settings, &JsValue::from_str("hour"), &JsValue::from_str("numeric"));
        let format = js_sys::Intl::DateTimeFormat::new(&js_sys::Array::new(), &settings);
        let twelve = js_sys::Reflect::get(&format.resolved_options(), &JsValue::from_str("hour12"))
            .ok()
            .and_then(|value| value.as_bool())
            .unwrap_or(false);
        if twelve {
            return TimeStyle::TwelveHour;
        }
    }
    TimeStyle::TwentyFourHour
}

#[wasm_bindgen]
impl Core {
    /// What replanning the rest of today proposes, in this browser's clock, keeping to `pins`
    /// (none at first). Nothing changes until `applyReplan`. `now` is for tests.
    pub fn replan(&self, pins: Option<Ts<ReplanPins>>, now: Option<String>) -> Out<Replan> {
        let pins = pins.map(|p| p.to_rust()).transpose()?.unwrap_or_default();
        out(self.lumenna.replan(style(), pins, now))
    }

    /// Makes the changes of `proposal` checked, and proposes again around those left.
    #[wasm_bindgen(js_name = applyReplan)]
    pub fn apply_replan(
        &self,
        proposal: Ts<Replan>,
        accepted: Vec<Ts<ReplanChange>>,
        now: Option<String>,
    ) -> Out<Replanned> {
        let accepted = accepted.into_iter().map(|c| c.to_rust()).collect::<Result<Vec<_>, _>>()?;
        out(self.lumenna.apply_replan(proposal.to_rust()?, accepted, now))
    }

    /// What planning `date` (a phrase; today when absent) proposes, in this browser's clock,
    /// keeping to `pins` (none at first). Nothing changes until `applyDayPlan`.
    #[wasm_bindgen(js_name = proposeDayPlan)]
    pub fn propose_day_plan(&self, date: Option<String>, pins: Option<Ts<DayPins>>, now: Option<String>) -> Out<DayPlan> {
        let pins = pins.map(|p| p.to_rust()).transpose()?.unwrap_or_default();
        out(self.lumenna.propose_day_plan(date, style(), pins, now))
    }

    /// Makes the sittings of `plan` checked, and plans again around those turned down.
    #[wasm_bindgen(js_name = applyDayPlan)]
    pub fn apply_day_plan(
        &self,
        plan: Ts<DayPlan>,
        accepted: Vec<Ts<ProposedSitting>>,
        now: Option<String>,
    ) -> Out<DayPlanned> {
        let accepted = accepted.into_iter().map(|s| s.to_rust()).collect::<Result<Vec<_>, _>>()?;
        out(self.lumenna.apply_day_plan(plan.to_rust()?, accepted, now))
    }

    /// Carries the rest of every sitting a block (`<series>@<date>`) left short forward: a
    /// reminder's Carry Forward.
    #[wasm_bindgen(js_name = carryRestForward)]
    pub fn carry_rest_forward(&self, block: &str) -> Out<Change> {
        out(self.lumenna.carry_rest_forward(block))
    }

    /// What to work on now, best first, each with why.
    #[wasm_bindgen(js_name = whatShouldIWorkOn)]
    pub fn what_should_i_work_on(&self, now: Option<String>) -> Out<Choices> {
        out(self.lumenna.what_should_i_work_on(now))
    }

    /// Starts on one of `whatShouldIWorkOn`'s choices now.
    #[wasm_bindgen(js_name = startOn)]
    pub fn start_on(&self, choice: &str, now: Option<String>) -> Out<Change> {
        out(self.lumenna.start_on(choice, now))
    }
}

/// Every word of Replan from Now's screen (`replan`) or Plan the Day's (`plan_day`).
#[wasm_bindgen(js_name = proposalWords)]
pub fn proposal_words(kind: Ts<ActionKind>) -> Out<ProposalWords> {
    js(&lumenna_host::proposal_words(kind.to_rust()?))
}
