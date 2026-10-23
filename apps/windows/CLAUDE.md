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
  Space, Delete and the Task menu's commands run its action as the app's by kind (`Run`), and a
  context menu, a button or Enter the one chosen (`ActOn`, naming the row), only `Asks::Form` the
  window's (the details pane). The sidebar's selection is its held row
  when the app lands it there, else the place shown; moving through it opens each place.
- **The app's dialogs are opened once the app is still** (polled every 50 ms from a turn that
  has one, `ask_when_still`): its open question (`asking.rs`), the block form (`block_form.rs`),
  a proposal's screen (`proposal.rs`), a reminder list (`reminders.rs`). Straight after an answer
  the question is still open while the write is under way, so asking then asks it twice. Only
  a question counts as asking, since a dialog of the app's asks questions of its own. Each dialog
  draws from the app's current view and closes when the app's screen does; a refusal is said
  once and what was typed kept. Put in a Block's pick ends in the app's own Another Day, which
  the app follows with Go to Day's question and that day's blocks.
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
- **Settings is a property sheet over the app's Settings and Devices** (`settings.rs`): each
  page draws `view.settings` and `view.devices` once it exists (a sheet makes a page the first
  time it is shown), a page coming to the front taking the app's sayings to its status line.
  A line is sent as typed (`SettingsEvent::Type`) and applied when left. Rename and Unpair hold
  the device first. Back Up Now is the device's own backup (`Device::backup`); an export the
  app makes is saved where the person chooses, and an import is read and handed to the app.
  Pairing (`pairing.rs`) is a dialog over `view.devices.pairing`, on the app's network. The
  shortcuts from anywhere, this PC's alone, stay the window's.
- **Opening at sign-in is the app's background setting** (`background`, `WindowsSignIn`): the
  General page's box and the hint under it are the row's words, its toggle `SettingsEvent::Set`;
  a turn's `background` is carried out on the current user's Run key (`settings::keep_running`)
  and answered with `Offered` as the key now stands, or `Failed` with Windows' message.
  `Offered` is sent at start too. The key's command is `--background`, with `--profile` for a
  profile other than the usual one; its value is `LUMENNA_RUN_NAME` for tests, so they never
  change the person's.
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
  app's with `ActOn`. A row the tree folds by a click is put in hand to fold it, and the
  selected row put back after (`actions::fold`).
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
- **Global shortcuts are this PC's** (`HKCU\Software\Lumenna\Shortcuts`, `shortcuts.rs`).
  They are unregistered while new keys are chosen. Windows does not say who holds a refused
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
  (`Device::deliver_reminders`, their buttons writing through the device), under an app
  identity of their own (`LUMENNA_APP_ID` for tests), their buttons the core's: Carry Forward and Replan where a
  firing has them, then Dismiss and Snooze. Tests can read a shown toast's XML from the
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

