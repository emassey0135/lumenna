//! Every fixed word a shell draws around the app's view, once: what its panes and lists are
//! called, its dialogs' buttons, its file dialogs' kinds, a row's hint, a keyboard help's
//! headings, what the device calls itself. A shell asks for [`ShellWords`] as it starts, in its
//! capitals and as it calls itself (`shell_words`, over the FFI and RPC's `app.words`), and
//! writes no word of its own.
//!
//! A word with parts (a name, a key, a reason) is a function here or in `lumenna_records::words`,
//! since it is said with them. What changes with what is shown (the menus, the settings pages,
//! pairing's words, a reminder's buttons) comes with the view.

use boltffi::data;
use lumenna_records::fields::{
    DetailsWords, FileWords, FolderWords, NotificationWords, QuickAddWords, ReadBackWords, SettingWords, file_words,
    folder_words, notification_words, picker_words, quick_add_words, read_back_words, setting_words,
};
use lumenna_records::words::{self as core, AppMetadata};
use lumenna_records::{Choice, PickerWords};
use serde::{Deserialize, Serialize};

use crate::Capitals;
use crate::hotkeys::{HotKeyWords, hotkey_words};

/// What a device is, as it calls itself in the core's sentences ("this PC") and as another
/// device names it when it has no name of its own ("Windows PC").
#[data]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThisDevice {
    /// A Windows PC.
    Pc,
    /// A Linux computer, or a computer of no kind the core knows.
    Computer,
    /// A Mac.
    Mac,
    /// An iPhone.
    IPhone,
    /// An iPad.
    IPad,
    /// An Android phone or tablet.
    Phone,
    /// A watch.
    Watch,
    /// A web browser.
    Browser,
    /// Anything else: a BTSpeak.
    #[default]
    Device,
}

impl ThisDevice {
    /// How it calls itself: "this PC".
    #[must_use]
    pub fn itself(self) -> &'static str {
        match self {
            Self::Pc => "this PC",
            Self::Computer => "this computer",
            Self::Mac => "this Mac",
            Self::IPhone => "this iPhone",
            Self::IPad => "this iPad",
            Self::Phone => "this phone",
            Self::Watch => "this watch",
            Self::Browser => "this browser",
            Self::Device => "this device",
        }
    }

    /// What another device calls it when it has no name of its own: "Windows PC".
    #[must_use]
    pub fn unnamed(self) -> &'static str {
        match self {
            Self::Pc => "Windows PC",
            Self::Computer => "Computer",
            Self::Mac => "Mac",
            Self::IPhone => "iPhone",
            Self::IPad => "iPad",
            Self::Phone => "Phone",
            Self::Watch => "Watch",
            Self::Browser => "Web browser",
            Self::Device => "Device",
        }
    }
}

