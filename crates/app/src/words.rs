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
use lumenna_records::{ActionKind, Choice, PickerWords};
use serde::{Deserialize, Serialize};

use crate::Capitals;
use crate::hotkeys::{HotKeyWords, hotkey_words};
use crate::keys::{COMMANDS, KeyGroup, grouped};

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
    /// A braille device's own screens and key help.
    pub braille: BrailleWords,
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

    // BTSpeak's.
    /// The key "/" as a menu says it after a label: "slash".
    pub slash: String,
    /// The key ",": "comma".
    pub comma: String,
    /// The key ".": "period".
    pub period: String,
    /// What a process serving the store without the screens calls itself to a client:
    /// "Lumenna background service".
    pub background_service: String,

    // Android's.
    /// The key that unfolds a row, where a keyboard help names it apart from a row: "Expand".
    pub expand: String,
    /// The key that folds one: "Collapse".
    pub collapse: String,
    // The web's.
    /// What Right arrow does in a list, in a keyboard help: "Expand the Row".
    pub expand_the_row: String,
    /// What Left arrow does there: "Collapse the Row".
    pub collapse_the_row: String,
    /// What Down arrow and Ctrl+Space do in a line that completes.
    pub offer_completions: String,
    /// The key that expands, as a keyboard help names it: "Right Arrow".
    pub right_arrow: String,
    /// The key that collapses: "Left Arrow".
    pub left_arrow: String,
    /// The key that offers completions: "Down Arrow".
    pub down_arrow: String,
    /// The key that closes a dialog: "Escape".
    pub escape: String,
    /// A pick's description after how many it offers: "Type part of one to narrow them."
    pub type_to_narrow: String,
    /// The tabs of the settings' pages: "Settings Pages".
    pub settings_pages: String,
    /// Asked of the store before it has opened.
    pub store_not_open: String,

    // Emacs's.
    /// Every command a menu lists, by its `id`, named in the shell's capitals: what a help
    /// names a binding by, where the binding runs one.
    pub commands: Vec<Titled>,
    /// The shared keys, grouped as a help lists them, each group's heading with them.
    pub key_groups: Vec<KeyGroup>,
    /// Each kind of row action by what it is, whatever it is done to: what a help names a key
    /// by that runs the row's action of that kind.
    pub action_titles: Vec<KindTitle>,
    /// What a shell that draws its own key help, prompts and buffer names says: Emacs.
    pub emacs: EmacsWords,
    /// What the Orbit Speak's and Player's app says of its own keys.
    pub orbit: OrbitWords,

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

/// A braille device's own screens' names and its key help's headings: what Orbit's and the
/// BrailleSense's apps would otherwise each write.
#[data]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BrailleWords {
    /// The main menu's name: "Main menu".
    pub main_menu: String,
    /// A row's menu of its actions: "Actions".
    pub actions_menu: String,
    /// A key help's name: "Key help".
    pub key_help: String,
    /// Its heading over the commands pressed with Space: "Space with a letter".
    pub with_space: String,
    /// Its heading over first-letter moving: "A letter by itself".
    pub letter_alone: String,
    /// What first-letter moving does, said under that heading.
    pub finds_a_row: String,
    /// Its heading over the keys that move and open: "Moving and opening".
    pub moving: String,
    /// Enter on what opens nothing: said, so that a press is never left unanswered.
    pub opens_nothing: String,
    /// The actions menu asked for on a row with none.
    pub no_actions: String,
    /// The screen of reminders due asked for with none due.
    pub nothing_due: String,
    /// A space, typed or erased, said by a device that says what is typed.
    pub space: String,
    /// What a BrailleSense's edit box says as it edits, in its own words.
    pub edit_box: EditBoxWords,
}

