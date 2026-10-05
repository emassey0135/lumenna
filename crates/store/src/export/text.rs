//! Tasks for people: Markdown, and org for the Emacs client's users.
//!
//! Both are the present state only — no trash, no history — laid out the way the app shows
//! it: projects as headings in their own tree order, tasks under them in theirs, subtasks
//! nested beneath their parents. Neither is read back; JSON is the format that round-trips.

use std::collections::{BTreeMap, BTreeSet};

use jiff::Zoned;
use jiff::civil::{Date, Time};
use lumenna_core::id::{ProjectId, TaskId};
use lumenna_core::model::{Due, Priority, Project, Task};
use lumenna_core::snapshot::Snapshot;
use lumenna_core::state::Facts;

/// The live tasks of one project, as a forest in display order.
struct Outline<'a> {
    snapshot: &'a Snapshot,
    facts: Facts<'a>,
    /// Projects in tree order, with their depth.
    projects: Vec<(&'a Project, usize)>,
    /// Tasks by the project they are in, then by parent within it.
    tasks: BTreeMap<ProjectId, BTreeMap<Option<TaskId>, Vec<&'a Task>>>,
}

impl<'a> Outline<'a> {
    fn new(snapshot: &'a Snapshot) -> Self {
        let live: BTreeSet<ProjectId> = snapshot
            .projects
            .values()
            .filter(|p| p.deleted_at.is_none())
            .map(|p| p.id)
            .collect();

        // Projects: children under parents, each list in order. A project whose parent is
        // trashed or missing is shown at the top rather than lost.
        let mut by_parent: BTreeMap<Option<ProjectId>, Vec<&Project>> = BTreeMap::new();
        for project in snapshot.projects.values().filter(|p| p.deleted_at.is_none()) {
            let parent = project.parent_id.filter(|id| live.contains(id) && *id != project.id);
            by_parent.entry(parent).or_default().push(project);
        }
        for group in by_parent.values_mut() {
            group.sort_by(|a, b| {
                // The Inbox first, as every view shows it.
                b.is_inbox.cmp(&a.is_inbox).then(a.order.cmp_with(&a.id, &b.order, &b.id))
            });
        }
        let mut projects = Vec::new();
        let mut seen = BTreeSet::new();
        walk_projects(&by_parent, None, 0, &mut seen, &mut projects);

        // Tasks: grouped by project, then by parent if the parent is live and in the same
        // project, otherwise at the project's top level.
        let mut tasks: BTreeMap<ProjectId, BTreeMap<Option<TaskId>, Vec<&Task>>> =
            BTreeMap::new();
        for task in snapshot.tasks.values().filter(|t| !t.is_deleted()) {
            let parent = task.parent_id.filter(|id| {
                snapshot
                    .tasks
                    .get(id)
                    .is_some_and(|p| !p.is_deleted() && p.project_id == task.project_id)
            });
            tasks.entry(task.project_id).or_default().entry(parent).or_default().push(task);
        }
        for children in tasks.values_mut() {
            for group in children.values_mut() {
                group.sort_by(|a, b| a.order.cmp_with(&a.id, &b.order, &b.id));
            }
        }
        Self { snapshot, facts: snapshot.facts(), projects, tasks }
    }

    /// Calls `visit` for each task of `project` with its depth, parents before children.
    fn each_task(&self, project: ProjectId, mut visit: impl FnMut(&Task, usize)) {
        let Some(children) = self.tasks.get(&project) else {
            return;
        };
        let mut seen = BTreeSet::new();
        let mut stack: Vec<(&Task, usize)> =
            children.get(&None).into_iter().flatten().rev().map(|t| (*t, 0)).collect();
        while let Some((task, depth)) = stack.pop() {
            if !seen.insert(task.id) {
                continue;
            }
            visit(task, depth);
            if let Some(under) = children.get(&Some(task.id)) {
                stack.extend(under.iter().rev().map(|t| (*t, depth + 1)));
            }
        }
    }

    fn label_names(&self, task: &Task) -> Vec<String> {
        self.snapshot.labels_of(task).iter().map(|l| l.name.clone()).collect()
    }
}

