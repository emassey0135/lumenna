//! How often each task's due date was moved later, read from the core document's history:
//! the one fact about a task the records themselves forget. Repeatedly pushing a date back
//! says what putting it in a block and leaving it says (`core::history`), for someone who
//! plans by dates rather than blocks.
//!
//! A due date is written as a new `due` map on the task, its `date` inside, in one change
//! (`Writer::set_nested`), so each change that makes one is a new due date. A move later counts
//! unless the same change completed something: completing a recurring task moves its date to
//! the next occurrence, which is the task done, not put off. The tally is kept as the document
//! grows, each change read once; changes are taken in the order this device has them, so two
//! devices moving one date at once may count it differently.

use std::collections::{BTreeMap, HashMap};

use automerge::legacy::{Key, ObjectId, OpId, OpType};
use automerge::{AutoCommit, ChangeHash, ObjId, ROOT, ReadDoc, ScalarValue, Value};

/// The running count, and what it needs to read the next changes.
#[derive(Debug, Default)]
pub(crate) struct Postponed {
    /// The heads already read.
    heads: Vec<ChangeHash>,
    /// Each task's map, by the operation that made it.
    tasks: HashMap<OpId, String>,
    /// Each due map made, by the operation that made it, to its task.
    dues: HashMap<OpId, String>,
    /// Each task's due date as last read.
    last: HashMap<String, String>,
    /// How often each task's date was moved later.
    counts: BTreeMap<String, u32>,
}

impl Postponed {
    /// Reads the changes `doc` has gained since last time, and returns every count.
    pub(crate) fn update(&mut self, doc: &mut AutoCommit) -> BTreeMap<String, u32> {
        let (Some(tasks), Some(completions)) = (collection(doc, "tasks"), collection(doc, "completions")) else {
            return self.counts.clone();
        };
        for change in doc.get_changes(&self.heads) {
            let change = change.decode();
            let first = change.start_op.get();
            let mut completed = false;
            let mut dated: Vec<(String, String)> = Vec::new();
            for (index, op) in change.operations.iter().enumerate() {
                let id = OpId(first + index as u64, change.actor_id.clone());
                let key = match &op.key {
                    Key::Map(key) => key.as_str(),
                    Key::Seq(_) => continue,
                };
                match (&op.obj, &op.action) {
                    (obj, OpType::Make(_)) if *obj == completions => completed = true,
                    (obj, OpType::Make(_)) if *obj == tasks => {
                        self.tasks.insert(id, key.to_owned());
                    }
                    (ObjectId::Id(obj), action) if key == "due" => {
                        let Some(task) = self.tasks.get(obj).cloned() else { continue };
                        match action {
                            OpType::Make(_) => {
                                self.dues.insert(id, task);
                            }
                            OpType::Delete => {
                                self.last.remove(&task);
                            }
                            _ => {}
                        }
                    }
                    (ObjectId::Id(obj), OpType::Put(ScalarValue::Str(date))) if key == "date" => {
                        if let Some(task) = self.dues.get(obj) {
                            dated.push((task.clone(), date.to_string()));
                        }
                    }
                    _ => {}
                }
            }
            for (task, date) in dated {
                // ISO dates order as text does.
                let later = self.last.get(&task).is_some_and(|before| date.as_str() > before.as_str());
                if later && !completed {
                    *self.counts.entry(task.clone()).or_default() += 1;
                }
                self.last.insert(task, date);
            }
        }
        self.heads = doc.get_heads();
        self.counts.clone()
    }
}

/// A root collection's object, as a change's operations name it.
fn collection(doc: &AutoCommit, name: &str) -> Option<ObjectId> {
    match doc.get(ROOT, name).ok()?? {
        (Value::Object(_), ObjId::Id(counter, actor, _)) => Some(ObjectId::Id(OpId(counter, actor))),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use automerge::transaction::Transactable;
    use automerge::ObjType;

    /// A document shaped as the core's: a task whose due date is replaced as the store does.
    fn with_task() -> (AutoCommit, ObjId, ObjId) {
        let mut doc = AutoCommit::new();
        let tasks = doc.put_object(ROOT, "tasks", ObjType::Map).unwrap();
        let completions = doc.put_object(ROOT, "completions", ObjType::Map).unwrap();
        let task = doc.put_object(&tasks, "t1", ObjType::Map).unwrap();
        doc.commit();
        (doc, task, completions)
    }

    fn due(doc: &mut AutoCommit, task: &ObjId, date: &str) {
        let due = doc.put_object(task, "due", ObjType::Map).unwrap();
        doc.put(&due, "date", date).unwrap();
        doc.commit();
    }

    #[test]
    fn a_date_moved_later_counts_and_one_moved_earlier_or_set_first_does_not() {
        let (mut doc, task, _) = with_task();
        let mut postponed = Postponed::default();
        due(&mut doc, &task, "2026-10-20");
        due(&mut doc, &task, "2026-10-22");
        due(&mut doc, &task, "2026-10-21");
        assert_eq!(postponed.update(&mut doc).get("t1"), Some(&1));
        due(&mut doc, &task, "2026-10-25");
        assert_eq!(postponed.update(&mut doc).get("t1"), Some(&2), "read on from where it left off");
    }

    #[test]
    fn a_date_moved_on_by_completing_it_is_not_put_off() {
        let (mut doc, task, completions) = with_task();
        due(&mut doc, &task, "2026-10-20");
        doc.put_object(&completions, "c1", ObjType::Map).unwrap();
        due(&mut doc, &task, "2026-10-27");
        let counts = Postponed::default().update(&mut doc);
        assert_eq!(counts.get("t1"), None);
    }
}
