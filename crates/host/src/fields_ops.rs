//! What the forms offer that only the store knows: the projects to choose from, and when a
//! new block starts.

use crate::Lumenna;
use crate::actions::Choice;

impl Lumenna {
    /// The projects the task form's Project field offers, in tree order: every one not
    /// archived, `id` its name as `task_edit` takes it.
    ///
    /// # Errors
    ///
    /// If the store cannot be read.
    pub fn project_options(&self) -> crate::Result<Vec<Choice>> {
        self.with(|store| Ok(crate::tree::project_options(&crate::repaired(store))))
    }
}

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
        self.with(|store| lumenna_records::fields::new_block_start(&crate::repaired(store), date.as_deref(), &now))
    }
}
