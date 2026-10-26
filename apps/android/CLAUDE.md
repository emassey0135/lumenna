# The Android apps

Read when working in `apps/android/`: the phone and Wear OS. The root `CLAUDE.md` holds what every part of Lumenna shares; this is what is easy to get wrong here.

## The Android app

`apps/android/`: Kotlin and Jetpack Compose over the Crux app (`crates/ffi`), through
BoltFFI's Kotlin bindings. Its README gives the toolchain. Three modules: `app` (the phone),
`wear` (Wear OS) and `shared`.

- **Every screen draws the app's view model and sends it what the person did**
  (`ShellEvent`), as the web does. One library (`liblumenna-ffi.so`) and its Kotlin
  (`dev.emassey0135.lumenna.app`) come from `build-core.sh` (`boltffi pack android`, an
  overlay pointing its outputs into the build directory). What runs without the screens —
  reminders' alarms and receivers, `SyncWorker`, the watch's link, backups — calls `Device`
  (`crates/ffi/src/device.rs`), the host's operations of the same names over the one
  connection the `App` runs on; so do the tests, to seed and check the store. BoltFFI's
  records have no defaults, so the tests seed blocks through `newBlock` (`Seeds.kt`).
  `#[boltffi::data]` marks every record the view reaches. Its scanner does not see a marker
  inside `cfg_attr`, so it is unconditional, and it resolves a type of another crate only by
  that crate's root path or an import, never a path into its modules. BoltFFI cannot leave a
  variant out, so the shell's events are `ShellEvent`, the app's `Event` less what its own
  effects answer; an exhaustive match each way sorts a new one. The network is the bridge's
  native middleware (`lumenna_bridge::network`, the `network` feature), syncing only while the
  app is in front (`App::in_front`), which is the platform's to say. Turns reach the Kotlin
  `Shell` from one thread of the app's, in order, what arrived together as one.
