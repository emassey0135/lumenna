//! What each screen offers of its own beside its rows, and Home.
//!
//! **A screen's own actions** (`ScreenAction`, `ViewModel::screen_actions`) are what a place
//! offers that is no row's: New Task, the filter, the day's moves, Undo and Redo. Desktop and
//! phone shells draw them as buttons (a toolbar, an app bar); a braille device lists them on
//! its screen's menu, after the row's actions. Each runs as `Event::Key(id)`, but for what is the
//! shell's own to run (`shell`: the filter field, Key Help, About, the list of reminders due).
//!
//! **Home** (`HomeView`, `Event::Home`) is where a shell without a sidebar or tabs (a braille
//! device, and an Emacs buffer if it wants one) starts: the places, as the sidebar has them,
//! then the app-wide actions. A desktop or a phone never shows it; its places are its sidebar or
//! its tabs, and its app-wide actions are the menu bar's or the Browse tab's.

use lumenna_records::places::{Place, SidebarEntry, SidebarGroup, SidebarKind};
use serde::{Deserialize, Serialize};

use crate::keys::command_of;
use crate::model::Screen;

/// One of a screen's own actions, beside its rows.
#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScreenAction {
    /// What it is: the shared table's `id` ("new-task", "go-to-day", "what-now"), or
    /// `new-project-inside`, `new-section`, `reminders-due`. Sent back as `Event::Key(id)`, unless `shell`.
    pub id: String,
    /// Its name, in the shell's capitals.
    pub title: String,
    /// Its letter on a braille device (`braille::command_letter`); empty for none.
    pub letter: String,
    /// Whether it asks something first, which a platform marks its own way ("…").
    pub asks: bool,
    /// Whether it is the shell's own to run, not the app's: the filter field above the list, the
    /// keyboard help, About, the list of reminders due (`ViewModel::due`). Every other one is
    /// sent as `Event::Key(id)`.
    pub shell: bool,
    /// Whether it does anything now: Undo with nothing to undo is still offered, but What Should
    /// I Work On? is not where today offers nothing to work on.
    pub enabled: bool,
}

/// Home: the places, then what can be done from anywhere. Only a shell that shows Home asks for
/// it (`Event::Home`); while it is shown, `ViewModel::home` is set.
#[boltffi::data]
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HomeView {
    /// What it is called, in the shell's capitals.
    pub title: String,
    /// The places, each a sidebar entry (`ViewModel::sidebar`'s, folded as the sidebar is), with
    /// its letter. The row in hand is the sidebar's (`ViewModel::sidebar_held`; a shell reports a
    /// move with `Event::Hold(Side::Sidebar, key)`), and `Key("open")` opens it: a place goes
    /// there, a heading folds or unfolds.
    pub places: Vec<HomePlace>,
    /// What can be done from anywhere, in order, after the places: Reminders Due first while any
    /// are due, then New Task, Undo, Redo, What Should I Work On?, Sync Now, Settings, Key Help
    /// and About. Also `ViewModel::screen_actions` while Home is shown.
    pub actions: Vec<ScreenAction>,
}

/// One place on Home.
#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HomePlace {
    /// The sidebar's entry: its key, its line, its folding and its own actions.
    pub entry: SidebarEntry,
    /// Its letter: Today's, a heading's; empty for a project, label or saved filter, which a
    /// letter alone finds by its first letter.
    pub letter: String,
}

/// The ids of what `place` offers of its own, in order.
pub(crate) fn place_action_ids(place: &Place) -> &'static [&'static str] {
    match place {
        // What Should I Work On? is the day's own action here (`DayView::offered`), never a second time.
        Place::Today => &["new-block", "previous-day", "next-day", "go-to-day", "go-to-now", "undo", "redo"],
        Place::Blocks => &["new-block", "undo", "redo"],
        // Nothing is added to the trash or the history, and a filter there shows nothing useful.
        Place::Trash | Place::History => &["undo", "redo"],
        Place::Project(_) => &["new-task", "filter", "new-project-inside", "new-section", "undo", "redo"],
        _ => &["new-task", "filter", "undo", "redo"],
    }
}

