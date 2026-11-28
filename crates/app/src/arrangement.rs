//! Where a phone and a watch put the places, worked out from the sidebar: a phone's tabs, its
//! Browse tab's rows and each group's screen, and a watch's root list. A place added to the
//! sidebar reaches every phone and watch this way, and no shell picks places out of the sidebar
//! by kind.

use lumenna_records::places::{Place, SidebarEntry, SidebarGroup, SidebarKind};
use serde::{Deserialize, Serialize};

use crate::ScreenAction;

/// The places as a phone and a watch arrange them.
#[boltffi::data]
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Arrangement {
    /// A phone's tabs, in order: Today, Tasks, Browse, Settings.
    pub tabs: Vec<Tab>,
    /// Browse's rows, in the sidebar's order: every place that is not a tab of its own, and each
    /// group (Projects, Labels, Saved Filters), which opens its own screen. A group's row says how
    /// many places it holds (`detail`); a place's carries the sidebar's detail and swatch.
    pub browse: Vec<SidebarEntry>,
    /// Each group's screen, in order: what a group's row in Browse opens.
    pub groups: Vec<GroupScreen>,
    /// A watch's root list, from Home, in order: New Task, every place as the sidebar has it
    /// (headings folding what is under them; a heading's New is among its `offered`, for the watch
    /// to draw after it while it is unfolded), then the rest of Home's own actions that the watch
    /// has what they need for (`needs`, `watch_has`): Undo, Redo, What Should I Work On?, Sync Now
    /// on a watch that syncs itself, Settings.
    pub watch: Vec<RootRow>,
}

/// One of a phone's tabs.
#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tab {
    /// What it shows.
    pub kind: TabKind,
    /// Its title, in the shell's capitals.
    pub title: String,
}

/// What a tab shows.
#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TabKind {
    /// A place: Today, Tasks. Any other place opens inside Browse.
    Place(Place),
    /// Browse: `Arrangement::browse`.
    Browse,
    /// Settings.
    Settings,
}

/// A group's own screen, opened from Browse.
#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupScreen {
    /// Which group.
    pub group: SidebarGroup,
    /// Its heading in the sidebar: its key, its title (the screen's), and its New among its
    /// `offered`.
    pub heading: SidebarEntry,
    /// The places under it, as the sidebar folds them, each one level shallower than in the
    /// sidebar, the levels said where they change among those shown.
    pub entries: Vec<SidebarEntry>,
    /// What it says with none: "No projects."
    pub empty: String,
}

/// A row of a watch's root list.
// Crossing BoltFFI by value in a short list: boxing a place would only add an allocation.
#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[allow(clippy::large_enum_variant)]
pub enum RootRow {
    /// A place or a heading, as the sidebar has it.
    Place(SidebarEntry),
    /// One of what can be done from anywhere: sent as `Event::Key(id)`.
    Action(ScreenAction),
}

/// The places Browse leaves to tabs of their own.
fn is_a_tab(place: &Place) -> bool {
    matches!(place, Place::Today | Place::Tasks)
}

/// The rows under the depth-0 row at `at`: up to the next row of depth 0.
fn under(sidebar: &[SidebarEntry], at: usize) -> &[SidebarEntry] {
    let rest = &sidebar[at + 1..];
    let end = rest.iter().position(|entry| entry.depth == 0).unwrap_or(rest.len());
    &rest[..end]
}