fn walk_projects<'a>(
    by_parent: &BTreeMap<Option<ProjectId>, Vec<&'a Project>>,
    parent: Option<ProjectId>,
    depth: usize,
    seen: &mut BTreeSet<ProjectId>,
    out: &mut Vec<(&'a Project, usize)>,
) {
    for project in by_parent.get(&parent).into_iter().flatten() {
        if seen.insert(project.id) {
            out.push((project, depth));
            walk_projects(by_parent, Some(project.id), depth + 1, seen, out);
        }
    }
}

fn long_date(date: Date) -> String {
    const MONTHS: [&str; 12] = [
        "January", "February", "March", "April", "May", "June", "July", "August", "September",
        "October", "November", "December",
    ];
    let month = MONTHS.get(usize::try_from(date.month() - 1).unwrap_or(0)).copied().unwrap_or("");
    format!(
        "{} {} {month} {}",
        lumenna_core::time::weekday_name(date.weekday()),
        date.day(),
        date.year()
    )
}

fn clock(time: Time) -> String {
    format!("{:02}:{:02}", time.hour(), time.minute())
}

fn due_words(due: &Due) -> String {
    let mut text = format!("due {}", due.date);
    if let Some(time) = due.time {
        text.push_str(&format!(" at {}", clock(time)));
    }
    if let Some(zone) = &due.timezone {
        text.push_str(&format!(" {zone}"));
    }
    if let Some(recurrence) = &due.recurrence {
        text.push_str(&format!(", repeats {}", recurrence.rrule));
        if recurrence.from_completion {
            text.push_str(" from completion");
        }
    }
    text
}

/// The present state as Markdown: projects as headings, tasks as checklists.
#[must_use]
pub fn markdown(snapshot: &Snapshot, now: &Zoned) -> String {
    let outline = Outline::new(snapshot);
    let mut out = String::from("# Lumenna\n\n");
    out.push_str(&format!(
        "Exported {} at {}. Current state only: no history, nothing from the trash.\n",
        long_date(now.date()),
        clock(now.time())
    ));

    for (project, depth) in &outline.projects {
        let level = (depth + 2).min(6);
        out.push_str(&format!("\n{} {}", "#".repeat(level), project.name));
        if project.archived {
            out.push_str(" (archived)");
        }
        out.push_str("\n\n");
        let mut any = false;
        outline.each_task(project.id, |task, depth| {
            any = true;
            let indent = "  ".repeat(depth);
            let mark = if outline.facts.is_completed(task) { "x" } else { " " };
            let mut details = Vec::new();
            if let Some(due) = &task.due {
                details.push(due_words(due));
            }
            if task.priority != Priority::P4 {
                details.push(format!("priority {}", task.priority.as_u8()));
            }
            let labels = outline.label_names(task);
            if !labels.is_empty() {
                let tagged: Vec<String> = labels.iter().map(|l| format!("@{l}")).collect();
                details.push(tagged.join(" "));
            }
            if let Some(minutes) = task.estimate_mins {
                details.push(format!("estimate {minutes} minutes"));
            }
            out.push_str(&format!("{indent}- [{mark}] {}", task.title));
            if !details.is_empty() {
                out.push_str(&format!(" — {}", details.join("; ")));
            }
            out.push('\n');
            for line in task.notes.lines().filter(|l| !l.trim().is_empty()) {
                out.push_str(&format!("{indent}  {line}\n"));
            }
        });
        if !any {
            out.push_str("No tasks.\n");
        }
    }
    out
}

