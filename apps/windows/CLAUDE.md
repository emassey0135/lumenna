# The Windows app

Read when working in `apps/windows/`. The root `CLAUDE.md` holds what every part of Lumenna shares; this is what is easy to get wrong here.

`apps/windows/`: Rust over `windows-rs`, binary `lumenna.exe`, drawn from the Crux app:
`lumenna-ffi` as an rlib (`App` and `Device`, as the Mac and Android run them), with
`lumenna-app` for the view model's types and how every line reads and `lumenna-records` for
the records it is made of. It never names `lumenna-host` (bar the UI tests, which seed a
store of their own). Everything Win32 is in `src/win/` behind `cfg(windows)`.

- **The app's turns are posted to the window** (`shell.rs`, `WM_TURN`) and taken in order
  (`App::take_turn`): every screen draws, then what the turn says goes to the status line, in
  the core's own words and casing. A screen draws `view` with `lines` and sends what the person
  did; the window follows the app's place, judged by its current view (`crux.view()`), since a
  turn made before an `Open` still names the place left. The app also takes in, once a second
  (`App::tick`), what another process wrote and a new minute.
- **A list's selection moves only when the app lands it** (`view.landing`, a side list's
  `*_held.landing`) or the rows were rebuilt, never for a sync or a minute; focus goes into the
  tree on landing only when it is lost. Selecting a row sends `Focus` (a side list's `Hold`), and
  Space and Delete send the key (`Key("toggle")`, `Key("delete")`: what each runs is
  `keys::runs`, the core's), a side list's after holding its row; Enter and a double-click run
  the row's Open as it offers it (`keys::runs("open")`, `Rows::open`), else hand `Key("open")`
  to the app to refuse; a context menu or a button the one chosen (`ActOn`, naming the row),
  only a task's own form the window's (the details pane, also opened for a `Question::Form` the
  app leaves open). The tasks, the Blocks list and the day are one tree of rows (`rows.rs`:
  drawing, landing, keys, folding, the context menu), each view adding its own. The sidebar's
  selection is its held row when the app lands it there, else the place shown; moving through
  it opens each place.
- **The menu bar, the tray's menu and the accelerators are the core's tables** (`menu.rs`:
  `keys::menus`, `keys::tray`, `keys::SHARED`). What is Windows' own: each command's number
  (`menu::NUMBERS`, which the UI tests post), its access key, Exit for Quit, the ellipsis where it
  `asks`, Cut, Copy, Paste and Select All, and the keys a control answers written beside its
  command (Enter, Delete, Alt+Enter). A command is run as its id (`App::run_command`): the
  window's own, else `Key(id)`, a Task-menu row command `Details(Key)` away from a task list's
  row. As a menu opens its items take the app's names and states (`ViewModel::commands`: Mark Not
  Done), grey only where they act on what the app judged, and are enabled again as it closes,
  since an accelerator does nothing while its item is grey. The tray adds the timer's control
  (`Device::timer_control`, `toggle_timer`) before Exit.
- **The app's dialogs are opened from the turn that has them** (`App::ask`, after the turn):
  its open question (`asking.rs`), the block form (`block_form.rs`), a proposal's screen
  (`proposal.rs`), a reminder list (`reminders.rs`). A question is asked by its number
  (`Asking::number`): a new number is a new question, and the one last answered is asked again
  only once refused, starting from what was typed (`Asking::typed`), so a turn made while its
  answer is written never asks it twice. Only a question counts as asking, since a dialog of
  the app's asks questions of its own. Each dialog draws from the app's current view and closes
  when the app's screen does; a refusal is said once. Put in a Block's pick ends in the app's own
  Another Day, which the app follows with Go to Day's question and that day's blocks.
- **What the app says goes to the status line of the dialog in front** (`App::open_live`), a
  dialog hiding the window's: a proposal's screen, a settings page, pairing, a reminder list.
  What it says as the app closes that dialog is said where it goes next, once it has gone.
- **A field being typed in takes the app's value only once the app is still**
  (`controls::write_typed`, then a 50 ms poll): a turn can come from typing a moment old, and
  writing it back undoes what was typed since; once still, a change from elsewhere or the app's
  correction is taken. A turn with no task must not make the details forget the one being
  typed in, or the next rewrites every field.
- **The tree's own folding is the app's** Expand and Collapse (`TVN_ITEMEXPANDED`, a tree
  item's `expanded` set from the row's); the app's "Expanded" or "Collapsed" is not said after
  one the tree made, since the tree reports the state itself.
- **Times and dates are Windows' own** (`clock.rs`: `GetTimeFormatEx`, and the locale's long
  date with its year taken out where the app leaves it out), handed to the app as its `Locale`.
