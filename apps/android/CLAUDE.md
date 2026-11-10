# The Android apps

Read when working in `apps/android/`: the phone, Wear OS and the Orbit devices. The root `CLAUDE.md` holds what every part of Lumenna shares; this is what is easy to get wrong here.

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
  `AppWords.kt` (every fixed word drawn, `appWords`: the core's `shellWords` in sentence case as
  `thisDevice` calls itself, which the watch's Application makes `WATCH` before anything is
  worded; and what a tree control would say by itself and a Compose list does not: folding and
  the level), so no Kotlin writes a word, landing on a row (`Landing.kt`: `rememberLanding`, `RowFocus`, `RowKey`), a
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
- **An overrun's Replan** from its notification is the app's `DueEvent.ReplanFiring(key,
  occurrence)`, the firing the notification carries, so replanning dismisses the warning on every
  device though the app's own list of what is due, which Android's alarms do not fill, never held
  it (`AppModel.replan`). Never `ProposalEvent.Replan`.
- **The version, name and package are the workspace's** (`[workspace.metadata.lumenna]` in the
  root `Cargo.toml`), read by the root build script into `versionName`, a `versionCode` counting
  up with it, the `app_name` resource and the `applicationId` (`package_id`), for the phone and
  the watch: a version bump is one edit
  there. A test of the core checks the build scripts read them.
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
  against the row shown before, after folding; a list the app narrows or cuts out itself (a pick
  narrowed by typing, Browse's groups, the details' projects) takes the core's `levels(depths)`
  (`levelsOf`), so no Kotlin counts where a level changes.
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
  `ChooseTime`, named by the core's picker words, `appWords.picker`), in the block form, Go to
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
  function. It is in the smoke set, so a branch that adds an operation finds out there. The records answered (`functions/Records.kt`) carry each answer's announcement and
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
  `GenericDocument` each under a schema of `SearchDocument`'s fields (a task's with its
  `SearchTask`: project, labels, priority, states, done…), written again as the store
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

## The braille apps' shared Kotlin

`apps/android/braille/`: only what the Orbit app and the BrailleSense app really share; what the
three braille apps share beyond Kotlin is the core's (`lumenna_app`), and each device's code stays
in its own module. Line editors and menus were compared and left: Orbit's editor is built around
its T9 and its cell buffer over its own service, and its menus are described to its screen reader.

- **`ServiceLink`** is one bound vendor service, each call on one ordered worker, bound anew when
  Android's `onBindingDied` says so; `now` makes a call on the caller's thread, for giving the
  device up as the app leaves, which cannot wait behind what is queued.
- **`Claim`** holds the keys and display only while the activity is resumed and its window has
  focus (and, for Orbit, the service is up). Something drawn over the app takes only the window's
  focus, and whatever drives it needs the keys. `release` comes first on losing either, on the
  caller's thread, once per take, and carries on past a failure; `guardCrashes` releases on a crash.
- **`KeyDelivery`** takes a decoded key from the binder thread to the app's: taken only while
  held, and looked at again when it runs. A key not taken goes on to the device, since answering
  "taken" for one that nothing will act on leaves the person with a keyboard that does nothing.

## The BrailleSense app

`apps/android/braillesense/`: Kotlin over `shared` and the Crux app's Kotlin, drawn on the device's own speech and cells, for the HIMS BrailleSense 7 (Android 15). Package `com.extra.han.lumenna`, the one Android ID that is not `package_id`: the device's launcher lists `com.extra.han.*` in its Programs menu and its screen reader yields the braille keys to such a package by its name alone (a test in `records` says so). Arm64 only. No Compose in its screens: what is shown on the LCD is the line in front, for a sighted helper.

