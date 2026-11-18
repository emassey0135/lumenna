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
  description, settings in order, caption, buttons and lines to read. Which pages are listed is
  the app's (`SettingsView.pages`, only those with something on them here), drawn whole, from
  what each app says it can do as it starts (`ShellEvent.Abilities`, sent from `LumennaApp`'s
  and `WearApp`'s root beside `PlanFrom`): the phone saves and picks files, keeps backups on a
  schedule in its own storage (`backUpIfDue`) and has no About of its own; the watch the same
  but no files, so the core lists it no Export and Import and no Restore. Neither has an About of
  its own, so **About is a settings page**, drawn from the page's `lines` (`SettingsPage::lines`,
  the core's `about_lines`), a line of text each after its settings: on the phone a line holding
  an address is a link, the whole line pressed opening it in the browser, the address found by
  where `https://` starts, never by the words before it. `SettingsPages.shown` is the
  BrailleSense's until it takes the view's. Where they are Android's own (whether notifications
  and exact alarms are allowed), the words are `notification_words`.
- **A place's own actions are the core's** (`ViewModel.screenActions`, `ScreenButtons`,
  `ScreenActions.kt`): Today's, Blocks' and a task list's top bar draws them in the core's order,
  named by its titles and disabled while not `enabled`, as icons while they fit (`iconRoom`, by
  the pane's width) and the rest in the bar's overflow menu (named `appWords.more`), where one
  that `asks` ends in "…". The icon for each `id` is Android's look (`iconOf`); an `id` with none
  goes to the menu. Each is `ShellEvent.Key(id)`, but what is the shell's (`shell`): the filter is
  the field above the list, which the action focuses, and every task list has it, as the core
  offers it on each, the trash's included. The screens follow what the app opens: quick add as its
  line opens (`FollowLine`, only as it opens, so a line closing as the person comes back does not
  reopen it), the block form and the question as before. A screen that is no place (the details,
  a form, settings, Browse) has Undo and Redo in its bar, Android's own, since the core lists
  screen actions for places alone. Home (`ShellEvent.Home`) is never shown: the tabs and the
  sidebar are the places. Today's What should I work on? is also one of the day's own actions,
  so it shows twice there, as the core lists it. Tests press a bar's action with `pressInBar`,
  which opens the overflow menu where the action is not an icon.
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
  narrowed by typing, the details' projects) takes the core's `levels(depths)` (`levelsOf`), so
  no Kotlin counts where a level changes. A group's screen has its levels from the core already.
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
  (`DetailsEvent.Key`); the day's moves, New Block, Undo and Redo are `ShellEvent.Key` too, as
  their buttons are, and a block's Enter is `open`. Space is not bound: on a row it is a click. Compose's root takes the keys while
  anything has focus; `MainActivity.dispatchKeyEvent` takes them when nothing does.
- **Erasing for good is a menu away, never a key's**: a trashed task's Erase Permanently is
  in its long-press menu and custom actions with the rest, and Delete, the core's key, runs
  Delete from Trash there. The store's own actions (`SettingsView.offered`, Erase Everything
  Deleted) are destructive buttons on Export and Import, the watch's under Backups, with
  `offeredCaption` beneath, sent as `ActOn(ActingOn.Settings, …)`; the app's question is asked
  at the root.
- **Buttons take focus only out of touch mode**, which a key press leaves; the keyboard
  tests leave it first (`setInTouchMode(false)`), or no tab could be focused.
- **The tabs, Browse and the group screens are the core's** (`place_arrangement`, shared's
  `arrangement(sidebar)`, from the view's own sidebar): the tabs in order with their titles
  (`Arrangement.tabs`; the icon and the stack's first screen are Android's, by `TabKind`), Browse's
  rows (`browse`: every place that is no tab, opened inside Browse, and each group with how many
  places it holds, opening `Screen.Group`), and each group's screen (`groups`: titled by its
  heading, whose `offered` New is the add button, its places a level shallower and folded as the
  app has them, its `empty` said with none). No screen here picks places out of the sidebar by
  kind, counts them or titles them.
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
- **A place's own actions are the core's** (`ViewModel.screenActions`, `screenActionItems`): a
  button each, in its order, named by its title, disabled while not `enabled`, sent as
  `ShellEvent.Key(id)`; no "…" on a watch's button. A task list's and Blocks' come first, under
  the heading, a task list's filter being the shell's line in its place; the day's come after its
  rows and its own actions, so the day is first. The app opening quick add's line opens its screen
  (`Follow`). The list of places is no place: it is the core's root list (`Arrangement.watch`,
  `place_arrangement` with `WatchKind.SYNCS_ITSELF`, Home as a watch that syncs itself has it): New
  task, every place as the sidebar has it, then Undo, Redo, What should I work on?, Sync now and
  Settings, each action `ShellEvent.Key(id)` but Settings, which opens the watch's own screen (it
  asks the app for its settings). What should I work on? takes the app to today and asks its pick,
  drawn over the list by `WearQuestions` as the day's own; Sync now's sentences are said as every
  announcement is. A heading's New follows it while it is unfolded, from its `offered`. No screen
  here picks places out of the sidebar by kind or titles them.
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

