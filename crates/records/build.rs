//! What Lumenna is, read once from the workspace's `[workspace.metadata.lumenna]` (and its
//! licence from `[workspace.package]`), handed to the crate as `LUMENNA_*` for
//! `words::metadata`: the one source every platform's files are checked against.

use std::path::Path;

fn main() {
    let manifest = Path::new(&std::env::var("CARGO_MANIFEST_DIR").expect("cargo sets it")).join("../../Cargo.toml");
    println!("cargo:rerun-if-changed={}", manifest.display());
    let text = std::fs::read_to_string(&manifest).expect("the workspace's Cargo.toml");
    let mut section = String::new();
    let mut found = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            section = line.trim_matches(|c| c == '[' || c == ']').to_owned();
            continue;
        }
        let Some((key, value)) = line.split_once('=') else { continue };
        let (key, value) = (key.trim(), value.trim().trim_matches('"'));
        match (section.as_str(), key) {
            ("workspace.metadata.lumenna", key) => found.push((key.to_uppercase(), value.to_owned())),
            ("workspace.package", "license") => found.push(("LICENCE".to_owned(), value.to_owned())),
            _ => {}
        }
    }
    for key in ["NAME", "DESCRIPTION", "VERSION", "HOMEPAGE", "ISSUES", "PUBLISHER", "ID", "LICENCE"] {
        let value = found.iter().find(|(k, _)| k == key).map(|(_, v)| v.clone()).unwrap_or_else(|| panic!("the workspace's metadata has no {key}"));
        println!("cargo:rustc-env=LUMENNA_{key}={value}");
    }
}
