//! Small pieces of wording every client composes the same way.

/// "no tasks", "1 task", "17 tasks".
#[must_use]
pub fn count_line(count: usize, noun: &str) -> String {
    match count {
        0 => format!("no {noun}s"),
        1 => format!("1 {noun}"),
        n => format!("{n} {noun}s"),
    }
}

/// A time of day as `HH:MM`, without the seconds nobody schedules by.
#[must_use]
pub fn time_text(time: jiff::civil::Time) -> String {
    format!("{:02}:{:02}", time.hour(), time.minute())
}

/// "45 minutes", "1 hour", "2 hours 30 minutes".
#[must_use]
pub fn duration(minutes: u32) -> String {
    let (hours, rest) = (minutes / 60, minutes % 60);
    match (hours, rest) {
        (0, m) => count_line(m as usize, "minute"),
        (h, 0) => count_line(h as usize, "hour"),
        (h, m) => format!("{} {}", count_line(h as usize, "hour"), count_line(m as usize, "minute")),
    }
}

/// A length as a braille display shows it, hours and minutes, shorter than words and never
/// unclear: "1:15", "1:00", "0:15".
#[must_use]
pub fn duration_braille(minutes: u32) -> String {
    format!("{}:{:02}", minutes / 60, minutes % 60)
}

/// [`block_details`] as a braille display shows them: the length as [`duration_braille`],
/// the kind's short form ("wrk"), tasks assigned as "3 tsk"; the rest as said.
#[must_use]
pub fn block_braille(block: &crate::types::PlanBlock) -> Vec<String> {
    let kind = crate::resolve::block_kind(&block.kind)
        .map_or_else(|_| format!("{} block", block.kind), |kind| kind.abbreviation().to_owned());
    let tasks = lumenna_core::row::Role::Task.abbreviation();
    block_parts(block, duration_braille(block.duration_mins), kind, |n| format!("{n} {tasks}"))
}

/// How long ago a timestamp was, as a person says it: "just now", "5 minutes ago",
/// "2 hours ago", "3 days ago". An unreadable timestamp is returned as it came.
#[must_use]
pub fn ago(timestamp: &str, now: jiff::Timestamp) -> String {
    let Ok(then) = timestamp.parse::<jiff::Timestamp>() else { return timestamp.to_owned() };
    let seconds = now.as_second() - then.as_second();
    let plural = |n: i64, unit: &str| if n == 1 { format!("1 {unit} ago") } else { format!("{n} {unit}s ago") };
    match seconds {
        ..60 => "just now".to_owned(),
        60..3_600 => plural(seconds / 60, "minute"),
        3_600..86_400 => plural(seconds / 3_600, "hour"),
        _ => plural(seconds / 86_400, "day"),
    }
}

/// What a line about a block says after its time and title: its length, its kind,
/// where it falls today, whether this day was changed, what about it differs from its kind,
/// and — for a block that takes tasks — how many are in it.
///
/// The kind is said because it says which actions exist; a flag is said only where it
/// differs from the kind's own, so a work block is not "work block, takes tasks, movable".
#[must_use]
pub fn block_details(block: &crate::types::PlanBlock) -> Vec<String> {
    block_parts(block, duration(block.duration_mins), format!("{} block", block.kind), |n| match n {
        0 => "nothing assigned".to_owned(),
        1 => "1 task assigned".to_owned(),
        n => format!("{n} tasks assigned"),
    })
}

/// A block's details from its length and kind as worded, and how its tasks are counted.
fn block_parts(block: &crate::types::PlanBlock, length: String, kind: String, assigned: impl Fn(usize) -> String) -> Vec<String> {
    let mut parts = vec![length, kind];
    if !block.when.is_empty() {
        parts.push(block.when.clone());
    }
    if block.changed_for_this_day {
        parts.push("changed for this day".to_owned());
    }
    if let Some(zone) = &block.timezone {
        parts.push(format!("{zone} time"));
    }
    // An unknown kind is taken as an event.
    let kind = crate::form::block_defaults(block.kind.clone())
        .unwrap_or(crate::form::BlockDefaults { accepts_tasks: false, counts_capacity: false, anchored: true });
    if block.accepts_tasks != kind.accepts_tasks {
        parts.push(if block.accepts_tasks { "takes tasks" } else { "takes no tasks" }.to_owned());
    }
    if block.anchored != kind.anchored {
        parts.push(if block.anchored { "anchored" } else { "movable" }.to_owned());
    }
    if block.accepts_tasks {
        parts.push(assigned(block.assignments.len()));
    }
    parts
}