/// Every fixed word a shell draws, in its capitals (`shell_words`).
#[data]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShellWords {
    /// What Lumenna is: its name, version, description and links.
    pub metadata: AppMetadata,
    /// How this device calls itself in the core's sentences: "this PC". What a shell says as
    /// `ShellEvent::PairingAs`.
    pub this_device: String,

    // The words of a screen or dialog, each a record of its own.
    /// A pick's.
    pub picker: PickerWords,
    /// Settings', the shell's own controls among them.
    pub settings: SettingWords,
    /// The task details'.
    pub details: DetailsWords,
    /// Quick add's, the filter's and completion's.
    pub quick_add: QuickAddWords,
    /// Notifications', on this device.
    pub notifications: NotificationWords,
    /// The file dialogs' questions.
    pub files: FileWords,
    /// Backing up into a folder of the person's, on this device.
    pub folder: FolderWords,
    /// Reading a line back before adding it.
    pub read_back: ReadBackWords,
    /// The shortcuts from anywhere.
    pub hotkeys: HotKeyWords,
    /// Yes and No, for a question a shell asks as a choice of the two.
    pub yes_no: Vec<Choice>,

    // Buttons.
    /// A dialog's Cancel.
    pub cancel: String,
    /// A dialog's Close.
    pub close: String,
    /// What goes ahead with a pick: "Go Ahead".
    pub go_ahead: String,
    /// What chooses: "Choose".
    pub choose: String,
    /// Back to the screen before: "Back".
    pub back: String,
    /// What the folder dialog of backups chooses with: "Back Up Here".
    pub back_up_here: String,
    /// What the open dialog of an import opens with: "Import".
    pub import: String,
    /// Where an export's kind is chosen: "Export As".
    pub export_as: String,
    /// What adds the completion to the line: "Add to the Line".
    pub add_to_line: String,
    /// The editing a platform's Edit menu offers: "Cut".
    pub cut: String,
    /// "Copy".
    pub copy: String,
    /// "Paste".
    pub paste: String,
    /// "Select All".
    pub select_all: String,
    /// Quitting where the platform says Exit: "Exit".
    pub exit: String,

    // Names: panes, lists, headings, menus.
    /// The sidebar's: "Places".
    pub places: String,
    /// The middle pane's: "List".
    pub list: String,
    /// The day's list: "The Day".
    pub the_day: String,
    /// Browse's tab: "Browse".
    pub browse: String,
    /// Saved filters, where a short title is wanted: "Filters".
    pub filters: String,
    /// The list of reminders due: "Reminders Due".
    pub reminders_due: String,
    /// A device's own menu of the day's actions: "Day Actions".
    pub day_actions: String,
    /// A device's own menu of a place's actions: "Place Actions".
    pub place_actions: String,
    /// The list of completions: "Completions".
    pub completions: String,
    /// A pick's list, where it is named apart from its question: "Choices".
    pub choices: String,
    /// A section of a form beyond its first: "More".
    pub more: String,
    /// A block's length, where a stepper is named apart from its field: "Length".
    pub length: String,
    /// A filter typed into the list: "Query".
    pub query: String,
    /// What narrows a pick's list: "Narrow the List".
    pub narrow_the_list: String,
    /// The key a keyboard help lists in its first column: "Key".
    pub key: String,
    /// What it does, in its second: "Command".
    pub command: String,
    /// The label of a field taking keys: "Keys".
    pub keys: String,
    /// The label over a keyboard help's list: "Shortcuts".
    pub shortcuts: String,
    /// Between two keys that do the same, in a keyboard help: "Ctrl+Y or Ctrl+Shift+Z".
    pub or: String,
    /// The keyboard help's group of what works in any dialog: "In a Dialog".
    pub in_a_dialog: String,
    /// What Escape does there: "Close the Dialog".
    pub close_the_dialog: String,
    /// What F10 reaches: "Menu Bar".
    pub menu_bar: String,
    /// Alt+Enter's, in a keyboard help: "Properties: the Task's Details, or the Block's Form".
    pub properties: String,
    /// A rotor or filter of the blocks with a timer running: "Running".
    pub running: String,
    /// A rotor of the tasks overdue: "Overdue".
    pub overdue: String,
    /// A reminder that is one of the defaults, shown beside it: "Default".
    pub default_reminder: String,

    // What a row does when chosen, said as its hint or its action's name.
    /// "Open".
    pub open: String,
    /// "Show".
    pub show: String,
    /// A project's or a saved filter's: "Show Its Tasks".
    pub show_its_tasks: String,
    /// A label's: "Show the Tasks Wearing It".
    pub show_tasks_wearing_it: String,
    /// A sitting's: "Show the Task".
    pub show_the_task: String,
    /// A task's: "Show Details".
    pub show_details: String,
    /// A repeating block's: "Edit Every Occurrence".
    pub edit_every_occurrence: String,
    /// A project's, adding a task to it: "Add a Task to It".
    pub add_task_to_it: String,
    /// A label's: "Add a Task Wearing It".
    pub add_task_wearing_it: String,
    /// A task row's hint: "Shows details".
    pub shows_details: String,
    /// A repeating block's hint: "Edits every occurrence".
    pub edits_every_occurrence: String,

    // Sentences and lines.
    /// Which of several: "Which?"
    pub which: String,
    /// A pick with nothing to offer.
    pub nothing_to_choose: String,
    /// A pick answered with nothing chosen.
    pub choose_one_first: String,
    /// A device list's description: what Delete does.
    pub delete_unpairs: String,
    /// The hint of the field naming this device.
    pub device_name_hint: String,
    /// A pairing code that could not be copied.
    pub copy_failed: String,
    /// What a line that completes says of it.
    pub completion_hint: String,
    /// How many completions are shown at once.
    pub completions_shown: u32,
    /// An empty field, where its value is shown apart from it: "Empty".
    pub empty_field: String,
    /// An empty value inside a line: "none".
    pub none_value: String,
    /// An empty field's placeholder: "None".
    pub none: String,
    /// Settings with no page open.
    pub no_settings_page: String,
    /// A list with nothing in it, where the core gives nothing else: "Nothing here".
    pub nothing_here: String,
    /// The reminders due, when none are: "Nothing has come due."
    pub nothing_due: String,
    /// A day with nothing in it: "Nothing planned."
    pub nothing_planned: String,
    /// A file dialog's kind of file for every file: "All files".
    pub all_files: String,
    /// Its kind for a backup: "Lumenna backups".
    pub backups: String,
    /// Its kind for what can be read in: "Lumenna exports and backups".
    pub importable: String,
    /// Said when the store is being synced: "Syncing".
    pub syncing: String,
    /// A shortcut turned off, where its keys would be: "Off".
    pub shortcut_off: String,
    /// Why the shortcuts from anywhere cannot be had: the keyboard has no key for them.
    pub no_key_for_them: String,
    /// When the core's process stopped answering.
    pub stopped_responding: String,
    /// When there is nowhere to keep the store.
    pub no_data_folder: String,
    /// Why Windows would not start Lumenna at sign-in: turned off in Startup apps.
    pub startup_apps_off: String,
    /// Why not: a policy turns it off.
    pub policy_off: String,
    /// Why not: the desktop did not allow it.
    pub not_allowed: String,

    // A browser's.
    /// What a tab adds to the reminders setting's hint: they come only while it is open.
    pub reminders_while_open: String,
    /// When the browser may clear Lumenna's data short of space.
    pub storage_may_be_cleared: String,
    /// While the store opens.
    pub opening: String,
    /// A second tab, while another has the store open.
    pub open_elsewhere: String,

    // Apple's.
    /// A form's button that keeps what was typed: "Save".
    pub save: String,
    /// The Mac's application menu: "Hide Lumenna".
    pub hide_app: String,
    /// "Hide Others".
    pub hide_others: String,
    /// "Show All".
    pub show_all: String,
    /// The system's menu of other apps' services: "Services".
    pub services: String,
    /// "Quit Lumenna".
    pub quit_app: String,
    /// The Mac's Window menu: "Window".
    pub window: String,
    /// "Minimize".
    pub minimize: String,
    /// "Zoom".
    pub zoom: String,
    /// The View menu's: "Toggle Sidebar".
    pub toggle_sidebar: String,
    /// The Edit menu's completion of a word: "Complete".
    pub complete: String,
    /// Undo where it undoes typing in a field: "Undo Typing".
    pub undo_typing: String,
    /// "Redo Typing".
    pub redo_typing: String,
    /// A shortcut's modifiers, as the Mac writes them out ("Control-Command-Y"): "Control".
    pub control_key: String,
    /// "Option".
    pub option_key: String,
    /// "Shift".
    pub shift_key: String,
    /// "Command".
    pub command_key: String,
    /// The space bar, as a shortcut's key: "Space".
    pub space_key: String,
    /// The Mac's caption under its shortcuts from anywhere, whose keys are Control-Command.
    pub mac_hotkeys_caption: String,
    /// Why keys are taken: Control-Option is VoiceOver's.
    pub taken_by_voiceover: String,
    /// macOS locks the screen with them.
    pub taken_to_lock: String,
    /// macOS goes full screen with them.
    pub taken_for_full_screen: String,
    /// macOS looks up a word with them.
    pub taken_to_look_up: String,
    /// macOS opens emoji and symbols with them.
    pub taken_for_emoji: String,
    /// macOS takes screenshots with them.
    pub taken_for_screenshots: String,
    /// VOCR uses them.
    pub taken_by_vocr: String,
    /// VOCR uses them while moving through a scan.
    pub taken_by_vocr_scanning: String,
    /// What a watch calls its phone in a sentence: "the iPhone".
    pub the_phone: String,
    /// The devices page, where a short title is wanted (a bar's one line, which the page's
    /// "Devices and Sync" overflows at large text sizes): "Devices".
    pub devices: String,
}