/// What a BrailleSense's edit box says as it is edited: a new line, a line gone back over, and its
/// selection and the clipboard, in the device's own words so that Lumenna's boxes sound like its own.
#[data]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditBoxWords {
    /// A new line typed with Enter: "Enter".
    pub enter: String,
    /// Backspace over a new line, back onto the line before: "previous line".
    pub previous_line: String,
    /// A line with nothing on it, moved to or read: "blank".
    pub blank: String,
    /// A selection started at the cursor: "Start Selection".
    pub start_selection: String,
    /// Everything selected: "Select All".
    pub select_all: String,
    /// A selection left with Escape: "Selection canceled."
    pub selection_canceled: String,
    /// The selection copied to the clipboard: "Copy complete".
    pub copied: String,
    /// The selection cut to the clipboard: "Cut".
    pub cut: String,
    /// The clipboard pasted at the cursor: "Paste".
    pub pasted: String,
    /// The selection added to what the clipboard holds: "Add To Clipboard".
    pub appended: String,
    /// The clipboard emptied: "Clear Clipboard".
    pub clipboard_cleared: String,
    /// The selection taken away with Backspace: "Delete complete."
    pub deleted: String,
    /// Copy, cut or read with nothing selected: "Block is not set."
    pub nothing_selected: String,
    /// Paste with nothing on the clipboard.
    pub clipboard_empty: String,
    /// Backspace-2-5 at the end of a line of a multi-line box, joining the next one on: "new
    /// paragraph".
    pub new_paragraph: String,
    /// Asked before Backspace or Space-D takes a selection: "Delete selected text?"
    pub delete_selected: String,
}

/// A command and its name (`ShellWords::commands`).
#[data]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Titled {
    /// What it is: "new-task".
    pub id: String,
    /// Its name, in the shell's capitals: "New Task".
    pub title: String,
}

/// A kind of row action and its name, whatever row it is on (`ShellWords::action_titles`).
#[data]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KindTitle {
    /// The kind.
    pub kind: ActionKind,
    /// Its name, in the shell's capitals: "Move Up".
    pub title: String,
}

/// What the Orbit devices' app says of its own keys: the Speak's braille keys and the Player's
/// phone keypad, which no other shell has.
#[data]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrbitWords {
    /// Holding star on the Player turned finding a row by letter on.
    pub letters_on: String,
    /// Holding star on the Player turned it off.
    pub letters_off: String,
    /// The Speak's keys, a sentence each, for its key help.
    pub speak_keys: Vec<String>,
    /// The Player's keys, a sentence each, for its key help.
    pub player_keys: Vec<String>,
}