/// What Home offers of its own, after Reminders Due while any are due.
pub(crate) const HOME_ACTIONS: &[&str] = &["new-task", "undo", "redo", "what-now", "sync-now", "settings", "keyboard-help", "about"];

/// What a shell runs itself, of the screens' own actions.
const SHELLS: &[&str] = &["filter", "keyboard-help", "about", "reminders-due"];

/// Every place a screen's actions are worked out for, one of each kind: what the tests and the
/// braille menu bar range over.
#[must_use]
pub fn every_kind_of_place() -> Vec<Place> {
    vec![
        Place::Today,
        Place::Tasks,
        Place::Blocks,
        Place::History,
        Place::Trash,
        Place::Project(String::new()),
        Place::Label(String::new()),
        Place::Filter { name: String::new(), query: String::new() },
    ]
}

/// The ids of what the screen offers of its own: Home's (`home`), else `place`'s.
#[must_use]
pub fn screen_action_ids(place: Option<&Place>, home: bool, due: bool) -> Vec<&'static str> {
    if home {
        return due.then_some("reminders-due").into_iter().chain(HOME_ACTIONS.iter().copied()).collect();
    }
    place.map(|place| place_action_ids(place).to_vec()).unwrap_or_default()
}

/// A screen's own action's name by its `id`, in sentence case where `sentence`: the shared
/// table's where it has one.
#[must_use]
pub fn action_title(id: &str, sentence: bool) -> String {
    let title = match id {
        "new-project-inside" => "New Project Inside",
        "new-section" => "New Section",
        "reminders-due" => return lumenna_records::words::reminders_due(sentence),
        other => command_of(other).map_or(other, |command| command.title),
    };
    if sentence { lumenna_records::fields::sentence_case(title.to_owned()) } else { title.to_owned() }
}

/// The record of the action `id`, enabled as `enabled`.
#[must_use]
pub fn screen_action(id: &str, sentence: bool, enabled: bool) -> ScreenAction {
    ScreenAction {
        id: id.to_owned(),
        title: action_title(id, sentence),
        letter: crate::braille::command_letter(id).unwrap_or_default().to_owned(),
        asks: matches!(id, "new-project-inside" | "new-section") || command_of(id).is_some_and(|command| command.asks),
        shell: SHELLS.contains(&id),
        enabled,
    }
}

/// What Home calls itself: one word, the same in either capitals.
#[must_use]
pub fn home_title() -> String {
    "Home".to_owned()
}

/// The letter a place has on Home, by what it opens.
#[must_use]
pub fn place_letter(kind: &SidebarKind) -> Option<&'static str> {
    let id = match kind {
        SidebarKind::Place(Place::Today) => "go-today",
        SidebarKind::Place(Place::Tasks) => "go-tasks",
        SidebarKind::Place(Place::Blocks) => "go-blocks",
        SidebarKind::Place(Place::History) => "go-history",
        SidebarKind::Place(Place::Trash) => "go-trash",
        SidebarKind::Group(SidebarGroup::Projects) => "projects",
        SidebarKind::Group(SidebarGroup::Labels) => "labels",
        SidebarKind::Group(SidebarGroup::Filters) => "filters",
        SidebarKind::Place(_) => return None,
    };
    crate::braille::command_letter(id)
}

impl Screen {
    /// Whether `id`, one of a screen's own actions, does anything now.
    fn action_enabled(&self, id: &str) -> bool {
        match id {
            "what-now" => self.today_offers(lumenna_records::ActionKind::WhatNow),
            "new-project-inside" => self.project_shown_offers_new_inside(),
            "new-section" => self.new_section_shown().is_some(),
            "reminders-due" => !self.due.is_empty(),
            _ => true,
        }
    }

    /// Whether today's plan offers `kind` of its own.
    pub(crate) fn today_offers(&self, kind: lumenna_records::ActionKind) -> bool {
        self.today_view().is_some_and(|day| day.actions.iter().any(|action| action.kind == kind))
    }