/// What a line about a sitting says after its title: its status with its planned
/// length, the time logged, and a timer that looks forgotten.
#[must_use]
pub fn sitting_details(sitting: &crate::types::PlanAssignment) -> Vec<String> {
    let mut parts = crate::form::sitting_status(sitting.clone());
    if sitting.minutes > 0 {
        parts.push(format!("{} logged", duration(sitting.minutes)));
    }
    if sitting.capped {
        parts.push("capped, the timer looks forgotten".to_owned());
    }
    parts
}

/// [`sitting_details`] as a braille display shows them: lengths as [`duration_braille`],
/// "plan 0:30" for what it is planned to take.
#[must_use]
pub fn sitting_braille(sitting: &crate::types::PlanAssignment) -> Vec<String> {
    let mut parts = match sitting.planned_mins {
        None => vec![sitting.status.clone()],
        Some(planned) if sitting.status == "planned" => vec![format!("plan {}", duration_braille(planned))],
        Some(planned) => vec![sitting.status.clone(), format!("plan {}", duration_braille(planned))],
    };
    if sitting.minutes > 0 {
        parts.push(format!("{} logged", duration_braille(sitting.minutes)));
    }
    if sitting.capped {
        parts.push("capped".to_owned());
    }
    parts
}

/// A day of a repeating block cancelled, as a braille display shows it.
#[must_use]
pub fn cancelled_braille() -> Vec<String> {
    vec!["cancelled".to_owned()]
}

/// How syncing with a device is going, in words rather than an icon: "this device";
/// "last synced 5 minutes ago"; or, when the last attempt failed, when and why, then when
/// it last worked. Not `direct` — on a watch, which syncs through its phone — only what is
/// true of the device itself, its version.
#[must_use]
pub fn device_status(device: &crate::types::DeviceView, now: jiff::Timestamp, direct: bool) -> Vec<String> {
    if device.this_device {
        return vec!["this device".to_owned()];
    }
    let mut parts = Vec::new();
    // Builds of different versions go on syncing; what one cannot read, it does not show.
    let ours = lumenna_core::model::SCHEMA_VERSION;
    if device.schema_version < ours {
        parts.push("runs an older version of Lumenna, so update it".to_owned());
    } else if device.schema_version > ours {
        parts.push("runs a newer version of Lumenna, so update this device".to_owned());
    }
    // A watch syncs through its phone, never with the others: it has no attempts to say.
    if !direct {
        return parts;
    }
    if let Some(error) = &device.last_error {
        parts.push(match &device.last_attempt {
            Some(attempt) => format!("last attempt {} failed: {error}", ago(attempt, now)),
            None => format!("last attempt failed: {error}"),
        });
    }
    match &device.last_success {
        Some(success) => parts.push(format!("last synced {}", ago(success, now))),
        None if device.last_error.is_none() => parts.push("not synced yet".to_owned()),
        None => {}
    }
    parts
}

