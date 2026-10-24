# The GTK app

Read when working in `apps/gtk/`. The root `CLAUDE.md` holds what every part of Lumenna shares; this is what is easy to get wrong here.

`apps/gtk/`: Rust over `gtk4-rs`, binary `lumenna-gtk`, drawn from the Crux app:
`lumenna-ffi` as an rlib (`App` and `Device`, as the Mac runs them), with `lumenna-app` for
the view model's types and how every line reads and `lumenna-records` for the records it is
made of. It names neither `lumenna-host` nor a connection of its own to the store.

- **The app's turns go to GTK's main thread** (`model.rs`, `glib::MainContext::invoke`) and
  are taken in order (`window::take`): the screens draw, then what the turn says is said, on
  the status line of whichever of its windows is in front. The screens: the task list and the
  Blocks list (`tasks.rs`, one list for every listing), the sidebar (`sidebar.rs`), the day
  (`day.rs`), the details (`detail.rs`), the block form (`block_form.rs`), the proposals
  (`proposal.rs`), Settings with the Devices page and pairing (`settings.rs`, `pairing.rs`), a
  reminder list (`reminders.rs`) and quick add (`quick_add.rs`). In a list a move sends
  `Focus`; Space, Delete, Enter and the Task menu send `Run`; its menu and Left and Right (the
  row's Collapse and Expand, `Tree::connect_fold`, `Item::expanded`) run the row's action. The
  app's "Expanded" or "Collapsed" after a fold the tree asked for is not said (`QUIET_FOLD`):
  the tree item reports its state. The one form the app hands back is a task's details.
- **The sidebar is the app's places** (`view.sidebar`, `sidebar_held`): a move holds the row
  (`Hold(Side::Sidebar, …)`) and opens a place; a row's actions are held there, so a change
  made there lands there. The middle pane follows the app's place (`window::follow_place`) as
  the app holds it now, since a turn made before the person last went somewhere still names
  where they were. The window opens before the places come, so the first drawn take focus
  from their empty pane.
- **The day is the app's** (`view.day`, `lines.day`, `lines.day_title` as the window's
  title): its moves (`Day`: Previous, Next, Now, Ask for Go to Day) and a reminder's block
  (`ShowBlock`). The day's own actions are its heading row's (the summary) and the Day
  menu's, which sends `RunDay` with each kind, going to Today first from another place.
- **The details are the app's** (`view.details`): each field is sent as `Details(Field)` and
  refilled from the app except where focus is. Focus coming into them sends `Details(Show)`,
  so they keep their task while the person works there. The Task menu runs the details'
  task's actions with `ActOn(Details, …)` away from a task list's row.
- **The block form is a window the app opens and closes** (`view.block_form`): every field
  the core's form has for its purpose is made once, and shown or hidden as the app lists them
  (Until while it repeats). A refusal is told once.
- **Replan from Now and Plan the Day are a window the app opens and closes**
  (`view.proposal`): a new proposal (`shown` moving) is a new list of check boxes whose first
  takes focus. What the app says while it is open is said on its own status line
  (`proposal::say`), the first held until the window is in front. An overrun's Replan button
  sends `Proposal(Replan)`.
- **A reminder list is a dialog the app opens and closes** (`view.reminders`), named after
  what it is for from the row it was opened on; its rows' actions are held there
  (`Hold(Side::Reminders, …)`), and focus goes to a reminder just added, or to Add Reminder
  once the last is removed.
- **Quick add is a window the app opens and closes** (`view.line`): each keystroke is
  `Typing(Text)`, a completion chosen `Typing(Choose)`, Add `QuickAdd`; a refused line stays,
  told once. The filter's completion asks `App::complete` at keystroke rate.
- **Focus moves only when `view.landing` goes up** (`tree::Landing`, shared by every list; or
  once a list gone to from a key has rows), and only if focus is in the list or nowhere; a
  rebuild for a sync or a minute puts the selection back without moving focus.
- **The app's open question is asked once the app is still** (`asking.rs`, polled every 50 ms
  from a turn that has one): straight after an answer it is still open while the write is
  under way. A refusal is said, then a line is asked again with what was typed. A key a row
  lacks is the app's to refuse: `Run` after a side list's `Hold`, `RunDay` for the day's own
  actions, `Details(Run)` for the details' task; nothing here words a refusal.