/// The present state as an org file: projects as headings, tasks as TODO items beneath them.
#[must_use]
pub fn org(snapshot: &Snapshot, now: &Zoned) -> String {
    let outline = Outline::new(snapshot);
    let mut out = format!(
        "#+TITLE: Lumenna\n#+DATE: {}\n# Current state only: no history, nothing from the \
         trash.\n",
        org_date(now.date(), Some(now.time()), "[", "]", None)
    );

    for (project, depth) in &outline.projects {
        let level = depth + 1;
        out.push_str(&format!("\n{} {}", "*".repeat(level), project.name));
        if project.archived {
            out.push_str(" :ARCHIVE:");
        }
        out.push('\n');
        outline.each_task(project.id, |task, depth| {
            let stars = "*".repeat(level + 1 + depth);
            let keyword = if outline.facts.is_completed(task) { "DONE" } else { "TODO" };
            let priority = match task.priority {
                Priority::P1 => " [#A]",
                Priority::P2 => " [#B]",
                Priority::P3 => " [#C]",
                Priority::P4 => "",
            };
            out.push_str(&format!("{stars} {keyword}{priority} {}", task.title));
            let tags: Vec<String> = outline.label_names(task).iter().map(|l| org_tag(l)).collect();
            if !tags.is_empty() {
                out.push_str(&format!(" :{}:", tags.join(":")));
            }
            out.push('\n');

            let body = " ".repeat(stars.len() + 1);
            if let Some(due) = &task.due {
                let repeater = due.recurrence.as_ref().and_then(|r| org_repeater(&r.rrule, r.from_completion));
                out.push_str(&format!(
                    "{body}DEADLINE: {}\n",
                    org_date(due.date, due.time, "<", ">", repeater.as_deref())
                ));
            }
            out.push_str(&format!("{body}:PROPERTIES:\n{body}:ID: {}\n", task.id));
            if let Some(minutes) = task.estimate_mins {
                out.push_str(&format!("{body}:EFFORT: {}:{:02}\n", minutes / 60, minutes % 60));
            }
            if let Some(recurrence) = task.due.as_ref().and_then(|d| d.recurrence.as_ref()) {
                out.push_str(&format!("{body}:REPEAT: {}\n", recurrence.rrule));
            }
            out.push_str(&format!("{body}:END:\n"));
            for line in task.notes.lines() {
                // Indented, so a line of notes that starts with `*` stays body text rather
                // than becoming a heading.
                out.push_str(&format!("{body}{line}\n"));
            }
        });
    }
    out
}

fn org_date(date: Date, time: Option<Time>, open: &str, close: &str, repeater: Option<&str>) -> String {
    let day = &lumenna_core::time::weekday_name(date.weekday())[..3];
    let mut text = format!("{open}{date} {day}");
    if let Some(time) = time {
        text.push_str(&format!(" {}", clock(time)));
    }
    if let Some(repeater) = repeater {
        text.push_str(&format!(" {repeater}"));
    }
    text.push_str(close);
    text
}

/// An org repeater for the rules org can express — a frequency and an interval, nothing
/// more. Anything richer stays in the `REPEAT` property rather than being approximated.
fn org_repeater(rrule: &str, from_completion: bool) -> Option<String> {
    let mut unit = None;
    let mut interval = 1u32;
    for part in rrule.split(';') {
        match part.split_once('=')? {
            ("FREQ", "DAILY") => unit = Some('d'),
            ("FREQ", "WEEKLY") => unit = Some('w'),
            ("FREQ", "MONTHLY") => unit = Some('m'),
            ("FREQ", "YEARLY") => unit = Some('y'),
            ("INTERVAL", n) => interval = n.parse().ok()?,
            _ => return None,
        }
    }
    let mark = if from_completion { ".+" } else { "+" };
    Some(format!("{mark}{interval}{}", unit?))
}

/// Org tags allow letters, digits, `_`, `@`, `#` and `%`; anything else becomes `_`.
fn org_tag(label: &str) -> String {
    label
        .chars()
        .map(|c| if c.is_alphanumeric() || "_@#%".contains(c) { c } else { '_' })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_rules_become_org_repeaters_and_others_do_not() {
        assert_eq!(org_repeater("FREQ=WEEKLY", false).as_deref(), Some("+1w"));
        assert_eq!(org_repeater("FREQ=DAILY;INTERVAL=3", true).as_deref(), Some(".+3d"));
        assert_eq!(org_repeater("FREQ=WEEKLY;BYDAY=MO,WE", false), None);
    }

    #[test]
    fn tags_lose_only_what_org_cannot_hold() {
        assert_eq!(org_tag("deep work"), "deep_work");
        assert_eq!(org_tag("café@home"), "café@home");
    }
}