/// Why a task is suggested, as the one sentence that is its description: "Suggested because
/// overdue by 2 days, priority 1, and its estimate of 30 minutes fits the 35 minutes left";
/// what counts against it comes after ("…, though it waits on another task"). Nothing when
/// there is nothing to say. `terse` (the verbosity setting) leaves out "Suggested because" and
/// the minutes, said in every choice of a list: "Overdue by 2 days, priority 1, fits the time
/// left."
#[must_use]
pub fn suggested_because(reasons: &[lumenna_core::suggestions::Reason], terse: bool) -> Option<String> {
    use lumenna_core::suggestions::Reason as R;
    let days = |n: i64| count_line(usize::try_from(n).unwrap_or(0), "day");
    let mut for_it = Vec::new();
    let mut against = Vec::new();
    for reason in reasons {
        match reason {
            R::Overdue(0) => for_it.push("overdue".to_owned()),
            R::Overdue(n) => for_it.push(format!("overdue by {}", days(*n))),
            R::DueIn(0) => for_it.push("due today".to_owned()),
            R::DueIn(1) => for_it.push("due tomorrow".to_owned()),
            R::DueIn(n) => for_it.push(format!("due in {}", days(*n))),
            R::Priority(p) => for_it.push(format!("priority {p}")),
            R::Started => for_it.push("partly worked".to_owned()),
            R::Blocking(1) => for_it.push("a task waits on it".to_owned()),
            R::Blocking(n) => for_it.push(format!("{n} tasks wait on it")),
            R::Fits { .. } if terse => for_it.push("fits the time left".to_owned()),
            R::Fits { estimate, remaining } => {
                for_it.push(format!("its estimate of {} fits the {} left", duration(*estimate), duration(*remaining)));
            }
            R::Longer { .. } if terse => against.push("longer than the time left".to_owned()),
            R::Longer { estimate, remaining } => {
                against.push(format!("its estimate of {} is more than the {} left", duration(*estimate), duration(*remaining)));
            }
            R::Blocked => against.push("it waits on another task".to_owned()),
            R::AssignedToday => against.push("it is in a block already today".to_owned()),
        }
    }
    if terse {
        let mut parts = for_it.join(", ");
        if !against.is_empty() {
            parts = if parts.is_empty() { listed(&against) } else { format!("{parts}, though {}", listed(&against)) };
        }
        return (!parts.is_empty()).then(|| format!("{}{}.", parts[..1].to_uppercase(), &parts[1..]));
    }
    match (for_it.is_empty(), against.is_empty()) {
        (true, true) => None,
        (false, true) => Some(format!("Suggested because {}.", listed(&for_it))),
        (false, false) => Some(format!("Suggested because {}, though {}.", listed(&for_it), listed(&against))),
        (true, false) => Some(format!("Offered, though {}.", listed(&against))),
    }
}

/// Parts as a list is said: "a", "a and b", "a, b, and c".
#[must_use]
pub fn listed(parts: &[String]) -> String {
    match parts {
        [] => String::new(),
        [one] => one.clone(),
        [first, second] => format!("{first} and {second}"),
        [rest @ .., last] => format!("{}, and {last}", rest.join(", ")),
    }
}

/// The same with its first letter a capital: "3 blocks" as a sentence starts.
#[must_use]
pub fn capitalised(text: &str) -> String {
    let mut chars = text.chars();
    chars.next().map_or_else(String::new, |first| first.to_uppercase().chain(chars).collect())
}

/// Where a day is, said after "to Deep work": "today", "tomorrow", "on Friday".
pub fn on_day(date: jiff::civil::Date, today: jiff::civil::Date) -> String {
    let words = lumenna_core::time::day_words(date, today);
    match words.as_str() {
        "today" => "later today".to_owned(),
        "tomorrow" | "yesterday" => words,
        _ => format!("on {words}"),
    }
}

/// What every app and widget says when the store would not open, with the platform's `reason`
/// (the error's own sentence) after it when there is one. Without one it is a heading, as a Mac
/// alert's title is, the reason beneath it.
#[must_use]
pub fn store_unopened(reason: &str) -> String {
    let reason = reason.trim();
    if reason.is_empty() {
        "Lumenna could not open its store".to_owned()
    } else {
        format!("Lumenna could not open its store. {reason}")
    }
}

/// A record's sentence, then anything else worth saying, as one line to say.
#[must_use]
pub fn spoken(announcement: &str, notices: &[String]) -> String {
    std::iter::once(announcement).chain(notices.iter().map(String::as_str)).filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" ")
}

/// A count as the `u32` a record carries, saturating rather than wrapping.
#[must_use]
pub fn to_u32(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}
/// What Sync Now says when syncing stopped before its round could run.
#[must_use]
pub fn sync_stopped() -> String {
    "Syncing has stopped.".to_owned()
}

/// What is said as a round of syncing starts.
#[must_use]
pub fn syncing() -> String {
    "Syncing".to_owned()
}

/// What is said when keeping in sync could not start, and why.
#[must_use]
pub fn sync_not_started(why: &str) -> String {
    format!("Syncing could not start: {why}")
}

/// What is said once a row is folded or unfolded: "Expanded", "Collapsed".
#[must_use]
pub fn folded_said(expanded: bool) -> String {
    if expanded { "Expanded" } else { "Collapsed" }.to_owned()
}

/// A row with rows under it, as its state is said where the platform's control does not say it:
/// "expanded", "collapsed".
#[must_use]
pub fn fold_state(expanded: bool) -> String {
    if expanded { "expanded" } else { "collapsed" }.to_owned()
}

/// How deep a row sits, said where it changes from the row before: "level 2" for `depth` 1.
#[must_use]
pub fn level(depth: u32) -> String {
    format!("level {}", depth + 1)
}