/// What a shell that lists its own keys, asks in its own prompts and names its own buffers
/// says (Emacs): the headings and lines of its key help, its prompts and refusals, and the
/// names its buffers and modes are made from. A word with a part is a template, `%s` where the
/// part goes, in the order given.
#[data]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmacsWords {
    // A key help's headings.
    /// What every list has: "Every List".
    pub every_list: String,
    /// "The Place at Point".
    pub place_at_point: String,
    /// "The Task at Point".
    pub task_at_point: String,
    /// "The Block at Point".
    pub block_at_point: String,
    /// "The Project at Point".
    pub project_at_point: String,
    /// "The Label at Point".
    pub label_at_point: String,
    /// "The Filter at Point".
    pub filter_at_point: String,
    /// "The Setting at Point".
    pub setting_at_point: String,
    /// "The Device at Point".
    pub device_at_point: String,
    /// "The Reminder at Point".
    pub reminder_at_point: String,
    /// "This List".
    pub this_list: String,
    /// "This Task".
    pub this_task: String,
    /// A block in the day: "A Block".
    pub a_block: String,
    /// A sitting in the day: "A Task in a Block".
    pub a_task_in_a_block: String,
    /// The proposal shown: "The Proposal".
    pub the_proposal: String,
    /// What is done from anywhere: "Do".
    pub doing: String,
    /// Backups, export and import: "Data".
    pub data: String,
    /// Syncing and pairing: "Syncing".
    pub syncing: String,

    // A key help's lines.
    /// "Next Line".
    pub next_line: String,
    /// "Previous Line".
    pub previous_line: String,
    /// What RET does in any list: "Do the Main Thing for This Line".
    pub main_thing: String,
    /// What `.` does: "This Line's Actions, by Name".
    pub line_actions: String,
    /// "Read the List Again".
    pub read_again: String,
    /// The help itself: "These Keys".
    pub these_keys: String,
    /// The places: "Lumenna's Places".
    pub all_places: String,
    /// "Leave This List".
    pub leave_list: String,
    /// RET on a place: "Open It, or Fold a Heading".
    pub open_or_fold: String,
    /// A filter typed from anywhere: "Search or Filter".
    pub search: String,
    /// Quick add, from a list: "Add a Task".
    pub add_task: String,
    /// "Filter This List".
    pub filter_list: String,
    /// A field of the task's own buffer: "Change the Field at Point, or Choose One".
    pub change_field: String,
    /// RET on a sitting: "The Task Itself".
    pub the_task_itself: String,
    /// A proposal's change: "Check or Uncheck the Change at Point".
    pub check_change: String,
    /// RET in a proposal: "Check or Uncheck a Change, or Press a Button".
    pub check_or_press: String,
    /// "Take the Other Option at Point".
    pub take_other: String,
    /// "Make the Changes Checked".
    pub make_checked: String,
    /// "Cancel, Making Nothing".
    pub cancel_making_nothing: String,
    /// RET on a setting: "Change It".
    pub change_it: String,
    /// "Import an Export or a Backup".
    pub import_any: String,
    /// "Pair a Device".
    pub pair_a_device: String,
    /// "Copy Code".
    pub copy_code: String,
    /// "Cancel a Pairing".
    pub cancel_pairing: String,
    /// RET on a reminder due: "Open What It Is About".
    pub open_about: String,
    /// "Dismiss".
    pub dismiss: String,
    /// "Snooze".
    pub snooze: String,

    // Names a buffer or a mode is made from, beside the commands' and the places'.
    /// "Projects".
    pub projects: String,
    /// "Labels".
    pub labels: String,
    /// "Notes".
    pub notes: String,
    /// "Proposal".
    pub proposal: String,
    /// "Reminders".
    pub reminders: String,
    /// The key help's own title: "Lumenna, from Anywhere".
    pub from_anywhere: String,

    // Prompts and what is said.
    /// Where a file is kept: "Save it as".
    pub save_as: String,
    /// The file `%s` is there already.
    pub already_there: String,
    /// "Not saved".
    pub not_saved: String,
    /// The file `%s` was written.
    pub saved: String,
    /// A command on a line with nothing on it.
    pub no_item: String,
    /// A command where it does nothing.
    pub nothing_to_do: String,
    /// A question let go.
    pub nothing_done: String,
    /// A line read back with nothing in it.
    pub nothing_typed: String,
    /// Quick add's prompt: "Add a task".
    pub add_a_task: String,
    /// Quick add entered empty.
    pub nothing_to_add: String,
    /// The key help's line, `%s` the buffer's name, the key that runs one, the key that goes
    /// back.
    pub keys_in: String,
    /// RET on a heading of the key help.
    pub a_heading: String,
    /// A task's command where no task is shown.
    pub no_task: String,
    /// A field's command where there is none.
    pub no_field: String,
    /// Which field to change: "Change".
    pub change: String,
    /// Editing a task's notes: `%s` its title, the key that saves, the key that cancels.
    pub notes_for: String,
    /// Notes left as they were.
    pub notes_unchanged: String,
    /// A yes or no field, `%s` its name and what it holds now: "All Day, now no".
    pub now_is: String,
    /// Copying the code where none is shown.
    pub no_code: String,
    /// Cancelling a pairing where none is under way.
    pub no_pairing: String,
    /// A program that is not there, `%s` its name.
    pub cannot_find: String,
    /// Looking for reminders failed, `%s` why.
    pub reminders_failed: String,
    /// An action whose form is not drawn here, `%s` its name.
    pub no_form_here: String,
    /// A question this shell cannot ask, `%s` the action's name.
    pub cannot_ask_here: String,
}