/// The places as a phone and a watch of `watch`'s kind arrange them, from `sidebar` as the view gives it (folded,
/// in the shell's capitals, its actions in short), its words in sentence case where `sentence`.
#[must_use]
pub fn arrangement(sidebar: &[SidebarEntry], watch: WatchKind, sentence: bool) -> Arrangement {
    let title = |id: &str| crate::screen_actions::action_title(id, sentence);
    let tabs = vec![
        Tab { kind: TabKind::Place(Place::Today), title: Place::Today.title() },
        Tab { kind: TabKind::Place(Place::Tasks), title: Place::Tasks.title() },
        Tab { kind: TabKind::Browse, title: lumenna_records::words::browse(sentence) },
        Tab { kind: TabKind::Settings, title: title("settings") },
    ];
    let mut browse = Vec::new();
    let mut groups = Vec::new();
    for (at, entry) in sidebar.iter().enumerate().filter(|(_, entry)| entry.depth == 0) {
        match &entry.kind {
            SidebarKind::Place(place) if is_a_tab(place) => {}
            SidebarKind::Place(_) => browse.push(entry.clone()),
            SidebarKind::Group(group) => {
                let places = under(sidebar, at);
                // Every place under it is counted: a project inside another is a project too.
                let mut row = entry.clone();
                row.detail = group.count(places.len());
                row.text = lumenna_records::places::line(&row.title, &row.detail);
                row.expanded = None;
                browse.push(row);
                let mut entries: Vec<SidebarEntry> = places
                    .iter()
                    .cloned()
                    .map(|mut place| {
                        place.depth = place.depth.saturating_sub(1);
                        place
                    })
                    .collect();
                // The group's screen is opened past its heading, so the heading's own folding in
                // the sidebar hides nothing there: only a folded place hides what is under it.
                let mut hidden_below: Option<u32> = None;
                for place in &mut entries {
                    if hidden_below.is_some_and(|depth| place.depth <= depth) {
                        hidden_below = None;
                    }
                    place.hidden = hidden_below.is_some();
                    if !place.hidden && place.expanded == Some(false) {
                        hidden_below = Some(place.depth);
                    }
                }
                let shown: Vec<u32> = entries.iter().filter(|place| !place.hidden).map(|place| place.depth).collect();
                let mut levels = crate::lines::levels(&shown).into_iter();
                for place in &mut entries {
                    place.level = if place.hidden { None } else { levels.next().flatten() };
                }
                groups.push(GroupScreen { group: *group, heading: entry.clone(), entries, empty: group.empty() });
            }
        }
    }
    Arrangement { tabs, browse, groups, watch: watch_root(sidebar, watch, sentence) }
}

/// What a watch is, as far as its root list goes: whether it syncs with other devices itself.
#[boltffi::data]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum WatchKind {
    /// It syncs itself, with Sync Now of its own: a Wear OS watch.
    #[default]
    SyncsItself,
    /// It syncs only through its phone, which a section of its own says (the Apple Watch's
    /// "iPhone"): no Sync Now among Home's actions.
    ThroughItsPhone,
}

/// What one of Home's actions needs of a device that a watch may not have: an action is on a
/// watch's root list unless it needs something the watch lacks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Needs {
    /// A keyboard: Key Help.
    Keyboard,
    /// A list of reminders due in the app: a watch gives each as a notification instead.
    RemindersInTheApp,
    /// An About of its own beside Settings: a watch's About is a page of its Settings.
    AboutBesideSettings,
    /// To sync itself: Sync Now.
    SyncingItself,
}

/// What Home's action `id` needs of the device, if anything a watch may lack.
#[must_use]
pub fn needs(id: &str) -> Option<Needs> {
    match id {
        "keyboard-help" => Some(Needs::Keyboard),
        "reminders-due" => Some(Needs::RemindersInTheApp),
        "about" => Some(Needs::AboutBesideSettings),
        "sync-now" => Some(Needs::SyncingItself),
        _ => None,
    }
}

/// Whether a watch of `kind` has what `need` asks for.
#[must_use]
pub fn watch_has(kind: WatchKind, need: Needs) -> bool {
    match need {
        Needs::Keyboard | Needs::RemindersInTheApp | Needs::AboutBesideSettings => false,
        Needs::SyncingItself => kind == WatchKind::SyncsItself,
    }
}

/// The action a watch's list leads with, its main one at the top as watches put it.
const LEADS: &str = "new-task";

