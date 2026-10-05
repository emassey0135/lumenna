//! The shortcuts that work from anywhere (§16.2): what they are, how they are named, and
//! which choices of keys to warn about.

/// What a shortcut from anywhere does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Shows the main window.
    Show,
    /// Opens quick add over whatever is in front.
    QuickAdd,
}

impl Kind {
    pub const ALL: [Self; 2] = [Self::Show, Self::QuickAdd];

    /// What it is called in Settings and in a failure.
    pub fn name(self) -> &'static str {
        match self {
            Self::Show => "Show Lumenna",
            Self::QuickAdd => "Quick add a task",
        }
    }

    /// The registry value it is kept under.
    pub fn value_name(self) -> &'static str {
        match self {
            Self::Show => "Show",
            Self::QuickAdd => "QuickAdd",
        }
    }

    /// Its identifier with `RegisterHotKey`.
    pub fn id(self) -> i32 {
        match self {
            Self::Show => 1,
            Self::QuickAdd => 2,
        }
    }

    /// What it is until the person changes it: Control+Alt+Shift with L for Lumenna, and K —
    /// the letters the Mac uses with Control-Command.
    pub fn standard(self) -> Shortcut {
        let key = match self {
            Self::Show => u16::from(b'L'),
            Self::QuickAdd => u16::from(b'K'),
        };
        Shortcut { control: true, alt: true, shift: true, windows: false, key }
    }
}

/// A combination of keys: modifiers and one virtual key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Shortcut {
    pub control: bool,
    pub alt: bool,
    pub shift: bool,
    pub windows: bool,
    /// A Windows virtual-key code.
    pub key: u16,
}

// `RegisterHotKey`'s modifier flags, which are also how a shortcut is kept.
const ALT: u32 = 1;
const CONTROL: u32 = 2;
const SHIFT: u32 = 4;
const WINDOWS: u32 = 8;

impl Shortcut {
    /// `RegisterHotKey`'s modifier flags.
    pub fn modifiers(self) -> u32 {
        [(self.alt, ALT), (self.control, CONTROL), (self.shift, SHIFT), (self.windows, WINDOWS)]
            .into_iter()
            .filter(|(on, _)| *on)
            .fold(0, |flags, (_, flag)| flags | flag)
    }

    /// As one number, for the registry: the modifiers above the key. Never zero, which
    /// stands for a shortcut turned off.
    pub fn encode(self) -> u32 {
        (self.modifiers() << 16) | u32::from(self.key)
    }

    /// From [`encode`](Self::encode); `None` for zero, a shortcut turned off.
    pub fn decode(value: u32) -> Option<Self> {
        let key = (value & 0xFFFF) as u16;
        if key == 0 {
            return None;
        }
        let modifiers = value >> 16;
        Some(Self {
            control: modifiers & CONTROL != 0,
            alt: modifiers & ALT != 0,
            shift: modifiers & SHIFT != 0,
            windows: modifiers & WINDOWS != 0,
            key,
        })
    }

    /// How it is said and written: "Control+Alt+Shift+L".
    pub fn describe(self) -> String {
        let mut parts: Vec<String> = [
            (self.control, "Control"),
            (self.alt, "Alt"),
            (self.shift, "Shift"),
            (self.windows, "Windows"),
        ]
        .into_iter()
        .filter(|(on, _)| *on)
        .map(|(_, name)| name.to_owned())
        .collect();
        parts.push(key_name(self.key));
        parts.join("+")
    }

    /// Why this combination is a poor choice, if it is: it is said before it is saved, so
    /// the person can choose again.
    pub fn warning(self) -> Option<&'static str> {
        if !self.control && !self.alt && !self.windows {
            return Some("A shortcut from anywhere needs Control, Alt or the Windows key, or it takes that key away from typing in every program.");
        }
        if self.control && self.alt && !self.shift && !self.windows {
            return Some("Control+Alt is AltGr on many keyboards, where it types letters; those letters would stop working everywhere.");
        }
        if self.windows {
            return Some("Windows keeps many Windows-key combinations for itself, and may take this one later.");
        }
        None
    }
}

/// What is said when Windows would not give this copy its shortcuts.
///
/// Windows does not say who holds a combination of keys, but when another copy of Lumenna is
/// open — on another profile — it is almost certainly that one, and calling it another
/// program would send the person looking for one that is not there.
pub fn taken(described: &[String], by_another_copy: bool) -> String {
    let keys = described.join(" and ");
    let they = if described.len() == 1 { "it does" } else { "they do" };
    if by_another_copy {
        format!("Another copy of Lumenna, open on another profile, already uses {keys}, so {they} nothing in this one.")
    } else {
        format!("Another program already uses {keys}, so {they} nothing here.")
    }
}

