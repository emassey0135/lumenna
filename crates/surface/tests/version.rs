//! How a client learns that another process wrote to the store.

use lumenna_surface::Lumenna;

#[test]
fn the_outside_version_moves_for_another_process_and_not_for_this_ones_own_edits() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().to_str().unwrap();
    let app = Lumenna::open(path).unwrap();
    let lum = Lumenna::open(path).unwrap();
    let start = app.outside_version().unwrap();

    app.add_task("Buy milk").unwrap();
    assert_eq!(app.outside_version().unwrap(), start, "the app's own edit is not news to it");

    lum.add_task("Call the bank").unwrap();
    // Taken in by something else on the app's connection first, as its sync loop would.
    assert!(app.refresh().unwrap());
    let after = app.outside_version().unwrap();
    assert_ne!(after, start, "another process wrote");
    assert_eq!(app.outside_version().unwrap(), after, "and asking again is not news");
}

#[test]
fn the_outside_version_stays_put_when_nothing_is_written() {
    let directory = tempfile::tempdir().unwrap();
    let app = Lumenna::open(directory.path().to_str().unwrap()).unwrap();
    app.list_tasks("").unwrap();
    assert_eq!(app.outside_version().unwrap(), app.outside_version().unwrap());
}
