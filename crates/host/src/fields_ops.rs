//! What the forms offer that only the store knows: the projects to choose from, and when a
//! new block starts.

use crate::Lumenna;
use crate::actions::Choice;

#[cfg_attr(feature = "uniffi", uniffi::export)]
impl Lumenna {
    /// The projects the task form's Project field offers, in tree order: every one not
    /// archived, `id` its name as `task_edit` takes it.
    ///
    /// # Errors
    ///
    /// If the store cannot be read.
    pub fn project_options(&self) -> crate::Result<Vec<Choice>> {
        self.with(|store| {
            let snapshot = crate::repaired(store);
            Ok(crate::tree::in_tree_order(&snapshot)
                .into_iter()
                .filter(|(project, _)| !project.archived)
                .map(|(project, depth)| Choice { id: project.name.clone(), title: project.name.clone(), depth, ..Choice::default() })
                .collect())
        })
    }
}

#[cfg_attr(feature = "uniffi", uniffi::export)]
impl Lumenna {
    /// When a new block on `date` (a date phrase, today when absent) starts unless the person
    /// says otherwise, `HH:MM`: today, the next whole hour, never before the day starts; any
    /// other day, when the day starts. A fixed 09:00 was over before most of the day was.
    ///
    /// # Errors
    ///
    /// If `date` cannot be read.
    pub fn new_block_start(&self, date: Option<String>) -> crate::Result<String> {
        let now = jiff::Zoned::now();
        let day = crate::resolve::date(date.as_deref(), &now)?;
        let day_start = self.with(|store| Ok(crate::repaired(store).settings.day_window.0))?;
        let start = if day == now.date() {
            // The hour after this one, or the last hour of the day once there is no next one
            // today.
            let next = jiff::civil::time((now.hour() + 1).min(23), 0, 0, 0);
            next.max(day_start)
        } else {
            day_start
        };
        Ok(crate::words::time_text(start))
    }
}