- **A day or a time has the system's picker beside its line** (`picker.rs`, a
  `SysDateTimePick32`): Go to Day's (a question with `day` set), the block form's start and
  day, and the time settings. The line comes first and stays the way in; the picker, after it in
  tab order, writes `YYYY-MM-DD` or `HH:MM` there as if typed (a setting is applied when the clock
  is left), and takes the line's value only when it is one. It is named after its line and the
  core's `picker_words`, and so is a date picker's button (its MSAA child 2). UI Automation reads
  a picker as a pane with no value of its own: its value is MSAA's, which NVDA and JAWS read.
- **Syncing is the app's network's**, as on the Mac: resident, so in front for as long as it
  runs, with `serve_commands("Lumenna for Windows", …)`. The app asks to keep in sync once
  another device is paired, and only then holds the endpoint and serves the pipe; unpaired,
  `lum rpc` answers on its own and `lum sync` says it is not paired.
- **Settings is a property sheet over the app's Settings and Devices** (`settings.rs`), its
  pages the core's (`SettingsView::pages`): their titles, settings in order, buttons and captions,
  each setting a control by its `kind` (a toggle a check box with its hint under it, a choice a
  drop-down, a time a line with its clock, a number, a line, a folder with Choose), two short
  ones to a row; access keys are given in reading order from each label's first free letter.
  What only Windows has goes on its page: the shortcuts from anywhere on General (their words
  `hotkey_words`), the devices on Devices and Sync, the exports on Export and Import. Each
  page draws `view.settings` and `view.devices` once it exists (a sheet makes a page the first
  time it is shown), a page coming to the front taking the app's sayings to its status line.
  A line is sent as typed (`SettingsEvent::Type`) and applied when left. Rename and Unpair hold
  the device first. Back Up Now is the device's own backup (`Device::backup`); an export the
  app makes is saved where the person chooses, and an import is read and handed to the app.
  The store's own actions (`SettingsView::offered`, Erase Everything Deleted) are buttons on
  Export and Import after the rest, taken as the sheet opens, handed back with
  `ActOn(ActingOn::Settings, …)`, `offered_caption` under them and as each one's description;
  their questions open over the sheet. Pairing (`pairing.rs`) is a dialog over `view.devices.pairing`, on the app's network. The
  shortcuts from anywhere, this PC's alone, stay the window's.
- **Opening at sign-in is the app's background setting** (`background`, `WindowsSignIn`): the
  General page's box and the hint under it are the row's words, its toggle `SettingsEvent::Set`;
  a turn's `background` is carried out on the current user's Run key (`settings::keep_running`)
  and answered with `Offered` as the key now stands, or `Failed` with Windows' message.
  `Offered` is sent at start too. The key's command is `--background`, with `--profile` for a
  profile other than the usual one; its value is "Lumenna" for the usual profile and
  "Lumenna <profile key>" for each other (`tray::profile_key`), so each profile's is its own, and
  it counts as on only while it starts this executable on this profile; `LUMENNA_RUN_NAME` for
  tests, so they never change the person's.
