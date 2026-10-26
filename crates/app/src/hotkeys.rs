//! The shortcuts that work from anywhere, outside the app: what they are, their keys until the
//! person changes them, how a combination is named and kept, which choices to warn about, and
//! what is said when one is taken. Pure, so every desktop shell holds the same rules: the
//! Windows app registers them with `RegisterHotKey`, whose modifier flags and virtual keys are
//! how a combination is kept here; GTK and the Mac name the same two by `id`.

/// What a shortcut from anywhere does.
#[boltffi::data]
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum HotKind {
    /// Shows the main window.
    Show,
    /// Opens quick add over whatever is in front.
    QuickAdd,
}

impl HotKind {
    /// Every shortcut from anywhere, in the order Settings lists them.
    pub const ALL: [Self; 2] = [Self::Show, Self::QuickAdd];

    /// What tells it apart, where a shell keeps it: `show`, `quick-add`.
    #[must_use]
    pub fn id(self) -> &'static str {
        match self {
            Self::Show => "show",
            Self::QuickAdd => "quick-add",
        }
    }

    /// What it is called in Settings and in a failure, Title Case.
    #[must_use]
    pub fn title(self) -> &'static str {
        match self {
            Self::Show => "Show Lumenna",
            Self::QuickAdd => "Quick Add a Task",
        }
    }

    /// Its name in the shell's capitals.
    #[must_use]
    pub fn name(self, sentence: bool) -> String {
        if sentence { lumenna_records::fields::sentence_case(self.title().to_owned()) } else { self.title().to_owned() }
    }

    /// The registry value Windows keeps it under.
    #[must_use]
    pub fn value_name(self) -> &'static str {
        match self {
            Self::Show => "Show",
            Self::QuickAdd => "QuickAdd",
        }
    }

    /// Its identifier with `RegisterHotKey`.
    #[must_use]
    pub fn number(self) -> i32 {
        match self {
            Self::Show => 1,
            Self::QuickAdd => 2,
        }
    }

    /// The letter it takes until the person changes it: L for Lumenna, and K, on every platform,
    /// each with its own modifiers (Control+Alt+Shift where Control+Alt alone is AltGr,
    /// Control-Command on the Mac, where Control-Option is VoiceOver's).
    #[must_use]
    pub fn letter(self) -> char {
        match self {
            Self::Show => 'L',
            Self::QuickAdd => 'K',
        }
    }

    /// What it is until the person changes it, on Windows and Linux: Control+Alt+Shift with its
    /// letter.
    #[must_use]
    pub fn standard(self) -> HotKeys {
        HotKeys { control: true, alt: true, shift: true, windows: false, key: u16::from(self.letter() as u8) }
    }
}

/// A combination of keys: modifiers and one Windows virtual key (a letter or a digit is its
/// own capital's code).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HotKeys {
    /// Control.
    pub control: bool,
    /// Alt.
    pub alt: bool,
    /// Shift.
    pub shift: bool,
    /// The Windows key, or Super.
    pub windows: bool,
    /// A Windows virtual-key code.
    pub key: u16,
}

// `RegisterHotKey`'s modifier flags, which are also how a combination is kept.
const ALT: u32 = 1;
const CONTROL: u32 = 2;
const SHIFT: u32 = 4;
const WINDOWS: u32 = 8;

impl HotKeys {
    /// `RegisterHotKey`'s modifier flags.
    #[must_use]
    pub fn modifiers(self) -> u32 {
        [(self.alt, ALT), (self.control, CONTROL), (self.shift, SHIFT), (self.windows, WINDOWS)]
            .into_iter()
            .filter(|(on, _)| *on)
            .fold(0, |flags, (_, flag)| flags | flag)
    }

    /// As one number, for keeping: the modifiers above the key. Never zero, which stands for a
    /// shortcut turned off.
    #[must_use]
    pub fn encode(self) -> u32 {
        (self.modifiers() << 16) | u32::from(self.key)
    }