/// How many a pick offers: "1 choice", "3 choices", "no choices".
#[must_use]
pub fn choice_count(count: usize) -> String {
    count_line(count, "choice")
}

/// What a pick says that has nothing to offer, where a shell shows it at all: the core closes
/// such a question and says why, so this is for a list a shell offers of its own.
#[must_use]
pub fn nothing_to_choose() -> String {
    "There is nothing to choose from.".to_owned()
}

/// A reminder that is a default rather than its own, as its line says it.
#[must_use]
pub fn default_reminder() -> String {
    "a default".to_owned()
}

/// A proposal's change, or anything checked, as its state is said: "checked", "not checked".
#[must_use]
pub fn checked(on: bool) -> String {
    if on { "checked" } else { "not checked" }.to_owned()
}

/// Why a new saved filter's name is refused when it has none.
#[must_use]
pub fn filter_needs_name() -> String {
    "A saved filter needs a name.".to_owned()
}

/// What is said when a change the platform was asked for failed and the platform gave no words.
#[must_use]
pub fn could_not_change() -> String {
    "That could not be changed".to_owned()
}

/// What an app says when its automatic backup failed, and why (`reason`, a sentence).
#[must_use]
pub fn backup_failed(reason: &str) -> String {
    let reason = reason.trim();
    if reason.is_empty() { "The automatic backup failed.".to_owned() } else { format!("The automatic backup failed. {}", capitalised(reason)) }
}

/// The title of the list of `name`'s reminders: "Reminders for Pay rent".
#[must_use]
pub fn reminders_for(name: &str) -> String {
    format!("Reminders for {name}")
}

/// What Lumenna is, in one line: the About box, the store's listing, the desktop entry's comment.
pub const DESCRIPTION: &str = env!("LUMENNA_DESCRIPTION");

/// What Lumenna is, in one line (`DESCRIPTION`).
#[must_use]
pub fn app_description() -> String {
    DESCRIPTION.to_owned()
}

/// What a browser calls itself when it pairs, from what it runs as: "Firefox on Windows", or
/// the browser alone when the system is not known. `browser` and `system` are the platform's
/// names for them, found from its user agent.
#[must_use]
pub fn browser_device_name(browser: &str, system: &str) -> String {
    let browser = if browser.trim().is_empty() { "Web browser" } else { browser.trim() };
    if system.trim().is_empty() { browser.to_owned() } else { format!("{browser} on {}", system.trim()) }
}

/// How a watch that syncs through its phone stands with it, `phone` being what the phone is
/// called ("the iPhone"): not yet synced, out of reach, synced at `time` (the watch's clock),
/// or failed for `reason`.
#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PhoneSync {
    /// Not synced since the watch started.
    NotYet,
    /// The phone cannot be reached; changes go when it can.
    OutOfReach,
    /// Synced, at this time as the watch says one.
    Synced {
        /// The time, in the watch's clock.
        time: String,
    },
    /// The last sync failed.
    Failed {
        /// Why, as a sentence.
        reason: String,
    },
}

/// A watch's sync with its phone, said: "Synced with the iPhone at 9:41 AM".
#[must_use]
pub fn phone_sync(phone: &str, status: &PhoneSync) -> String {
    match status {
        PhoneSync::NotYet => format!("Not synced with {phone} yet"),
        PhoneSync::OutOfReach => format!("{} is not in reach; changes go to it when it is", capitalised(phone)),
        PhoneSync::Synced { time } => format!("Synced with {phone} at {time}"),
        PhoneSync::Failed { reason } => format!("Could not sync with {phone}. {}", capitalised(reason.trim())),
    }
}

/// A timer running, as a glance says it beside its task.
#[must_use]
pub fn timer_running() -> String {
    "timer running".to_owned()
}

/// Where something happens, as a glance says it: "in Deep work".
#[must_use]
pub fn in_block(title: &str) -> String {
    format!("in {title}")
}

/// Until when, as a glance says it: "until 11:00", the time in the device's clock.
#[must_use]
pub fn until(time: &str) -> String {
    format!("until {time}")
}

/// What comes after, as a glance says it: "then Meeting".
#[must_use]
pub fn then(title: &str) -> String {
    format!("then {title}")
}

/// What follows the moment a glance shows, as it says it: "Next, 12:00 to 13:00, Lunch".
#[must_use]
pub fn next(line: &str) -> String {
    format!("Next, {line}")
}