`apps/android/braille/`: only what the Orbit app and the BrailleSense app really share, which is the
dialog controller over the core's forms. What the three braille apps share beyond Kotlin is the
core's (`lumenna_app`), and what is a device's own is its SDK's: holding the keys while in front
(`Claim`, `KeyDelivery`), its services (`ServiceLink`), and its lists', menus', edit boxes' and
dialogs' keys. Each SDK keeps its own copies of `ServiceLink`, `Claim` and `KeyDelivery`, since each
stands alone. Line editors and menus were compared and left apart: Orbit's editor is built around
its T9 and its cell buffer over its own service, and its menus are described to its screen reader.

- **`DialogController`** works a form the core describes (`DialogShown`: a question, the task form,
  the block form, quick add, a settings page) as a braille device works one, with no keys and no
  words: each device maps its keys onto it and says the control in hand in its own roles. It holds
  the control in hand (`index`, `control`); `next` and `previous` go round with Tab and Shift-Tab and
  `moveTo` reaches one directly, each taking the line left (`commit`, a `Set` only if it changed);
  `type(text, cursor)` sends a line as it is typed, `step(by)`, `choose(option)` and `find(letter)` a
  choice (sent as it changes) or a list (held until its button), `toggle` a check; `press` the button
  in hand or a line that submits (quick add's, a question's), a button sending what a line or list
  holds being pressed with that control's value; `pressDefault`, `pressLetter` and `cancel` (the
  dialog's `cancel` events). Everything goes through the core's `control_event`; a
  `ControlSends.Shell` button is the device's (`shell`). What is typed, chosen or toggled is held
  until the core's next description has it (`update`), so a quick second press goes on from the
  first, and a value the core changed underneath (a completion chosen) wins; another dialog (a new
  `id`) starts afresh on its `focus`. `text`, `selected`, `checked` and `value` say what a control
  holds now. Tested on the JVM (`DialogControllerTest`) with the core's records built by hand and
  `control_event` given as a function, since the native library is absent there.

Both SDKs' `ServiceLink`, `Claim` and `KeyDelivery` are the same:

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

**Each SDK has a README** (`orbit-sdk/README.md`, `braillesense-sdk/README.md`) for an app other
than Lumenna: what a device needs, a first activity, and the API. A change to an SDK's public API
changes its README too.

**Each SDK is a git submodule, a repository of its own** (github.com/emassey0135/orbit-sdk and
braillesense-sdk, Apache-2.0, each with its own CI), checked out at a commit Lumenna names. A change
to one is made in the submodule's checkout, on a branch of that repository, pushed and merged there
(its CI runs its tests and builds its sample); Lumenna then moves to that commit (`git add
apps/android/orbit-sdk`) in a commit of its own. A change that needs both at once goes to the SDK
first. Lumenna's CI checks out the submodules for the Android jobs (`submodules: true`) and still
runs both SDKs' JVM tests. A checkout made before the move needs `git submodule update --init`.

## The BrailleSense SDK

`apps/android/braillesense-sdk/` (`:braillesense-sdk`, package `dev.emassey0135.braillesense`): an
unofficial SDK for the HIMS BrailleSense 7, Apache-2.0, depending on nothing of Lumenna's so other
apps can use it, and a Gradle build of its own as well as Lumenna's module (its root the library,
`sample/` its README's quick start). **Read its own notes before changing it**
(`braillesense-sdk/CLAUDE.md`): its API, explicit API mode with KDoc on everything public, and
everything found on the device. Its README is for an app that uses it, and `docs/DEVICE.md` the
device for a developer without one. Its screen stack, `BrailleSenseUi`, has the Orbit SDK's
`OrbitUi`'s common API (`list`, the asks, `refresh`, `land`, `announce`, `refused`, `batch`,
`launch`, `Item`, `Action`, `MenuItem`, `Field`, `FormValues`, the common `UiWords`): a change to
one is a change to both. What the device says is in its own words there, not in the core (the
root's exception for a device SDK's words). Tests: `./gradlew :braillesense-sdk:testDebugUnitTest`,
on the JVM.