    /// Whether the project shown has New Project Inside.
    fn project_shown_offers_new_inside(&self) -> bool {
        self.new_inside_shown().is_some()
    }

    /// The project shown's New Project Inside, as its sidebar row offers it.
    pub(crate) fn new_inside_shown(&self) -> Option<lumenna_records::Action> {
        let place = self.place();
        if !matches!(place, Place::Project(_)) {
            return None;
        }
        let store = self.store.as_ref()?;
        lumenna_records::places::sidebar(&store.snapshot, &store.now)
            .into_iter()
            .find(|entry| matches!(&entry.kind, SidebarKind::Place(found) if *found == place))
            .and_then(|entry| entry.actions.into_iter().find(|action| action.kind == lumenna_records::ActionKind::NewInside))
    }

    /// The project shown's New Section, as its sidebar row offers it.
    pub(crate) fn new_section_shown(&self) -> Option<lumenna_records::Action> {
        self.project_shown_offers(|action| action.kind == lumenna_records::ActionKind::New && action.subject == lumenna_records::Subject::Section)
    }

    /// The first of the project shown's sidebar row's actions `wanted` takes.
    fn project_shown_offers(&self, wanted: impl Fn(&lumenna_records::Action) -> bool) -> Option<lumenna_records::Action> {
        let place = self.place();
        if !matches!(place, Place::Project(_)) {
            return None;
        }
        let store = self.store.as_ref()?;
        lumenna_records::places::sidebar(&store.snapshot, &store.now)
            .into_iter()
            .find(|entry| matches!(&entry.kind, SidebarKind::Place(found) if *found == place))
            .and_then(|entry| entry.actions.into_iter().find(|action| wanted(action)))
    }

    /// The screen's own actions now: Home's while it is shown, else the place's.
    pub(crate) fn screen_actions(&self) -> Vec<ScreenAction> {
        let sentence = self.capitals == crate::Capitals::Sentence;
        let place = self.place();
        screen_action_ids(Some(&place), self.home, !self.due.is_empty())
            .into_iter()
            .map(|id| screen_action(id, sentence, self.action_enabled(id)))
            .collect()
    }

