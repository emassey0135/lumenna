//! Energy through the surface: a task rated, a block marked, the settings, the day's check-in,
//! and what Plan the Day and Replan from Now make of them, said in words.

use lumenna_host::{BlockEdit, BlockScope, DayPins, Lumenna, NewBlock, ReplanPins, TaskEdit, TimeStyle};

const NOW: &str = "2026-05-06T08:00";
const DAY: &str = "2026-05-06";

fn open() -> (tempfile::TempDir, Lumenna) {
    let directory = tempfile::tempdir().unwrap();
    let lumenna = Lumenna::open(directory.path().to_str().unwrap()).unwrap();
    (directory, std::sync::Arc::try_unwrap(lumenna).ok().unwrap())
}

fn block(lumenna: &Lumenna, title: &str, at: &str, minutes: u32) -> String {
    lumenna
        .add_block(NewBlock {
            title: title.to_owned(),
            at: at.to_owned(),
            minutes,
            date: Some(DAY.to_owned()),
            kind: "work".to_owned(),
            ..NewBlock::default()
        })
        .unwrap();
    lumenna.list_blocks().unwrap().rows.iter().find(|r| r.title == title).unwrap().id.clone()
}

fn task(lumenna: &Lumenna, line: &str, energy: Option<&str>) -> String {
    let id = lumenna.add_task(line).unwrap().task.unwrap().id;
    if let Some(energy) = energy {
        lumenna.edit_task(&id, TaskEdit { energy: Some(energy.to_owned()), ..TaskEdit::default() }).unwrap();
    }
    id
}

#[test]
fn a_tasks_energy_is_set_shown_and_cleared() {
    let (_directory, lumenna) = open();
    let essay = task(&lumenna, "Essay", Some("high"));
    assert_eq!(lumenna.show_task(&essay).unwrap().task.energy.as_deref(), Some("high"));
    assert!(lumenna.edit_task(&essay, TaskEdit { energy: Some("loud".to_owned()), ..TaskEdit::default() }).is_err());
    lumenna.edit_task(&essay, TaskEdit { energy: Some("none".to_owned()), ..TaskEdit::default() }).unwrap();
    assert_eq!(lumenna.show_task(&essay).unwrap().task.energy, None);
}

#[test]
fn the_energy_settings_read_back_as_they_are_said() {
    let (_directory, lumenna) = open();
    lumenna.set_setting("energy-capacity", "4h").unwrap();
    lumenna.set_setting("best-hours", "9am to noon").unwrap();
    lumenna.set_setting("energy-check-in", "8:30am").unwrap();
    let value = |key: &str| lumenna.settings(Some(key.to_owned())).unwrap().settings[0].value.clone();
    assert_eq!(value("energy-capacity"), "4h");
    assert_eq!(value("best-hours"), "09:00 to 12:00");
    assert_eq!(value("worst-hours"), "");
    assert_eq!(value("energy-check-in"), "08:30");
    assert!(lumenna.set_setting("best-hours", "noon to 9am").is_err());
    lumenna.set_setting("energy-capacity", "none").unwrap();
    assert_eq!(value("energy-capacity"), "none");
}

#[test]
fn checking_in_low_says_what_planning_makes_of_it_and_is_undone_whole() {
    let (_directory, lumenna) = open();
    lumenna.set_setting("energy-capacity", "4h").unwrap();
    let said = lumenna.check_in_energy("low", Some(NOW.to_owned())).unwrap();
    assert_eq!(said.announcement, "You are low today. Plan the Day plans up to 2 hours of work, half the usual 4 hours.");
    assert!(lumenna.check_in_energy("sleepy", Some(NOW.to_owned())).is_err());
    lumenna.undo().unwrap();
    assert!(lumenna.check_in_energy("usual", Some(NOW.to_owned())).unwrap().announcement.starts_with("Your energy is as usual today."));
}

#[test]
fn plan_the_day_puts_demanding_work_in_the_best_hours_and_stops_at_the_days_room() {
    let (_directory, lumenna) = open();
    let morning = block(&lumenna, "Morning", "09:00", 120);
    block(&lumenna, "Afternoon", "14:00", 120);
    let series = morning.split('@').next().unwrap().to_owned();
    lumenna
        .edit_block(&series, BlockEdit { hours: Some("best".to_owned()), ..BlockEdit::default() }, BlockScope::Series)
        .unwrap();
    for (line, energy) in [("Filing p1", "low"), ("Essay p2", "high")] {
        let id = task(&lumenna, line, Some(energy));
        lumenna.edit_task(&id, TaskEdit { estimate: Some("60m".to_owned()), ..TaskEdit::default() }).unwrap();
    }
    lumenna.set_setting("energy-capacity", "2h").unwrap();
    let plan = lumenna.propose_day_plan(Some(DAY.to_owned()), TimeStyle::TwelveHour, DayPins::default(), Some(NOW.to_owned())).unwrap();
    let essay = plan.sittings.iter().find(|s| s.sentence.starts_with("Essay")).expect("Essay is planned");
    assert!(essay.sentence.contains("Morning"), "{}", essay.sentence);
    assert!(essay.reason.as_deref().unwrap_or_default().contains("these are your best hours"), "{:?}", essay.reason);
    // Essay spends the two hours alone, counting double: Filing is left for another day.
    assert_eq!(plan.sittings.len(), 1, "{}", plan.announcement);
    assert!(plan.announcement.contains("Planned to your energy: 2 hours of the 2 hours today has room for"), "{}", plan.announcement);
}

#[test]
fn replan_from_now_lightens_a_low_day() {
    let (_directory, lumenna) = open();
    let morning = block(&lumenna, "Morning", "09:00", 120);
    lumenna.set_setting("energy-capacity", "2h").unwrap();
    for (line, energy) in [("Essay", Some("high")), ("Email", None)] {
        let id = task(&lumenna, line, energy);
        lumenna.assign(&id, &morning, Some(DAY.to_owned()), Some(45)).unwrap();
    }
    let calm = lumenna.replan(TimeStyle::TwelveHour, ReplanPins::default(), Some(NOW.to_owned())).unwrap();
    assert!(calm.changes.is_empty(), "{}", calm.announcement);
    let said = lumenna.check_in_energy("low", Some(NOW.to_owned())).unwrap();
    assert!(said.announcement.ends_with("and Replan from Now can lighten the rest of the day."), "{}", said.announcement);
    let lighter = lumenna.replan(TimeStyle::TwelveHour, ReplanPins::default(), Some(NOW.to_owned())).unwrap();
    assert_eq!(
        lighter.announcement,
        "Essay leaves Morning to lighten today, since it takes high energy, and you are low today, and goes back to the list."
    );
    lumenna.apply_replan(lighter.clone(), lighter.changes, Some(NOW.to_owned())).unwrap();
    let day = lumenna.plan(Some(DAY.to_owned())).unwrap();
    let left: Vec<&str> = day.blocks[0].assignments.iter().map(|a| a.title.as_str()).collect();
    assert_eq!(left, ["Email"]);
}
