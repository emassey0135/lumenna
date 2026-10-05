//! Lumenna's core for the apps that link it: iOS and macOS in Swift, Android and Wear OS in
//! Kotlin. UniFFI generates the bindings.
//!
//! There is nothing to define here. The surface — the `Lumenna` object, its operations, and
//! the records they return — is `lumenna_surface`, the same crate the command line calls, built
//! with its `uniffi` feature. This crate exists to be the library an app links: a static one
//! for the Apple targets, a dynamic one for Android and for `uniffi-bindgen` to read the
//! interface out of.
//!
//! So a Swift `try lumenna.addTask(text:)` and `lum task add` run the same function, and a
//! record reshaped in Rust fails to compile in Swift rather than failing to decode at run time.

pub use lumenna_surface::*;

// Without a reference from this crate, a linker may drop the surface's exported functions
// from the library, since nothing here calls them.
lumenna_surface::uniffi_reexport_scaffolding!();

#[cfg(test)]
mod tests {
    use super::*;

    fn open() -> (tempfile::TempDir, std::sync::Arc<Lumenna>) {
        let dir = tempfile::tempdir().unwrap();
        let profile = dir.path().join("profile");
        let lumenna = Lumenna::open(&profile.display().to_string()).unwrap();
        (dir, lumenna)
    }

    #[test]
    fn a_task_added_comes_back_whole_and_is_listed() {
        let (_dir, lumenna) = open();
        let added = lumenna.add_task("review PR p1").unwrap();
        let task = added.task.expect("the created task comes back");
        assert_eq!(task.title, "review PR");
        assert_eq!(task.priority, 1);

        let listed = lumenna.list_tasks("").unwrap();
        assert_eq!(listed.count, 1);
        assert_eq!(listed.rows[0].id, task.id);
        assert!(!lumenna.refresh().unwrap(), "nothing else wrote");
    }

    #[test]
    fn a_store_that_cannot_open_is_an_error_not_a_crash() {
        let file = tempfile::NamedTempFile::new().unwrap();
        // A file where a directory should be.
        assert!(Lumenna::open(&file.path().join("profile").display().to_string()).is_err());
    }
}