/// A pick's words, its buttons in sentence case where `sentence`.
fn picker(sentence: bool) -> PickerWords {
    let words = picker_words();
    let c = |text: String| if sentence { lumenna_records::sentence_case(text) } else { text };
    PickerWords {
        day: c(words.day),
        time: c(words.time),
        choose: c(words.choose),
        cancel: c(words.cancel),
        hours: c(words.hours),
        minutes: c(words.minutes),
    }
}

/// Every fixed word a shell draws, in its `capitals`, for a device that is `device`.
#[must_use]
pub fn shell_words(capitals: Capitals, device: ThisDevice) -> ShellWords {
    let sentence = capitals == Capitals::Sentence;
    let c = |text: &str| if sentence { lumenna_records::sentence_case(text.to_owned()) } else { text.to_owned() };
    let this_device = device.itself();
    ShellWords {
        metadata: core::metadata(),
        this_device: this_device.to_owned(),
        picker: picker(sentence),
        settings: setting_words(),
        details: lumenna_records::fields::details_words(sentence),
        quick_add: quick_add_words(sentence),
        notifications: notification_words(this_device, sentence),
        files: file_words(sentence),
        folder: folder_words(this_device, sentence),
        read_back: read_back_words(sentence),
        hotkeys: hotkey_words(),
        yes_no: lumenna_records::fields::yes_no(),

        cancel: core::cancel(sentence),
        close: core::close(sentence),
        go_ahead: c("Go Ahead"),
        choose: c("Choose"),
        back: c("Back"),
        back_up_here: c("Back Up Here"),
        import: c("Import"),
        export_as: c("Export As"),
        add_to_line: core::add_to_line(sentence),
        cut: c("Cut"),
        copy: c("Copy"),
        paste: c("Paste"),
        select_all: c("Select All"),
        exit: c("Exit"),

        places: c("Places"),
        list: c("List"),
        the_day: c("The Day"),
        browse: core::browse(sentence),
        filters: c("Filters"),
        reminders_due: core::reminders_due(sentence),
        day_actions: core::day_actions(sentence),
        place_actions: core::place_actions(sentence),
        completions: core::completions(sentence),
        choices: c("Choices"),
        more: c("More"),
        length: c("Length"),
        query: c("Query"),
        narrow_the_list: c("Narrow the List"),
        key: c("Key"),
        command: c("Command"),
        keys: c("Keys"),
        shortcuts: c("Shortcuts"),
        or: "or".to_owned(),
        in_a_dialog: c("In a Dialog"),
        close_the_dialog: c("Close the Dialog"),
        menu_bar: c("Menu Bar"),
        properties: "Properties: the task's details, or the block's form".to_owned(),
        running: c("Running"),
        overdue: c("Overdue"),
        default_reminder: c("Default"),

        open: c("Open"),
        show: c("Show"),
        show_its_tasks: c("Show Its Tasks"),
        show_tasks_wearing_it: c("Show the Tasks Wearing It"),
        show_the_task: c("Show the Task"),
        show_details: c("Show Details"),
        edit_every_occurrence: c("Edit Every Occurrence"),
        add_task_to_it: c("Add a Task to It"),
        add_task_wearing_it: c("Add a Task Wearing It"),
        shows_details: "Shows details".to_owned(),
        edits_every_occurrence: "Edits every occurrence".to_owned(),

        which: core::which(),
        nothing_to_choose: core::nothing_to_choose(),
        choose_one_first: core::choose_one_first(),
        delete_unpairs: core::delete_unpairs(),
        device_name_hint: core::device_name_hint(),
        copy_failed: core::copy_failed(),
        completion_hint: core::completion_hint(),
        completions_shown: u32::try_from(core::COMPLETIONS_SHOWN).unwrap_or(u32::MAX),
        empty_field: core::empty_field(),
        none_value: core::none_value(),
        none: "None".to_owned(),
        no_settings_page: core::no_settings_page(),
        nothing_here: "Nothing here".to_owned(),
        nothing_due: "Nothing has come due.".to_owned(),
        nothing_planned: "Nothing planned.".to_owned(),
        all_files: "All files".to_owned(),
        backups: "Lumenna backups".to_owned(),
        importable: "Lumenna exports and backups".to_owned(),
        syncing: core::syncing(),
        shortcut_off: crate::hotkeys::off(),
        no_key_for_them: "This keyboard has no key for them.".to_owned(),
        stopped_responding: "Lumenna stopped responding".to_owned(),
        no_data_folder: "Lumenna cannot find a folder for its data. Set LUMENNA_PROFILE to one.".to_owned(),
        startup_apps_off: "it is turned off in Startup apps, in Settings or Task Manager, which is where it is turned back on"
            .to_owned(),
        policy_off: "your organisation's policy turns it off".to_owned(),
        not_allowed: "it was not allowed".to_owned(),

        reminders_while_open: core::reminders_while_open(),
        storage_may_be_cleared: core::storage_may_be_cleared(),
        opening: core::opening(),
        open_elsewhere: core::open_elsewhere(),

        save: c("Save"),
        hide_app: c(&format!("Hide {}", core::NAME)),
        hide_others: c("Hide Others"),
        show_all: c("Show All"),
        services: c("Services"),
        quit_app: c(&format!("Quit {}", core::NAME)),
        window: c("Window"),
        minimize: c("Minimize"),
        zoom: c("Zoom"),
        toggle_sidebar: c("Toggle Sidebar"),
        complete: c("Complete"),
        undo_typing: c("Undo Typing"),
        redo_typing: c("Redo Typing"),
        control_key: "Control".to_owned(),
        option_key: "Option".to_owned(),
        shift_key: "Shift".to_owned(),
        command_key: "Command".to_owned(),
        space_key: "Space".to_owned(),
        mac_hotkeys_caption: "These work in any app, so they take their keys from whatever app is in front. Control-Command, \
                              because Control-Option is VoiceOver's."
            .to_owned(),
        taken_by_voiceover: "Control-Option is VoiceOver's modifier, so VoiceOver would take this first.".to_owned(),
        taken_to_lock: "macOS uses this to lock the screen.".to_owned(),
        taken_for_full_screen: "macOS uses this for full screen.".to_owned(),
        taken_to_look_up: "macOS uses this to look up a word.".to_owned(),
        taken_for_emoji: "macOS uses this for emoji and symbols.".to_owned(),
        taken_for_screenshots: "macOS uses this for screenshots.".to_owned(),
        taken_by_vocr: "VOCR uses this.".to_owned(),
        taken_by_vocr_scanning: "VOCR uses this while you move through a scan, and not otherwise.".to_owned(),
        the_phone: "the iPhone".to_owned(),
        devices: c("Devices"),
    }
}