/// What every kind of row action is called, whatever it is done to.
#[must_use]
pub fn kind_title(kind: ActionKind) -> String {
    use ActionKind as K;
    let title = match kind {
        K::MarkDone => lumenna_records::actions::MARK_DONE,
        K::MarkNotDone => "Mark Not Done",
        K::Edit => "Edit",
        K::EditTask => "Edit Task Details",
        K::PutInBlock => "Put in a Block",
        K::MoveToProject => "Move to Project",
        K::MakeSubtaskOf => "Make Subtask Of",
        K::MoveToTopLevel => "Move to Top Level",
        K::WaitFor => "Wait For",
        K::StopWaiting => "Stop Waiting",
        K::Delete => "Delete",
        K::Restore => "Restore",
        K::DeleteForGood => "Delete from Trash",
        K::AssignTask => "Assign a Task",
        K::CancelDay => "Cancel This Day",
        K::RestoreDay => "Restore This Day",
        K::StartTimer => "Start Timer",
        K::ResumeTimer => "Resume Timer",
        K::PauseTimer => "Pause Timer",
        K::StopTimer => "Stop Timer",
        K::PlannedLength => "Planned Length",
        K::LogMinutes => "Log Minutes",
        K::Unassign => "Unassign",
        K::AddBlock => "Add Block Here",
        K::Rename => "Rename",
        K::NewInside => "New Project Inside",
        K::MoveUnder => "Move Under",
        K::MoveUp => "Move Up",
        K::MoveDown => "Move Down",
        K::Weight => "Weight",
        K::Archive => "Archive",
        K::Unarchive => "Unarchive",
        K::MergeInto => "Merge Into",
        K::Colour => "Colour",
        K::ChangeQuery => "Change Query",
        K::New => "New",
        K::Unpair => "Unpair",
        K::Reminders => "Reminders",
        K::EndNow => "End Now",
        K::Extend => return format!("Add {} Minutes", lumenna_records::day::EXTEND_MINS),
        K::EndDay => "End the Day",
        K::ClearDay => "Clear the Rest of the Day",
        K::CarryForward => "Carry Forward",
        K::WhatNow => "What Should I Work On?",
        K::CarryRest => "Carry the Rest Forward",
        K::Replan => "Replan from Now",
        K::PlanDay => "Plan the Day",
        K::Expand => "Expand",
        K::Collapse => "Collapse",
        K::Skip => "Skip",
        K::Unskip => "Plan Again",
        K::DeliverOn => "Deliver On",
        K::TimeZone => "Time Zone",
        K::ErasePermanently => "Erase Permanently",
        K::EraseEverythingDeleted => "Erase Everything Deleted",
    };
    title.to_owned()
}

/// Every kind of row action, in the order `ActionKind` declares them.
pub const ACTION_KINDS: &[ActionKind] = {
    use ActionKind as K;
    &[
        K::MarkDone, K::MarkNotDone, K::Edit, K::EditTask, K::PutInBlock, K::MoveToProject, K::MakeSubtaskOf,
        K::MoveToTopLevel, K::WaitFor, K::StopWaiting, K::Delete, K::Restore, K::DeleteForGood, K::AssignTask,
        K::CancelDay, K::RestoreDay, K::StartTimer, K::ResumeTimer, K::PauseTimer, K::StopTimer, K::PlannedLength,
        K::LogMinutes, K::Unassign, K::AddBlock, K::Rename, K::NewInside, K::MoveUnder, K::MoveUp, K::MoveDown,
        K::Weight, K::Archive, K::Unarchive, K::MergeInto, K::Colour, K::ChangeQuery, K::New, K::Unpair, K::Reminders,
        K::EndNow, K::Extend, K::EndDay, K::ClearDay, K::CarryForward, K::WhatNow, K::CarryRest, K::Replan,
        K::PlanDay, K::Expand, K::Collapse, K::Skip, K::Unskip, K::DeliverOn, K::TimeZone, K::ErasePermanently,
        K::EraseEverythingDeleted,
    ]
};