/// A glance with nothing more planned today.
#[must_use]
pub fn nothing_more_planned() -> String {
    "Nothing more planned today".to_owned()
}

/// Free time, as a glance names it.
#[must_use]
pub fn free() -> String {
    "Free".to_owned()
}

/// How many tasks are due, as a glance says it: "3 tasks due", or "Nothing due".
#[must_use]
pub fn tasks_due(count: u32) -> String {
    if count == 0 { "Nothing due".to_owned() } else { format!("{} due", count_line(count as usize, "task")) }
}

/// How many of them are overdue: "1 overdue".
#[must_use]
pub fn overdue_count(count: u32) -> String {
    format!("{count} overdue")
}


/// Why a summary of missed reminders cannot be snoozed: it has no Snooze, only Dismiss.
#[must_use]
pub fn snooze_not_offered() -> String {
    "What was missed can be dismissed, not snoozed.".to_owned()
}

/// `text` in sentence case where `sentence`, else as given (Title Case).
fn cased(text: &str, sentence: bool) -> String {
    if sentence { crate::fields::sentence_case(text.to_owned()) } else { text.to_owned() }
}

/// What a shell asks when a command could run more than one of the row's actions of a kind:
/// "Which?", over the actions' names.
#[must_use]
pub fn which() -> String {
    "Which?".to_owned()
}

/// What a shell says of an action whose form it has not drawn yet: "Edit Details has no form in
/// this app yet", `action` its name.
#[must_use]
pub fn no_form_here(action: &str) -> String {
    format!("{action} has no form in this app yet")
}

/// What a shell says of a question it cannot ask yet, built before the core asked it so:
/// "This app cannot ask that question yet; update it to Put in a Block", `action` its name.
#[must_use]
pub fn cannot_ask_here(action: &str) -> String {
    format!("This app cannot ask that question yet; update it to {action}")
}

/// A shell's own menu of what can be done where the person is, named after it: "Day menu",
/// "Lumenna menu". In sentence case, as the shells that have such menus (BTSpeak) write.
#[must_use]
pub fn menu_of(name: &str) -> String {
    format!("{name} menu")
}

/// What an empty list says of the key that adds to it: "Press a to add one." `key` is the
/// platform's own, as it reads it.
#[must_use]
pub fn press_to_add(key: &str) -> String {
    format!("Press {key} to add one.")
}

/// What an empty list that can be filtered says: "Press a to add one, or slash to filter now."
#[must_use]
pub fn press_to_add_or_filter(key: &str, filter: &str) -> String {
    format!("Press {key} to add one, or {filter} to filter now.")
}

/// What an empty list of devices says: "Press p to pair one."
#[must_use]
pub fn press_to_pair(key: &str) -> String {
    format!("Press {key} to pair one.")
}

/// The list of reminders due, as its heading or screen names it.
#[must_use]
pub fn reminders_due(sentence: bool) -> String {
    cased("Reminders Due", sentence)
}

/// A device's own menu of the day's actions, as a toolbar or a menu button names it.
#[must_use]
pub fn day_actions(sentence: bool) -> String {
    cased("Day Actions", sentence)
}

/// A place's own actions (a project's, a label's), as a menu button names it.
#[must_use]
pub fn place_actions(sentence: bool) -> String {
    cased("Place Actions", sentence)
}

/// Whether reminders are given on this device, said once changed: "Reminders are on in this
/// browser", `this_device` being how the device calls itself.
#[must_use]
pub fn reminders_on_here(on: bool, this_device: &str) -> String {
    format!("Reminders are {} in {this_device}", if on { "on" } else { "off" })
}

/// Where reminders come only while Lumenna is open: what a browser tab adds to the setting's hint.
#[must_use]
pub fn reminders_while_open() -> String {
    "Reminders are given while Lumenna is open in this tab.".to_owned()
}

/// What a browser says when it may clear Lumenna's data short of space.
#[must_use]
pub fn storage_may_be_cleared() -> String {
    "This browser may clear Lumenna's data when it runs short of space. Installing Lumenna, or allowing it to keep data, prevents that."
        .to_owned()
}

/// What shows while the store opens.
#[must_use]
pub fn opening() -> String {
    "Opening Lumenna…".to_owned()
}

/// What a second tab says while another has the store open.
#[must_use]
pub fn open_elsewhere() -> String {
    "Lumenna is open in another tab. Close it there to use it here.".to_owned()
}