// ---------------------------------------------------------------------------------------
// The words with parts
// ---------------------------------------------------------------------------------------

/// What another device calls this one: the system's name for it without `.local`, else what it
/// is ("Windows PC").
#[must_use]
pub fn device_named(host: &str, device: ThisDevice) -> String {
    let host = host.trim();
    let host = host.strip_suffix(".local").unwrap_or(host).trim();
    if host.is_empty() { device.unnamed().to_owned() } else { host.to_owned() }
}

/// What answers a client that reaches the device's store through an app: "Lumenna for Windows",
/// `system` the platform's own name for itself.
#[must_use]
pub fn lumenna_for(system: &str) -> String {
    format!("{} for {system}", core::NAME)
}

/// Where About says the store is: "Data folder: C:\Users\…".
#[must_use]
pub fn data_folder(path: &str) -> String {
    format!("Data folder: {path}")
}

/// A file that could not be read in, `what` its name or path, the error's sentence after it.
#[must_use]
pub fn could_not_read(what: &str, reason: &str) -> String {
    with_reason(&format!("{} could not read {what}", core::NAME), reason)
}

/// A file that could not be written, as `could_not_read`.
#[must_use]
pub fn could_not_save(what: &str, reason: &str) -> String {
    with_reason(&format!("{} could not save {what}", core::NAME), reason)
}

