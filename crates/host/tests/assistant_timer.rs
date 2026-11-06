//! An assistant's timer: read, pressed, paused and stopped without naming a sitting. Its own
//! binary, since the assistant's operations take the clock as it is, and this one sets its
//! zone so that the clock reads midday: in the last minutes of a day nothing would start.

use lumenna_host::Lumenna;
use lumenna_records::assistant::Assisted;

/// A zone, as POSIX writes it, where the clock now reads between noon and one.
fn midday() -> String {
    let hour = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() / 3600 % 24;
    let ahead = (12 - hour as i64 + 36) % 24 - 12;
    // A POSIX zone counts west of Greenwich as positive.
    format!("LUM{}", -ahead)
}

#[test]
fn an_assistant_reads_the_timer_presses_it_and_pauses_and_stops_it_without_naming_a_sitting() {
    // SAFETY: the binary's only test, setting the zone before anything reads it or the clock.
    unsafe { std::env::set_var("TZ", midday()) };
    let directory = tempfile::tempdir().unwrap();
    let lumenna = Lumenna::open(directory.path().to_str().unwrap()).unwrap();
    let refused = lumenna.assist("stop_timer", Vec::new()).unwrap_err();
    assert_eq!(refused.message(), "No timer is running.");
    assert!(lumenna.assist("pause_timer", Vec::new()).is_err());

    lumenna.add_task("Email p1").unwrap();
    let Assisted::Timer(idle) = lumenna.assist("timer", Vec::new()).unwrap() else { panic!("not the timer") };
    assert!(!idle.running && idle.assignment.is_empty(), "{idle:?}");
    assert_eq!(idle.announcement, "No timer is running. Starting it would start on Email.");

    let Assisted::Change(started) = lumenna.assist("toggle_timer", Vec::new()).unwrap() else { panic!("not a change") };
    assert!(started.changed && started.announcement.starts_with("Started on Email"), "{started:?}");
    let Assisted::Timer(running) = lumenna.assist("timer", Vec::new()).unwrap() else { panic!("not the timer") };
    assert!(running.running && !running.assignment.is_empty(), "{running:?}");
    assert!(running.announcement.starts_with("The timer is running on Email: "), "{}", running.announcement);

    let Assisted::Timer(paused) = lumenna.assist("pause_timer", Vec::new()).unwrap() else { panic!("not the timer") };
    assert!(paused.changed && !paused.running, "{paused:?}");
    assert_eq!(paused.assignment, running.assignment, "the one that ran");
    lumenna.start_timer(&running.assignment).unwrap();
    let Assisted::Timer(stopped) = lumenna.assist("stop_timer", Vec::new()).unwrap() else { panic!("not the timer") };
    assert!(stopped.changed, "{stopped:?}");
    assert_eq!(lumenna.assist("stop_timer", Vec::new()).unwrap_err().message(), "No timer is running.");
}
