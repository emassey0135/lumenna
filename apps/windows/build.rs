//! Embeds the application manifest and the icon, without a resource compiler.
//!
//! The manifest is what selects Common Controls 6 and per-monitor DPI awareness. The MSVC
//! linker embeds it itself, and takes a compiled resource file (`.res`) as input like an object,
//! so the icon is written as one here: no `.rc` file or `rc.exe` is needed.

/// The icon's resource id, which the app loads it by (`win::app::ICON`).
const ICON: u16 = 1;

fn main() {
    println!("cargo:rerun-if-changed=lumenna.manifest");
    println!("cargo:rerun-if-changed=lumenna.ico");
    let os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    if os != "windows" {
        return;
    }
    let directory = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default());
    let manifest = directory.join("lumenna.manifest");
    if env == "msvc" {
        println!("cargo:rustc-link-arg-bins=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg-bins=/MANIFESTINPUT:{}", manifest.display());
        let icon = std::fs::read(directory.join("lumenna.ico")).expect("apps/windows/lumenna.ico is there");
        let resources = std::path::PathBuf::from(std::env::var("OUT_DIR").expect("Cargo sets OUT_DIR")).join("lumenna.res");
        std::fs::write(&resources, icon_resources(&icon)).expect("the resources are written");
        println!("cargo:rustc-link-arg-bins={}", resources.display());
    } else {
        println!(
            "cargo:warning=the manifest and icon are only embedded by the MSVC linker; without the \
             manifest the app has no Common Controls 6 and no DPI awareness"
        );
    }
}

/// An icon file as a compiled resource file: each image an `RT_ICON` of its own, numbered from
/// 1, and an `RT_GROUP_ICON` (`ICON`) listing them, which is what Explorer and `LoadIcon` read.
fn icon_resources(ico: &[u8]) -> Vec<u8> {
    const RT_ICON: u16 = 3;
    const RT_GROUP_ICON: u16 = 14;
    let word = |at: usize| u16::from_le_bytes([ico[at], ico[at + 1]]);
    let dword = |at: usize| u32::from_le_bytes([ico[at], ico[at + 1], ico[at + 2], ico[at + 3]]);

    // A resource file starts with an empty entry, by which it is told from a 16-bit one.
    let mut out = entry(0, 0, &[]);
    let count = word(4);
    let mut group = Vec::new();
    group.extend(0u16.to_le_bytes());
    group.extend(1u16.to_le_bytes());
    group.extend(count.to_le_bytes());
    for index in 0..count {
        let at = 6 + 16 * usize::from(index);
        let (size, offset) = (dword(at + 8) as usize, dword(at + 12) as usize);
        let id = index + 1;
        out.extend(entry(RT_ICON, id, &ico[offset..offset + size]));
        // The file's entry less its offset, and the image's id in its place.
        group.extend(&ico[at..at + 12]);
        group.extend(id.to_le_bytes());
    }
    out.extend(entry(RT_GROUP_ICON, ICON, &group));
    out
}

/// One resource, named and typed by number, in US English, padded to a whole double word.
fn entry(kind: u16, name: u16, data: &[u8]) -> Vec<u8> {
    // Movable, pure, discardable, as rc.exe marks an icon; nothing for the empty first entry.
    let flags: u16 = if kind == 0 { 0 } else { 0x1030 };
    let language: u16 = if kind == 0 { 0 } else { 0x0409 };
    let mut out = Vec::new();
    out.extend((data.len() as u32).to_le_bytes());
    out.extend(32u32.to_le_bytes());
    for number in [kind, name] {
        out.extend(0xFFFFu16.to_le_bytes());
        out.extend(number.to_le_bytes());
    }
    out.extend(0u32.to_le_bytes());
    out.extend(flags.to_le_bytes());
    out.extend(language.to_le_bytes());
    out.extend(0u32.to_le_bytes());
    out.extend(0u32.to_le_bytes());
    out.extend(data);
    out.resize(out.len().next_multiple_of(4), 0);
    out
}