/// When Lumenna could not start, the error's sentence after it.
#[must_use]
pub fn could_not_start(reason: &str) -> String {
    with_reason(&format!("{} could not start", core::NAME), reason)
}

/// What is asked before a file is replaced: "Tasks.json is already there. Replace it?"
#[must_use]
pub fn already_there(name: &str) -> String {
    format!("{name} is already there. Replace it?")
}

/// What is said once a file is written: "Saved Tasks.json".
#[must_use]
pub fn saved(what: &str) -> String {
    format!("Saved {what}")
}

/// A file dialog's kind for an extension: "JSON files".
#[must_use]
pub fn files_of(extension: &str) -> String {
    format!("{} files", extension.trim_start_matches('.').to_uppercase())
}

/// What narrows the pick called `title`: "Narrow Move to Project".
#[must_use]
pub fn narrow(title: &str) -> String {
    format!("Narrow {title}")
}

/// What syncs a watch with its phone now, `phone` as the watch calls it: "Sync with the iPhone".
#[must_use]
pub fn sync_with(phone: &str) -> String {
    format!("Sync with {phone}")
}

/// A device list's description where a key renames: "Delete unpairs the selected device, and F2
/// renames it."
#[must_use]
pub fn delete_unpairs_or_renames(key: &str) -> String {
    format!("{} and {key} renames it.", core::delete_unpairs().trim_end_matches('.').to_owned() + ",")
}

