//! How a client learns that another process wrote to the store.

use lumenna_surface::Lumenna;

#[test]
fn the_version_moves_when_another_process_writes_even_after_a_refresh_took_it_in() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().to_str().unwrap();
    let app = Lumenna::open(path).unwrap();
    let lum = Lumenna::open(path).unwrap();
    let before = app.version().unwrap();

    lum.add_task("Call the bank").unwrap();
    // Something else on the app's connection — its sync loop, any operation — refreshes
    // first, so asking `refresh` now would say nothing changed.
    assert!(app.refresh().unwrap());
    assert!(!app.refresh().unwrap());

    assert_ne!(app.version().unwrap(), before);
}

#[test]
fn the_version_moves_for_this_connections_own_writes_too() {
    let directory = tempfile::tempdir().unwrap();
    let app = Lumenna::open(directory.path().to_str().unwrap()).unwrap();
    let before = app.version().unwrap();
    app.add_task("Buy milk").unwrap();
    assert_ne!(app.version().unwrap(), before);
}

#[test]
fn the_version_stays_put_when_nothing_is_written() {
    let directory = tempfile::tempdir().unwrap();
    let app = Lumenna::open(directory.path().to_str().unwrap()).unwrap();
    app.list_tasks("").unwrap();
    assert_eq!(app.version().unwrap(), app.version().unwrap());
}