- **`shared` holds the core and what decides nothing about looks**: `Core` (the `Device` and,
  over it, the app), `AppModel` (turns taken on the main thread in order, speech held while a
  list lands), `Front` (the activity coming to the front and leaving it, the same on both),
  `Launching` and `ReminderOpened.reminded` (where a launch or a tapped reminder takes the
  screens), `Words.kt` (the locale's time and date, handed to the core as `Clock.locale`),
  `AppWords.kt` (what a tree control would say by itself and a Compose list does not: folding
  and the level), landing on a row (`Landing.kt`: `rememberLanding`, `RowFocus`, `RowKey`), a
  screen on what the app has open (`Showing.kt`), the forms' fields by the core's keys
  (`FormFields.kt`), reminders, the watch's link and `SyncWorker` (through `CoreHolder`, which
  each app's Application is). Each app draws the app's questions once at its root
  (`AppQuestions`, `WearQuestions`). A shared file must not be named like one of the phone's in
  the same package: both compile to one `…Kt` class, and the dex keeps only one.
- **Gradle builds the core itself**: `buildCore<Variant>` runs `build-core.sh`, which builds each
  ABI's static library alone (`cargo rustc --crate-type staticlib`, for the root's whole-program
  reason, the NDK's clang at API 28 compiling the C its dependencies bring), then has `boltffi
  pack android --no-build` link it with its JNI glue into one library per ABI; only the Kotlin
  beside it is copied. A release library is stripped there, with that NDK's `llvm-strip`:
  BoltFFI's link is past Cargo's `strip`, and the Gradle plugin strips only with the NDK version
  it expects, copying the library whole when another is installed.
- **Release builds are shrunk by R8** (`isMinifyEnabled`, `isShrinkResources`). The core reaches
  its Kotlin by name through JNI both ways, the `Native` object's externals and the callback
  objects it calls back (`ShellCallbacks.turn`), so `:shared`'s `consumer-rules.pro` keeps the
  bindings' package whole. The instrumented tests run on debug builds: against a shrunk app the
  test APK finds missing whatever the app does not use itself, so a release build is checked by
  hand, installed signed with the debug key, opened, and a task added; CI builds it, so R8's
  refusals show. The phone takes Arm64 and x86_64 (Googlebooks have both, and CI's emulator is
  x86_64), the watch 32-bit ARM too (`abiFilters`); minSdk 28, the watch's 30. Every library is
  16 KB aligned, which CI checks. `LUMENNA_ANDROID_ABIS` narrows the build.
- **Espresso is pinned to 3.7**: Compose's test library brings 3.5, which calls an
  `InputManager` method Android 17 removed.
- **The tests run Google's accessibility checks on every interaction**, on a store of their
  own (`Core(directory)`). Buttons scroll into view before a click: a click on an
  off-screen node lands on nothing.
- **The tests wait on the app as Compose waits on itself** (`stillApp`, an idling resource
  over `Core.still`: `App::is_still`, nothing with the store and every turn handed on, and no
  turn posted to the main thread undrawn). A turn comes from the app's own thread, so without
  it a test looks at the screen between pressing Add and the row arriving. A pairing or a sync
  under way does not count.
- **What follows the store looks again only when it moved** (`Core.changes`, `Heads`): after a
  turn the app leaves, the store's heads are read off the main thread, and `changes` moves only
  if they did, so a keystroke or a new minute arms no alarms and redraws no widget; coming to
  the front moves it too. The app ticks (another process's writes, the minutes) only while it is
  in front (`AppModel.inFront`), and is told the clock again there if the 12- or 24-hour setting
  changed meanwhile. The watch's link follows `changes` only while the activity is started
  (`repeatOnLifecycle`); `LinkService` answers the other side otherwise.
- **The time zone is Android's own**: `TZ` is never set, so the core reads
  `persist.sys.timezone` through jiff, the zone Java's clock reads, and follows a change within
  jiff's five minutes. Setting `TZ` from Kotlin raced the core's threads reading it.
- **Nothing that reads the store runs on the main thread from outside the screens**: the store
  opens on a thread of its own as the process starts; arming reminders, an alarm going off and a
  notification's buttons run on reminders' own thread, a receiver holding its broadcast open
  (`goAsync`); a backup, an export and an import are off it too.
- **What decides nothing about Android is tested on the JVM** (`src/test` in `shared` and
  `app`: `./gradlew :shared:testDebugUnitTest :app:testDebugUnitTest`), with the core's records
  built by hand: there is no native library there, so nothing a test calls may reach it.
- **Settings are the core's pages** (`SettingsPages`, `settings_pages`): each page's title,
  description, settings in order, caption and buttons; a page with nothing the store lists here is
  left out, Export and Import on the watch, which has nowhere to save a file. Where they are
  Android's own (whether notifications and exact alarms are allowed), the words are
  `notification_words`.
- **An overrun's Replan** is the app's `DueEvent.Replan`, so replanning dismisses the warning on
  every device: the notification carries its firing, and the app looks at that one moment first
  (`DueEvent.Look`), since its own list of what is due is not filled by Android's alarms
  (`AppModel.replan`).
- **Material's text and outlined buttons are 40dp**, and a radio row is as tall as its
  padding: both fail the touch-target check. `Target` gives 48dp.
- **A row is one node** (`ListRow`: `clearAndSetSemantics`, custom actions) with one
  description, the title first: as a separate state description, the rest is said before the
  title. `RowTitle` carries the title alone, for tests to find a row by.
- **A colour is a dot beside its name** (`SwatchDot`, shared's `Swatch.colour`): a row's, a
  pick's choice's (`Option.swatch`) and the block form's colour options, the core's light or
  dark value by `isSystemInDarkTheme()` on the phone and always dark on the watch, whose
  backgrounds are. It is outside the row's semantics, so TalkBack says the name and the
  colour's word the line already holds, nothing more. The block form reads each choice's
  value by its key (`BlockFields.text`), the kind and the colour alike.
- **Nested lists fold, as the app folds them**: a row with something under it has the app's
  Collapse or Expand among its actions and says which it is. The level is the app's, said
  against the row shown before, after folding.
- **What changes say, the phone shows as a snackbar**, a newer one taking the place of the one
  showing (`collectLatest`), which TalkBack has begun saying: waiting for each, the last of a
  run came long after. The announcements are a buffer of 64 that drops the oldest, so a run
  never loses its last, and nothing is kept while no screen listens.
- **TalkBack does not follow input focus under touch**, on the phone or the watch. `RowFocus`
  gives the row input focus, then performs the accessibility-focus action through the view's
  `AccessibilityNodeProvider`, found by the `RowKey` property; the announcement is held
  (`Core.hold`/`release`) until then. The test of it is skipped unless TalkBack is on.
  Compose sends accessibility events only while a screen reader runs.
- **Keyboard commands are the Windows and GTK apps' keys**, taken from the core's shared table
  (`keyboard_shortcuts()`, `lumenna_app::keys`) by each command's id, keys and name both;
  only the folding arrows are Android's own (`Shortcuts.kt`). On Ctrl: TalkBack's are on Alt
  or Search. A screen offers a command while shown (`Offer`), the newest winning, and the
  system's shortcuts helper (Meta+/) lists what is offered now. A row's keys are the core's:
  the row put in hand, then `ShellEvent.Key(id)` (`RowKeys.key`), which runs what
  `keys::what_runs(id)` names that the row offers, or says why not; the folding arrows
  (`Command.runs`: `expand`, `collapse`) go on to the list where the row does not fold. Shift+F10
  opens the row's own menu. With no row focused they are the details' task's
  (`DetailsEvent.Key`); the day's moves, New Block, Undo and Redo are `ShellEvent.Key` too, and a
  block's Enter is `open`. Space is not bound: on a row it is a click. Compose's root takes the keys while
  anything has focus; `MainActivity.dispatchKeyEvent` takes them when nothing does.
- **Erasing for good is a menu away, never a key's**: a trashed task's Erase Permanently is
  in its long-press menu and custom actions with the rest, and Delete, the core's key, runs
  Delete from Trash there. The store's own actions (`SettingsView.offered`, Erase Everything
  Deleted) are destructive buttons on Export and Import, the watch's under Backups, with
  `offeredCaption` beneath, sent as `ActOn(ActingOn.Settings, …)`; the app's question is asked
  at the root.
- **Buttons take focus only out of touch mode**, which a key press leaves; the keyboard
  tests leave it first (`setInTouchMode(false)`), or no tab could be focused.
- **A wide window has the desktop apps' sidebar** (`Sidebar.kt`, from the app's
  `ViewModel.sidebar`, plus Settings) from 840dp, with a stack of its own for the place chosen;
  a heading's or place's actions are the app's, run as Browse runs them. Between 600 and 840dp
  the tabs are a rail. `LumennaTest`, `NewTaskTest` and `ReminderScreenTest` show the app at a
  phone's width (`PhoneWidth`), so they run on the desktop-sized emulator too; `SidebarTest`
  and `KeyboardTest` are the wide window's.
- **A wide window** shows a screen beside the one it was opened from (`Navigator`'s depth: a
  row chosen in the parent replaces the child rather than stacking). F6 moves between the tabs
  and panes, back onto the row a pane was left on (`Pane`): Compose's `saveFocusedChild` keeps
  only the pane's immediate child. Tests force only the width: a forced size bigger than the
  window is drawn at a smaller density, and every touch target measures small. So touch
  targets are excused only when the forced width exceeds the screen; CI's desktop-screen run
  checks them in the wide layout for real. The desktop system images end at API 34:
  `desktop_medium` on 36 is a large screen without freeform windows. A real Googlebook
  (Android 17) has those, a title bar per window, and no menu bar.
- **Pairing is tested against a `lum pair`** as on the iPhone: `pairingByCodeBringsTheOtherDevicesTasks`
  takes its code as `INSTRUMENT_ARGS="PAIR_CODE=…"` (a `lum daemon` on the same profile answering
  the first sync, which brings a task called "written on the mac"), and
  `pairingOnTheLocalNetworkNeedsNoCode` runs with `PAIR_LOCAL=1` on a real phone; both skip
  without them. The emulator pairs over the relays.
- **`run-instrumented-tests.sh`** runs the instrumented tests from APKs already built, with
  `am instrument`, which exits 0 whatever happened, so its summary decides. CI builds once
  and runs it on each emulator. It never uninstalls, unlike `connectedAndroidTest`, but
  never point it at a device whose own data matters. It first sets the device up as a
  person's would be (past setup, screen kept on, a watch unplugged) and waits for the same
  ordinary activity to stay in front: otherwise something takes the foreground part way through
  a run, and the test then finds "No compose hierarchies".
- **The store is in no-backup storage** and `allowBackup` is off: Google's backup would copy
  a store that reaches other devices by pairing.
- **Background sync is WorkManager** (`SyncWorker`): a round on leaving (expedited on
  Android 12 and later; earlier, expedited work needs a foreground notification) and every
  15 minutes. Discovery hears multicast only under a `MulticastLock`, held while the app is in
  front, where it syncs and pairs; it is not exclusive and `mdns-sd` binds 5353 with
  `SO_REUSEPORT`, so other apps' mDNS is unaffected. The emulator's NAT passes no multicast.
- **Reminders are AlarmManager alarms** (`shared/Reminders.kt`, for the phone and the watch):
  the next 48 hours of `upcoming_reminders`, at most 100, each a broadcast whose data is its
  firing (`key@occurrence`), so re-arming replaces it; what was set and is no longer listed is
  cancelled. Armed when the store changes (`Reminders.follow`, which the app's return to the
  front moves too, since it says the store changed), after `SyncWorker`'s round, on boot, an
  update, a clock or zone change and exact alarms being granted (`ReminderRearmReceiver`), and
  by an alarm a day on. `ReminderSchedule` over an `Alarms` is the seam the tests check.
- **Exact where the person allows it**: `SCHEDULE_EXACT_ALARM`, not `USE_EXACT_ALARM`, which
  Google Play keeps for alarm clocks and calendars. Refused, alarms are inexact and the
  Reminders settings page says so, with the system's page that grants it. Revoking exact
  alarms kills the app, tests and all, so the page reads what is allowed through
  `LocalReminderAccess`, which tests set.
- **An alarm going off asks the core again** (`reminders_due` over that one moment): a firing
  dismissed or done elsewhere since it was armed is not shown, and one Android held back past
  the core's few minutes is shown in its `missed` words. Each arming also takes away shown
  notifications dealt with since on another device, but only its own store's (a notification
  names its store in its extras), or the app's store and a test's take each other's away.
  Posting is asynchronous, so tests judge what `fire` showed, not the manager's list. Dismiss
  and Snooze are `ReminderReceiver`, not exported. A notification is a reminder's category, so
  Do Not Disturb decides.
- **Notifications are asked for once** (Android 13 and later), when the person first adds a
  reminder or turns reminders on; after that the Reminders page says they are off and opens
  the system's page. Tests grant the permission first, or the system's question covers the app.
- **Carry Forward on a reminder** (`DueReminder.carry`) is `ReminderReceiver`'s too: it runs
  `carryRestForward("<target>@<occurrence>")` and says the change in a quiet notification of its
  own (tag `carried`, no `@`, so arming does not take it for a firing), since the app may not be
  open to say it.
- **A day or a time has Android's own picker beside its line** (`Pickers.kt`: `ChooseDay`,
  `ChooseTime`, named by the core's `picker_words`, shared's `pickers`), in the block form, Go to
  Day and a time setting's dialog, filling the line. The day is Material's calendar without its
  switch to typing (the line is that, and the switch is a 40dp target); the time is Material's
  time input, not its dial, whose inner numbers are 32dp targets. On a 12-hour clock its AM and
  PM are 36dp, Material's and not ours to size, so the test of it runs on a 24-hour clock.
- **The planner's screens are the app's proposal** (`Proposal.kt` in each app): Replan from
  Now and Plan the Day, which the day's own actions (the day's `offered`, on its heading as
  custom actions and as buttons) open. The proposal is said on opening; after a partial apply
  the core's `next` is the screen's list until it is empty. `LocalPlannerNow` fixes the
  moment proposed against, so the tests' proposals do not depend on when they run. A pick's
  `Choice.reason` is `Option.reason`, a line of its own beneath the name. A replan's other
  options are buttons under the core's `others`, each proposing again in place
  (`ProposalEvent.Take`).
- **An overrun's Replan button** (`DueReminder.replan`) opens the activity, not the receiver,
  with `ReminderOpened.kind` `replan`: the app goes to Replan from Now for today. A button that
  opens an activity does not cancel its notification, so `Reminders.opened(context, intent)`
  takes it away.
- **Gemini's App Functions are the core's assistant operations** (`functions/`,
  `androidx.appfunctions`): one function per operation of `assistant_operations()` in
  `LumennaFunctions`, an `@AppFunctionServiceEntryPoint` from which the library's compiler (KSP)
  writes the service (`LumennaFunctionService`) and the XML the system indexes into the APK's
  assets (`lumenna_functions.xml`, and `-v1` for Android 16's indexer). Each runs through
  `Device.assist`; a refusal, the question an operation that `confirms` asks included, is an
  `AppFunctionInvalidArgumentException` in the core's words. The library reads a description only
  from an annotation written ahead (`@AppFunctionInstruction`), so the table's words are copied
  there, and `AppFunctionsTest` reads the indexed XML and fails when a function is missing, extra,
  or worded or parametered otherwise than the table: an operation added to the table needs its
  function. The records answered (`functions/Records.kt`) carry each answer's announcement and
  identifiers; a replan or day plan is carried field for field, since it goes back whole.
- **Tasker's action plugin is the core's automation** (`tasker/`, `records::automation`), spoken
  directly over Locale's API rather than through joaomgcd's Tasker Plugin library, which runs
  every firing in a special-use foreground service with a notification. `TaskerEditActivity`
  (`EDIT_SETTING`) is the setup and the person's consent: `automation_operations()` (the
  assistant table less what `confirms`, since nobody is there to answer, and what takes a
  proposal), each a button named by its sentence, description and, if `destructive`, the core's
  "Removes something"; then the chosen one's `automation_fields`, each a line labelled and
  described by the core (TalkBack reads the hint with the field), with the core's
  `Device.automationChoices` behind a Choose button and a calendar for a day. A value may be a
  Tasker variable (`%name`), so every value is text, typed by the core as it fires
  (`Device.automate`); `automation_check` refuses Save beside the button, leaving a variable
  until it fires. Saving makes a 256-bit secret, kept as its hash beside the operation in the
  app's own preferences (`TaskerSecrets`) and in the bundle Tasker keeps, never among its
  `VARIABLE_REPLACE_KEYS`; a firing (`TaskerFireReceiver`, `FIRE_SETTING`) whose secret was not
  set up here for that operation is refused with the core's `notSetUp`. The receiver holds the
  broadcast open (`goAsync`) while the core runs on a thread of its own, and answers Tasker's
  ordered broadcast in its result code and extras (a refusal `%errmsg`), or its completion intent
  where it did not wait. The variables are the core's (`automation_outputs`, declared to
  Tasker's picker as `%name` or `%name()`, given back as `%name1`, `%name2`…), the blurb its
  `automation_summary`. `TaskerTest` covers the list against the table, secrets refused, the
  variables a firing answers, and the receiver itself, in the phone's smoke set so a branch runs
  it; Tasker itself is checked by hand.
- **The widgets, the watch's tile and its complications draw the core's glance** (shared's
  `Glances`: `Core.glance`, the app's while it runs, else the device's): the entry true now, drawn
  again when the store changes and at the next moment the timeline names (`GlanceWorker`, one
  unique WorkManager job), on what each Application shows it on (`GlanceShower`). A running timer
  counts by itself from `countingFrom`: the phone's chronometer (`AndroidRemoteViews` over
  `widget_timer.xml`), the tile's dynamic time. Each is one TalkBack stop named by the glance's
  `spoken`. The larger widget's tasks are the glance's own lines (`GlanceEntry.tasks`), each with a
  button of the core's `markDoneSentence`, named with the task's title, marking it done through
  `Device.assist("complete_task")`. Glance composes again only when its state changes, so a
  refresh moves `Widgets.REFRESH`. The two widgets are the core's (`glance_widgets()`: Now and
  Today), their names and descriptions in `strings.xml` as `widget_<id>` and
  `widget_<id>_description`, which a test of the core checks; each draws the parts the core lists
  for it, in its order (`Widgets.parts`), so a widget short of room loses the last. Today's last
  is New Task (`GlancePart.NEW_TASK`): Glance's `FilledButton` with a plus, in the `new-task`
  launch shortcut's sentence and named by it, a TalkBack stop of its own as Mark done is, opening
  the app on quick add as New Task's tile does.
- **The ways in from outside are the core's** (`Launches`, shared's `Launching`): the icon's
  shortcuts are set at run time from `launch_shortcuts()` (`ShortcutManagerCompat`), and they, the
  Quick Settings tile, a widget, the tile and a complication open the app on a launch `id`, sent
  as `ShellEvent.Launch`; the screens follow the app once it is still (its line open is quick add,
  else its place, a task's details over the tasks). `NEW_TASK` stays for a shortcut pinned before.
- **The timer's Quick Settings tile is the core's control** (`TimerTile`, `controls()`,
  `Device.timerControl`): its label the Timer control's sentence, its subtitle the control's
  `detail` (the task running, else what a press starts, else why nothing would), its state
  `STATE_ACTIVE` while `running`, `STATE_UNAVAILABLE` when not `enabled`, else `STATE_INACTIVE`,
  and TalkBack's description the name, `actionSentence` and detail (`TimerTile.shown`, which the
  tests check). A press is `Device.toggleTimer` off the main thread; after it the subtitle is the
  core's announcement until Quick Settings closes, since a tile has no announcement of its own.
  It is an active tile, drawn when the store changes (`requestListeningState`, from
  `showGlances`) and, while shown, as `Core.changes` moves. **A press works on a locked phone**:
  it writes only the timer and is undone as any change is, so it gives nothing away and harms
  nothing. What a task is called is behind the lock, so behind a secure lock screen (`isSecure`)
  the tile shows and says only what a press does, never the task or the announcement, and a
  press is told by its state. New Task's tile asks for the unlock, since quick add is the app.
  The manifest names each tile ahead (`control_<id>` in `strings.xml`), which a test of the core
  checks against `controls()`.
- **The system's search index holds the core's search documents** (`SearchIndex`,
  `androidx.appsearch` platform storage, Android 12 and later): `Device.searchDocuments()`, one
  `GenericDocument` each under a schema of `SearchDocument`'s fields, written again as the store
  changes (a fingerprint per document, kept in preferences, says what changed and what went).
  Visible to the system's UI, the home screen's and assistant's search, and to callers that may
  execute App Functions, so an agent finds a task there to name it to a function. What matches
  and how a result reads stay the core's. Opening one is `Launching.ACTION` with its `launch`.

## The Wear OS app

`apps/android/wear/`: Compose for Wear OS over `shared`, the phone's application identifier,
standalone. A full peer: Wear OS allows sockets, so it runs Iroh and pairs and syncs as the
phone does.

- **No text field on a watch**: every line comes from the system's input screen
  (`RemoteInput`, through `TextEntry`), which replaces a whole line; quick add changes or adds
  to it, with completions for its last word beneath. Tests provide their own `TextEntry`.
- **`AppScaffold` shows the time** at the top of every screen, as Wear OS asks; each list's
  `ScreenScaffold` moves it aside as it scrolls. A row's primary actions come first in its
  custom actions and its long-press list.
- **Rows keep full size at the screen's edges** (`edgeScale = 1f`): scaled, a button falls
  under 48dp and fails the touch-target check.
- **A row's actions are TalkBack's custom actions and a long press**; the day's rows offer
  theirs when tapped too. A row is one description, its title first, as the phone's: as a state
  description the rest was said before the title. Announcements are a 1dp polite live region,
  never shown: drawn over the list, they hide the rows beneath, and the checks find a button
  partly visible. They are said in turn, each held long enough to be heard (`sayInTurn`), and
  every other one carries an unheard mark (`Said.description`), so the same sentence twice is a
  change TalkBack says.
- **A refused line is asked again with why**: the input screen's label is the app's refusal,
  then the question; backing out of the input screen gives the question up (`TextEntry`'s
  `cancelled`). A place merged or deleted takes its list to where the app went, as the phone's.
- **The watch's own link to its phone is preferred to Iroh while they are near**
  (`shared/WatchLink.kt`): the Wearable Data Layer's `ChannelClient`, a stream over Bluetooth
  or the local network. Only nodes Google reports `isNearby`: further off the Data Layer can
  route through Google's servers, and what syncs is the store. Each app's `LinkService`
  answers a channel the other opens, so neither need be open; each syncs over it on opening
  and two seconds after its store last changed. Iroh stays for the two apart and for every
  other device, and its round counts a device the link has just synced as synced
  (`Store::note_linked`).
- **The first link pairs the phone and the watch without words** (`lumenna_sync::introduce`,
  `Lumenna::sync_over_link`): the Data Layer joins only a phone and its paired watch, between
  apps with the same package and signing key. Both are enrolled, and the watch joins every
  device's list. Each side remembers, in `local_state`, that it introduced itself, so a watch
  unpaired since is not enrolled again: the link refuses on both sides.
- **A firing is shown once on the wrist.** The watch gives its own store's reminders, as the
  phone does (`shared/Reminders.kt`), and turns off bridging of the phone's reminder
  notifications only (`BridgingManager`, excluding `Reminders.BRIDGE_TAG`, which each carries).
  Not the phone's local-only flag: with that, a watch without this app would get no
  reminders at all. A dismissal on either reaches the other at the next sync, which takes
  its copy away.
- **A day or a time has the watch's own picker** (`wear/Pickers.kt`: Wear's `DatePicker` and
  `TimePicker` in a `Dialog`) beside its line in the block form and each time setting, the
  setting's name beneath its clock; Go to Day offers the line, then the calendar. While Wear's
  dialog shows, the screen behind it shrinks and the checks measure its buttons as under 48dp,
  so a test leaves them out for the presses inside it.
- **A row's custom action that opens a screen runs on the main thread in tests**
  (`actOnMain`): Wear's navigation refuses the test thread, where TalkBack never calls it.
- **The tile and the complication take the glance's timeline whole** (`Glance.kt`): a tile
  `TimelineEntry` and a `ComplicationDataTimeline` entry for each moment, so the system switches
  between them itself. The ranged value is the block's progress from its start and end instants,
  counted by the watch where dynamic values exist (Wear OS 4 and later).
- **New Task is the core's control on the watch too** (`Controls.kt`, `controls()`): a
  complication (`NewTaskComplication`: short text in the core's sentence with its icon, or the
  icon alone, named by the sentence; never asked for again, since it never changes) and an edge
  button beneath the tile's glance (`newTaskButton`), a TalkBack stop of its own named by the
  sentence, while a tap anywhere else still opens Today. Both open the app on `new-task`, which
  is quick add with the system's input screen straight away. The complication's pending intent
  has a request code of its own: an intent differing only in its extras is the same pending
  intent, and the Now complication's would open on New Task. Its picker name
  (`complication_new_task`) is checked by a test of the core. The timer is not on the watch.
- The watch says it runs `wearos` (`PLATFORM`), in pairing and over the link.
- **32-bit ARM too** (`armeabi-v7a`): many Wear OS watches run a 32-bit Android. The phone's
  APK leaves it out.
- Tests (`WearTest`) run on a Wear OS emulator with the accessibility checks, as the phone's;
  `APP=wear ./run-instrumented-tests.sh`. A fresh Wear emulator opens on its setup screen,
  which the script passes (`device_provisioned`, `user_setup_complete`); a watch counted as
  charging brings its charging screen over the tests, so the script unplugs it.

## CI

`android-build` builds once, checks 16 KB alignment and builds the core optimized
(`LUMENNA_CORE_PROFILE`), then a phone, a desktop-sized emulator and a Wear OS watch run
`run-instrumented-tests.sh` from that build, on Android 36: every Android 37 image crashes its
display server on the runners' software renderer. The test jobs need no Rust.