- **The clock setting reaches the app's thread as a number** (`clock::Locale::twelve`, kept
  from GSettings' change signal): the app's `Locale` is called off the main thread, where a
  `gio::Settings` cannot go.
- **Syncing and the command surface are the app's** (`in_front(true)` for as long as it runs,
  `serve_commands`): it syncs once the device list shows another device, and serves the
  socket only while it syncs, so an unpaired app holds no endpoint and `lum rpc` answers
  itself (`test_commands.py`). Quit takes it out of the front first, which stops serving, then
  syncing. The automatic backup is the device's (`Device::back_up_if_due`), on a thread.
- **Settings are the app's** (`view.settings`): each page's controls are made from the core's
  descriptions, in the page's own order with this app's mnemonics; a toggle or choice is
  `Settings(Set)` as it changes, a line `Settings(Type)` as typed and `Set` when left. The
  Devices page sends `Devices(Show)` as it is shown, its rows held there
  (`Hold(Side::Devices, …)`). Pairing is a dialog over `view.devices.pairing`: the words are
  asked once each time they come, and Cancel while a pairing runs gives it up and then closes.
  An export's file is chosen first, then the app's `SaveFile` written there; Back Up Now is
  the device's.
- **Opening at sign-in is the app's background setting** (`autostart.rs`,
  `BackgroundKind::DesktopSignIn`): this profile's XDG autostart entry. The app is told at
  start whether it is there; a turn's `background` makes or removes it (`window::take`). The
  UI tests give each session its own `XDG_CONFIG_HOME`, so none writes the desktop's
  autostart folder. **In a sandbox (`crate::sandboxed`, a Flatpak) it is the Background
  portal's**, which a sandbox's own autostart folder never reaches: asked over the window with
  the core's reason (`BackgroundWords::asking`), and since the portal never says whether it is
  on, the app keeps the choice granted in the sandbox's configuration. At each start while that
  says on it asks again (`renew`), which asks the person nothing once allowed, keeps the
  portal's command current, and is the one reading of the value: refused, as when the person
  turned it off in the desktop's settings, the record is forgotten and the setting told it is
  off. A portal that cannot be reached changes nothing.
- **The Flatpak** (`flatpak/dev.emassey0135.Lumenna.yml`, GNOME 51, `lum` beside the app,
  `data/…metainfo.xml` for app stores) is for a repository of our own: it fetches crates as it
  builds, which Flathub refuses. Its store is the sandbox's, `~/.var/app/<id>/data/lumenna`;
  its tray serves without a well-known name (`disable_dbus_name`), which a sandbox cannot own.
  `flatpak-builder` needs `--disable-rofiles-fuse` where FUSE mounts are refused (a VM's
  shared folder). `release.yml` builds its bundle in Flatpak's own container (GNOME 51) on each
  architecture's runner, installs it, and has `lum` add a task in the sandbox; uninstalling it
  there wants a session bus, which a CI container lacks, and the container goes anyway.