/// What the devices list says Delete does there.
#[must_use]
pub fn delete_unpairs() -> String {
    "Delete unpairs the selected device.".to_owned()
}

/// The field naming this device for pairing: "Name for this browser", and its description.
#[must_use]
pub fn device_name_label(this_device: &str) -> String {
    format!("Name for {this_device}")
}

/// What the field naming this device for pairing is for.
#[must_use]
pub fn device_name_hint() -> String {
    "What your other devices call it.".to_owned()
}

/// What is said when a pairing code could not be copied.
#[must_use]
pub fn copy_failed() -> String {
    "Could not copy the code; select it and copy it instead.".to_owned()
}

/// What a pick says when its answer is given with nothing chosen.
#[must_use]
pub fn choose_one_first() -> String {
    "Choose one first.".to_owned()
}

/// A dialog's button that closes it, making nothing.
#[must_use]
pub fn cancel(sentence: bool) -> String {
    cased("Cancel", sentence)
}

/// A dialog's button that closes what only shows something.
#[must_use]
pub fn close(sentence: bool) -> String {
    cased("Close", sentence)
}

/// What a line that completes says of it, after its own hint: the completion's key.
#[must_use]
pub fn completion_hint() -> String {
    "Down arrow offers what could come next.".to_owned()
}

/// What is said when the shortcuts from anywhere cannot be had on this desktop, and why
/// (`reason`, the platform's).
#[must_use]
pub fn hotkeys_unavailable(reason: &str) -> String {
    let reason = reason.trim();
    if reason.is_empty() {
        "The shortcuts from anywhere are not available here.".to_owned()
    } else {
        format!("The shortcuts from anywhere are not available here. {reason}")
    }
}

/// The tab or screen of every project, label and saved filter.
#[must_use]
pub fn browse(sentence: bool) -> String {
    cased("Browse", sentence)
}

/// A field's placeholder showing the shape of an answer: "For example, tomorrow".
#[must_use]
pub fn for_example(example: &str) -> String {
    format!("For example, {example}")
}

/// What an empty field says of itself where its value is shown apart from it: "Empty".
#[must_use]
pub fn empty_field() -> String {
    "Empty".to_owned()
}

/// An empty value as a line of text says it: "none".
#[must_use]
pub fn none_value() -> String {
    "none".to_owned()
}

/// What a place is, as the heading over its own actions says it: "Project", "Label", "Saved
/// Filter"; none for a place of the app's own.
#[must_use]
pub fn place_heading(place: &crate::places::Place, sentence: bool) -> Option<String> {
    use crate::places::Place;
    let heading = match place {
        Place::Project(_) => "Project",
        Place::Label(_) => "Label",
        Place::Filter { .. } => "Saved Filter",
        _ => return None,
    };
    Some(cased(heading, sentence))
}

/// A button that asks for more of a line to put at its end.
#[must_use]
pub fn add_to_line(sentence: bool) -> String {
    cased("Add to the Line", sentence)
}

/// The list of what could come next, as its dialog or menu is called.
#[must_use]
pub fn completions(sentence: bool) -> String {
    cased("Completions", sentence)
}

/// How many completions a shell lists at once: the most likely come first.
pub const COMPLETIONS_SHOWN: usize = 8;

/// A widget's button marking `task` done, as a screen reader names it: "Mark done, Pay rent",
/// `mark_done` the core's name for the action.
#[must_use]
pub fn mark_done_described(mark_done: &str, task: &str) -> String {
    format!("{mark_done}, {task}")
}

/// A timer control, as a screen reader names it: its name, what pressing it does, and what it
/// would run or runs, each that says something, joined. A locked device leaves out what runs.
#[must_use]
pub fn timer_described(name: &str, action: &str, detail: &str, locked: bool) -> String {
    let parts: Vec<&str> = if locked { vec![name, action] } else { vec![name, action, detail] };
    parts.into_iter().filter(|part| !part.trim().is_empty()).collect::<Vec<_>>().join(", ")
}

/// What an iPad's settings say with no page open.
#[must_use]
pub fn no_settings_page() -> String {
    "No settings page open".to_owned()
}

