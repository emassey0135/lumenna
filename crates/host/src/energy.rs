//! The day's energy check-in: how the person says they are today, which the planner weighs for
//! that day alone ([`lumenna_core::energy`]). An ordinary change to the synced settings, so it
//! reaches every device and one undo takes it back.

use lumenna_core::edit;
use lumenna_core::energy::{self, CheckIn, Energy};
use lumenna_records::energy::{EnergyToday, checked_in, energy_today};

use crate::error::{LumennaError, Result};
use crate::types::Change;
use crate::{Lumenna, repaired};

impl Lumenna {
    /// Says how the person's energy is today: `low`, `usual` (or `medium`) or `high`, for the day
    /// `now` falls on (the clock's, unless given as a local date and time). Said back with what
    /// planning makes of it, and on a low day whether Replan from Now can lighten the rest of it.
    ///
    /// # Errors
    ///
    /// If `level` is none of those, or the day cannot be worked out.
    pub fn check_in_energy(&self, level: &str, now: Option<String>) -> Result<Change> {
        let now = crate::replan::now_or(now.as_deref())?;
        let level: Energy = level
            .parse()
            .map_err(|()| LumennaError::new(format!("'{level}' is not how energy is; say low, usual or high")))?;
        let day = now.date();
        self.told(|store| {
            store.load_all_years()?;
            let before = repaired(store).settings;
            let mut after = before.clone();
            after.energy_today = Some(CheckIn { date: day, level });
            let change = edit::update_settings(before, after);
            if !change.is_empty() {
                store.apply_recorded(&change)?;
            }
            let snapshot = repaired(store);
            let capacity = energy::capacity(&snapshot.settings, day, energy::work_mins(&snapshot, day)?);
            // Whether Replan from Now would take anything out: only a low day is lightened.
            let window = lumenna_core::time::minutes_of_day(snapshot.settings.day_window.1).max(24 * 60);
            let lighter = level == Energy::Low
                && lumenna_core::reflow::propose_reflow(&snapshot, day, lumenna_core::time::minutes_of_day(now.time()), window, &[])?
                    .changes
                    .iter()
                    .any(|c| matches!(c, lumenna_core::reflow::Proposed::Lighten { .. }));
            let said = checked_in(level, capacity.as_ref(), lighter);
            Ok(if change.is_empty() { Change::unchanged(said) } else { Change::announced(said, &change) })
        })
    }

    /// How the day's energy stands today, as a glance, a control or an assistant shows it.
    ///
    /// # Errors
    ///
    /// If the store cannot be read.
    pub fn energy_today(&self) -> Result<EnergyToday> {
        let today = jiff::Zoned::now().date();
        self.with(|store| Ok(energy_today(&repaired(store).settings, today)))
    }
}