## The BrailleSense app

`apps/android/braillesense/`: Kotlin over `shared`, the Crux app's Kotlin and the BrailleSense SDK, drawn on the device's own speech and cells, for the HIMS BrailleSense 7 (Android 15). Package `com.extra.han.lumenna`, the one Android ID that is not `package_id`: the device's launcher lists `com.extra.han.*` in its Programs menu and its screen reader yields the braille keys to such a package by its name alone (a test in `records` says so). Arm64 only. No Compose in its screens: what is shown on the LCD is the line in front, for a sighted helper.

- **What the device does is the SDK's, written down there**: holding the keys and display only while in front, never sending `setActiveOutputService(false)`, the AIDL under the vendor's descriptors, the cells taking Braille ASCII and liblouis in the person's table, panning, the keys and the touch cursor, Global Options and what each means, how a control is read, one menu per screen, and the stock edit box (cells as typed, Keyboard Echo, moving, selection, deleting, a new table) are in `braillesense-sdk/CLAUDE.md` and `docs/DEVICE.md`. Lumenna applies `SupportShortCut` to its menus and Home, and its Home, End, Top and Bottom say the word they land in (the user asked for the four to say one thing).
- **The app opens on Home** (`ShellEvent.Home`, `ViewModel::home`, `HomeView`), the core's screen for a device without a sidebar: the places, folded as the sidebar is, each with its letter (`HomePlace`), then Home's own actions as rows of their own (Reminders Due while any are due, New Task, Undo, Redo, What Should I Work On?, Sync Now, Settings, Key Help, About), each with its letter. Its row in hand is the sidebar's. Enter is `Key("open")`: a place opens, a heading folds; Space-3 and Space-6 fold too. A letter, with Space or Enter or alone, opens its place or runs its action (only goes to it, where Automatically Open Menus with Shortcut is off), a place's own action letters first; alone it otherwise moves by first letter. **The shell follows the app off Home** (`Shell.follow`): once Home is shown, a place it opens (a letter, Enter, What Should I Work On?) is pushed as the list shown, and going back from a place sends `Home` again. Projects, Labels and Saved Filters from the menu bar are Home, on that heading, unfolded. Escape on Home closes the app, as Exit does: Home is the bottom of the stack.
- **Menus are the device's** (the SDK's `MenuFrame`, read as the device reads a menu); Space-1 and Space-4 move and wrap, Space-1-2-3 and Space-4-5-6 go to the ends, Enter opens or runs, an item's letter jumps to it and runs it, and **a hot key an item shows runs it while the menu is open too**, from any level. **Space-M or F2 is the core's braille menu bar** (`braille_menu_bar`), drawn as it comes: first the screen's own menu (`braille::SCREEN_MENU`, named for the screen, "Tasks", "Today", a project's name, "Home", where the bar opens), holding the screen's actions with their letters as hot keys, then the shared `keys::MENUS` with letters, which hold every one of the screen's own actions too (`ViewModel::screen_actions`; Home's on Home), each with its letter as a real hot key; each name and whether it applies from `ViewModel::commands` and `screen_actions`, so what does not apply is left out, as the device's own menus hide theirs. A screen action the shell runs itself (`ScreenAction::shell`: the filter, Key Help, About, Reminders Due) is the shell's; Settings and the pages the menu opens are the shell's pages; every other command is `Key(id)`, sync and back-up included, from the menu, Home, a settings page's button and the devices page alike. **Enter-I and the Application key are the row's actions** (`offered`, with `action_letter`), the same source as every other context menu.
- **Enter on a row is `Key("open")`**, on every list the app holds (a task's details through the app's form question, a block's form, a reminder, a day's row; on the day's heading it is refused in the core's words, its actions being Enter-I and the Day menu), never a choice made here. A reminder due is the exception: its buttons are its actions, which Enter shows as Enter-I does.
- **Reminders due** look back over the last day as their list opens (`DueEvent::LookBack`), and a button is handed back by its id (`DueEvent::Press`); the page leaves once the last is dealt with, and says so when there are none.
- **Settings pages are the core's, unfiltered** (`SettingsView::pages`: only those with something on them here).
- **Every form and dialog is the core's** (`DialogShown`, `lumenna_app::dialog`): a question, the block form and quick add from `ScreenView::dialog`, the task form from `AppModel.taskForm` (`App::task_form_on`) and a settings page from `AppModel.settingsPage` (`App::settings_page_on`). `Shell.dialog` draws it as a `FormFrame` over the braille apps' shared `DialogController` (`:braille`), kept while the dialog stays open, which holds the control in hand, the tab order, what is typed or chosen until the core has it, and every press, through `control_event`; the frame maps the device's keys onto it and reads each `ControlType` in the device's role: Line an edit box and Lines a multi-line one (the SDK's `TextBox`, kept by the dialog's id), Choice a combo box, Check a check box, List a list in place (`Control.Pick`: Space-1 and Space-4, a first letter, Enter picks), Button a button, and a question of buttons alone prompt buttons that Space and Backspace step between, said as its prompt rather than "dialog"; Text a static box, those ahead of the focus said as it opens. A line's text is the SDK's box, handed to the controller as it changes (and before Tab or a press, so a word still in cells is not lost); the controller takes it as Tab leaves the line, only where it changed. A dialog that held only lines to read, its controls still arriving, starts again on the core's focus once it has more, and settings already open are not opened again for a page. A line the core changed underneath (a completion chosen) starts its box again from the core's. The devices page and the Reminders page's lines about Android's own permissions are built here as the core's records (`ControlShown`, `DialogShown`), so they are worked as every other dialog is. Enter in a line presses the dialog's default; Escape sends the dialog's `cancel`. What a button leaves to the device (`ControlSends.Shell`): import and restore-backup pick a file, back-up is `Key("back-up")`, pair opens Devices, and a folder setting (`backup-dir`) picks a folder and sets it. **Files and folders are chosen in the device's own File Manager** (the SDK's `FilePicker`, `MainActivity`), which reads itself as every screen of the device does, and exports are saved there too; Android's picker is used only where the File Manager cannot be started (whether its picker is open to another app is still to be tried on a device). It answers with a path alone, so reading a file Lumenna did not make needs all-files access. A task's Edit (`Question::Form`) asks the details to show that task, then is the task form, its Save and Cancel closing the question too. The box's "Delete selected text?" is the stack's prompt.
- **A dialog's buttons have letters**, pressed with Space or Enter: the core's where it gives one (a task's actions, a settings page's buttons), else the first free one of its name (`menu_shortcuts`), so Save has one beside the task's own. Leaving the task's form with Escape throws away what was not saved (`DetailsEvent::Discard`), as its Cancel does elsewhere.
- **The filter** (Space-F, Control-F, or the screen's Filter) is a line over the list shown, and only there: anywhere else it says there is no list to filter. Enter keeps the filter and goes back to the list; Escape leaves it cleared, still on the list.
- **A press that does nothing says why**, in `BrailleWords`' words: Tab in a list, a row's actions asked for on a dialog or a menu, a letter with nothing on it.
- **Every screen is a screen on the SDK's stack** (`ui/Shell.kt` over `BrailleSenseUi`), which takes the keys, the menus, the edit boxes, panning, routing, messages and when to speak; the shell follows the core's view. Each page the person goes to is a `PageScreen` (Home at the bottom, a place, a task's form, Settings and a settings page, Devices, Reminders Due, Key Help) whose `back` and `closed` put away what it held (Discard, Settings Close, pairing cancelled, `Home` sent again on reaching Home); what the app opens itself (`ScreenView::dialog`, a form asked for, a proposal, a reminder list) is one `OpenedScreen` over them, pushed as the view has it, whose Escape asks the app to close it, and which goes when the view no longer has it (`stands`); the filter is a `FilterScreen`. Each turn is the stack's `land()` when `ViewModel::landing` rose, else its `refresh()`. Frames are laid out afresh from the view: the SDK's `ListFrame`, `MenuFrame` and `LineFrame`, and this app's `FormFrame` over the dialog controller. The row in hand is kept per frame, by key; a frame is said when it is new or the app moved what is in hand, and only put on the cells otherwise, so a sync or a minute never cuts off reading.
- **The letters are the core's** (`braille_letter`, `ShellEvent.Letter`, `action_letter` and its kin); how one is pressed is the core's too (`braillesense_letter`, `braillesense_press`): Space with the letter is its command, except where Space with those dots is already something on every screen: A, B, K and L (Up, Shift-Tab, the line's start, the first row) and C, D, E, F, H, M, O, R and Z (the device's Repeat, the manual's Delete, Escape, Find, Help, Menu, Options, Repeat and Exit), which Enter with the letter reaches, as the device's own hotkeys are. A menu shows each item's core letter and the press for it. `,`, `.` and `/` have no dots: Move Up, Move Down and the filter are the row's menu and Space-F.
- **A letter alone moves by first letter**: `ShellEvent.FindLetter` in a list the app holds as its main one (its `landing` rises, so the row is said), the core's `first_letter` in a list drawn here (a side list, a pick, a menu's own list); none found is said in the core's words.
- **What is said comes from the core's description** (`Turn.described`, handed to `AppModel.described` on the main thread before the view moves): a question's prompt, buttons, choices and what was typed are its; a row arrived at or read again (Space-R) says its place ("3 of 12"), a person's own move does not, so moving stays quick. A refusal no sentence said is spoken from `changes`.
- **What changes say goes on the cells as well as in speech**, for the device's Message Display Time, then the line returns (the SDK's `Messages`): someone reading braille alone hears none of it. Any key clears it.
- **A side list shown tells the app so at once** (`Event::Hold`, in `Shell.refresh`), or a letter pressed on it would run on the row in hand in the main list, which the person cannot see.
- **Tests**: `./gradlew :braillesense:testDebugUnitTest` covers the forms without a device, `:braillesense-sdk:testDebugUnitTest` the device's parts, its lists and menus among them. On the device, `adb shell service call han_brailleinput 1 i32 <chord> i32 0 i32 0` injects a chord from the shell, and in a debug build the service's calls are logged (`LumennaOutput`).

## The Orbit SDK

`apps/android/orbit-sdk/`: an unofficial SDK for Orbit Research's devices, package
`dev.emassey0135.orbit`, Apache-2.0 (`LICENSE`, `NOTICE`), depending on nothing but Android (not
`:shared`, not the core, not `:braille`), so that Satellite and other apps can use it. It is
everything about Orbit itself; Lumenna's app is over it. **Its own docs are for an app other than
Lumenna and hold the detail**: `README.md` (what a device needs, adding it to a build, the quick
start, the stack's API, testing), `docs/DEVICE.md` (what the devices do that an app must fit: the
keys each keeps, Satellite and `orbit.self_voicing`, package names, root, the reader's queue), and
the KDoc. A change to its public API changes the README too. What is easy to get wrong here:

- **It is a standalone Gradle build as well as Lumenna's module** (its own repository, a
  submodule here): its own `settings.gradle.kts` (the library at the root, `:sample` beside it), wrapper,
  `gradle/libs.versions.toml` holding the same `libs` aliases its `build.gradle.kts` takes from
  Lumenna's catalog (`android-library`, `junit`), and `.github/workflows/ci.yml`. Lumenna's settings
  include it as `:orbit-sdk` and never read the nested settings. Dokka's version is written in the
  build file, not a catalog, for the same reason. `sample/` is the README's quick start, compiled;
  Lumenna's build does not include it.
- **Explicit API mode** (`kotlin { explicitApi() }`): every declaration says `public` or `internal`,
  and every public one has KDoc (`ServiceLink` is internal). `./gradlew dokkaGenerate`, run inside
  the directory, writes the reference to `build/dokka/html`.
- **The screen stack's shape is the BrailleSense SDK's**: `OrbitUi.on(session, words, letters)`,
  `list(title, items, empty, actions, moved, find)`, `menu`, `askLine`, `confirm`, `choose`, `form`
  (each a callback and a `suspend` twin), `refresh`, `land(key)`, `announce`/`message`, `refused`,
  `batch`, `launch`, `onEmpty`; `Item(key, text, open, actions, letter, detail, expanded, fold,
  toggle, delete, title)`, `Action(name, letter, asks, run)`, `MenuItem(name, letter, children,
  value, asks)`, `Field`, `FormValues` (public constructor, an unknown key throws) and `UiWords`'
  common words (`noRowStartsWith` and `nothingOnLetter` take a `Char`), so an app's screens compile
  against either by their imports. Change one, change the other. On Orbit `detail` is told after
  the text with a comma (`Item.line`), `toggle` is Select on a row with no `open`, `delete` is Space
  with D (Dot 8 with D where the app's `Letters` keep Space), and `asks` changes nothing, since
  Orbit's menus mark no dialog. `MenuItem` is the SDK's own, made `OrbitMenu.Item` inside.
- **`OrbitSession`** (`session`) is the device for one activity, as `BrailleSenseSession` is, its
  lifecycle methods named the same (`created`, `resumed`, `paused`, `windowFocus`, `destroyed`):
  the `OrbitReader` (the session is its `Listener`), `BrailleInput`, the reader's `Claim`, keys
  delivered to the main thread (`onKey`), `onReady` after `appOpened`, `onSettingsChanged`,
  `onTableChanged`, `onRootFailed`, and `DebugKeys` on a broadcast the app names, in a debuggable
  build only (`takesDebugKeys`). `OrbitUi.on` wires the stack into it; a `reader` given there wraps
  the session's (Lumenna's `HelperText`).
- **Everything Orbit is reached through is the clean-room AIDL** (`src/main/aidl`), the screen
  reader's in its order and never #44 (`canSpeakLanguage`), which the Player lacks. Copy nothing
  from Orbit's own APKs: port only the Satellite launcher's code (Apache-2.0, the same author's).
- **The device's words are the SDK's** (`OrbitWords`, `CellNames`): the stock strings of Orbit's
  English build (Satellite's for the echo of dots and deletions), the root's exception to "a shell
  writes no sentence". The stack's own few (`UiWords`) are English defaults an app replaces.
- **`announce` waits behind a description** (400 ms, `SETTLE_MS`): `speakText` jumps Orbit's queue
  of descriptions, so a sentence sent at once is said before the list it follows.
- How each piece works (`EditBox` and its echo, `OrbitMenu`, `OrbitList`, `ListFinder`,
  `OrbitDialog`, `BrailleInput` and the G chord, root) is in its KDoc and `docs/DEVICE.md`.
- JVM tests: `./gradlew :orbit-sdk:testDebugUnitTest`, or `./gradlew testDebugUnitTest` inside the
  directory. The stack's (`UiTest`) run over a `Reader` that records what it is told.

## The Orbit app

`apps/android/orbit/`: the Orbit Speak and the Orbit Player (LAVA LZG403_25I and LZG403_25, Android
11, arm64), over `shared` and the Crux app's Kotlin. Not Compose: one `TextView` (seen only in a screenshot: the devices have no screen),
while Orbit's screen reader voices the screens the app describes. Sideloaded, or from
GitHub's releases; installing it needs nothing else but Satellite (its hook and launcher) on the
device. Its identifier is the phone's `package_id` (never on the same device): not `com.orbit.`, and
holding no "inputmethod", which Orbit's routing would misread. `check-apk.sh`, run by CI and
`release.yml`, checks what Satellite reads of an APK.

- **The activity is the SDK's `OrbitSession`** (`OrbitActivity`): it hands the session its
  lifecycle and nothing else of Orbit's; the store not opening still binds the reader, so Orbit
  says the app and the message shown. `Ui` builds the stack with `OrbitUi.on(session, …)`, its
  reader wrapped in `HelperText`, which wires the keys, the reader's readiness, a new braille table
  and a root refusal.
- **Files and folders are chosen in Orbit's own File Manager** (the SDK's `FilePicker`, `OrbitActivity`):
  importing and restoring a file, saving what the app makes (`AppModel.files`), and a folder setting
  (`backup-dir`, a settings page's button, set to the folder chosen). Never Android's own picker,
  which Orbit's screen reader clicks through by itself. The File Manager grants the file it answers
  with, so no storage permission is needed for it; a backup folder is written by the core by its
  path, which Android lets an app without all-files access do only in shared folders such as
  Documents and Download (still to be tried on a device).
- **What is told to Orbit's reader, and when, is the SDK's stack** (`OrbitUi`, which `Ui` hosts,
  built with the core's letter rule, `CoreLetters`, and the core's words): a list is
  `onFocusChanged` of a list box, a move `onItemChanged`, a question `onDialogOpened` then its first
  answer, every menu Orbit's own (`menuOpened`, unnamed, so Orbit says "Context menu", then each
  item as a menu's focus), a row's actions and a reminder's buttons included. A turn that only
  redrew says nothing, and a row reworded by what the person just did is read again. What is shown
  and what changed are the core's (`Turn::described`, handed over by `AppModel.described`, a
  callback, since a flow keeps only the latest and a refusal must not be lost): each turn `Ui.turned`
  follows the view inside the stack's `batch`, then `land`s when the core's `Moved` says the list
  shown moved, else `refresh`es; the core's announcements go through `announce` and its refusals
  (`Refused`, said only when `said` is false) through `refused`, a moment behind the description.
  A person's own move is said at once; the app landing on the row just said says nothing more.
- **Screens** (`Screens.kt`, `Forms.kt`, `Dialogs.kt`): each is an `AppList` (the SDK's
  `ListScreen` with Lumenna's hooks) or a `DialogScreen`, on the stack. The screens the app itself
  opens (a question, the block form, a proposal, a reminder list, the quick-add line) are pushed and
  closed from the view each turn (`Ui.follow`, as BTSpeak's `follow`), places followed with the
  stack's `insert` and `replace`. A screen tells itself only once `loaded`, so nothing says a list is
  empty on the way to it. The `TextView` shows what Orbit is last told (`HelperText`, around the
  reader), which only a screenshot or a mirror over adb shows: the Speak and the Player have no
  screen.
- **The app opens on Home** (`ShellEvent.Home`, `ViewModel.home`): its places as the sidebar has
  them, folded as it is, each with its letter and press, then Home's own actions. Its row in hand is
  the sidebar's (`Hold(SIDEBAR, key)`), and Enter on any row of any list is `Key("open")`: on Home a
  place opens (a heading folds), elsewhere the core decides (a task's details, a block's form, a
  trashed task restored). The app follows where the core goes (`Ui.followPlace`): Home left shows the
  place over it, and the place shown moving (What Should I Work On?, a project deleted) shows the
  new one in its stead; going back to Home sends `Home` again. Settings the core opens (Home's
  Settings, its letter) are shown too. Home looks back for reminders due as it first opens, and the app as
  it comes back to the front (`DueEvent.LookBack`, `Ui.lookBack`; each look says what was missed), since Android's alarms show them but the app's list holds none until
  something looks, and Home offers Reminders Due only while any are.
- **On a place, the day and Home, the menu is the core's menu bar** (Space with M, the Player's
  Menu: a screen's `menuBar()`, `MenuBar.kt`, the SDK's `MenuBar`), since Orbit's one menu key is a row's
  menu and the screen's both: `braille_menu_bar_on_row`, the row in hand's actions first, named for
  the row (its line's `title`), then the screen's own menu, then File, Edit, View, Task, Day and
  Help, each a submenu moved as Orbit's Editor's are. Each name and whether it applies are
  `ViewModel::commands` and `screen_actions`, so what does not apply is left out, as Orbit's menus
  leave it, and a menu left empty too. A row's action is handed back with `ActOn`, and a command is
  `Key(id)` but for what is this app's to run (`Ui.runMenuCommand`: a `shell` screen action, the
  filter, Key Help, About, Reminders Due, a heading's places on Home, the Devices and settings pages,
  quitting). A letter alone is an item's access key in the level shown; Space with a letter (Dot 8
  with one the Speak keeps) is a hot key from anywhere in the bar, as from the screen, and every
  letter the bar shows as a hot key still runs from the screen under it. On the Player a digit
  chooses that item of the level, and Back closes it. A choice closes the bar without saying the
  screen under it again; Escape, Back, the menu key or Left at the top dismiss it, and the screen
  is said again, as Orbit's do. **Every other screen's menu is the row's actions, then the
  screen's own** (the stack's flat menu, told as Orbit's own). On the
  Player a line's menu is its Confirm and Cancel, which Tab reaches on the Speak. A reminder due's
  buttons are its menu and its letters, each handed back as `DueEvent.Press(id, button)`.
- **A turn is taken from `AppModel.described`**, once its view and lines are in place: a collector
  of the view on `Main.immediate` runs between the view being set and the lines, with the last
  turn's lines.
- **Keys**: `KeyDecoder` turns Orbit's bundle into a `Key`. On the Speak, as its user guide has them: Space with dots 1 and 4 are Up and Down (Space with dots 3 and 6 are the screen reader's, reviewing the item's text, except in an edit box, where they move the caret), Space with dots 4 5 and dots 1 2 Tab and Shift-Tab, Space with L and dots 4 5 6 the top and bottom, Space with E or Z back whatever the row, Space with M only the menu (the menu bar on a place, the day and Home), dot 7 back and dot 8 open. Left goes back out of a list, never out of a dialog, on the Player too, whose Back leaves it. Which key presses a letter is the core's (`speak_letter`): Space with it, or Dot 8 (Enter) with it where the device keeps Space with those dots while Lumenna is in front: C and T the screen reader's; A, B, E, L, M and Z on every screen. What Orbit's own apps do with Space and a letter (the Editor's D, K and S, the Recorder's and Media Player's) is theirs alone. G is the screen reader's with both keys, so what it names is a menu away. Space with a letter that takes Dot 8 says so (`speak_press_instead`). A letter alone is a first letter (`AppList.findLetter`: `FindLetter` on the app's main list, `first_letter` on this app's own); the letter rule is `CoreLetters`, given to the stack. On the Player digits open rows, holding star finds by letter (T9), its modes named in the SDK's words, Menu the menu, Back goes back; a device with no braille translator is taken for the Player until a key says otherwise, so no Speak press is shown there. A letter is the core's: `brailleLetter(dots)` (a to z, none with dot 7 or 8) then `ShellEvent.Letter`. D is the delete key's, which the core's table does not have. Letters a row or screen shows on the Speak are `action_letter`, `ScreenAction::letter`, `HomePlace::letter` and their kin, with their press (`speak_press`) said after the name; `,`, `.` and `/` have no dots, so Move Up, Move Down and the filter are the menu's.
- **Every form and dialog is the core's** (`Dialogs.kt`: `DialogScreen` over `:braille`'s `DialogController`): a question and the block form are `ScreenView.dialog` by its `id` (`question:<n>`, `block`), quick add `quick-add`, the task form `AppModel.taskForm()` (`App::task_form_on`) once the details show the task, a settings page `AppModel.settingsPage(id)`. Only this app's own lines (the filter, a field on the Player, a code to pair with) are dialogs it builds itself (`DialogScreen.ownLine`, its buttons `ControlSends.Shell`). On the Speak a dialog is Orbit's: Tab and Shift-Tab move round its controls, each active as soon as it is reached, as in the Editor's Save As; a `Line` or `Lines` control is the SDK's edit box (several lines for Notes), a `Choice` a combo box and a `List` a list box that Up, Down and a first letter move, a `Check` a check box Up, Down or Select toggle, a `Button` a button, a `Text` a read-only line, each told to Orbit by that role. Select in a line presses it if it submits, else the default (Save, Add, the question's answer); a `Lines` control takes Select as a new line. Space with E or Z (and Back, and Dot 7 off a line) send the dialog's `cancel`; the task form's is `DetailsEvent::Discard`, so Back throws a form away and it saves only by its Save. Confirm and Cancel are the core's words. A setting applies as it changes, so a page has no Confirm, and what the core leaves to this app (`Shell`: back up, import, pair, a folder) is done here. A refusal the dialog holds (`refusal`, `refused`, `problem:<key>`) is said once with the error tone. The Player has no Tab: the same controls are its rows, a line or a choice opening a screen of its own, a list its options; a dialog that is a line (quick add, a question's line, the filter) fills the screen, and its Menu is the dialog's buttons.
- **Typing is the SDK's `EditBox`** (`Ui.editor`, over `ReaderEditOutput`), the Speak's or the
  Player's by the last key, with completions on Down where a line has them. What the device says
  as the person types is the SDK's `OrbitWords`; the key help describing it is the core's
  (`OrbitWords::speak_keys`). Orbit's settings unread (root refused) are said once in the SDK's
  words, typing going on in the table last read.
- **What the core decides, this app does not**: settings list every page the core gives, a pairing
  code goes as typed, spaces and all, no question is answered Yes by this app, and Undo, Redo and
  Sync Now are `Key(id)` wherever they are offered.
- **Back throws a form away, never saves it**, on both devices: the task form's Back is its Cancel
  (`DetailsEvent::Discard`), and it saves only by its Save.
- **Trying it**: a debug build takes keys over the broadcast `OrbitSession` is given, `DEBUG_KEY`
  (the SDK's `DebugKeys`; the SDK's `docs/DEVICE.md` has the command, waking the device, and
  reading what Orbit said from its log). The real keys, by hand, are still to be tried.

## CI

`android-build` builds once, checks 16 KB alignment (the BrailleSense APK too, its only check here beside the JVM tests: no runner has its services) and builds the core optimized
(`LUMENNA_CORE_PROFILE`), then a phone, a desktop-sized emulator and a Wear OS watch run
`run-instrumented-tests.sh` from that build, on Android 36: every Android 37 image crashes its
display server on the runners' software renderer. The test jobs need no Rust.