    /// From [`encode`](Self::encode); `None` for zero, a shortcut turned off.
    #[must_use]
    pub fn decode(value: u32) -> Option<Self> {
        let key = u16::try_from(value & 0xFFFF).unwrap_or(0);
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
    #[must_use]
    pub fn describe(self) -> String {
        let mut parts: Vec<String> = [(self.control, "Control"), (self.alt, "Alt"), (self.shift, "Shift"), (self.windows, "Windows")]
            .into_iter()
            .filter(|(on, _)| *on)
            .map(|(_, name)| name.to_owned())
            .collect();
        parts.push(key_name(self.key));
        parts.join("+")
    }

    /// Why this combination is a poor choice, if it is: said before it is saved, so the person
    /// can choose again.
    #[must_use]
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

/// What is said when the system would not give this copy its shortcuts, `described` each as
/// [`HotKeys::describe`] says it.
///
/// The system does not say who holds a combination of keys, but when another copy of Lumenna is
/// open — on another profile — it is almost certainly that one, and calling it another program
/// would send the person looking for one that is not there.
#[must_use]
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
#[must_use]
pub fn refused(described: &str, by_another_copy: bool) -> String {
    if by_another_copy {
        format!("Another copy of Lumenna, open on another profile, already uses {described}. Choose other keys, or change them there.")
    } else {
        format!("Another program already uses {described}. Choose other keys.")
    }
}

/// A virtual key's name: letters and digits as themselves, F-keys and the common rest by name.
#[must_use]
pub fn key_name(key: u16) -> String {
    match key {
        0x30..=0x39 | 0x41..=0x5A => char::from(u8::try_from(key).unwrap_or(b'?')).to_string(),
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

/// What Settings says of the shortcuts from anywhere, Title Case: a shell in sentence case
/// cases the headings and buttons with `sentence_case`.
#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct HotKeyWords {
    /// The heading over them.
    pub heading: String,
    /// Under them: why these modifiers, and that they work from anywhere.
    pub caption: String,
    /// The button that changes one's keys.
    pub change: String,
    /// The button that turns one off.
    pub turn_off: String,
    /// The button that turns one on again.
    pub turn_on: String,
    /// The dialog that takes new keys.
    pub dialog: String,
    /// What it asks before keys are pressed.
    pub press_first: String,
    /// The button that keeps keys warned about.
    pub use_anyway: String,
}

/// What Settings says of the shortcuts from anywhere (`HotKeyWords`).
#[must_use]
pub fn hotkey_words() -> HotKeyWords {
    HotKeyWords {
        heading: "Shortcuts from Anywhere".to_owned(),
        caption: "These work in any program, so they take their keys from whatever is in front. Control+Alt+Shift, because \
                  Control+Alt alone is AltGr on many keyboards, where it types letters."
            .to_owned(),
        change: "Change".to_owned(),
        turn_off: "Turn Off".to_owned(),
        turn_on: "Turn On".to_owned(),
        dialog: "Change Shortcut".to_owned(),
        press_first: "Press the keys first, or choose Cancel.".to_owned(),
        use_anyway: "Use These Keys Anyway".to_owned(),
    }
}

/// How a shortcut turned off is shown where its keys would be.
#[must_use]
pub fn off() -> String {
    "Off".to_owned()
}

/// What is said once the shortcut called `name` is changed: its keys now, `described` as
/// [`HotKeys::describe`] says them, or that it is off.
#[must_use]
pub fn changed(name: &str, described: Option<&str>) -> String {
    match described {
        Some(keys) => format!("{name} is {keys}"),
        None => format!("{name} is off"),
    }
}

/// What the dialog for new keys asks, for the shortcut called `name`.
#[must_use]
pub fn press_keys_for(name: &str) -> String {
    format!(
        "Press the new keys for {name}: Control, Alt or both, with Shift, and a letter, digit or function key. Backspace clears them."
    )
}

/// The button that changes the keys of the shortcut called `name`, as a screen reader names it.
#[must_use]
pub fn change_shortcut_for(name: &str) -> String {
    format!("Change the shortcut for {name}")
}

/// What is asked before keys warned about are kept: "Use Control+Alt+L?"
#[must_use]
pub fn use_keys(described: &str) -> String {
    format!("Use {described}?")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_standard_shortcuts_are_named_as_they_are_pressed() {
        assert_eq!(HotKind::Show.standard().describe(), "Control+Alt+Shift+L");
        assert_eq!(HotKind::QuickAdd.standard().describe(), "Control+Alt+Shift+K");
    }

    #[test]
    fn a_shortcut_survives_being_kept() {
        let keys = HotKeys { control: true, alt: false, shift: true, windows: true, key: 0x75 };
        assert_eq!(HotKeys::decode(keys.encode()), Some(keys));
        assert_eq!(keys.describe(), "Control+Shift+Windows+F6");
        assert_eq!(HotKeys::decode(0), None, "zero is a shortcut turned off");
    }

    #[test]
    fn its_modifiers_are_register_hot_keys_own() {
        assert_eq!(HotKind::Show.standard().modifiers(), ALT | CONTROL | SHIFT);
    }

    #[test]
    fn the_standard_shortcuts_need_no_warning() {
        assert_eq!(HotKind::Show.standard().warning(), None);
    }

    #[test]
    fn shortcuts_another_copy_holds_are_put_down_to_it() {
        let both = ["Control+Alt+Shift+L".to_owned(), "Control+Alt+Shift+K".to_owned()];
        assert_eq!(
            taken(&both, true),
            "Another copy of Lumenna, open on another profile, already uses Control+Alt+Shift+L and Control+Alt+Shift+K, so they do nothing in this one."
        );
        assert_eq!(taken(&both[..1], false), "Another program already uses Control+Alt+Shift+L, so it does nothing here.");
    }

    #[test]
    fn new_keys_another_copy_holds_say_where_to_change_them() {
        assert!(refused("Control+Alt+Shift+J", true).ends_with("Choose other keys, or change them there."));
        assert_eq!(refused("Control+Alt+Shift+J", false), "Another program already uses Control+Alt+Shift+J. Choose other keys.");
    }

    #[test]
    fn altgr_and_bare_keys_are_warned_about() {
        let altgr = HotKeys { control: true, alt: true, shift: false, windows: false, key: u16::from(b'L') };
        assert!(altgr.warning().unwrap().contains("AltGr"));
        let bare = HotKeys { control: false, alt: false, shift: true, windows: false, key: u16::from(b'L') };
        assert!(bare.warning().unwrap().contains("typing"));
    }

    #[test]
    fn a_shortcut_changed_says_its_keys_now_or_that_it_is_off() {
        assert_eq!(changed("Show Lumenna", Some("Control+Alt+Shift+L")), "Show Lumenna is Control+Alt+Shift+L");
        assert_eq!(changed("Show Lumenna", None), "Show Lumenna is off");
    }

    #[test]
    fn each_shortcut_is_named_in_the_shells_capitals() {
        assert_eq!(HotKind::QuickAdd.name(false), "Quick Add a Task");
        assert_eq!(HotKind::QuickAdd.name(true), "Quick add a task");
    }
}