- **The AppImage** (`release.yml`'s GTK job), the release beside the Flatpak. No tarball: the
  AppImage is the program alone with GTK inside, and `install.sh` is for a checkout.
  `linuxdeploy`'s GTK plugin gathers GTK 4 and libadwaita into the AppDir, `appimagetool` packs it with the static runtime, which needs no
  FUSE 2, in zstd at level 19 with 1 MiB blocks (42 MB; its own settings made 45, and xz's 38
  starts slower). Built on Ubuntu 24.04, it needs glibc 2.39 (Ubuntu 24.04, Debian 13, Fedora 40 and
  later); older systems have the Flatpak. The job starts it under `xvfb-run` with
  `--background` and requires it still running, its store made. The tools run as AppImages
  themselves, so the job sets `APPIMAGE_EXTRACT_AND_RUN`. Not yet: its updater, and offering a
  desktop entry on first run, which the shortcuts portal needs for an unsandboxed app.

- **Every list is a `GtkListView` that says it is a tree** (`tree.rs`). `GtkTreeView`
  exposes no rows to AT-SPI in GTK 4, and a list view with `TreeExpander` reports a flat
  list. So the view has `AccessibleRole::Tree`, the list item is not focusable, and a
  `TreeExpander` with `AccessibleRole::TreeItem` is, given its level and position in
  `bind`. Left and Right are ours, and `ListTabBehavior::Item` makes Tab leave the tree
  rather than go through every row. Every list is one, the Devices list and the chooser
  dialog's included; a `GtkListBox` has no such setting. Type-ahead is ours too
  (`Tree::type_ahead`): letters typed within a second are one prefix, one letter again steps
  on, and a space only partway through a prefix, so a pause gives Space back to the row's
  action. A row's `description` (`Item::description`) is how a choice's reason reaches Orca,
  after its name.
- **GTK gives AT-SPI a tree item's level from 4.16, and its position and set size from
  4.22.** Below 4.16 (Ubuntu 24.04, Debian 13) a row says "level 2" in its text where the
  level changes against the row shown before; position is left to the platform. Decided at
  run time; `LUMENNA_LEVEL_IN_TEXT=1` forces it for tests. CI runs the UI tests on Arch, so
  the native path is what it covers by default.
- **Space, Delete and Enter send a fixed set of kinds** (`actions::SPACE`, `DELETE`, `ENTER`)
  as `Run`, so the row in hand runs its own; the Task menu's commands do the same, so Mark
  Done toggles. A menu's item, folding, a details button and Add Reminder send the row's
  action in short with `ActOn`. The question it asks is asked over the window it came from.
- **A dialog's buttons are GNOME's order**: Cancel first and the default, the answer last
  and a verb (`Question::Text.yes`, a pick's action title, a form's Save). A yes-or-no
  question puts the safe answer where Cancel would be (`prompts::yes_or_no`). Alerts are
  `AdwAlertDialog`, a destructive answer drawn as one. It lays its answers side by side or
  stacked as they fit, and Tab follows the layout, so tests `tab_to` a button by name. Its
  message is a nameless Tab stop ("scroll pane") that libadwaita makes focusable once shown;
  making it unfocusable loses focus altogether, so it stays.
- **libadwaita 1.5 is the floor** (Ubuntu 24.04), for `AdwAlertDialog` and `AdwAboutDialog`.
  Not used, by what Orca says: `AdwShortcutsDialog` (1.8) reads its items by title alone,
  never their keys, so the keyboard help (`help.rs`, from `lumenna_app::keys`) is a tree of
  ours; `AdwPreferencesDialog` drops each entry row's hint, opens a page only on Space, and
  does not say which page opened; a toast is announced as "A toast appeared: …" beside the
  status line. `AdwApplicationWindow` does not show a menu bar, which is how commands are
  found here.
- **Orca does not say a tree item's checked state**, so a done task says "completed" in its
  text.
- **Focus into a row waits for the row's widget**, which does not exist until GTK lays the
  list out. `Tree::move_to` retries each frame until it lands. `scroll_to(FOCUS)` also
  does not move focus into a list from outside it, so `land` grabs it too; and a rebuild
  takes focus out of the list for a moment, so a tree that had focus before one still
  counts as having it. `App::say` waits behind it (`tree::when_settled`): Orca reads an
  announcement as a message, and a later focus change cuts it off.
- **Announcements are `gtk_accessible_announce` from the window**, not from a label: one
  from a label in Settings never arrives.
- **GTK's accessible names need help**: a mnemonic check box keeps the underscore in its
  name (`prompts::check`); a button is named by its text whatever label it is given;
  read-only text is a non-editable entry, since a label named by the label above reads as
  that name.
- **Settings' tabs are a `GtkStackSwitcher`, not a notebook.** A notebook keeps focus
  itself, so Orca reaches its tab bar as an unnamed "grouping". Only the current tab is
  focusable, so the row is one Tab stop; the arrows and Ctrl+Tab/Page Up/Down are ours.
- **GTK adds "Alt+" and the mnemonic to a menu item's key shortcuts**, after whatever the
  property holds, so Orca reads "Control+N Alt+N". Alt+N does nothing; the letter alone works
  in an open menu. Only dropping the mnemonic removes it.
- **A text view keeps Tab**; `prompts::leaves_on_tab` steps it out of the focus chain.
- **A short list is a tree sized to its rows** (`Tree::fit`), hidden when empty. A list box
  in a scroller makes the scroller a nameless Tab stop, and making the scroller
  non-focusable breaks Tab for the whole page.
- **A day or a time has GTK's own picker beneath its line** (`pickers.rs`), filling it: a
  `GtkCalendar` for a block's day and Go to Day's, for a pointer alone (a named group out of
  the focus chain: GTK 4 gives AT-SPI its month and year buttons and none of its days), and,
  since GTK has no time picker, two spin buttons, hours and minutes, for a block's start and a
  time setting, each described by `picker_words` after the line's name where a page has
  several. Not a named group: Orca says a group's name twice going into a spin button. Orca
  says "Text unselected" after a spin button's value as it comes in (GTK selects its text on
  focus and then sets it); neither a non-editable spin button (its arrows then say nothing)
  nor clearing the selection on focus stops it. A setting's clock is applied when it is left,
  as its line is.