/// Emacs's words, its names in `c`'s capitals.
fn orbit_words() -> OrbitWords {
    let lines = |keys: &[&str]| keys.iter().map(|key| (*key).to_owned()).collect();
    OrbitWords {
        letters_on: "Finding by letter on".to_owned(),
        letters_off: "Finding by letter off".to_owned(),
        speak_keys: lines(&[
            "Up and Down move through the list. Held, they go to its ends.",
            "Right, OK or dot 8 opens the row. Right opens a folded row first.",
            "Left or dot 7 goes back. Left folds an open row first.",
            "Space with M lists what can be done to the row: its Move to Project, too.",
            "While typing, Space ends a word, dot 7 deletes, and OK or dot 8 finishes.",
            "While typing, Space with Z cancels, and Down offers what could come next.",
        ]),
        player_keys: lines(&[
            "Up and Down move through the list. Held, they go to its ends.",
            "OK opens the row. Right opens a folded row first, and Left folds an open one.",
            "Menu lists what can be done to the row.",
            "Back goes back.",
            "A digit opens that row: 2 the second, 1 then 2 the twelfth, 0 the tenth.",
            "Holding star finds rows by letter instead, typed as on a phone.",
            "While typing, tap a digit for its letters, star changes capitals and digits, and OK finishes.",
            "While typing, Back deletes, and held cancels.",
        ]),
    }
}