- **What Lumenna is, its name, version, publisher, links and identity, is the workspace's
  `[workspace.metadata.lumenna]`**, read as each package is built, never copied into it: the Inno
  Setup script reads Cargo.toml itself (ISPP's `ReadIni`), and `pack.ps1` and `test.ps1` through
  `installer/metadata.ps1`, filling the MSIX manifest's `@NAME@`, `@DESCRIPTION@` and
  `@VERSION@`. `records`' metadata test holds both to it. About is the core's
  `words::metadata`.
- **The installer is Inno Setup's** (`installer/lumenna.iss`, built per architecture by
  `release.yml`): the app and `lum` for one person, with no administrator, into
  `%LOCALAPPDATA%\Programs\Lumenna`, `lum` on their `PATH`, the Start menu shortcut carrying the
  notification identity (`dev.emassey0135.Lumenna`), so toasts belong to it. Its `AppId` never
  changes: Windows and winget know an upgrade by it. Uninstalling takes what the app wrote (the
  sign-in Run values, the notification identity, a `lum daemon` logon task, the Jump List) and
  asks, No by
  default and never when silent, whether to delete the person's data too
  (`%LOCALAPPDATA%\lumenna`, the store and its backups, and `HKCU\Software\Lumenna`). The
  workflow installs, runs and silently uninstalls it on the runner, checking the store stays.
  The uninstall question itself is checked by hand. What only the app can name, every
  profile's Run value and the Jump List, it takes away itself, started by the uninstaller with
  `--uninstall` (`win/mod.rs`): it opens no window and takes no instance lock.