- **The keys and the display are the app's only while it is resumed and its window has focus** (`Claim`, in `:braille`, tested). Something drawn over it (the F1-F4 task list, the global options, an alert) takes only the window's focus, without a pause, and the keys have to go then too. Giving back comes first in `onPause`, is safe to repeat, and carries on past a failure; a chord that arrives late is answered "not taken", since "taken" leaves the person with a keyboard that does nothing. Nothing is said or put on the cells while away (a turn can still arrive then). The release is made on the caller's thread rather than queued, so nothing of ours lands after the next app has started to write.
- **`setActiveOutputService` is one switch for the whole device, never an app's claim.** The service keeps a single flag and every HIMS app's `HanDevice.isActiveDevice()` checks it before it speaks or writes a cell; the vendor's own code only turns it off for a moment inside one call. Turning it off on leaving silences the launcher and every other app, and leaves our last line on the display: what looked like keys doing nothing after Home. Never send `false`. The app sends `true` as it comes to the front, which also repairs a device left off. What an app shows is simply overwritten by the next app's first write.
- **Speech and cells are the app's own**, through `IHanOutputService` (`device/HanOutputClient.kt`, on `:braille`'s `ServiceLink`). `displayRawBraille` shows a window of cells. Speech is `displayAndPlayTTS` in mode 1 with an empty `braille` field, as measured on the device: mode 1 plays in the person's own voice settings and stays silent with the voice off, a new line cuts off the last, and cells written straight after do not stop it; mode 2 is taken (returns 0) and never sounds; a null `braille` is refused (-1). Modes 0 and 3 also sounded when tried, but only 1 has been listened to. No callback is needed.
- **The AIDL is in our own package with the vendor's descriptors** (`src/main/aidl/com/extra/han/lumenna/han`, `@Descriptor`): the device's framework holds its own `com.jawon.han.*` classes, which shadow a bundled copy of the same name (the boot class path wins) and are hidden-API blocked for an app, so a copy in the vendor's own package does not link. `HanOutput` is ours likewise: its parcel is written field for field, the class name never travels. The transaction order is the service's and must not change.
- **The cells take dots**: `displayBraille` and `HanOutput.braille` are Braille ASCII (BANA), letter for letter, so nothing the app sends in text reaches the person's table. Every line goes through liblouis (`device/Louis.kt`, `native/louis.c`, built by `build-louis.sh` with the NDK's clang, the release's tarball checked by hash; the tables in the APK's assets, copied to files once) into dot patterns in the person's table and grade, read from the device's settings provider (`device/BrailleSettings.kt`, `content://com.selvashc.settingsprovider/braille_settings`, in `<queries>`). The table for each `BrailleCode` is `BrailleTables`.
- **Panning is the app's** (`device/Cells.kt`, tested): windows break at spaces as the device's Word Wrap does. The display's width is measured once from the service: a long word wrapped and its line count read back (40 cells on the unit tried; the model H732B is sold as 32, so a real 32-cell one needs checking).
- **The braille keys are `han_brailleinput`'s callback** (`device/HanInputClient.kt`): `getSystemService("han_brailleinput")` returns a proxy, only usable as `IInterface` (its class is the framework's), so the binder goes through our stubs. Registered on resume, unregistered on pause. A press arrives as `onResponse(-1, keycode, "keycode meta chord")`, the chord in the third field; injected from the shell (`service call han_brailleinput 1 i32 <chord> i32 0 i32 0`) the chord is the value and the first field. Returning `true` keeps it from the window; a chord left as `false` arrives as a key event with no device and the chord as scan code, which `MainActivity.dispatchKeyEvent` swallows. `device/Chords.kt` decodes (tested): Space with dots is the manual's command, Enter with dots a hotkey, dots alone a cell.
- **Typing is the app's own, not the device's input method**: for a package that counts as self-voicing, the screen reader that turns chords into text for the input method steps aside, so a focused field gets raw key events (Braille ASCII). Lumenna reaches the device only through its AIDL interfaces; the decompiled SDK is a reference, never code it ships. The stock edit box lives inside HIMS's own apps, and no AIDL interface offers one, so Lumenna edits cells itself: `ui/LineEditor.kt` holds a line as the stock edit box does (`BrailleInfo.braillePara`): **in cells as typed, the caret a cell**, so Backspace and Delete take one cell, a routing key lands on the cell under it, and the line shows what was typed. It is read as text only when asked, a word at a time in the person's table, and a word not touched keeps the text it came with, so an edit never rewords the rest of a field through a translation there and back. Text already in a field starts as its translation. The QWERTY keyboard types text, so that word's cells follow its text.
- **Keyboard Echo** (`KeyboardEcho`: 0 off, 1 characters and words, 2 characters, 3 words; the stock edit box takes 4 as 1 and 5 as 2): a typed cell is said in 1 and 2, the finished word at Space in 1 and 3, "Space" in 2 or where Space ended no word. What Backspace takes is said whatever the setting, as in the stock edit box. Moving by a character says the cell at the caret, by a word the word. **A cell by itself is said as the stock edit box says it** (`HimsEditSoundFunc`'s table, now `lumenna_app::braille::braillesense::cell_echo`, `braillesenseCellEcho` over the FFI: the BrailleSense's convention alone, used by no other app, since a BTSpeak's echo is its own libraries' translation and the Orbit Speak edits contracted braille its own way), never back-translated alone: by its letters inside a word, never the whole word it stands for alone (dots 3-4 are "s t", even first in a word, never "still"; 2-3-4-6 "the"), and a cell with no letters by its dots (the capital sign "dot six", 4-6 "four six"). After a number sign a to j are digits until something else; in contracted braille a lower cell ending its word is its punctuation ("period"), uncontracted wherever it is; in computer braille a cell is its character, and a punctuation mark by itself is said by its name (`getChangeToMessage`'s table, `braillesense::character_echo`: "comma", "period", "back slash"). The words are the device's English ones.
- **Home, End, Top and Bottom say the word they land in**, in any box: Space-1-3 and Space-4-6 (Home, End) as the stock box's `onMoveTopOfCurrentLine` and `onMoveBottomOfCurrentLine` do (`getCurrentTextWord`), nothing where the cursor was there already. The stock box's Space-1-2-3 and Space-4-5-6 (`onMoveFirst`, `onMoveLast`, also Control-Home and Control-End) read the whole first or last paragraph instead; the user asked for the four to say one thing, so Top and Bottom say the word too. The stock Home and End go to the ends of the display line, not of the paragraph; Lumenna's go to the paragraph's.
- **Reading at the caret** is the stock edit box's (`BrailleRead.onBrailleKey`): Space-3-6 the character at the cursor (the last one at the line's end), Space-2-5 the word, Space-C the line (as Space-R). Outside a line, those two chords are what Space with their dots is.
- **The BrailleSense 7 has no routing keys**: its display is touch-sensitive, and a cell touched with either front Cursor button pressed routes there (again for each cell as the finger slides with the button held). It comes on `han_brailleinput`'s callback with the keys, as a gesture: code -7, the cell in the integer, counted from 1. With Gestures on and Double-Tap to Route Cursor (`GestureDoubleTapCursor`) set, a one-finger double tap, or one held, routes too: code -5, the kind in the integer (2, or 17 held) and the cell in the string. The id the SDK matches is `-100 × code + value` (`BrailleGestureType`: 700 smart cursor, 5xx taps and swipes). Lumenna takes only the routing; every other gesture is left to the device. Routing in an edit box only moves the cursor and says nothing, as the device's own does.
- **An edit box is a `ui/TextBox.kt`**: a `LineEditor` per line, the cells shown being the cursor's line. A single-line box takes a new line pasted as a space; a multi-line one (a `FieldKind::Lines` field, the task's and block's Notes) reads as the stock's multi-line box (`meb`, "multi line edit box", `ControlRole::MultiLineEditBox`). In it Enter starts a new line and says the word it finished where the echo says words, else "Enter"; Backspace at a line's start joins it to the line before and says "previous line"; Space-1 and Space-4 go up and down a line, the cursor as far along as the line allows, saying the line they land on ("blank" for none), and Space-1-2-3 and Space-4-5-6 to the top and bottom; Space-C says the cursor's line. Tab (F3, Space-4-5) and Shift-Tab leave it: the stock box hands Tab to the system's focus, so nothing traps the person there. Space-2-3 and Space-5-6 (paragraphs, which the stock box tells apart from wrapped lines) are not taken: a line here is a paragraph.
- **Selection and the clipboard are the stock box's** (`BrailleEdit.onBrailleKey`): Enter-B starts a selection at the cursor, running to wherever the cursor goes; Enter-A selects everything; Enter-C copies, Enter-X cuts, Enter-P adds to the clipboard, Enter-V pastes at the cursor, Enter-D clears the clipboard, Backspace-1-2 reads the selection, Enter-U its first line (a selection within one line whole, one across lines from its start to the end of that line as the display breaks it). Control with the letter on the QWERTY keyboard is the same, and Control-Insert and Control-Delete add to and clear the clipboard. The clipboard is the system's. With a selection, Backspace or Space-D asks "Delete selected text?" with Yes and No as prompt buttons, as the stock box's `PromptDialog` does (a question of the shell's own, `Shell.confirm`), then says "Delete complete.", or on No the line again; Escape leaves it ("Selection canceled."), and typing ends it. What each says is the stock's words (`BrailleWords::edit_box`). The stock box shows a selection by its cursor, not its extent: cursor type 2, blinking (`brailleCursorType`, `brailleCursorBlinkOn`), and type 0 still after; so does Lumenna (`Output.selecting`). In a box these Enter-letters are the box's, ahead of a dialog's buttons, which Space with the letter still presses.
- **Deleting is the stock box's** (`BrailleEdit.onBrailleKey`, the manual's "Delete current…"; on the QWERTY keyboard by the command summary's table): Space-D (Delete) takes the character at the cursor and says it as Backspace's is said, or at a line's end in a multi-line box the break, joining the next line on, silently, as the device's says nothing for it. Backspace-2-5 (Alt-Backspace) takes the word at the cursor and the spaces after it and says the word; a space between words goes by itself ("Space"; the device sends a bare space to its voice), and at the end of a line with another after it, the break ("new paragraph"). Backspace-1-4, C (Control-Backspace), takes the line the display shows at the cursor (`Speaker.cursorLine`: the window holding the cursor, less the label and role beside the text), a line left empty in a multi-line box going too. Backspace-1-3-4-6, X, and Backspace-2-3-5-6 (Alt-Delete) take the cursor's whole line: the manual and the command summary's general section give the first, the stock code and the summary's word processor the second, so both are taken. The last two say "Delete complete." Any of the three ends a selection, as the device's do; nothing to take is silence, where the device beeps.
- **A new braille table rewrites every box** (the G chord, or Global Options while away): before the settings are read again each box's words are read as text in the table they were typed in (`TextBox.settle`), and after, if the table changed, written again in the new one (`recell`), as the device's own boxes follow it.
- **The QWERTY keyboard is ordinary key events** (`device/Qwerty.kt`), its keys by the manual's table (Alt is the menus, Control Enter's hotkeys, Control-Shift Space's commands, the Application key a row's menu).
- **How the device reads a control** (found in its SDK and built-in apps; the words are the stock strings, English, and what is composed is `lumenna_app::braille::device_reading`, tested there, as the words and order are the core's):
  - Each control has a role, a word for the voice and a symbol for the cells: Menu `mn` (a submenu: the voice says "Menu"), menu item `mi`, list item `li`, edit box `eb` (also multi line edit box `meb`, computer edit box `ce`, edit combo box `ecb`), combo box `cb`, check box checked `schb`, check box unchecked `uchb`, radio button `rb`, button `bt`, prompt button `pb`, static box `st`, link `ln`. Symbols are shown as computer braille, cell for cell, never in the person's contractions.
  - Where the role goes is the person's: **Control Information** (`ControlInformation`, for the cells) and **Voice Control Information** (`VoiceControlInformation`), each 0 off, 1 before, 2 after, default 2. Read with the rest of `BrailleSettings`. The unit tried has 1 for the cells and 2 for the voice: `mi New (n)`, said "New (n) menu item".
  - A menu item's order: name, shortcut letter in brackets, hot key, then the sign of what it opens. Spoken, a submenu adds "pulldown" and an item that opens a dialog "dialog", ahead of the role: "File (f) pulldown Menu", "Rename (r) dialog menu item". On the cells a submenu ends in ` 3o` (dots 2-5, 1-3-5, and `:o` in some codes) and a dialog in `"""` (three dot-5 cells), both Braille ASCII, the role's symbol where the setting puts it: `mn File (f) 3o`, `mi Rename (r) """`. The brackets are the code's: UEB `"<` and `">`, US `7` and `7`, grade 0 and the rest `(` and `)`. **Announce Shortcut Keys** (0 off, 1 on, 2 the cells alone) and **Hot Key Information** (0 off, 1 letters, 2 dot patterns, 3 the cells alone; 0 on the unit tried) say what is shown.
  - In a dialog: the title is said with "dialog" as it opens ("Find dialog"); a control's text is its label, then its value, then "2/5" when **Index Reporting** is on and it is one of several; an edit box's label is shown too (`eb Title: Buy milk`). Space and Backspace change a combo box, a check box or a prompt button. **Tab (Space-4-5, F3) and Shift-Tab (Space-1-2, Space-F3) are all that move between controls**: the dialog swallows the arrow keys, so Space-1 and Space-4 move only within a control that is itself a list. Backspace or Escape (Space-E, F4) in a submenu goes up a level and at the top closes it, saying Cancel; the menu key (Space-M, F2) closes it all.
  - Nothing in the SDK, the manual or the built-in apps folds a row, so Lumenna's are Space-3 (Collapse) and Space-6 (Expand), which only an edit box uses otherwise (a character left and right); they run the row's action.
  - **The four keys long read as scroll keys are the function keys**: F1 to F4 are `0x40000`, `0x80000`, `0x100000`, `0x200000` in the chord, with Space held for Space-F3 (Shift-Tab). F2 is the menu, F3 Tab, F4 Escape, F1 with F2 Page Up, F3 with F4 Page Down; F1, F1-F4, F2-F3 and either with a letter are the system's and are not taken. The real scroll keys are `0x400000` (up) and `0x800000` (down), holding them `0x1000000` and `0x2000000`. Routing keys, on models that have them, are `272` to `303`, or `352` on.
  - A row of a list says its number on every move, on the cells and in the voice, after its text and ahead of its role (`Buy milk 3/12 list item`), while **Index Reporting** is on: the device's own lists append it the same way.
  - **Global Options are one set of values for the whole device, and nothing applies them for an app.** They live in `content://com.selvashc.settingsprovider/braille_settings` as name and value. The stock widgets (`HanMenuListView`, `HanListView`, `HanEditText`, `HanSpinner`, `HanButton`, `HanCheckBox`) read them through `HanOption`, cached by `HanApplication` and kept current by a content observer on that URI, so an app built from them follows the settings for nothing. An app that draws its own cells has to read and apply each one: `BrailleSettings` does, on coming to the front and on the provider's change notices (some have a hot key that works while the app is in front). They mean, by the values the options list stores:
    - `ControlInformation`: 0 off, 1 before, 2 after. Only models for France, Italy, Denmark and Japan offer After (the `.lang` file in `/mnt/selvas`); elsewhere the list is Off and Before, and a stored or default 2 reads as Before, as the stock controls treat it.
    - `VoiceControlInformation`: 0 off, 1 before, 2 after, on every model. Default 2.
    - `AnnounceShortcutKeys`: 0 off, 1 on, 2 braille only (the voice drops the letter and the cells keep it). Default 1.
    - `HotKeyInformation`: 0 off, 1 letters, 2 dot patterns, 3 braille only (default 1). Spoken, 2 gives a letter's hot key as its dots ("Enter-dots 1-3-4-5", "Space-dot 1"). On the cells a hot key is the short form with its letter in 1, 2 and 3 alike (`ent-n`, `sp-x`), as the SDK's `buildHotKeyBrl` does.
    - `SupportShortCut` ("Automatically Open Menus with Shortcut"): 1 a letter opens or runs its item, 0 it only goes to it. Default 1. Of the stock code only the launcher's home menu reads it; the SDK's menus always run. Lumenna applies it to its menus and its list of places.
    - `IndexReporting`: 1 on, 0 off. Default 1. Each list's adapter reads it itself.
  - **One menu per screen, and the key for it is Space-M or F2.** In File Manager, Email and the Media Player that menu is a menu bar (top-level groups, each a submenu); in the To-Do List, the Address Manager and the launcher it is a flat list of commands. Of those six, read in full, none uses both and none has a context menu: no `onCreateContextMenu`, and a row's actions are items of that one menu acting on the focused row, with hot keys. The menu's contents change with the screen, so File Manager has one for a folder's list, one for Favorites, one for a disk. The only separate "item's actions" keys the manual names are on the Android side: Enter-I for an app's Actions Menu in the App Manager, and the QWERTY keyboard's Application key (FN-Slash), "Application / Context Menu". In a native app Enter-I is that app's own hot key, usually Insert or Information.
- **Menus are the device's** (`ui/Menu.kt`), read as above; Space-1 and Space-4 move and wrap, Space-1-2-3 and Space-4-5-6 go to the ends, Enter opens or runs, an item's letter jumps to it and runs it. **Space-M or F2 is the shared menu bar** (`keys::MENUS` through `braille::menu_bar`, the one every other platform draws), each name and whether it applies from `ViewModel::commands`, each letter the core's (`GROUP_LETTERS` for the menus, `menu_shortcuts` for what no letter is given); what does not apply is left out, as the device's own menus hide theirs. What a device with no sidebar needs is added there: Projects, Labels, Saved Filters and Reminders Due under View. **Enter-I and the Application key are the row's actions** (`offered`, with `action_letter`), the same source as every other context menu. A hot key is shown only where its letter really runs from the screen under the menu (`LetteredItem::hot_key`). The list of places shows the main menu's letters and a letter alone runs them, as in a device's menu; what has no letter moves by first letter. **Enter on a heading opens that heading's own list** (`Page.Group`, as BTSpeak's screens), with the core's words when it is empty. A press that leaves nothing done says why, in `BrailleWords`' words.
- **Every screen draws the app's view** (`ui/Shell.kt`), as `Frame`s rebuilt each turn: `ListFrame`, `MenuFrame`, `LineFrame`, `FormFrame`. The row in hand is kept per frame, by key; a frame is said when it is new or the app moved what is in hand (`landing`), and only put on the cells otherwise, so a sync or a minute never cuts off reading.
- **The letters are the core's** (`braille_letter`, `ShellEvent.Letter`, `action_letter` and its kin); how one is pressed is this app's (`BrailleLetters.press`). Space with the letter is its command, except where Space with those dots is already something on every screen: A, B, K and L (Up, Shift-Tab, the line's start, the first row) and C, D, E, F, H, M, O, R and Z (the device's Repeat, the manual's Delete, Escape, Find, Help, Menu, Options, Repeat and Exit), which Enter with the letter reaches, as the device's own hotkeys are. A menu shows each item's core letter and the press for it. `,`, `.` and `/` have no dots: Move Up, Move Down and the filter are the row's menu and Space-F.
- **A letter alone moves by first letter**: `ShellEvent.FindLetter` in a list the app holds as its main one (its `landing` rises, so the row is said), the core's `first_letter` in a list drawn here (a side list, a pick, a menu's own list); none found is said in the core's words.
- **What is said comes from the core's description** (`Turn.described`, handed to `AppModel.described` on the main thread before the view moves): a question's prompt, buttons, choices and what was typed are its; a row arrived at or read again (Space-R) says its place ("3 of 12"), a person's own move does not, so moving stays quick. A refusal no sentence said is spoken from `changes`.
- **What changes say goes on the cells as well as in speech**, for the device's Message Display Time, then the line returns: someone reading braille alone hears none of it. Any key clears it.
- **A side list shown tells the app so at once** (`Event::Hold`, in `Shell.refresh`), or a letter pressed on it would run on the row in hand in the main list, which the person cannot see.
- **Tests**: `./gradlew :braillesense:testDebugUnitTest` covers the chords, panning, menus and the line editor without a device. On the device, `adb shell service call han_brailleinput 1 i32 <chord> i32 0 i32 0` injects a chord from the shell, and in a debug build the service's calls are logged (`LumennaOutput`).

## The Orbit app

`apps/android/orbit/`: the Orbit Speak and the Orbit Player (LAVA LZG403_25I and LZG403_25, Android
11, arm64), over `shared` and the Crux app's Kotlin. Not Compose: one `TextView` for a sighted
helper, while Orbit's screen reader voices the screens the app describes. Sideloaded, or from
GitHub's releases; installing it needs nothing else but Satellite (its hook and launcher) on the
device. Its identifier is the phone's `package_id` (never on the same device): not `com.orbit.`, and
holding no "inputmethod", which Orbit's routing would misread. `check-apk.sh`, run by CI and
`release.yml`, checks what Satellite reads of an APK.

- **Everything Orbit is reached through is the clean-room AIDL** (`src/main/aidl`), the screen
  reader's in its order and never #44 (`canSpeakLanguage`), which the Player lacks: it stops before
  it. Each service is a `ServiceLink` and the keys a `Claim`, from `:braille`.
- **`OrbitReader` describes, and says only what the app says**: a list is `onFocusChanged` of a list
  box, a move `onItemChanged`, a question `onDialogOpened` then its first answer, a row's actions
  `onContextMenuOpened`, a line an edit box with `onEditBoxContentChanged`. Nothing is sent before
  `onScreenReaderReady`, which Orbit calls at once when it is ready already; speech sent earlier is
  dropped. `speakText` jumps Orbit's event queue, so what follows a description (a hint) waits a
  moment. What is described is `describe` (`Describe.kt`), pure, tested on the JVM: a turn that only
  redrew says nothing, and a row reworded by what the person just did is read again. What is shown
  and what changed are the core's (`Turn::described`, handed over by `AppModel.described`, a
  callback, since a flow keeps only the latest and a refusal must not be lost): the list's name and
  `readback`, a move only when the core's `Moved` says so, a question's `prompt`, `buttons` and
  `choices`, and refusals (`Refused`: its sentence said only when `said` is false). This app's own
  lists (the main menu, settings, the forms' fields, a menu) go through `describe` beside them.
- **Screens** (`Screens.kt`, `Forms.kt`, `Dialogs.kt`): a stack the person moves, over the app's view. The screens
  the app itself opens (a question, the block form, a proposal, a reminder list, the quick-add line)
  are pushed and popped from the view each turn (`Ui.follow`), as BTSpeak's `follow`. A screen
  describes itself only once `loaded`, so nothing says a list is empty on the way to it. The main
  menu is the core's (`braille_main_menu`), and a day's heading is a row of it, as the core counts.
- **A turn is taken from `AppModel.described`**, once its view and lines are in place: a collector
  of the view on `Main.immediate` runs between the view being set and the lines, with the last
  turn's lines.
- **Keys**: `KeyDecoder` turns Orbit's bundle into a `Key`. On the Speak, as its user guide has them: Space with dots 1 and 4 are Up and Down (Space with dots 3 and 6 are the screen reader's, reviewing the item's text, except in an edit box, where they move the caret), Space with dots 4 5 and dots 1 2 Tab and Shift-Tab, Space with L and dots 4 5 6 the top and bottom, Space with E or Z back whatever the row, Space with M only the menu, dot 7 back and dot 8 open. Left goes back out of a list, never out of a dialog, on the Player too, whose Back leaves it. Which key presses a letter is the core's (`speak_letter`): Space with it, or Dot 8 (Enter) with it where the device keeps Space with those dots while Lumenna is in front: C and T the screen reader's; A, B, E, L, M and Z on every screen. What Orbit's own apps do with Space and a letter (the Editor's D, K and S, the Recorder's and Media Player's) is theirs alone. G is the screen reader's with both keys, so what it names is a menu away. Space with a letter that takes Dot 8 says so (`speak_press_instead`). A letter alone is a first letter (`FindLetter` on the app's main list, `first_letter` on this app's own). On the Player digits open rows, holding star finds by letter (T9), Menu the row's menu, Back goes back. A letter is the core's: `brailleLetter(dots)` (a to z, none with dot 7 or 8) then `ShellEvent.Letter`. D is the delete key's, which the core's table does not have. Letters a row or screen shows on the Speak are `action_letter` and its kin, with their press (`speak_press`) said after the name; `,`, `.` and `/` have no dots, so Move Up, Move Down and the filter are the menu's.
- **Dialogs on the Speak are Orbit's** (`Dialogs.kt`: `DialogScreen` and its `Control`s): a question, the task and block forms, a settings page, a typed line. Tab and Shift-Tab move between controls, each active as soon as it is reached, as in the Editor's Save As: a line types at once, Up and Down change a combo box or check box, Select presses a button or confirms the dialog. Space with E or Z cancels. Each is told to Orbit by its role. What is typed goes to the form on leaving the control. Editing dialogs end with their Confirm and Cancel, in the core's names: a question's `buttons`, the block form's `yes`, the task form's `details.save`, quick add's `add`, and `cancel`; the task form's Cancel is `DetailsEvent::Discard`. A setting is changed as it is changed, as on every platform, so a settings page has no Confirm. The Player has no Tab: the same `Control`s are its rows, a line or a choice opening its own screen.
- **Typing** (`LineEditor`): the Speak's cell buffer through Orbit's liblouis service, one word at a
  time, mode 388 for each cell's echo and 132 at Space; the Player's multi-tap T9, ported from
  Satellite's `T9Input`. The Speak's echo is its own fields', in `OrbitWords` and the Orbit's cell
  names (`braille::orbit_cell_name`: the capital sign, the capital word sign, the number sign, the
  lower cells; never the BrailleSense's `cell_echo`), except that dots 3 6 are "hyphen", where the
  device's table says "greek sign". In contracted braille a named cell says its
  name, else the last character its word now makes; otherwise the character, else its name; else
  "dot" and its numbers. Dot 7 with a selection deletes it unasked, as the Editor's does, and
  tells the reader `selection_deleted`, which says so in its own words. Otherwise Dot 7 says what it took: a cell of the word not yet committed by its name,
  what it alone makes, or its dots; a committed character by itself ("space", "new line"); "blank"
  at the start. At Space the word's cells and a blank cell are translated together, as Orbit's
  fields commit a word, and the word is said in print. The screen reader takes no word from an app
  (`onCharacterTyped`'s `resolvedWord` is ignored): it echoes a word made of the characters it was
  told, which for a contracted word are its cells' echoes. So a cell's character goes as its
  `characterName` with the character empty, which the reader says by the person's character echo
  but keeps out of its word, and the word is said by this app when the reader's typing echo
  (`typing_echo` in its own settings, read through root at `/proc/1/root`, since `su` in an app
  sees only the app's own /data) says words. The space after a word said is not echoed, since the
  reader stops speech for every echo. The Editor's selection and clipboard are every line's:
  dots 1 2 8 start and end a selection (to the character at the caret), dots 1 8 select all,
  dots 1 4 8 copy, 1 3 4 6 8 cut, 1 2 3 6 8 paste, over the system clipboard, each with Space too
  in computer braille, where dot 8 with a cell types it; Space with dots 1 2 7 reads the
  selection. Its words are the Editor's, in `OrbitWords`. Moves go to Orbit as
  edit-box changes, so it says them by the person's own settings. A field of several lines (a
  form's `FieldKind::Lines`, the task's notes) is an edit box Orbit calls multiline: dot 8 starts a
  line, Up and Down move by line (`line_navigation`, Orbit saying the line), Space with dots 1 3
  and 4 6 go to the line's ends, Tab leaves it, as in Orbit's Editor. The table comes from Orbit's
  settings through root (`Liblouis.readTable`). The screen reader's G chord changes it there and
  tells no app but Orbit's own: watching the settings provider needs Orbit's signature, and the
  screen reader's settings callback is only for its own settings. The settings app hands the new
  table to the liblouis service, though, so `Liblouis.follow` asks the service which table it
  holds (its transaction 5, called by number) as a line is reached and as the app comes back, and
  a change there reads the table again; a new table redraws what is shown.
- **Back throws a form away, never saves it**, on both devices: the task form's Back is its Cancel
  (`DetailsEvent::Discard`), and it saves only by its Save.
- **Trying it**: nothing can press Orbit's keypad over adb, so a debug build takes keys over a
  broadcast (`DEBUG_KEY`, `am broadcast --es name DOWN --ei action 0 --ez long false --ez phone
  false`). What Orbit said is in its log (`setprop log.tag D`, then `olympus_SpeechManager`'s
  `sequence items`). Wake a sleeping device first (`input keyevent KEYCODE_WAKEUP`), or the window
  has no focus and nothing is claimed. The real keys, by hand, are still to be tried.

## CI

`android-build` builds once, checks 16 KB alignment (the BrailleSense APK too, its only check here beside the JVM tests: no runner has its services) and builds the core optimized
(`LUMENNA_CORE_PROFILE`), then a phone, a desktop-sized emulator and a Wear OS watch run
`run-instrumented-tests.sh` from that build, on Android 36: every Android 37 image crashes its
display server on the runners' software renderer. The test jobs need no Rust.