/// A watch's root list, from Home: New Task first, then every place shown as Home lists them,
/// then the rest of Home's own actions in Home's order, but those needing what the watch lacks.
fn watch_root(sidebar: &[SidebarEntry], kind: WatchKind, sentence: bool) -> Vec<RootRow> {
    let action = |id: &str| RootRow::Action(crate::screen_actions::screen_action(id, sentence, true));
    let fits = |id: &&str| needs(id).is_none_or(|need| watch_has(kind, need));
    // Home's actions as they would be with anything due: what is left out is left out by a rule.
    let home = crate::screen_actions::screen_action_ids(None, true, true);
    let leads = home.iter().copied().filter(|id| *id == LEADS).filter(fits).map(action);
    let rest = home.iter().copied().filter(|id| *id != LEADS).filter(fits).map(action);
    leads.chain(sidebar.iter().filter(|entry| !entry.hidden).cloned().map(RootRow::Place)).chain(rest).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(kind: SidebarKind, title: &str, depth: u32) -> SidebarEntry {
        SidebarEntry::new(kind, title, "", depth, Vec::new())
    }

    fn sidebar() -> Vec<SidebarEntry> {
        vec![
            entry(SidebarKind::Place(Place::Today), "Today", 0),
            entry(SidebarKind::Place(Place::Tasks), "Tasks", 0),
            entry(SidebarKind::Group(SidebarGroup::Projects), "Projects", 0),
            entry(SidebarKind::Place(Place::Project("Work".to_owned())), "Work", 1),
            entry(SidebarKind::Place(Place::Project("Website".to_owned())), "Website", 2),
            entry(SidebarKind::Group(SidebarGroup::Labels), "Labels", 0),
            entry(SidebarKind::Group(SidebarGroup::Filters), "Saved Filters", 0),
            SidebarEntry::new(SidebarKind::Place(Place::Blocks), "Blocks", "2 blocks", 0, Vec::new()),
            entry(SidebarKind::Place(Place::History), "History", 0),
            entry(SidebarKind::Place(Place::Trash), "Trash", 0),
        ]
    }

    #[test]
    fn a_phone_has_its_tabs_and_browse_holds_every_other_place_in_the_sidebars_order() {
        let arranged = arrangement(&sidebar(), WatchKind::SyncsItself, true);
        let tabs: Vec<&str> = arranged.tabs.iter().map(|tab| tab.title.as_str()).collect();
        assert_eq!(tabs, ["Today", "Tasks", "Browse", "Settings"]);
        let browse: Vec<&str> = arranged.browse.iter().map(|row| row.text.as_str()).collect();
        assert_eq!(browse, ["Projects, 2 projects", "Labels, no labels", "Saved Filters, no filters", "Blocks, 2 blocks", "History", "Trash"]);
    }

    #[test]
    fn a_groups_screen_holds_its_places_a_level_shallower() {
        let arranged = arrangement(&sidebar(), WatchKind::SyncsItself, true);
        let projects = &arranged.groups[0];
        assert_eq!(projects.group, SidebarGroup::Projects);
        let depths: Vec<u32> = projects.entries.iter().map(|place| place.depth).collect();
        assert_eq!(depths, [0, 1]);
        assert_eq!(projects.entries[0].level, None, "the first is at the top");
        assert!(projects.entries[1].level.is_some(), "its level said where it changes");
        assert!(arranged.groups[1].entries.is_empty());
        assert!(!arranged.groups[1].empty.is_empty());
    }

    #[test]
    fn a_groups_screen_shows_its_places_while_the_sidebar_folds_its_heading() {
        let mut folded = sidebar();
        folded[2].expanded = Some(false);
        folded[3].hidden = true;
        folded[3].expanded = Some(false);
        folded[4].hidden = true;
        let projects = &arrangement(&folded, WatchKind::SyncsItself, true).groups[0];
        let hidden: Vec<bool> = projects.entries.iter().map(|place| place.hidden).collect();
        assert_eq!(hidden, [false, true], "Work shown, folding Website under it");
    }

    #[test]
    fn a_watch_lists_new_task_every_place_then_the_rest_of_homes_actions() {
        let ids = |kind| {
            let arranged = arrangement(&sidebar(), kind, true);
            let first = arranged.watch.iter().position(|row| matches!(row, RootRow::Place(_))).unwrap();
            let places = arranged.watch.iter().filter(|row| matches!(row, RootRow::Place(_))).count();
            assert_eq!(places, sidebar().len(), "every place shown");
            let actions: Vec<String> = arranged
                .watch
                .iter()
                .filter_map(|row| match row {
                    RootRow::Action(action) => Some(action.id.clone()),
                    RootRow::Place(_) => None,
                })
                .collect();
            assert_eq!(first, 1, "New Task leads, then the places");
            actions
        };
        assert_eq!(ids(WatchKind::SyncsItself), ["new-task", "undo", "redo", "what-now", "time-left", "energy-today", "sync-now", "settings"]);
        assert_eq!(ids(WatchKind::ThroughItsPhone), ["new-task", "undo", "redo", "what-now", "time-left", "energy-today", "settings"]);
    }

    #[test]
    fn every_homes_action_is_on_a_watch_or_left_out_by_a_named_rule() {
        for kind in [WatchKind::SyncsItself, WatchKind::ThroughItsPhone] {
            let arranged = arrangement(&sidebar(), kind, true);
            let listed: Vec<&str> = arranged
                .watch
                .iter()
                .filter_map(|row| match row {
                    RootRow::Action(action) => Some(action.id.as_str()),
                    RootRow::Place(_) => None,
                })
                .collect();
            for id in crate::screen_actions::screen_action_ids(None, true, true) {
                let left_out = needs(id).filter(|need| !watch_has(kind, *need));
                assert_eq!(listed.contains(&id), left_out.is_none(), "{id} on a {kind:?} watch: left out by {left_out:?}");
            }
        }
    }
}