/// What is said when new keys for a shortcut are already taken.
pub fn refused(described: &str, by_another_copy: bool) -> String {
    if by_another_copy {
        format!("Another copy of Lumenna, open on another profile, already uses {described}. Choose other keys, or change them there.")
    } else {
        format!("Another program already uses {described}. Choose other keys.")
    }
}

/// A virtual key's name: letters and digits as themselves, F-keys and the common rest by
/// name.
pub fn key_name(key: u16) -> String {
    match key {
        0x30..=0x39 | 0x41..=0x5A => char::from(key as u8).to_string(),
        0x70..=0x87 => format!("F{}", key - 0x6F),
        0x08 => "Backspace".to_owned(),
        0x09 => "Tab".to_owned(),
        0x0D => "Enter".to_owned(),
        0x1B => "Escape".to_owned(),
        0x20 => "Space".to_owned(),
        0x21 => "Page Up".to_owned(),
        0x22 => "Page Down".to_owned(),
        0x23 => "End".to_owned(),
        0x24 => "Home".to_owned(),
        0x25 => "Left".to_owned(),
        0x26 => "Up".to_owned(),
        0x27 => "Right".to_owned(),
        0x28 => "Down".to_owned(),
        0x2D => "Insert".to_owned(),
        0x2E => "Delete".to_owned(),
        0x60..=0x69 => format!("Numpad {}", key - 0x60),
        0xBA => "Semicolon".to_owned(),
        0xBB => "Equals".to_owned(),
        0xBC => "Comma".to_owned(),
        0xBD => "Minus".to_owned(),
        0xBE => "Period".to_owned(),
        0xBF => "Slash".to_owned(),
        0xC0 => "Grave".to_owned(),
        0xDB => "Left Bracket".to_owned(),
        0xDC => "Backslash".to_owned(),
        0xDD => "Right Bracket".to_owned(),
        0xDE => "Apostrophe".to_owned(),
        other => format!("key {other}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_standard_shortcuts_are_named_as_they_are_pressed() {
        assert_eq!(Kind::Show.standard().describe(), "Control+Alt+Shift+L");
        assert_eq!(Kind::QuickAdd.standard().describe(), "Control+Alt+Shift+K");
    }

    #[test]
    fn a_shortcut_survives_being_kept() {
        let shortcut = Shortcut { control: true, alt: false, shift: true, windows: true, key: 0x75 };
        assert_eq!(Shortcut::decode(shortcut.encode()), Some(shortcut));
        assert_eq!(shortcut.describe(), "Control+Shift+Windows+F6");
        assert_eq!(Shortcut::decode(0), None, "zero is a shortcut turned off");
    }

    #[test]
    fn its_modifiers_are_register_hot_keys_own() {
        assert_eq!(Kind::Show.standard().modifiers(), ALT | CONTROL | SHIFT);
    }

    #[test]
    fn the_standard_shortcuts_need_no_warning() {
        assert_eq!(Kind::Show.standard().warning(), None);
    }

    #[test]
    fn shortcuts_another_copy_holds_are_put_down_to_it() {
        let both = ["Control+Alt+Shift+L".to_owned(), "Control+Alt+Shift+K".to_owned()];
        assert_eq!(
            taken(&both, true),
            "Another copy of Lumenna, open on another profile, already uses Control+Alt+Shift+L and Control+Alt+Shift+K, so they do nothing in this one."
        );
        assert_eq!(
            taken(&both[..1], false),
            "Another program already uses Control+Alt+Shift+L, so it does nothing here."
        );
    }

    #[test]
    fn new_keys_another_copy_holds_say_where_to_change_them() {
        assert!(refused("Control+Alt+Shift+J", true).ends_with("Choose other keys, or change them there."));
        assert_eq!(refused("Control+Alt+Shift+J", false), "Another program already uses Control+Alt+Shift+J. Choose other keys.");
    }

    #[test]
    fn altgr_and_bare_keys_are_warned_about() {
        let altgr = Shortcut { control: true, alt: true, shift: false, windows: false, key: u16::from(b'L') };
        assert!(altgr.warning().unwrap().contains("AltGr"));
        let bare = Shortcut { control: false, alt: false, shift: true, windows: false, key: u16::from(b'L') };
        assert!(bare.warning().unwrap().contains("typing"));
    }
}