/// Why the shortcuts from anywhere cannot be had: what was asked did not answer.
#[must_use]
pub fn did_not_answer(what: &str, reason: &str) -> String {
    let reason = reason.trim();
    if reason.is_empty() { format!("{what} did not answer") } else { format!("{what} did not answer: {reason}") }
}

/// Why a system would not start Lumenna at sign-in, where it gives no reason: "Windows did not
/// turn it on".
#[must_use]
pub fn did_not_turn_it_on(system: &str) -> String {
    format!("{system} did not turn it on")
}

/// The title of the warning that keys are someone else's: "Control-Command-L is taken".
#[must_use]
pub fn keys_taken(keys: &str) -> String {
    format!("{keys} is taken")
}

/// Its button that leaves the keys as they were: "Keep Control-Command-L".
#[must_use]
pub fn keep_keys(keys: &str) -> String {
    format!("Keep {keys}")
}

/// Its question, after whose they are: "Use them for New Task anyway?"
#[must_use]
pub fn use_anyway_for(whose: &str, name: &str) -> String {
    format!("{} Use them for {name} anyway?", whose.trim())
}

// Apple's.

/// What the Mac asks while it waits for new keys, `keeps` the keys Escape leaves in place.
#[must_use]
pub fn press_mac_keys(keeps: &str) -> String {
    format!(
        "Press the keys now, with Command or Control. With VoiceOver on, Control-Command keys reach {} as they are. \
         Escape keeps {keeps}.",
        core::NAME
    )
}

/// A block's length, as a stepper that changes it says it: "Lasts 1 hour 30 minutes".
#[must_use]
pub fn lasts(minutes: u32) -> String {
    format!("Lasts {}", lumenna_records::words::duration(minutes))
}

/// `sentence`, then the error's own sentence after it when there is one.
fn with_reason(sentence: &str, reason: &str) -> String {
    let reason = reason.trim();
    if reason.is_empty() { sentence.to_owned() } else { format!("{sentence}. {reason}") }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_shell_in_sentence_case_has_its_buttons_and_names_in_sentence_case() {
        let title = shell_words(Capitals::Title, ThisDevice::Pc);
        let sentence = shell_words(Capitals::Sentence, ThisDevice::Pc);
        assert_eq!((title.the_day.as_str(), sentence.the_day.as_str()), ("The Day", "The day"));
        assert_eq!((title.back_up_here.as_str(), sentence.back_up_here.as_str()), ("Back Up Here", "Back up here"));
        assert_eq!(title.metadata.name, "Lumenna");
    }

    #[test]
    fn a_device_calls_itself_by_what_it_is_and_says_so_in_the_cores_sentences() {
        let words = shell_words(Capitals::Title, ThisDevice::Watch);
        assert_eq!(words.this_device, "this watch");
        assert!(words.folder.restoring.contains("this watch"), "{}", words.folder.restoring);
    }

    #[test]
    fn a_device_with_no_name_is_called_by_what_it_is() {
        assert_eq!(device_named("studio.local", ThisDevice::Mac), "studio");
        assert_eq!(device_named("  ", ThisDevice::Pc), "Windows PC");
    }

    #[test]
    fn a_file_that_could_not_be_read_is_said_with_its_reason_after_it_when_there_is_one() {
        assert_eq!(could_not_read("Tasks.json", ""), "Lumenna could not read Tasks.json");
        assert_eq!(could_not_read("Tasks.json", "Access is denied."), "Lumenna could not read Tasks.json. Access is denied.");
        assert_eq!(files_of("json"), "JSON files");
    }

    #[test]
    fn a_device_list_where_a_key_renames_says_both() {
        assert_eq!(delete_unpairs_or_renames("F2"), "Delete unpairs the selected device, and F2 renames it.");
    }
}