/// What Lumenna is, its name, version, home and licence: read from the workspace's
/// `[workspace.metadata.lumenna]` and `[workspace.package]` as it builds, the one source every
/// platform's own files are checked against.
#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AppMetadata {
    /// "Lumenna".
    pub name: String,
    /// What it is, in one line.
    pub description: String,
    /// The version every platform's build says.
    pub version: String,
    /// Where it lives.
    pub homepage: String,
    /// Where problems are reported.
    pub issues: String,
    /// Who publishes it.
    pub publisher: String,
    /// Its licence, as SPDX names it.
    pub licence: String,
    /// The identifier it is known by on the desktop and in stores: "dev.emassey0135.Lumenna".
    pub id: String,
}

/// What Lumenna is (`AppMetadata`), from the one source.
#[must_use]
pub fn metadata() -> AppMetadata {
    AppMetadata {
        name: NAME.to_owned(),
        description: DESCRIPTION.to_owned(),
        version: VERSION.to_owned(),
        homepage: HOMEPAGE.to_owned(),
        issues: ISSUES.to_owned(),
        publisher: PUBLISHER.to_owned(),
        licence: LICENCE.to_owned(),
        id: ID.to_owned(),
    }
}

/// Lumenna's name.
pub const NAME: &str = env!("LUMENNA_NAME");
/// The version every platform's build says.
pub const VERSION: &str = env!("LUMENNA_VERSION");
/// Where Lumenna lives.
pub const HOMEPAGE: &str = env!("LUMENNA_HOMEPAGE");
/// Where problems are reported.
pub const ISSUES: &str = env!("LUMENNA_ISSUES");
/// Who publishes it.
pub const PUBLISHER: &str = env!("LUMENNA_PUBLISHER");
/// Its licence, as SPDX names it.
pub const LICENCE: &str = env!("LUMENNA_LICENCE");
/// The identifier it is known by on the desktop and in stores.
pub const ID: &str = env!("LUMENNA_ID");

#[cfg(test)]
mod tests {
    use super::*;

    /// A file of the repository's, from the workspace's root.
    fn repo_file(path: &str) -> String {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        std::fs::read_to_string(root.join(path)).unwrap_or_else(|error| panic!("{path}: {error}"))
    }

    /// Whether `file` has a line that is `line` once trimmed.
    fn has_line(file: &str, line: &str) -> bool {
        file.lines().any(|l| l.trim() == line)
    }

    #[test]
    fn every_platform_file_says_what_lumenna_is_as_the_workspace_does() {
        let m = metadata();
        let desktop = repo_file("apps/gtk/data/dev.emassey0135.Lumenna.desktop");
        assert!(has_line(&desktop, &format!("Comment={}", m.description)), "the desktop entry's Comment");
        assert!(has_line(&desktop, &format!("Name={}", m.name)), "the desktop entry's Name");
        let metainfo = repo_file("apps/gtk/data/dev.emassey0135.Lumenna.metainfo.xml");
        for line in [
            format!("<id>{}</id>", m.id),
            format!("<name>{}</name>", m.name),
            format!("<summary>{}</summary>", m.description),
            format!("<project_license>{}</project_license>", m.licence),
            format!("<url type=\"homepage\">{}</url>", m.homepage),
            format!("<url type=\"bugtracker\">{}</url>", m.issues),
        ] {
            assert!(metainfo.contains(&line), "the metainfo says {line}");
        }
        assert!(metainfo.contains(&format!("<name>{}</name>", m.publisher)), "the metainfo's developer");
        let msix = repo_file("apps/windows/installer/msix/AppxManifest.xml");
        assert!(msix.contains(&format!("Description=\"{}\"", m.description)), "the MSIX's description");
        assert!(msix.contains(&format!("<DisplayName>{}</DisplayName>", m.name)), "the MSIX's name");
        let inno = repo_file("apps/windows/installer/lumenna.iss");
        for line in [format!("AppName={}", m.name), format!("AppPublisher={}", m.publisher), format!("AppPublisherURL={}", m.homepage)] {
            assert!(has_line(&inno, &line), "the installer says {line}");
        }
        for gradle in ["apps/android/app/build.gradle.kts", "apps/android/wear/build.gradle.kts"] {
            assert!(has_line(&repo_file(gradle), &format!("versionName = \"{}\"", m.version)), "{gradle}'s versionName");
        }
        for strings in ["apps/android/app/src/main/res/values/strings.xml", "apps/android/wear/src/main/res/values/strings.xml"] {
            assert!(repo_file(strings).contains(&format!("<string name=\"app_name\">{}</string>", m.name)), "{strings}'s app_name");
        }
        let package = repo_file("apps/web/package.json");
        assert!(has_line(&package, &format!("\"version\": \"{}\",", m.version)), "the web package's version");
        let manifest = repo_file("apps/web/public/manifest.webmanifest");
        assert!(manifest.contains(&format!("\"name\": \"{}\"", m.name)), "the web manifest's name");
        assert!(manifest.contains(&format!("\"description\": \"{}\"", m.description)), "the web manifest's description");
        let module = repo_file("apps/powershell/Lumenna/Lumenna.psd1");
        for part in [
            format!("'{}'", m.version),
            format!("'{}'", m.publisher),
            format!("ProjectUri = '{}'", m.homepage),
            m.licence.clone(),
        ] {
            assert!(module.contains(&part), "the PowerShell module says {part}");
        }
        let pyproject = repo_file("apps/btspeak/pyproject.toml");
        assert!(has_line(&pyproject, &format!("version = \"{}\"", m.version)), "BTSpeak's version");
        assert!(pyproject.contains(&format!("name = \"{}\"", m.publisher)), "BTSpeak's author");
        for crate_manifest in ["apps/cli/Cargo.toml", "apps/windows/Cargo.toml", "apps/gtk/Cargo.toml"] {
            assert!(has_line(&repo_file(crate_manifest), &format!("version = \"{}\"", m.version)), "{crate_manifest}'s version");
        }
        for plist in [
            "apps/apple/Lumenna/Info.plist",
            "apps/apple/Mac/Info.plist",
            "apps/apple/Watch/Info.plist",
            "apps/apple/Widgets/iOS/Info.plist",
            "apps/apple/Widgets/Mac/Info.plist",
            "apps/apple/Widgets/Watch/Info.plist",
        ] {
            let text = repo_file(plist).replace(['\t', '\r', '\n'], "");
            assert!(text.contains(&format!("<key>CFBundleShortVersionString</key><string>{}</string>", m.version)), "{plist}'s version");
        }
    }