fn emacs_words(c: &dyn Fn(&str) -> String) -> EmacsWords {
    let name = core::NAME;
    EmacsWords {
        every_list: c("Every List"),
        place_at_point: c("The Place at Point"),
        task_at_point: c("The Task at Point"),
        block_at_point: c("The Block at Point"),
        project_at_point: c("The Project at Point"),
        label_at_point: c("The Label at Point"),
        filter_at_point: c("The Filter at Point"),
        setting_at_point: c("The Setting at Point"),
        device_at_point: c("The Device at Point"),
        reminder_at_point: c("The Reminder at Point"),
        this_list: c("This List"),
        this_task: c("This Task"),
        a_block: c("A Block"),
        a_task_in_a_block: c("A Task in a Block"),
        the_proposal: c("The Proposal"),
        doing: c("Do"),
        data: c("Data"),
        syncing: c("Syncing"),

        next_line: c("Next Line"),
        previous_line: c("Previous Line"),
        main_thing: c("Do the Main Thing for This Line"),
        line_actions: c("This Line's Actions, by Name"),
        read_again: c("Read the List Again"),
        these_keys: c("These Keys"),
        all_places: c(&format!("{name}'s Places")),
        leave_list: c("Leave This List"),
        open_or_fold: c("Open It, or Fold a Heading"),
        search: c("Search or Filter"),
        add_task: c("Add a Task"),
        filter_list: c("Filter This List"),
        change_field: c("Change the Field at Point, or Choose One"),
        the_task_itself: c("The Task Itself"),
        check_change: c("Check or Uncheck the Change at Point"),
        check_or_press: c("Check or Uncheck a Change, or Press a Button"),
        take_other: c("Take the Other Option at Point"),
        make_checked: c("Make the Changes Checked"),
        cancel_making_nothing: c("Cancel, Making Nothing"),
        change_it: c("Change It"),
        import_any: c("Import an Export or a Backup"),
        pair_a_device: c("Pair a Device"),
        copy_code: c("Copy Code"),
        cancel_pairing: c("Cancel a Pairing"),
        open_about: c("Open What It Is About"),
        dismiss: c("Dismiss"),
        snooze: c("Snooze"),

        projects: c("Projects"),
        labels: c("Labels"),
        notes: c("Notes"),
        proposal: c("Proposal"),
        reminders: c("Reminders"),
        from_anywhere: c(&format!("{name}, from Anywhere")),

        save_as: "Save it as".to_owned(),
        already_there: already_there("%s"),
        not_saved: "Not saved".to_owned(),
        saved: saved("%s"),
        no_item: "No item on this line".to_owned(),
        nothing_to_do: "Nothing to do here".to_owned(),
        nothing_done: "Nothing done".to_owned(),
        nothing_typed: "Nothing typed yet".to_owned(),
        add_a_task: "Add a task".to_owned(),
        nothing_to_add: "Nothing to add".to_owned(),
        keys_in: "Keys in %s. %s runs one there, %s goes back".to_owned(),
        a_heading: "A heading; its keys are under it".to_owned(),
        no_task: "No task here".to_owned(),
        no_field: "No field here".to_owned(),
        change: "Change".to_owned(),
        notes_for: "Notes for %s. %s saves, %s cancels".to_owned(),
        notes_unchanged: "Notes unchanged".to_owned(),
        now_is: "%s, now %s".to_owned(),
        no_code: "No code is shown; this device is not waiting to be found".to_owned(),
        no_pairing: "No pairing is under way".to_owned(),
        cannot_find: "Cannot find %s".to_owned(),
        reminders_failed: with_reason("Reminders could not be looked for", "%s"),
        no_form_here: core::no_form_here("%s"),
        cannot_ask_here: core::cannot_ask_here("%s"),
    }
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
        braille: BrailleWords {
            main_menu: c("Main Menu"),
            actions_menu: c("Actions"),
            key_help: c("Key Help"),
            with_space: c("Space with a Letter"),
            letter_alone: c("A Letter by Itself"),
            finds_a_row: "Moves to the next row starting with that letter.".to_owned(),
            moving: c("Moving and Opening"),
            opens_nothing: "Nothing to open here.".to_owned(),
            no_actions: "No actions on this row.".to_owned(),
            nothing_due: "No reminders are due.".to_owned(),
            space: c("Space"),
            edit_box: EditBoxWords {
                enter: c("Enter"),
                previous_line: "previous line".to_owned(),
                blank: "blank".to_owned(),
                start_selection: c("Start Selection"),
                select_all: c("Select All"),
                selection_canceled: "Selection canceled.".to_owned(),
                copied: c("Copy Complete"),
                cut: c("Cut"),
                pasted: c("Paste"),
                appended: c("Add to Clipboard"),
                clipboard_cleared: c("Clear Clipboard"),
                deleted: "Delete complete.".to_owned(),
                nothing_selected: "Block is not set.".to_owned(),
                clipboard_empty: "The clipboard is empty.".to_owned(),
                new_paragraph: "new paragraph".to_owned(),
                delete_selected: "Delete selected text?".to_owned(),
            },
        },
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

        slash: "slash".to_owned(),
        comma: "comma".to_owned(),
        period: "period".to_owned(),
        background_service: format!("{} background service", core::NAME),

        expand: c("Expand"),
        collapse: c("Collapse"),
        expand_the_row: c("Expand the Row"),
        collapse_the_row: c("Collapse the Row"),
        offer_completions: c(
            "Offer What Could Go at the Cursor, in Quick Add, the List's Filter, a Saved Filter's Query or a Block's Tasks From",
        ),
        right_arrow: c("Right Arrow"),
        left_arrow: c("Left Arrow"),
        down_arrow: c("Down Arrow"),
        escape: c("Escape"),
        type_to_narrow: "Type part of one to narrow them.".to_owned(),
        settings_pages: c("Settings Pages"),
        store_not_open: "The store is not open yet.".to_owned(),

        commands: COMMANDS.iter().map(|command| Titled { id: command.id.to_owned(), title: c(command.title) }).collect(),
        key_groups: grouped(sentence),
        action_titles: ACTION_KINDS.iter().map(|&kind| KindTitle { kind, title: c(&kind_title(kind)) }).collect(),
        emacs: emacs_words(&c),
        orbit: orbit_words(),
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
    fn every_kind_of_action_is_named_once_by_what_it_is() {
        let words = shell_words(Capitals::Title, ThisDevice::Computer);
        for (index, kind) in ACTION_KINDS.iter().enumerate() {
            assert!(!ACTION_KINDS[..index].contains(kind), "{kind:?} twice");
        }
        assert_eq!(words.action_titles.len(), ACTION_KINDS.len());
        let extend = words.action_titles.iter().find(|named| named.kind == ActionKind::Extend).map(|named| named.title.as_str());
        assert_eq!(extend, Some("Add 15 Minutes"));
    }

    #[test]
    fn a_word_with_parts_has_a_place_for_each_part() {
        let e = shell_words(Capitals::Title, ThisDevice::Computer).emacs;
        for (template, parts) in [
            (&e.already_there, 1),
            (&e.saved, 1),
            (&e.keys_in, 3),
            (&e.notes_for, 3),
            (&e.now_is, 2),
            (&e.cannot_find, 1),
            (&e.reminders_failed, 1),
            (&e.no_form_here, 1),
            (&e.cannot_ask_here, 1),
        ] {
            assert_eq!(template.matches("%s").count(), parts, "{template}");
        }
    }

    #[test]
    fn a_device_list_where_a_key_renames_says_both() {
        assert_eq!(delete_unpairs_or_renames("F2"), "Delete unpairs the selected device, and F2 renames it.");
    }
}


