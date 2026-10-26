//! A fingerprint of how this build reads records out of the documents, for the snapshot parts
//! kept between runs (`Store::snapshot`): a part one build materialized is used by another only
//! when they read every record alike. Every source of the model and of hydration is hashed, so a
//! changed field, default or reading invalidates what was kept, and nobody has to remember to.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

fn main() {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("set by Cargo"));
    let roots = [manifest.join("src"), manifest.join("../core/src")];
    let mut files = Vec::new();
    for root in &roots {
        println!("cargo:rerun-if-changed={}", root.display());
        collect(root, &mut files);
    }
    files.sort();
    let mut hasher = DefaultHasher::new();
    env!("CARGO_PKG_VERSION").hash(&mut hasher);
    for file in &files {
        file.strip_prefix(&manifest).unwrap_or(file).hash(&mut hasher);
        std::fs::read(file).expect("a source file reads").hash(&mut hasher);
    }
    // The dependencies' versions too: a new Automerge, jiff or rrule can read the same document
    // into other records, with not a line of this source changed.
    let lock = manifest.join("../../Cargo.lock");
    println!("cargo:rerun-if-changed={}", lock.display());
    if let Ok(bytes) = std::fs::read(&lock) {
        bytes.hash(&mut hasher);
    }
    println!("cargo:rustc-env=LUMENNA_HYDRATION_BUILD={:016x}", hasher.finish());
}

fn collect(dir: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(&path, files);
        } else if path.extension().is_some_and(|e| e == "rs") {
            files.push(path);
        }
    }
}