    /// Home, as shown, over the sidebar as the view has it (folded, in the shell's capitals).
    pub(crate) fn home_view(&self, sidebar: &[SidebarEntry]) -> HomeView {
        HomeView {
            title: home_title(),
            places: sidebar
                .iter()
                .map(|entry| HomePlace { letter: place_letter(&entry.kind).unwrap_or_default().to_owned(), entry: entry.clone() })
                .collect(),
            actions: self.screen_actions(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys::{MENUS, Runs, SEPARATOR, what_runs};

    /// What is a window's or a pane's own: a shell without windows or panes has no use for it.
    const A_WINDOWS_OWN: &[&str] = &["show", "next-pane", "previous-pane", "new-window", "close-window", "quit"];

    /// What a form has as its own button: the task form's Save (`DetailsEvent::Save`).
    const A_FORMS_OWN: &[&str] = &["save-task"];

    /// The places Home lists by a letter of their own, as the ids those letters are.
    fn home_places() -> Vec<&'static str> {
        vec!["go-today", "go-tasks", "go-blocks", "go-history", "go-trash"]
    }

    /// The heading whose New a command is.
    fn heading_of(id: &str) -> Option<SidebarGroup> {
        match id {
            "new-project" => Some(SidebarGroup::Projects),
            "new-label" => Some(SidebarGroup::Labels),
            "new-filter" => Some(SidebarGroup::Filters),
            _ => None,
        }
    }

    #[test]
    fn every_command_of_the_menu_bar_is_reachable_without_one() {
        let pages = lumenna_records::fields::settings_pages(false, false);
        let mut unreached = Vec::new();
        for id in MENUS.iter().flat_map(|menu| menu.items.iter().copied()).filter(|id| *id != SEPARATOR) {
            if A_WINDOWS_OWN.contains(&id) || A_FORMS_OWN.contains(&id) {
                continue;
            }
            let on_home = HOME_ACTIONS.contains(&id) || home_places().contains(&id);
            let on_a_place = every_kind_of_place().iter().any(|place| place_action_ids(place).contains(&id));
            let on_a_row = matches!(what_runs(id), Runs::Row(_) | Runs::Day(_));
            let on_a_heading = heading_of(id).is_some_and(|group| {
                lumenna_records::actions::heading(group).iter().any(|action| action.kind == lumenna_records::ActionKind::New)
            });
            let in_settings = pages.iter().any(|page| page.buttons.iter().any(|button| button.id == id) || Some(page.id.as_str()) == crate::keys::settings_page_of(id));
            if !(on_home || on_a_place || on_a_row || on_a_heading || in_settings) {
                unreached.push(id);
            }
        }
        assert!(unreached.is_empty(), "only a menu bar reaches {unreached:?}");
    }

    #[test]
    fn every_screen_action_is_in_the_braille_menu_bar_with_its_hot_key() {
        let screens: Vec<(Option<Place>, bool)> =
            every_kind_of_place().into_iter().map(|place| (Some(place), false)).chain([(None, true)]).collect();
        for (place, home) in screens {
            let bar = crate::braille::menu_bar(place.as_ref(), home, true);
            let first = &bar[0];
            assert_eq!(first.id, crate::braille::SCREEN_MENU, "the screen's own menu leads over {place:?} (home: {home})");
            let own: Vec<&str> = first.items.iter().map(|item| item.id.as_str()).collect();
            assert_eq!(own, screen_action_ids(place.as_ref(), home, true), "the screen's own menu over {place:?} (home: {home})");
            // Each is in the shared menus as well, and in both with its letter as a hot key.
            for id in screen_action_ids(place.as_ref(), home, true) {
                let mine = first.items.iter().find(|item| item.id == id).expect("in the screen's own menu");
                assert!(mine.hot_key && Some(mine.letter.as_str()) == crate::braille::command_letter(id), "{id} in the screen's own menu");
                let item = bar
                    .iter()
                    .skip(1)
                    .flat_map(|menu| &menu.items)
                    .find(|item| item.id == id)
                    .unwrap_or_else(|| panic!("{id} is not in the menu bar over {place:?} (home: {home})"));
                let letter = crate::braille::command_letter(id).unwrap_or_default();
                assert_eq!(item.letter, letter, "{id}'s letter in the menu bar over {place:?} (home: {home})");
                assert!(item.hot_key, "{id} is no hot key over {place:?} (home: {home})");
            }
        }
    }

    #[test]
    fn what_should_i_work_on_is_todays_own_once_and_home_ends_in_the_help() {
        assert!(!place_action_ids(&Place::Today).contains(&"what-now"), "the day offers it of its own");
        let today = crate::braille::menu_bar(Some(&Place::Today), false, true);
        let what_now = today.iter().flat_map(|menu| &menu.items).find(|item| item.id == "what-now").expect("in the Day menu");
        assert!(what_now.hot_key && what_now.letter == "w", "its letter runs the day's own: {what_now:?}");
        for quiet in [Place::Trash, Place::History] {
            assert_eq!(place_action_ids(&quiet), ["undo", "redo"], "nothing to add or filter in {quiet:?}");
        }
        let home = screen_action_ids(None, true, true);
        assert_eq!(home.first(), Some(&"reminders-due"), "Reminders Due leads while any are due");
        assert_eq!(&home[home.len() - 2..], ["keyboard-help", "about"]);
        assert!(!screen_action_ids(None, true, false).contains(&"reminders-due"));
    }

    #[test]
    fn what_the_shell_runs_itself_is_marked_and_the_rest_are_keys() {
        assert!(screen_action("filter", true, true).shell);
        assert!(screen_action("about", true, true).shell);
        assert!(!screen_action("new-task", true, true).shell);
        assert!(screen_action("new-task", true, true).asks);
        assert_eq!(screen_action("go-to-now", true, true).title, "Go to now");
        assert_eq!(screen_action("new-project-inside", false, true).title, "New Project Inside");
    }
}