/// One line of a braille device's key help: the keys, in the device's own terms, and what they do.
#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceKey {
    /** The keys: "Space-M or F2". */
    pub keys: String,
    /** What they do: "Open the menus". */
    pub what: String,
}

/// The BrailleSense's keys, for its key help: the device's own conventions (its manual's Common
/// Commands), and how each of this app's screens takes them. Keys are named as its manual names
/// them, which is why they sit here with their meaning, not as separate words.
#[must_use]
pub fn braillesense_keys() -> Vec<DeviceKey> {
    let row = |keys: &str, what: &str| DeviceKey { keys: keys.to_owned(), what: what.to_owned() };
    vec![
        row("Space-1 or the Up scroll key", "Previous row or option, or back through a long line"),
        row("Space-4 or the Down scroll key", "Next row or option, or on through a long line"),
        row("Space-1-2-3, Space-4-5-6", "First and last row"),
        row("Space-1-2-6, Space-3-4-5", "Previous and next page of 32 rows"),
        row("A letter by itself", "The next row beginning with it"),
        row("Enter", "Open the row, press the button"),
        row("Space-M or F2", "The menu bar"),
        row("Enter-I, or the Application key", "The row's own actions"),
        row("Space-3, Space-6", "Collapse and expand the row"),
        row("Space-E or F4", "Close, go back, or up one level of a menu"),
        row("Space-Z", "Exit Lumenna"),
        row("Space-4-5 or F3, Space-1-2 or Space-F3", "Next and previous control in a dialog"),
        row("Space", "Change a check box, a combo box, a prompt button, or mark a task done"),
        row("Space with a letter", "The row's command of that letter, or the screen's"),
        row("Enter with a letter", "The same, for A, B, C, D, E, F, H, K, L, M, O, R and Z, which Space keeps for moving and the device"),
        row("Enter-A", "New task"),
        row("Space-U, Space-Y", "Undo and redo"),
        row("Space-F", "Filter the list"),
        row("Space-D", "Delete the row"),
        row("Space-H, or F1 on the QWERTY keyboard", "These keys"),
        row("Space-O", "Settings"),
        row("Space-R or Space-C", "Say the row again"),
        row("Space-3-6, Space-2-5", "In a line of text, say the character and the word at the cursor"),
        row("Enter-B, Enter-A", "In a line of text, start a selection at the cursor, or select everything"),
        row("Enter-C, Enter-X, Enter-V", "Copy, cut and paste"),
        row("Enter-P, Enter-D", "Add the selection to the clipboard, clear the clipboard"),
        row("Backspace-1-2, Enter-U", "Read the selection, and the first line of it"),
        row("Space-D, Backspace-2-5", "In a line of text, delete the character or the word at the cursor"),
        row("Backspace-1-4, Backspace-1-3-4-6", "Delete the line shown at the cursor, or the whole paragraph"),
        row("Enter in a multi-line box", "A new line; Tab leaves the box"),
        row("Space-1, Space-4 in a multi-line box", "The line above and below"),
        row("A cell touched and a front Cursor button", "Move the cursor in a line of text to that cell"),
    ]
}