    #[test]
    fn the_windows_packages_app_says_what_lumenna_is_in_the_cores_words() {
        let manifest = include_str!("../../../apps/windows/installer/msix/AppxManifest.xml");
        let app = manifest.lines().find(|line| line.contains(r#"<uap:VisualElements DisplayName="Lumenna""#)).unwrap_or_default();
        assert!(app.contains(&format!(r#"Description="{DESCRIPTION}""#)), "{app}");
    }

    #[test]
    fn a_watch_says_how_it_stands_with_its_phone_naming_it_as_given() {
        assert_eq!(phone_sync("the iPhone", &PhoneSync::NotYet), "Not synced with the iPhone yet");
        assert_eq!(phone_sync("the iPhone", &PhoneSync::Synced { time: "9:41 AM".to_owned() }), "Synced with the iPhone at 9:41 AM");
        assert_eq!(phone_sync("the iPhone", &PhoneSync::OutOfReach), "The iPhone is not in reach; changes go to it when it is");
    }

    #[test]
    fn a_browser_is_named_by_what_it_is_and_where_it_runs() {
        assert_eq!(browser_device_name("Firefox", "Windows"), "Firefox on Windows");
        assert_eq!(browser_device_name("", ""), "Web browser");
    }

    #[test]
    fn a_store_that_would_not_open_is_said_with_its_reason_after_it_when_there_is_one() {
        assert_eq!(store_unopened(" "), "Lumenna could not open its store");
        assert_eq!(
            store_unopened("The disk is full."),
            "Lumenna could not open its store. The disk is full."
        );
    }

    #[test]
    fn a_terse_reason_leaves_out_suggested_because_and_the_minutes() {
        use lumenna_core::suggestions::Reason as R;
        let reasons = [R::Overdue(1), R::Priority(1), R::Fits { estimate: 30, remaining: 35 }, R::Blocked];
        assert_eq!(
            suggested_because(&reasons, false).unwrap(),
            "Suggested because overdue by 1 day, priority 1, and its estimate of 30 minutes fits the 35 minutes left, though it waits on another task."
        );
        assert_eq!(
            suggested_because(&reasons, true).unwrap(),
            "Overdue by 1 day, priority 1, fits the time left, though it waits on another task."
        );
        assert_eq!(suggested_because(&[R::Blocked], true).unwrap(), "It waits on another task.");
        assert_eq!(suggested_because(&[], true), None);
    }
}