- **Orca says "opens popup" on every entry** (GTK reports a text field's context menu) and a
  field's mnemonic letter before its hint ("A A time, such as 9am"): both Orca's and GTK's own.
- **Dialogs are futures** (GTK 4 has no blocking `run`). While a popover menu is open,
  `window::spawn` waits for it to close, so a dialog is not mapped under a closing menu.
- **A keyboard context menu opens when its keys are let go**: a popover opened on
  Shift+F10's press is closed by the releases.
- **Reminders are `GNotification`s** (`reminders.rs`), so GNOME Shell shows them and Orca
  reads them as it reads any other; their buttons and opening are application actions
  (`app.reminder-dismiss`, `-snooze`, `-carry`, `-replan`, `-open`), which reach the app with
  its window closed, and answer through the device (`Device::dismiss_reminder` and the rest).
  It asks `Device::deliver_reminders` every five seconds while another process gives them
  (`elsewhere`), and every 20 once it gives them itself. The UI tests stand in for the shell with a
  notification server on the private bus (`harness.Notifications`), started before the app,
  which picks its backend at the first notification.
- **`LUMENNA_NOW`** (a local date and time, `clock::pinned_now`) pins the now proposals
  (`PlanFrom`) and reminder delivery (`clock::pinned_instant`) pass the core, for tests of the
  rest of a day at any hour.
- **The core words a whole sentence in `Locale::style()`**: the desktop's clock setting,
  else whether the locale's own time says 13:00 as 13.
- **One instance per profile** through GApplication's bus name, tagged per profile.
  `--no-shortcuts` leaves the shortcuts from anywhere to another copy; the UI tests pass it.
- **First focus on an expandable row says "expanded" twice**: GTK reports the new
  accessible object's initial state as a change.
- **The global shortcuts go through the GlobalShortcuts portal**, which knows an
  unsandboxed app only by an installed desktop entry (`install.sh`), so it is always asked
  under the app's own identifier. Without the portal, on X11 (Xfce, GNOME before 48), the same
  keys are grabbed on the root window over an X connection of the app's own (x11rb), Caps Lock
  and Num Lock either way. The tray (`ksni`) waits quietly for a host; GNOME has none without
  an extension.
- **A colour is a dot before a row's text** (`Item::swatch`, `tree::swatches`): a
  `Presentation` box styled `.swatch-<name>` from the core's palette, the light or dark value
  by libadwaita's `StyleManager::is_dark`, restyled as that changes; the row's name is its
  text alone. The block form's Colour is a drop-down of the core's options, as Kind is.
- **The icon** (`data/icons`, an SVG and PNGs drawn from it) goes into the user's hicolor
  theme with `install.sh`; a build run from the checkout adds `data/icons` to the icon
  theme's search path (`icons_here`).
- **UI tests** (`tests/ui/`, Python): `apps/gtk/tests/ui/run` starts a private runtime
  directory, D-Bus session, headless mutter and accessibility bus, presses keys through
  mutter's RemoteDesktop API and reads AT-SPI. **The runtime directory must be private, and
  made before the bus**: a run in the desktop's own `/run/user/<uid>` replaces the desktop's
  accessibility socket, leaving every app started afterwards without a screen reader.
  `ORCA=1` runs Orca too. A new window takes well over half a second to show in the
  headless compositor: wait with `Session.wait_for_window`.
