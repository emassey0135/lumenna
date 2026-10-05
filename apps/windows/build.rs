//! Embeds the application manifest, without a resource compiler.
//!
//! The manifest is what selects Common Controls 6 and per-monitor DPI awareness. The MSVC
//! linker embeds it itself, so no `.rc` file or `rc.exe` is needed.

fn main() {
    println!("cargo:rerun-if-changed=lumenna.manifest");
    let os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    if os != "windows" {
        return;
    }
    let manifest = std::path::Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default())
        .join("lumenna.manifest");
    if env == "msvc" {
        println!("cargo:rustc-link-arg-bins=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg-bins=/MANIFESTINPUT:{}", manifest.display());
    } else {
        println!(
            "cargo:warning=the manifest is only embedded by the MSVC linker; without it the app \
             has no Common Controls 6 and no DPI awareness"
        );
    }
}