- **The MSIX is for the Microsoft Store** (`installer/msix/`: `AppxManifest.xml`, `pack.ps1`,
  `test.ps1`), unsigned as the Store takes it, its identity a stand-in until Partner Center's
  (the repository variables `MSIX_IDENTITY_NAME`, `MSIX_PUBLISHER`). One `lumenna.exe` serves
  both channels, asking at run time whether it is packaged (`profile::package_family`,
  `win/package.rs`). Packaged, its store is the package's own folder, named outright
  (`…\Packages\<family>\LocalCache\Local\lumenna\data`), where its AppData writes would land
  anyway, so its lock, window class and pipe, named from that path, never meet an installer
  copy's; it opens at sign-in through the package's startup task (`LumennaAtSignIn`, enabled off
  the window's thread and answered through `WM_EVENT`), since a package's registry writes never
  reach the Run key; its toasts are the package's; and `lum` comes with it under an app
  execution alias. `test.ps1` trusts a certificate machine-wide: for CI's throwaway runners only.
- **The Widgets board's widget is the MSIX's alone** (`win/widgets`): `lumenna.exe --widgets`, a
  COM server of the class the manifest's `windows.comServer` and `com.microsoft.windows.widgets`
  extension name, started by the board. The app in a mode of its own rather than a program of
  its own, since the statically linked core would be carried twice; started so, it opens no
  window, takes no instance lock and gives no reminders. It draws the glance (`Device::glance`,
  in Windows' clock) as Adaptive Cards (`src/widget.rs`, its templates in `src/widget/`), one
  definition for each of the core's widgets (`glance_widgets()`, whose test checks the
  manifest's names and descriptions), the card chosen by size: Now, small, the headline and the
  timer running; Today, medium, the headline, the next block and the tasks due,
  each the glance's own line (`GlanceEntry::tasks`) beside a button named as the glance names
  Mark Done (`mark_done`, Title Case), run as `assist("complete_task")`, then New Task (the
  core's `NewTask` part, its launch shortcut's name), whose `launch` verb starts the app with
  `--launch new-task` as the Jump List does, and only on an id the core has. The text is the card's data, never its template, so a `${` typed in
  a title stays text. While a widget is on screen it looks every two seconds whether the store
  moved, and draws again then, at each moment the timeline names and each minute a timer runs.
  The Widgets API is the Windows App SDK's, not the `windows` crate's: generated once into
  `win/widgets/bindings.rs` (how, at its top), and reached through the package's dependency on
  the Windows App Runtime framework, which the Store installs with it and `test.ps1` installs
  from Microsoft's NuGet package. **The Inno-installed app has no package identity, so it offers
  no widget**: a package with an external location would give it one once Windows signing
  exists (`ROADMAP.md`). What the board draws, and Mark Done, are checked by hand; CI checks the
  class is registered with the package and that COM starts the app as it.
- **The Jump List is the core's ways in** (`win/jump_list.rs`, `launch_shortcuts()`), made at
  start off the window's thread, on the taskbar button and in Start: a package's through
  `Windows.UI.StartScreen.JumpList`, whose items hand `--launch <id>` to Windows' activation
  (`package::launched_with`); an ordinary install's as the shell's tasks for the notification
  identity, each a link to the executable with `--launch <id>`. That identity is one for every
  profile, so only the usual profile's copy makes it, or the last profile to start would own it.
  A second copy started with `--launch` hands the id to the one
  running (`WM_COPYDATA`, `tray::claim`). Either way the window is shown and `Launch(id)` sent
  once the app is still, so it runs over the store read; New Task's line, opened by the app,
  then opens the window's quick add, which draws it.
- **Quick add sends its line as it is typed** (`Typing::Text`); its readback and adding are the
  app's (`QuickAdd`), which holds what it added. Completion asks the store at keystroke rate
  (`App::complete`): the view's offer can lag the typing.
- **Building on ARM64 Windows needs clang on `PATH`** for `ring`: Visual Studio's
  `VC\Tools\Llvm\ARM64\bin`. The manifest is embedded by the MSVC linker from `build.rs`, and
  so is the icon (`lumenna.ico`, drawn by `examples/icon.rs` from GTK's SVG's shapes), as a
  `.res` build.rs writes itself: no `rc.exe`. A toast's icon is a file the identity names
  (`IconUri`), so its largest image is written under `%LOCALAPPDATA%\Lumenna`.
- **Stock controls only.** Every list is a `SysTreeView32`, so level, position, set size,
  expansion and checkboxes are the control's to report; an item's text leaves exactly those
  out. Dialogs are in-memory templates run by the dialog manager.
- **Names and the live region go through Dynamic Annotation** (`IAccPropServices`), never a
  provider of our own. The status line is a polite live region; a dialog in front hides it,
  so each settings page and the pairing dialog has its own.
- **An empty static is named after the label made just before it**, so a page's status line
  is made first, or it reads the page's footer twice.
- **A reload updates a tree in place when its rows are the same ones**, so refreshes do not
  make the screen reader re-read the focused row.
- **Nothing that rebuilds a tree runs inside one of its notifications.** Space, Delete and
  double-click go through `App::defer`; `Tree::busy` marks notifications a refill sends.
- **A row's actions are offered in one place** (`win/actions.rs`): the context menus, the
  Task menu, the details buttons and Delete, Space and Enter all take the row's core actions
  in short (`offered`, a list's by place in `ListView::offered`) by kind, and run them as the
  app's with `ActOn`, the keys excepted. A trashed task's Erase Permanently is its menu's and
  the Task menu's alone, with no accelerator. A row the tree folds by a click is put in hand to fold it, and the
  selected row put back after (`actions::fold`). The details pane's buttons are Save and the
  task's `primary` actions as it offers them now (made again only when they change), then
  Actions, a menu of every one; its words and its lines besides the form (`Lines::details`) are
  the core's (`details_words`).
- **IsDialogMessage runs over the whole main window**, with the panes
  `WS_EX_CONTROLPARENT`, so menu command ids are never 1 or 2 (`IDOK`, `IDCANCEL`). Field
  mnemonics avoid the menu bar's letters (F E V T D H): the dialog manager gives
  Alt+letter to a field first.
- **A tree view passes a keyboard context menu up altered** (naming the pane, at −2, −2),
  so a request from a pane is taken as from the keyboard; right-clicks come from
  `NM_RCLICK`.
- **Never disable the control that has focus** without moving focus first: focus goes
  nowhere.
- **The Notes field gives Tab and Escape back** (`WM_GETDLGCODE`), and panes ignore
  `WM_CLOSE`, which a multi-line edit sends its parent on Escape.
- **Global shortcuts are this PC's** (`HKCU\Software\Lumenna\Shortcuts`, `shortcuts.rs`), what
  they are, their keys until changed, their warnings and every word about them the core's
  (`lumenna_app::hotkeys`). They are unregistered while new keys are chosen. Windows does not say who holds a refused
  shortcut, so another open Lumenna window (class `LumennaMain-`) is named rather than
  "another program". `--no-shortcuts` leaves them to another copy.
- **One instance per profile** (a mutex and window class named from the profile path). The
  first pairing triggers the firewall prompt, since the sync endpoint then listens; an
  unpaired app listens on nothing.
- **Completion is a popup menu** on Down or Ctrl+Space (`completion::attach`), in quick add, a
  list's filter, a question whose `Asking::syntax` is set and a form field with a `syntax`
  (the block form's Tasks from). Items lead with the name so a letter
  finds them; the first is highlighted by a Down queued before the menu opens, marked with
  a reserved bit of the key data so the field swallows it if the menu does not take it.
- **A list item's reason is its description, set twice** (`a11y::set_item_description`, on
  the item's child id: `TVM_MAPHTREEITEMTOACCID` in a tree, index plus one in a list box):
  as MSAA's `PROPID_ACC_DESCRIPTION`, which NVDA and JAWS read, and as UI Automation's
  `HelpText`, which Narrator reads. UIA's proxy does not carry the MSAA one over. A pick's
  `Choice::reason` and a proposal's sitting reasons go this way.
- **A colour is a mark beside the name** (`swatch.rs`): a filled circle in each tree's image
  list, the swatch's `light` or `dark` by the app's mode, the list given to a tree only while a
  row has a colour (so a tree without any has no gap) and taken away under high contrast. An
  item's image is no part of its name to MSAA or UI Automation. The Colour pick's list box
  draws its own items then (`LBS_OWNERDRAWFIXED | LBS_HASSTRINGS`, the strings kept for what a
  screen reader reads); the block form's Colour is a drop-down, as Kind is. A UI test reads the
  screen with its thread DPI aware, since UI Automation's places are physical pixels.
- **Reminders are toasts** (`toast.rs`), given by the device's own delivery as the Mac's are
  (`Device::deliver_reminders`, asked off the window's thread and shown back on it, `WM_THEN`, as
  Back Up Now's backup is taken; their buttons writing through the device), under an app
  identity of their own (`LUMENNA_APP_ID` for tests), each as the app lists it
  (`lumenna_app::due::arrived`: the summary of what was missed, then each due) with its buttons
  in the core's order and words (`Due::buttons`): Carry Forward and Replan where a firing has
  them, then Dismiss and Snooze; a button's sentence is its change's, else the core's `said`.
  An overrun's Replan is `DueEvent::ReplanFiring`, naming the firing as the device delivered it
  (not `DueEvent::Replan`, which works on the reminders the app looked for itself): the app opens
  Today with Replan from Now and dismisses the warning once something is replanned. Tests can read a shown toast's XML from the
  notification history but cannot press its buttons, so what each button does is checked by
  hand.
- **UI tests** (`tests/ui.rs`) drive the real app and assert on UI Automation. They take the
  foreground, so they are ignored by default and take turns:
  `cargo test -p lumenna-windows --test ui -- --ignored`. A posted key needs the app active
  and not minimized, and a window holding the foreground (a firewall prompt) fails the
  focus checks. They start the app with `--no-shortcuts`, or a test's copy takes the
  person's shortcuts and theirs says they are in use. While the person's own copy runs from
  `target/debug`, `lumenna.exe` cannot be replaced: build them with
  `CARGO_TARGET_DIR=target/ui-tests`.
- **`examples/inspect.rs`** is the same automation by hand (`cargo run -p lumenna-windows
  --example inspect -- "- Lumenna"`, with `--post` steps). Match `- Lumenna`: a terminal
  named after the checkout matches `Lumenna`. UI Automation's SetFocus does not move focus
  in a modal dialog, so `focus:` uses `WM_NEXTDLGCTL`, on the first *focusable* element of
  the name: a dialog's label is named like its field, and comes first. PowerShell's managed
  `System.Windows.Automation` is no substitute: under x64 emulation it saw unnamed panes.

