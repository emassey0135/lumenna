//! Backups and imports as a file's contents rather than a path — what a browser has.

use lumenna_surface::{Imported, Lumenna};

fn open() -> (tempfile::TempDir, std::sync::Arc<Lumenna>) {
    let directory = tempfile::tempdir().unwrap();
    let lumenna = Lumenna::open(directory.path().join("profile").to_str().unwrap()).unwrap();
    (directory, lumenna)
}

#[test]
fn a_backup_taken_as_bytes_restores_into_another_store_through_import_bytes() {
    let (_one, here) = open();
    here.add_task("Write report").unwrap();
    let file = here.backup_file().unwrap();
    assert!(file.name.starts_with("lumenna-") && file.name.ends_with(".lumbak"), "{}", file.name);

    let (_two, there) = open();
    let imported = there.import_bytes(&file.name, file.bytes).unwrap();
    assert!(matches!(imported, Imported::Backup { .. }), "{imported:?}");
    let titles: Vec<String> = there.list_tasks("").unwrap().rows.into_iter().map(|row| row.title).collect();
    assert_eq!(titles, ["Write report"]);
}

#[test]
fn contents_that_are_neither_an_export_nor_a_backup_are_refused() {
    let (_directory, lumenna) = open();
    let refused = lumenna.import_bytes("notes.txt", b"not lumenna".to_vec()).unwrap_err();
    assert!(refused.message().contains("not a Lumenna export"), "{}", refused.message());
}
