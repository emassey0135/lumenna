# The Apple apps

Read when working in `apps/apple/`: the iPhone and iPad, the Mac, and the Apple Watch. The root `CLAUDE.md` holds what every part of Lumenna shares; this is what is easy to get wrong here.

## The iOS app

`apps/apple/`: SwiftUI screens in a UIKit shell (the tab bar, the iPad's columns, navigation,
the keyboard's commands), drawing the Crux app through BoltFFI's Swift. What the screens show
is followed once for every Apple app in `Shared/Screens` (`TaskListModel`, `DayModel`,
`PairingModel`, the row actions' split `AppRowActions`, the Mac's `Outline` of nested rows),
and drawn by each platform its own way. UIKit stays for quick add, whose completion needs the
cursor's place, which a SwiftUI field gives only from iOS 18. `cd apps/apple && xcodegen` makes
the project from `project.yml`; the `.xcodeproj` and `Generated/` are not committed.

- **Every screen draws the app's view and sends it what the person did.** `AppModel`
  (`Shared/`) holds the app over its `Device`, takes each turn on the main thread in order and
  posts `AppModel.shown`; a screen draws `view` with the turn's `lines`, opens its place as it
  comes to the front unless the app shows it already, and moves VoiceOver only when `landing`
  went up. The app's questions are `AppQuestions`, asked through each platform's
  `QuestionAsking` at each window's root; what the app opens by itself (the block form, a
  proposal, a reminder list) is `AppSheets` (the watch's `WatchScreens`). A form takes the
  app's values only once the app is still (`AppModel.whenStill`): an answer for earlier typing
  would undo later typing, and the last turn's notice can come while a tick is under way.
- **The generated Swift is a framework of its own** (`LumennaApp`, one per platform, embedded
  in its app): BoltFFI emits a crate's exports only when it is the crate being built, so
  `crates/ffi` is the library, force-loaded into the framework, and the app, its tests and the
  framework share one core in the process. As a module of its own, its names (`Locale`,
  `Action`) do not shadow Foundation's; code names them `LumennaApp.Action`.
- **Xcode builds the core itself**: `build-core.sh` builds `lumenna-ffi` in a clean
  environment (Xcode's SDK variables break Cargo's host build scripts), as a static library
  alone (`cargo rustc --crate-type staticlib`), the Mac's and the phone's with the `rpc`
  feature, then has BoltFFI generate the Swift (`Generated/App`, the watch's
  `Generated/Watch/App`). What it makes is named as the build phase's output, or a fresh
  checkout looks for the Swift before the phase has run. The clean environment sets the
  deployment targets itself (`IPHONEOS_DEPLOYMENT_TARGET` and the rest), or C objects target
  the SDK's own version; heed that linker warning.
- **The core is linked statically, by the `.a`'s full path** (`-force_load`). `-llumenna_ffi`
  picks the dylib Cargo builds beside it, which works in the simulator and crashes on a phone
  at launch. A post-build phase fails the build on any `liblumenna_` reference. A launch is
  not proof it ran: check the process is alive, or the crash logs.
- **Iroh needs `SystemConfiguration` and `Network`** linked.
- **A row owns what VoiceOver says**: every list row sets its own label (the title) and
  value (the rest of the app's line, whether it is folded, and the level where it changes:
  `AppModel.spoken`), its children ignored; left alone, a row says its secondary text twice.
- **Every list is SwiftUI**, since hosted `Text` in a UIKit cell fails the audit for Dynamic
  Type and clipping, where a SwiftUI `List` passes. The task list, Blocks (the same screen: no
  filter, a row's tap its Edit Block) and the day are `TaskListViewController` and
  `DayViewController`, each a `UIHostingController` over its SwiftUI screen and the shared
  `TaskListModel` or `DayModel`; their rows split actions by `AppRowActions` (Mark Done swiped
  from the leading edge, the other primary actions and folding from the trailing, the rest
  accessibility actions, all in the context menu), and VoiceOver goes to the row in hand
  through `@AccessibilityFocusState` only when `landing` went up. Every other list (Browse, the
  iPad's sidebar, a reminder list, Devices, Settings, a pick) is an `AppListViewController`, a
  UIKit controller holding the SwiftUI `AppListScreen`, its subclasses saying only what they
  list and where it is held. A SwiftUI row is a button in its cell, so a UI test finds a row by
  its title with `app.row`. A SwiftUI field's first responder is SwiftUI's own text view, its
  name on SwiftUI's element, so focus is given through `@FocusState`
  (`TaskListModel.filterWanted`), never by finding a view. A model takes the person's own move
  into `focus` at once (`moved(to:)`), so a key pressed straight after acts on that row, not
  the one the app's next turn has yet to replace.
- **VoiceOver's rotors are UIKit's** (`RowRotor`): Overdue on a task list, Blocks and Running on
  the day, a `UIAccessibilityCustomRotor` on the hosting view and the list's collection view,
  whose search finds the next matching row in the model, scrolls the collection view to it and
  hands back its cell's element. SwiftUI's `accessibilityRotor` steps to nothing: a `List` row
  exists only while its cell is on the screen. On the day's heading VoiceOver offers no custom
  rotor (its SwiftUI element stops the search); from any row they work.
- **Text entry wraps** (`LineEntry`): a one-line field scrolls sideways at large sizes and
  the audit flags it as clipped.
- **A line in the filter or quick-add language completes two ways**
  (`Shared/Forms/Completions.swift`, `CompletionBar`): a strip of what fits, which changes as
  one types, and a list with clear edges behind a Show Completions action, its button, or Down
  on a keyboard, never by itself. The list is SwiftUI's `CompletionList` on iOS and the Mac's
  `PickerSheet` (a SwiftUI sheet's hosting view is a nameless group to the audit). A question
  in the filter language (`Asking::syntax`) is a sheet on iOS
  (`CompletingQuestionViewController`), since an alert's field can carry no completion, and the
  Mac's `CompletingField` in its alert.
- **A colour is a dot beside its name** (`Shared/Swatch.swift`, `SwatchMark`), on every Apple
  app, hidden from VoiceOver, light or dark by the appearance and growing with the text. The
  Mac's AppKit pick sheet draws a `SwatchDot`, a plain view VoiceOver does not see.
- **A SwiftUI `TextField`'s title is only a placeholder**: with text in it, VoiceOver never
  says the name. See `NamedRow` below.
- **The core's announcement is queued behind a focus change** (`Announcer`, a queued
  announcement), so neither cuts the other off.
- **What the app says is shown as well as spoken** (`Announcer` posts `Said`): a refusal or
  what a change did reaches someone without VoiceOver too. On iOS a banner over the window
  (`SaidBanner`), on the watch the same over each screen and sheet (`showsWhatIsSaid`), both
  hidden from VoiceOver, which has said it already, taking no touches, and left out under
  `LUMENNA_TEST_PROFILE`, where the audits would judge text fading out. On the Mac a status
  line at the foot of the main window and of the Devices page (`StatusLineController`),
  hidden until something is said: empty, the audit finds a nameless text.
- **A row's primary actions are its swipe actions; the rest are its custom actions**, and
  all of them are in its long-press menu. A trailing swipe never runs its first action by
  itself (`allowsFullSwipe: false`): the first can be Delete. UIKit offers swipe actions to
  VoiceOver, Switch Control and Full Keyboard Access and *adds* custom actions to them, so an
  action in both would be listed twice: each is in exactly one. A swipe action's title is its
  spoken name ("Mark Done", not "Done").
- **No bottom toolbars**: inside a tab bar the floating bar sits over a toolbar's buttons,
  so a tap on Undo lands on the Today tab. Rows scrolled under the glass tab bar fail
  contrast, so pushed forms set `hidesBottomBarWhenPushed`.
- **The store is in the App Group's container** (`Shared/AppGroup.swift`; the group
  `group.dev.emassey0135.lumenna`, shared with the extensions, named once in project.yml's
  `LUMENNA_APP_GROUP` and read from the Info.plist's `LumennaAppGroup`), where a widget or an
  intent can reach it. At the first launch of a build with the group, `AppGroup.profile` moves
  the profile there from Application Support, the app's own, before anything opens it: the
  SQLite file with its `-wal` and `-shm`, the device settings, the whole directory. Within one
  volume, as the two containers are, it is one rename; across volumes (`ProfileMove`) a copy
  beside it under `.lumenna.moving`, renamed into place once whole, then the old one removed, so
  a move cut short is made again. A profile already in the group is never replaced, and the old
  one is then left as it is; the backups (`lumenna-backups`) follow the profile to stay beside
  it, never in it. Without the group (an unsigned build, CI's; a UI test's store) or after a
  failed move, the store stays where it was and the next launch tries again. **An extension
  must never create a store in the group**: one found there first keeps the person's from
  moving.
- **The glance file** (`Shared/GlanceFile.swift`): `glance.json` in the group's container, the
  core's `glanceFile` of `App.glance(48)` (read back with `glanceFromFile`, so a widget that links
  the core decodes no shape of its own), written at launch, after every `Core.changed`, and in
  the background refresh before its task ends; then `WidgetCenter.reloadAllTimelines()`. Every
  Apple app writes it, the watch and both Mac editions too, since the direct download's widgets
  cannot reach its store; a widget writes it too, after its Mark Done, and the direct download
  after taking in a Mark Done its widget handed on (`WidgetRequests`). Under
  `LUMENNA_TEST_PROFILE` nothing is written to the group.
- **`TZ` is set from `TimeZone.current`** at launch and on returning to the foreground: jiff
  finds the zone through `TZ` or `/etc/localtime`, and the sandbox is no place to rely on
  the second.
- **`Core.changed` is what follows a change outside the screens** (reminders re-armed, the
  watch told): `Core` looks at the store's heads once a second (`Device.heads`, which move
  for the app's own writes too) and posts it, marked `arrived` unless the person did
  something in the last three seconds. The app ticks once a second itself, taking in what
  another process wrote.
- **Background sync** (`BackgroundSync`): a round in the time `beginBackgroundTask` grants
  on leaving, and a `BGAppRefreshTask` whenever iOS chooses. A refresh launches the app with
  no scene, so the `Core` is the app delegate's, one per process. Apple's simulated launch
  hangs in the iOS 27 simulator; the handler has only run on a device.
- **Reminders are scheduled, not delivered** (`Shared/Reminders.swift`): `ReminderSchedule`
  hands `UNUserNotificationCenter` what `upcoming_reminders` says fires in the next 48 hours,
  the earliest 60 (iOS keeps 64), identified `reminder <key> <occurrence>` so re-arming
  replaces and what is no longer listed is cancelled. It re-arms at launch, on every
  `Core.changed` (the foreground's included), and in `BackgroundSync`'s leaving round and
  refresh, before the task ends. On launch, the foreground and a refresh it first catches up
  (`reminders_missed`, `kept: true`): what fired past the last window is shown at once, the
  older as one summary. Dismiss and Snooze are a category whose buttons are the core's words,
  answered by `ReminderResponder`, set as delegate before launch finishes. Permission is asked
  when the person opens a Reminders list, turns reminders on, or makes an edit that leaves
  something to fire, never at launch, and never under `LUMENNA_TEST_PROFILE` (the system's
  alert would cover the app under test). A sync's change notice carries `arrived`, so it is
  not taken for the person's edit. The scheduling is unit-tested through the
  `NotificationCentre` seam (`ReminderScheduleTests`); UI tests cannot post real ones.
  A firing with `carry` set is in a second category, its third button the core's Carry
  Forward (`carry_rest_forward` of `<target>@<occurrence>`). Setting categories replaces them
  all, so the carry one is set from the last carry reminder seen whenever any is handed over:
  one already showing keeps its button. An overrun warning (`replan` set) is a third, its
  button the core's Replan, `.foreground`: `ReminderResponder` hands on a `RemindedOf` with
  `replan`, and each app opens today with Replan from Now over it (`openReplan`).
- **The day's own actions** (`Plan.actions`) are a Day Actions menu button among the day's
  buttons (in the navigation bar it crowds the title, which the iPad's narrow column clips)
  and the heading's custom actions, which make the heading at least 44 points tall: an element
  with actions fails the audit's hit area. Replan from Now and Plan the Day are one SwiftUI
  screen for all three apps (`Shared/Forms/AppProposalForm.swift`, the app's proposal): each
  change a toggle named by its sentence, its reason and the core's `item` its hint, and after a
  partial apply the core's `next` in its place until nothing is proposed. On iPad it is full
  screen: as a form sheet, the day's rows half under it fail the audit as cut-off text. A
  refusal is shown on the screen and the proposal asked for again with the same pins. The
  clock it asks in is `Clock.style`, the locale's. Its other options (`Replan.alternatives`)
  are buttons under `others`, each proposing again with its pins in place. A pick's
  `Choice.reason` is its second line and its hint.
- **A Reminders list is the app's** (`ViewModel.reminders`), opened by `AppSheets` when the
  app opens it and held in its own hand: on iOS an `AppListViewController`
  (`RemindersViewController`), whose Add Reminder in the bar asks the app's Text question,
  again with the reason when refused, and whose own reminders swipe Remove Reminder.
- **Pairing follows the app's pairing once for every Apple app** (`Shared/Screens/PairingModel`):
  the status, this device's code, and the three words, asked in the platform's own alert,
  whose safe answer, No, is the one a stray Return gives. One SwiftUI screen draws it for all
  three (`PairingScreen`), the iPhone's with SwiftUI's `PasteButton` beside the code field
  (UIKit's `UIPasteControl` hosted in SwiftUI gets the tap and never pastes). Its frame is
  drawn 34 points tall at every control size, but Apple's hit-region audit, which knows the
  area a finger can hit, passes it, so the test asks the audit rather than the frame.
- **Stock controls unless there is a reason**, and each in `FormParts.swift` has one:
  `NamedRow` (the field carries the name and the visible name is hidden, as UIKit forms do;
  combining the row loses the text-field role), `example(_:)` placeholders and
  `FormParts.caption` (the system greys fail contrast). Give a control an empty title when
  it has an accessibility label, or VoiceOver says the name twice.
- **The tint is `UIColor.lumennaTint`** (system blue is about 4:1 on white), on the window
  **and with `.tint` on every SwiftUI form**, whose accent ignores the window's. Red text is
  `.warningLabel`. The colours have Increase Contrast variants.
- **The audits collect every issue and fail once** (`audit()`), a second after the screen
  settles and never while a sheet or alert is closing: under its fading dimming, black text on
  white fails contrast (`add` waits for quick add to close, as the Mac tests' `addTask` does,
  where a click while it closes is lost). A contrast finding is looked at again, once, two
  seconds on, for text still fading in on a slow runner; text that fails at rest fails both
  looks. Exemptions, all narrow: "potentially inaccessible text" on `NamedRow` screens, the
  keyboard's `TUIPredictionViewCell`, contrast on text under a navigation bar or the soft
  scroll edge just beneath it, contrast the audit judges of a SwiftUI text on the iPad by its
  whole row's frame (`judgesAPadRowNotItsText`), and "partially unsupported" Dynamic Type on
  SwiftUI nodes (the audit reports it even for a stock button), or with no element at all on a
  `NamedRow` screen, where it comes and goes on the iPad between runs of one build. To see an
  unnamed finding, run with `AUDIT_ATTACH=1` and `xcrun xcresulttool export attachments`.
- **Keyboard commands are the Mac's keys and names, once** (`KeyboardCommands.swift`): on
  iPad the menu bar (`AppDelegate.buildMenu`, with Find and Format removed, which would take
  ⌘F and ⌘B), on iPhone, which has none, `RootTabs.keyCommands`; never both, or a key has
  two commands. Each is a selector up the responder chain, so the screen in front answers
  before `RootTabs`. **Something must be first responder** or the chain is empty and the
  Task and Day menus reach nothing: each screen with commands calls `takeKeyboardCommands`
  as it appears, unless text is being edited. ⌘Z in a field with typing to undo undoes
  the typing. Row commands act on the row with keyboard focus, else VoiceOver's (found
  through its containers when VoiceOver's element is not a view, as the iPad's is:
  `UIAccessibility.voiceOverView`), else the one open beside the list; the task open beside a
  list answers them itself.
- **Every assistant operation is an App Intent** (`Shared/Intents`, compiled into all three
  apps; the widgets compile Complete a Task alone, "The widgets"): one per operation of
  `assistantOperations()`, its `persistentIdentifier` the operation's `id`, each running
  through `Device.assist` on the store the app gives `Assistant.serve` as it launches
  (`AppDependencyManager`, since an intent may be what launched it). App Intents' metadata is
  read at build time, so titles, descriptions, categories and keywords are the table's as
  literals, and `AssistantIntentTests` compares every intent with the table at run time: a new
  operation fails it until its intent is written. A task, project, labels, a block, a sitting or
  a choice is an entity (`AssistantEntities.swift`) built from the record the core answers with
  and given back by its identifier (a project or label by name). An erasure asks first: the
  task's own Erase Permanently question, else the core's unconfirmed refusal, then runs with
  `confirm`. Apply a Replan and Apply a Day Plan propose afresh, say the proposal's sentences
  and ask before applying; what is accepted defaults to every change. Pause and Stop default to
  the sitting whose timer runs (`RunningSitting`). Shortcuts' Find Tasks (`TaskQuery`'s
  `EntityPropertyQuery`) builds each comparison into a filter-language query run through
  `list_tasks`, so what matches is the core's; a comparison the language cannot say is not
  offered, and only the order Shortcuts asks for (due, priority, title) is Swift's. Every
  listing returns its entities, and a write on a task returns the task, so a found task passes
  on to the next action. `LumennaShortcuts` is what Siri, Spotlight
  and the Action Button offer unasked (New Task, What Should I Work On?, start, pause and stop,
  Today, Complete). What Spotlight, Siri and Shortcuts find is the core's search
  (`records::search`), as on every platform: every search document (open tasks, projects,
  labels, saved filters, blocks) is indexed as its entity, its attributes the document's title,
  line and keywords (`SearchIndex`, `IndexedEntity`, iOS 18 and macOS 15; the watch has none),
  at launch and a few seconds after each `Core.changed`; a string query answers from
  `searchResults` in its order, never a match of Swift's (choices are only offered); and a
  result chosen is opened by its document's `launch` through `ShellEvent.launch` (the Open
  intents, the app's `launch` given to `Assistant.serve`). The task whose details show is the
  current user activity with its entity, for "this task" (`showingTask`). The SDK's Reminders schema (iOS
  and macOS 27, not watchOS) is not adopted: its reminder carries a list, flags, URLs, images
  and location triggers that no operation takes.
- **The iPad is the Mac's layout, not the iPhone's tabs** (`IPadRoot`, three columns): the
  app's places in a sidebar (`SidebarViewController`, from `ViewModel.sidebar`, plus
  Settings), the place chosen, and what was opened from it (`showBeside`, which pushes on
  iPhone and in a narrow window). The sidebar is an `AppListViewController`, so headings
  and project trees fold with Expand and Collapse actions and say which they are; a
  project's, label's or filter's actions are its own, the core's, as in Browse. A screen
  chosen by a command acts a turn later: it is not in the window until then.
- **The Day menu ends in the day's own actions**, one item per kind (`dayActionItems`), since
  the menu bar is built once: each runs the shown day's action of that kind through `RunDay`,
  which says why a day lacks one. From another place they go to today first.
- **⌘F never reaches the app in the iPad simulator**: no responder is asked about it, where
  ⌘3 is. Filter Tasks stays on ⌘F in the Edit menu, and its test skips on iPad.
- **Three layers of iOS tests.** `LumennaTests` (unit tests hosted in the app) asks each
  list for its swipe actions, custom actions and menu (`AppRowActions`) and runs them: what
  VoiceOver offers as a row's actions, with no gesture to land, so they block in CI. They seed
  the store through `core.app.device` (`newBlock`, in `Seeds.swift`, since BoltFFI's records
  have no defaults), tick the app to take it in, and wait on `core.app.isStill`.
  `VoiceOverUITests` turns VoiceOver on (`XCUIVoiceOverService`, iOS 27) and checks what it
  says, acting through the keyboard commands, which act on VoiceOver's row; its output
  carries a hint's first word only ("Actions"). VO-Command-Down Arrow steps the actions
  rotor, so a test reads a row's actions as VoiceOver lists them; what it says last after a
  change is the announcement, so a test of focus listens while it waits. The rest of the UI
  tests drive the app by gestures, and report rather than block.
- **iPad UI tests run in portrait**: in landscape XCUITest's coordinates come out rotated, so
  part-swipes and taps land on the wrong row, and screenshots come out half black. A hidden
  sidebar's rows still exist off the screen, so a place is used only once it is hittable.
  Swipe actions are revealed by a slow, held part-swipe (`reveal(actionsOf:)`): across a
  narrow column a whole swipe runs the first action itself. Every failure attaches a
  screenshot (`record(_:)`).
- **`apps/apple/test-all.sh`** runs every iOS UI test on the iPhone, the "e" and the iPad at
  once, built once, each device's tests shared over clones, each test stopped after three
  minutes, so one waiting forever cannot hold a whole run.
- **The iPad's menu bar is invisible to XCUITest while hidden**, so the keyboard tests
  (`KeyboardUITests`) prove it by its keys: on iPad they exist only there.
- **A UI test names a fresh store** through `LUMENNA_TEST_PROFILE`, and turns animations off
  (`LUMENNA_NO_ANIMATIONS`): under load, something left animating as the iPad's sidebar hides
  has XCUITest wait a minute a step for an idle app, and the audit judges text still fading in.
- **Run UI tests with `-collect-test-diagnostics never`**, or any failure hangs xcodebuild
  for ten minutes collecting simulator diagnostics:

  ```
  cd apps/apple && xcodebuild -project Lumenna.xcodeproj -scheme Lumenna \
    -destination 'platform=iOS Simulator,name=iPhone 18 Pro' \
    -collect-test-diagnostics never test
  ```

## The macOS app

`apps/apple/Mac/`: SwiftUI screens in an AppKit window (the split view, the menus, sheets,
alerts), AppKit beneath each list's outline, which is where its focus, clicks and keys are
taken. AppKit stays for quick add, as on the iPhone, and for the pick sheet, whose search field
keeps focus while the arrows move through the list and say each choice. `Shared/` holds what
the Apple apps compile, including **`Shared/Forms/`, the task, block and settings forms**, and
`Shared/Screens`, what the screens follow; each app supplies only presentation. Where the
platforms need different answers it is an `#if os` in `FormParts.swift` (`Named`, `Labelled`,
`ChoiceSection`) or at the one row that differs, so a fix to a form lands on both. Scheme
`LumennaMac`.

- **The direct download is not sandboxed, and shares `lum`'s profile**: the app and the
  command line on one Mac are one device with one store. The app ticks once a second, taking in
  what `lum` wrote. The App Store edition is sandboxed, with a store of its own (see "The two
  Mac editions").
- **The direct download carries `lum`** in `Contents/MacOS`: `build-core.sh` builds it for the Mac's
  architectures beside the core (`Generated/Mac/lum`), and the target's Put lum in the app
  phase copies it and signs it with the app's identity and the hardened runtime, before Xcode
  signs the app. Not Copy Files with Code Sign On Copy: its input would need naming as the
  framework's phase's output, a phase the phone and the watch share. `release.yml` checks both
  architectures are in it and that it runs, and makes `lum`'s own macOS tarball from it.
- **The window follows the app**: the middle pane shows the app's place, so a place renamed,
  merged or deleted takes the window with it, and the right pane the details of the task in
  hand, which follow the row in hand. Every list's selection is the row in hand, scrolled to
  only when the app moved it, and folding there is the row's own Expand or Collapse. The task
  list is SwiftUI's `List`, rows nested in disclosure groups, which is AppKit's outline
  underneath, reporting levels and expansion itself; so are Blocks (the same screen, its Return
  and double click Edit Block), the day, Devices and the sidebar (`SidebarScreen`, whose
  headings keep the heading trait and are never selected, and whose rows' context menus are
  their own, a heading's included). Only the pick sheet is an AppKit table.
  **A SwiftUI list takes focus through its own `NSOutlineView`** (`preferredFirstResponder`,
  for F6): the window gives it to the middle pane's list once SwiftUI has made it, as it first
  opens, since SwiftUI's `@FocusState` asked then goes nowhere and AppKit's own first choice is
  the sidebar; a list coming into the pane takes it only when focus is nowhere
  (`takeFocusIfNowhere`), or arrowing through the sidebar, which opens each place, would pull
  focus out of it at the first place. A click anywhere on a list gives it focus as the mouse
  goes down (`FocusOnClick`, an event monitor): a click on the row already selected does not
  by itself, and a recognizer's, at the click's end, comes after a key pressed straight away.
  Space, Delete and Return are taken the same way (`ListKeys`) while the outline or a view in
  it is first responder, never by SwiftUI's `onKeyPress`, which waits on SwiftUI's focus, a
  moment behind the first responder. Space and Delete are the core's keys
  (`AppModel.key`: `.key("toggle")`, `.key("delete")`), never kinds of the app's own, so a
  trashed task's Delete asks Delete from Trash, as iOS's Command-Delete does.
- **Resident in the menu bar, and the device's sync process.** No daemon is needed or used:
  the app's network middleware syncs for as long as it runs and, while it holds the endpoint,
  serves the command surface at the profile's address (`App.serveCommands`, the bridge's `rpc`
  feature), as the daemon would. `LUMENNA_NO_SYNC` (the UI tests') keeps it from both.
- **Opening at login is the app's background setting** (`LoginItem`, `BackgroundKind::MacLogin`,
  over `SMAppService.mainApp`); General draws the setting's row, its title the toggle's and its
  hint beneath. Under `LUMENNA_TEST_PROFILE` the login item is a stand-in in memory, so no
  build under test joins the login items (CI's unsigned one cannot).
- **The resident app delivers reminders** (`ReminderDelivery`): while it holds
  `take_reminders`, `deliver_reminders` every 20 seconds and on becoming active, each shown at
  once through `UNUserNotificationCenter` with the iPhone's category, the missed summary as one
  of its own. Without notification permission it lets the right go, for a process that can say
  them. The Reminders list is the shared SwiftUI form (`Shared/Forms/ReminderList.swift`) in a
  sheet, shared with the watch: its Add Reminder answers the app's question itself, a refusal
  kept in its field with the app's reason.
- **The Day menu ends in the day's own actions**, rebuilt as it opens (`MainMenu.DayMenu`):
  the day shown's, else today's, which goes to the day first; also a Day Actions menu beside
  the day's buttons. Plan the Day and Replan from Now have Command-Option-P and -R (`DayKeys`,
  the iPad's too), answered by `DayMenu.menuHasKeyEquivalent` with the menu closed; that
  stops AppKit looking through the menu's items, so it matches the fixed ones (Command-[)
  itself. The proposal screen is a `HostedForm` sheet with its own Cancel and default
  button. A picker sheet's reason is its row's second line and VoiceOver help.
- **The app's questions are asked where the person is** (`FrontAsker`): over Settings while it
  is in front, else over the main window, so what Settings offers (Erase Everything Deleted,
  a device's Unpair) asks there. Asked over the main window, they opened behind Settings. The
  store's own actions (`SettingsView.offered`) are destructive buttons under Backups
  (`StoreActions`, shared with iOS), the core's `offeredCaption` beneath them; the watch has
  no Backups page, so it offers none.
- **Global shortcuts through Carbon's `RegisterEventHotKey`**, which needs no accessibility
  permission. Never Control-Option: it is VoiceOver's. The recorder warns of clashes with
  VoiceOver, VOCR (Control-Command S, P, L, I; Shift-Control-Command more) and macOS
  (Control-Command Q, F, D, Space).
- **The core has a record called `Timer`**, so Foundation's is named in full.
- **Every stepper says what it stands for** (`SpokenStepper`: the block's length, a number
  setting): AppKit's and SwiftUI's say their number as a percentage of the range ("9.1%"), and
  SwiftUI's `accessibilityValue` on one is ignored. An `NSStepper` subclass overrides
  `accessibilityValue`, posts the change and names itself. Never inside a SwiftUI
  accessibility group or `LabeledContent` with only hidden text beside it: SwiftUI does not
  count an AppKit control as its group's child, and the audit finds the mismatch.
- **SwiftUI form controls go inside `Named`/`Labelled`**: on macOS a `LabeledContent`. A
  bare `TextField("Title", …)` shows its name as separate text and leaves the field unnamed;
  a bare `Toggle` is an unnamed switch. On iOS the opposite (see `NamedRow`). A growing
  text field draws its name in grey, under contrast, so Notes is a `TextEditor`.
- **SwiftUI is hosted through `HostedForm`**: the hosting view steps out of VoiceOver's tree
  and gives its name to the form's scroll area. It has to be the hosting view's own
  override; set from outside, it goes on reporting itself as a group.
- **The pick sheet's cell recolours itself on selection** (`TwoLineCell.backgroundStyle`): the
  quiet grey fails contrast on the selection highlight.
- **The window's minimum size is set after its content**, or the split view shrinks it to
  almost nothing.
- **UI tests enter text by pasting** (`enter(_:into:)`): the test Mac runs VoiceOver, and
  synthesized keystrokes reach it. Windows are found by identifier (`main`, `settings`):
  the first window can be Siri's.
- **The Mac audit judges only the window in front** (or the open sheet): dimmed windows
  fail contrast without being what anyone reads. It also skips contrast under VoiceOver's
  own caption and braille panels. Also excused: the Touch Bar, SwiftUI pop-ups' "Action is
  missing" (and a menu button's: the day's Day Actions, which AXPress opens), a "Parent/Child
  mismatch" with no element, and a contrast finding with no element whose text is in the
  dimmed window behind an open sheet and not in the sheet.
- UI tests need macOS to have authorized UI automation for Xcode once.
- **What VoiceOver says can be heard without listening**: with "Allow VoiceOver to be
  controlled with AppleScript" on, `tell application "VoiceOver" to return content of last
  phrase` gives the last thing it said. Interact with a list by VO-Shift-Down
  (`key code 125 using {control down, option down, shift down}` through System Events), then
  the arrows step its rows. The accessibility tree itself (roles, names, values, a row's
  `AXDisclosureLevel`) is read with `AXUIElementCopyAttributeValue`, which needs the terminal
  to have Accessibility permission. The tree alone does not say what VoiceOver will speak: a
  row whose element is static text is said by its value, without its level or expansion.
- **A SwiftUI list row is a group named by its whole line** (`MacTaskListScreen`): as static
  text VoiceOver says its value alone and no level or expansion. The check box is a pointer's
  alone, as the web's; a disclosure group's label, which SwiftUI makes a heading, has the
  heading taken off; the hosting view steps out of the tree (`FlatHostingView`), or it is a
  nameless group the audit finds. A row holding one text is read by that text, not the
  row's name, so the sidebar's text says the whole line itself (`accessibilityRepresentation`),
  with its folding and level in words, as the iPad's (`AppModel.sidebarItems`): hidden, the row
  is said with "group" after it. No tooltip on a row: VoiceOver reads it as the row's help,
  saying the count twice. A selection turned off on a disclosure group is off for everything
  in it, so a heading's is turned off on its label. Tests find a sidebar row by its
  identifier, the title.

## The two Mac editions

One target, `LumennaMac`, from the same sources, in two configurations, as ROADMAP's macOS
packaging decided:

- **The direct download** (Debug and Release; scheme `LumennaMac`): Developer ID, not
  sandboxed, its store in `~/Library/Application Support/lumenna` (or `LUMENNA_PROFILE`'s),
  shared with the `lum` inside it and with Emacs over its socket. Its App Group
  (`Mac/Lumenna.entitlements`) holds the glance file and the widgets' requests alone: its
  widgets, sandboxed, cannot read its store, so Today's Mark Done is handed to the app there
  (`WidgetRequests`, "The widgets").
- **The Mac App Store edition** (the `AppStore` configuration, scheme `LumennaMacAppStore`,
  compiled with `APP_STORE`, `LUMENNA_EDITION=appstore` for the scripts): sandboxed
  (`Mac/AppStore.entitlements`), its store in its App Group's container (`<group>/lumenna`),
  where its widgets reach it. It has only ever kept it there, so nothing moves. Built into
  `Build/Products/AppStore`, so the two `Lumenna.app`s never overwrite each other. The same
  bundle identifier: one app, two ways to get it, not meant to be installed side by side.

What the App Store edition does differently, and why:

- **Network client and server entitlements**, for Iroh, which dials and listens, and for local
  discovery through the system's responder; the local network's usage words and the Bonjour
  services are the Info.plist's, both editions'.
- **The store and the command socket are in the group** (`<group>/lumenna/lumenna.sock`), the
  one place the sandbox shares with the extensions. The Mac's group is named short
  (`<team>.lumenna`, both editions'), since a Unix socket's path holds at most 103 bytes and
  the container's path is long. Nothing reads `~/Library/Application Support`: inside the
  sandbox that is the app's own container.
- **No `lum` inside** (`build-core.sh` builds none and Put lum in the app skips): a sandboxed
  `lum` could open nothing named to it, nor be put on the path by the app. Whoever wants `lum`
  installs it apart and pairs it with the app as another device. So nothing that offers it
  either: no "Add to Claude Desktop" (it writes another app's settings, which the sandbox
  forbids), and no Sparkle, since the store updates the app. Neither is in the app yet; when
  they come they go under `#if !APP_STORE`, left out rather than replaced, so nothing needs words
  in their place.
- **A backup folder the person picks is kept as a security-scoped bookmark** (`BackupFolder`,
  `files.bookmarks.app-scope`), taken up again at launch before the first backup: the open
  panel's choice lasts only until the app quits. The default folder, beside the profile in the
  group, needs none. On the Mac the folder is only ever chosen in its panel, in both editions,
  so no path is typed that the sandbox would refuse. Exports, imports and restores go through
  the save and open panels (`files.user-selected.read-write`), as in the direct download.
- **The login item and the global shortcuts are the same code**: `SMAppService.mainApp` is the
  sandbox's own way to open at login, and Carbon's `RegisterEventHotKey` needs no entitlement.
- **Settings offers every setting in both**: none of the core's applies to one edition alone.
- **CI builds both, unsigned**, and checks the App Store edition carries no `lum`. Unsigned,
  neither is sandboxed and neither has its group: the sandbox, the group container and the
  bookmark take effect only in a signed build, and are checked by hand.

## The widgets

`Widgets/`: WidgetKit, one source set for three extensions (project.yml's `LumennaWidgets`
template): `LumennaWidgetsiOS` (the iPhone and iPad), `LumennaWidgetsMac` (both editions, one
target in both configurations) and `LumennaWidgetsWatch` (complications and the Smart Stack),
each embedded in its app.

- **An extension links its app's `LumennaApp` framework** (`embed: false`, found through the
  run path in the app's `Frameworks`), so the core is built once per platform and linked
  statically into that framework alone, by the `.a`'s path, as the apps have it; the framework
  is `APPLICATION_EXTENSION_API_ONLY`. The extension is a process of its own, so this is one
  copy per process.
- **Which widgets, and their words, are the core's** (`glanceWidgets()`): each one's id is its
  WidgetKit kind, its gallery name and description are read at run time, and the sizes it comes
  in are `GlanceWidgets.families`, keyed by id, which `WidgetTests` compares with the core's: a
  widget the core adds fails it until it has sizes. Now is `systemSmall` (StandBy's too) and, on
  iOS and the watch, the accessory families (the watch's corner too); Today is `systemMedium`
  and `systemLarge`, so the watch has Now alone. A size shows the first of a widget's parts that
  fit: `ViewThatFits` over each shorter run of them (`GlanceWidgets.fitting`), never a table of
  sizes, so a larger text size drops the tasks before the headline.
- **The timeline is the glance file** (`glance.json` in the group, `glanceFromFile`): one entry
  per moment, dated by its `at`, policy `.atEnd`. Run out (its last moment come: the app unopened
  for about two days, `GlanceWidgets.stale`) or never written, it is worked out afresh from the
  store where the widget reaches it (the iPhone and iPad, the App Store edition: `Device.glance`
  in this device's clock, opened as Mark Done opens it, and written back to the file);
  elsewhere, and with no store, the last moment is shown as now, or with none the core's empty
  day (`glanceEmpty`), never a sentence of ours, and asked for again in 15 minutes rather than
  at once. The core writes instants to the nanosecond, which Foundation's ISO 8601 parser refuses
  past milliseconds, so `GlanceWidgets.instant` reads the fraction itself. A running timer counts
  by itself (`Text(_, style: .timer)` from `countingFrom`) and a block under way fills its gauge
  (`ProgressView(timerInterval:)`), neither with an entry of its own. The Smart Stack is told a
  moment matters (`relevance`, score 1) while a block is under way or a timer runs.
- **VoiceOver hears a widget as one element**, the core's `spoken`, with a running timer as its
  value: the headline part, every widget's first (the core's test), carries it, and the other
  parts are hidden. Only Today's Mark Done buttons are elements of their own, named "Mark Done,
  <task>" (`GlanceWidgets.markDone`), as Android's are, and its New Task (the core's `NewTask`
  part, last, a `Link` to the `new-task` launch, its words the launch shortcut's). Text is `.primary`, `quietLabel` and
  `warningLabel` (`Shared/Palette.swift`, moved out of the forms for this) on the system's
  background; the title and the timer are `widgetAccentable` for the accented rendering. Block
  colours are not drawn.
- **Mark Done is Complete a Task** (`CompleteTaskIntent`, in its own file so the widgets compile
  it with the entities and nothing else of the intents; the two queries that read another
  intent's parameter read none under `LUMENNA_WIDGETS`). The widget's copy is not discoverable,
  so Shortcuts lists the app's once. Where it runs:
  - **iPhone, iPad and the App Store edition**: in the widget, on the store in the group
    (`WidgetStore`, through `AppDependencyManager`), and only if `lumenna.sqlite` is already
    there: a widget must never make a store in the group. After a write (the heads moved,
    `AssistantStore`'s `changed`) the widget writes the glance file again itself
    (`GlanceFile(device:)`), while the store is still open, since the app may not be running.
- **A widget never holds the store open.** iOS ends a process suspended with a lock on a file in
  a group (`0xdead10cc`), so the widget's `AssistantStore(opening:)` opens a `Device` afresh for
  each call (`GlanceStore.open`) and lets go of it before the call returns, as the timeline's
  reading does (`GlanceStore.visit`): nothing caches one, and a closure given one keeps nothing of
  it. Dropping the last `Device` closes SQLite, since nothing in the core keeps its store
  (`a_device_let_go_of_closes_its_store_whatever_it_did`); `WidgetTests` checks no connection is
  left by taking the file out of WAL, which SQLite allows only to a connection alone with it. A
  Mark Done opens the store a few times (the entity's query, the operation, the entity after),
  each a fresh open, which is the price of holding nothing.
  - **The direct download**: handed to the app through the group (`Shared/WidgetRequests.swift`),
    on every macOS from 14: its store is outside the sandbox every Mac extension runs in, and
    the app, resident in the menu bar, holds it. The widget's `perform` opens no store: it
    writes a request into `requests/` in the group (the operation's id and its arguments, one
    file each, named by the moment and a UUID, written under a dot-name and renamed, so the app
    never reads half of one), its `TaskQuery` handing back the identifier alone, and its
    intents served a store that opens nothing (`WidgetStore`), so nothing reaches for a missing
    dependency. The app (`Core.takeRequests`, compiled into the direct download alone) takes them
    in at launch, on its one-second tick and when the folder changes (a dispatch source), runs
    each through `Device.assist`, removes it once it ran, then writes the glance again, which
    reloads the timelines. **Only what `WidgetRequests.allowed` names runs**, each operation with
    the arguments it lists, whatever a file names: `complete_task` given the task alone, and the
    Timer control's `toggle_timer` given nothing ("The controls"). What can write the folder must
    not run another operation, an erasure among them, so the table is written out, never a pattern. A
    file naming another, or one that does not read, is removed unrun. A run that fails is kept
    and tried again, `WidgetRequests.tries` times in all, then removed: the core's refusal (a
    task deleted elsewhere since) reads as any other failure, so a request is never tried for
    ever. A request run whose file could not be removed is never run again in that process, or
    a repeating task would move on twice. With the app not running, a request waits for its
    next start; the widget shows the task until then. Nothing is said: a widget's button says
    nothing, and no words of ours are needed. `WidgetRequestTests` (in `LumennaTests`, where the
    shared code compiles too) checks the round trip, the refusal and the retry. Not
    `allowedExecutionTargets` `.main`, which the newest macOS alone has and no one had seen work
    on a Mac, nor `openAppWhenRun`, which would bring the window forward for every Shortcuts run of
    Complete a Task as well.
  - **The watch** has no Today, so no Mark Done, and compiles no intents.
- **A widget tapped opens Today, the core's launch** (`widgetURL` of `LaunchURL.of("today")`,
  `lumenna://launch/<id>`, the scheme each app registers): the app sends `ShellEvent.launch`
  and follows the place the app left (`PlaceFollowing` on the iPhone's tabs and the iPad's
  columns, the Mac's window by itself, the watch pushing it), so where a launch goes is the
  core's.
- **A Mac extension is always sandboxed**, so its entitlements (`Widgets/Mac/Widgets.entitlements`:
  the sandbox and the group) are the same in both editions; only the App Store edition's store is
  there for Mark Done. Its Info.plist names the group as the apps' do (`LumennaAppGroup`).
- **Not checked by CI**, since it builds unsigned with no group: the widgets reading the glance,
  Mark Done in the widget and handed to the direct download's app through the group, the gallery's words, a complication
  on a face, and VoiceOver on each.

## The controls

Control Center's (`Widgets/ControlWidgets.swift`, `ControlModel.swift`), in each app's widget
extension beside its widgets: iOS 18, macOS 26 and watchOS 26 are the first with controls, so the
iPhone's and iPad's and the Mac's are behind `#available`, the watch's always there.

- **Which controls, and their words, are the core's** (`controls()`): each one's id is its kind,
  its gallery name and description read at run time. `ControlTests` compares them with the core's,
  so a control the core adds fails until it is built here. App Intents' titles are literals, so the
  test keeps New Task's intent titled as the core names it.
- **The Timer is a `ControlWidgetToggle` over the core's `TimerControl`** (`Device.timerControl`,
  `TimerControlState`): on while a timer runs, its title the core's name, beneath it the core's
  `detail` (the task whose timer runs, what a press would start, or why nothing would), the
  core's `action` as the press's hint, dimmed when `enabled` is false. VoiceOver hears the name,
  the action and the detail, joined. Its `SetValueIntent` ignores the value the system hands it
  and calls `Device.toggleTimer`: a press is always the core's toggle, never on to start and off
  to stop, which a control drawn a minute ago would get wrong.
- **Where the Timer reaches the store**, as the widgets reach it (`TimerControlSource`):
  - **The iPhone, iPad, App Store edition and the watch**: the store in the group, opened and let
    go of for each reading and each press (`GlanceStore`, the `0xdead10cc` rule), never made.
    After a press the glance is written again while the store is open, and every widget and
    control told. **The watch's control opens the watch's own group**, where its store is: the
    extension has the group's entitlement, and nothing in the store asks for the app's process.
    The watch app takes the change in at its next tick, like any other process's write, and only
    then hands it to the phone, so a press made with the app not running reaches the phone when
    the app next runs.
  - **The direct download**: its store is outside the extension's sandbox. The app writes the
    core's control into the group (`TimerControlFile`, `timer-control.json` through the core's
    `timerControlFile`) with the glance, after every change and on the minute, since what a press
    would start moves with the clock; the control reads that. A press is handed to the app as a
    request (`WidgetRequests`, `toggle_timer`, beside Complete a Task's), which the app runs with
    `Device.toggleTimer` within a second, or at its next start. Before the app has written the
    file, the control is dimmed with the core's store-unopened sentence.
- **New Task is a `ControlWidgetButton` opening the app on the core's `new-task` launch**
  (`NewTaskControlIntent`, `openAppWhenRun`, compiled into the apps as well as the extensions,
  since an intent that opens its app runs in the app). Not a URL: a control cannot open its app by
  a scheme of its own (Apple's engineers on the forums: only a universal link, which this app has
  none of). The intent hands the launch to `Launches`, the one way in from outside, which a
  widget's URL takes too and which holds a launch asked for before the app shows anything until
  it does (a cold start). Each app then sends `ShellEvent.launch`, follows the place the app left,
  and opens quick add over it if the app opened its line, which New Task's launch does: the
  iPhone's and iPad's scene, the Mac's window, the watch's root (quick add's sheet alone there).
- **Every write reloads the controls** (`GlanceFile.reload`, `ControlCenter.reloadAllControls`),
  as it reloads the widgets' timelines: after the app's `Core.changed`, after a press, and after a
  widget's Mark Done.
- **The watch's New Task complication** (`NewTaskComplication`, `LaunchComplication`): a widget
  kind of its own, `launch.new-task`, in the circle, the corner and the line, a plus and the
  core's name for the launch shortcut (`launchShortcuts()`), tapped to open the app on
  `lumenna://launch/new-task`, which goes straight to quick add, its line dictated or typed on the
  system's input screen. It is the launch shortcut's, not a glance, so it is none of
  `glance_widgets()` and reads nothing; `ControlTests` fails if the core's launch shortcuts lose
  New Task.
- **Not checked by CI**, as the widgets are not: the controls in the gallery and Control Center on
  each device, the press on each store, the direct download's hand-off, the complication on a
  face, and VoiceOver on each.

## The Apple Watch app

`apps/apple/Watch/`: SwiftUI, embedded in the iPhone app. Scheme `LumennaWatch`.

- **No Iroh on the watch.** watchOS allows low-level networking only inside an audio session
  (TN3135), and that includes sockets and even `URLSessionWebSocketTask`; the simulator does
  not enforce it, so a build that syncs there proves nothing. The core is built without
  `lumenna-ffi`'s `network` feature (the app's network a stand-in, `offline.rs`, saying the
  watch syncs through its phone), with bindings of its own (`Generated/Watch/App`, generated
  in `target/watch-host`).
- **A whole store, synced through the phone** (`Device.linkStart` and `linkTake` on the
  watch, `linkAnswer` on the phone: `lumenna-sync`'s `exchange`): every document in each
  message, over WatchConnectivity's message and reply. The watch starts every exchange, since
  its message wakes the iPhone app and the reverse needs the watch app open, and starts one
  whenever its store's heads move; the phone only nudges an open watch app when its store
  changes. Each answers on its app's own connection, so the phone's sync carries what the
  watch wrote to every other device. The watch is not in the device list: trust is the
  system's pairing of watch and phone.
- **Out of reach, what changed is handed on** (`lumenna-sync`'s `parcel`, `Device.linkParcel`
  and `linkUnparcel`, `Shared/LinkParcel.swift`): every change since the two last agreed, which
  each side records when an exchange settles (`note_agreed`, in `local_state`), queued with
  `transferUserInfo` (a file past 48 KB), which the system delivers when the two are next in
  reach, the other app closed or not. Both directions: the watch when the phone is out of
  reach, the phone when the watch app cannot answer. A newer parcel holds all an older one did,
  so one still waiting is cancelled, and a change taken twice is taken once. The watch holds
  the background task the system wakes it with until nothing more is pending.
- **Its store and glance file are in an App Group of its own**
  (`group.dev.emassey0135.lumenna.watch`: a group's container is one device's), moved there as
  the iPhone's is (`AppGroup.profile`), the glance written for the complications after the
  phone's changes and, by its heads, its own.
- **The watch schedules its own reminders**, from its own store, as the iPhone does
  (`ReminderSchedule`, re-armed at launch, on becoming active, after each change and on a
  background refresh every six hours). A phone's notification goes to the watch when the phone
  is locked and the watch worn, so each firing would show twice. Apple's answer (DTS, forums
  thread 765669): watchOS waits for the phone's notification to judge whether the two are one,
  and they are one when their `UNNotificationRequest.identifier`s match and they fire at the
  same moment; without a phone's, the watch shows its own after about 13 seconds. So the
  identifier is the firing's (key and occurrence, the same on every device) and the trigger a
  calendar one at the core's moment on both. Unverified on hardware.
- **The phone's forms are the watch's** (`Shared/Forms`: task, block, planning settings),
  with its own answers beside the others' in `FormParts.swift`: the iPhone's wherever its
  SwiftUI behaves the same, else the name always above the field and Save in the form. So
  `WatchCore` is the forms' `Core` (a typealias), its `app` the same `AppModel`, and the watch
  has an `Announcer`. Backups and exports stay on the phone: a watch cannot save or pick a
  file. `Shared/Clock.swift` and `Words.swift` are shared too; the watch's core is its own.
- **The watch's lists and day follow the phone's models** (`TaskListModel`, `DayModel`): what
  is listed, the row in hand put back on returning, the filter, and VoiceOver taken to a row
  the app moved, its swipes split by the phone's `AppRowActions`.
- **A row's actions are the app's, run once for every Apple app** (`AppModel.run`: `ActOn`
  naming the row, or one held in a side list; `AppModel.act` for the day's own, the details'
  task and a reminder list's own), their questions `AppQuestions`' (alerts and a
  `ChoicePicker` on iOS, sheets on the Mac and, one at a time, a `WatchAsker`'s on the watch,
  each sheet's closing without an answer its own cancel). Keys and menu commands run the row's
  action of that kind, and a row without one runs it and the app says why
  (`AppModel.refuse`); on the Mac a menu item that asks ends in "…". Settings screens and the
  priority picker are built from the core's descriptions (`Setting.kind`, `priorities()`).
- **The device list builds without Iroh** (`crates/host/src/devices.rs`): the watch is in
  no list and syncs with no device directly, so nothing is "this device" and a device's
  status says only its version (`device_status(…, direct: false)`).
- **A row offers its actions when tapped** where the phone has more swipe actions than a
  watch row holds (a block, a sitting, free time); a project, label or filter's are buttons
  at the foot of its list. The day's heading, tapped, offers the day's own actions, Plan the
  Day and Replan from Now opening the shared proposal screen in the asker's sheet. Completion
  is offered after the line is entered, beside the field, as BTSpeak offers it: the system's
  input screen takes no suggestions.
- **A text field is named explicitly** (`accessibilityLabel`): with text in it, its title
  gives way to the text.
- **A row owns its speech**: the title as its name and the rest as its value, set inside the
  `NavigationLink`; set on the link, watchOS makes the whole line the name.
- **UI tests launch with `LUMENNA_NO_SYNC`**: a simulated watch is paired with a simulated
  phone, whose store otherwise arrives. A watch list holds only the rows on screen, so tests
  scroll to what they look for: by the Digital Crown, a little at a time, in a list, where a
  swipe moves past it; by swipes in a form, where the Crown goes to the control in focus. A
  text field opens the system's input screen: type into its text view, then Done. A place's
  row is named with what is in it ("Inbox, no tasks").
- **watchOS 27 and later**, the first without 32-bit watches, so the core is built for
  `aarch64-apple-watchos` on stable and `arm64_32` (tier 3, nightly) never.

## CI

`ios-build` runs `build-for-testing` once, then an iPhone, the small "e" and an iPad run
`test-without-building` from it in parallel jobs that need no Rust, reporting without blocking;
`ios-smoke` runs the smoke set and `LumennaTests` on an iPhone, and blocks.
