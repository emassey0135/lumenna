# Roadmap

What Lumenna does not do yet, with the decisions already made about it, in the order it is
to be done. What is built is not listed: the code says how it works, and `CLAUDE.md` says
what is easy to get wrong. Packaging is the exception: each platform's packages are said
beside what they still lack.

Every client gets every feature. A platform may get it later, never a smaller version of it.

The order, and why:

1. **Safeguards**: what CI still lacks, with packaging.
2. **Reminders**: what waits on hooks and on the encrypted store.
3. **The planner**: one question kept for if it is needed.
4. **Smaller gaps**, between the larger pieces.
5. **The watches**: what only a real watch can show.
6. **Integrations**, once reminders and the planner exist for them to expose.
7. **Before others use it**: packaging, a relay of our own, installing the web client.
8. **The encrypted store and accounts**, which the web's reminders and syncing alone wait on.
9. **Only when needed**: performance and network extras.

## 1. Safeguards

- **CI** runs (`.github/workflows/ci.yml`), and so do unsigned release builds of every app
  (`release.yml`). Still to come: **signing and publishing** from them, under Packaging (Before
  others use it).

## 2. Reminders and time events

- **Hooks' time events on the same scheduler**, once hooks exist (Integrations). The
  planner's overrun warning already runs on it.
- **The web's reminders** wait on the encrypted store (below); today one open tab delivers.

## 3. Planning

- **A staleness question**, if a task becomes permanent furniture at the top: "this has been at
  the top for three weeks; is it real?", rather than a bigger formula.
- **When to start, apart from when it is due**: a task's one date says both "do it Monday" and
  "it must be done Friday", which Things (When and Deadline), OmniFocus (defer, planned and due),
  Taskwarrior (`wait`, `scheduled`, `due`) and Todoist (due and, since 2025, deadlines) keep
  apart. A start date that keeps a task out of the listings and the planner until it comes, a
  deadline the urgency ranks by, and the due date as now; quick add says each ("from monday",
  "by friday"). And **Someday**: a task kept but never offered until it is given a date, so the
  planner and What Should I Work On? leave it alone. The planner's rule for which date it
  reads is the core's; what each list shows by it too.
- **A view of several days**: the days ahead at once, each day's blocks and the tasks due or
  starting then, as Todoist's and Things' Upcoming and OmniFocus' Forecast are; today the day
  is shown one at a time (Go to Day). A listing, worded as every line is, so it reads as well
  as it looks.
- **Sections within a project**: named headings a project's tasks sit under (Todoist's
  sections, Things' headings, Reminders' sections), lighter than a subproject. A record of
  their own, with the care in `crates/store/CLAUDE.md` for records two devices make alone.
- **A countdown, and a timer outside the app**: a sitting's planned length counted down, with
  the core's words when it runs out (Tiimo counts down; TickTick has Pomodoro), beside the
  count up there is now; and a running timer shown where the system shows one: a Live Activity
  on the iPhone's Lock Screen and Dynamic Island, an ongoing notification on Android, its
  buttons the core's Pause and Stop.
- **Time reports**: the minutes every sitting logs, summed by project, label and week, with
  estimates beside them, as Todoist's productivity view and Taskwarrior's reports have. A
  listing of the core's, read as one.
- **A review**: every project looked at in turn at an interval of its own, as OmniFocus' weekly
  review is, each marked reviewed; the staleness question above is the same kind of asking.
- **Habits, from what is kept already**: a repeating task already says its cadence (`every 2
  days`, `every! 2 days` from when it is done, `every mon, wed, fri`), which stands in for a
  target per period, and never piles up overdue copies, being one task whose date moves on.
  Two things to add. A **streak**, counted from its completions (each records the occurrence
  it completed) and said in words in its details and its glance line ("done 12 times running",
  "missed Tuesday"). And, from the `postponed` tally, the planner asking of a task put off
  again and again, repeating or not: "this has been put off five times; make it less often, or
  drop it?", the staleness question's twin. No "3 times a week" of any days: a cadence says it
  well enough.
- **Energy, as the person rates it**: capacity for focus and for starting things, which ADHD
  and other conditions make uneven, not calories, so it is estimated, as a length is, never
  measured.
  - **A task's energy** beside its estimate: low, medium or high. Unrated is medium, so rating
    nothing changes nothing.
  - **A day's capacity**: a usual amount, and a check-in to change today's ("I'm low today")
    from a notification, a widget, the watch, or the app.
  - **When energy is good**: a block, or a time of day, marked as the person's best or worst.
  - **The planner uses them**, deterministically and only proposing, as everywhere: Plan the
    Day stops before the day's capacity is spent and puts demanding tasks in the good hours;
    saying one is low offers a lighter Replan from Now of the rest of the day. What it weighs
    is said in the proposal's reasons, in words.
  - Optional throughout: worth having with a few tasks rated, never a chore of rating them all.
- **An emoji on a task, a block or a project**, drawn beside its name, as Structured and Tiimo
  draw icons: any emoji, chosen with the platform's own picker (Win+. on Windows,
  Control-Command-Space on a Mac, the keyboard on a phone, Ctrl+. in GNOME), so no app keeps a
  set of icons of its own. The core keeps one emoji (a single grapheme it checks) and words it
  nowhere: **left out of speech and braille**, since it is chosen to be seen and its name read
  aloud before every title is noise; the details may name it. The CLI, Emacs and BTSpeak show
  none.

## 4. Smaller gaps

A day or less each, for between the larger pieces.

- **A weekday of the month in a repetition**: "every first monday", "every last friday" and
  "every month on the last friday" are misread (the line keeps "every" or "on the last friday"
  in its title), though the model holds them already: a task's repetition is an RRULE
  (`BYDAY=1MO`, `BYDAY=-1FR`). The parser's (`crates/parse`), with the words to say them back.

### Completion

- **BTSpeak**: Control-L (dots 7-8 with L) inside the input dialog completes at the cursor,
  through the dialog's own `hotkeys` (no subclass needed); no dialog or device key claims it.
  Tested against the stub only: try it on the device, in computer and in literary braille.

### Each app

- **iPad**, found by its UI tests:
  - In the simulator, the first tap on a button of an alert with an untouched text field
    only ends editing (Skip when assigning a task). Try it on a real iPad; VoiceOver, which
    activates rather than taps, is not affected.
  - ⌘F never reaches the app in the simulator; try it on a real iPad too.
  - On the 11-inch iPad simulator in portrait, the first ⌘, after launch opens the system's
    Settings app instead of ours (the menu bar does replace that item); try a real iPad.
  - Since the lists became SwiftUI, a test under load (CI, or three simulators at once) now
    and then tapped a sidebar place a second after showing the sidebar, and the app never said
    its animations were done: every step after waited a minute. The app was idle (a spindump
    showed no work). UI tests now launch with animations off (`LUMENNA_NO_ANIMATIONS`), and the
    first full run after had no stall; what was left animating was never found, and is worth
    finding if anyone sees the sidebar misbehave.
- **Android**: pairing on the local network untried on a real phone (the emulator's NAT
  passes no multicast): `LumennaTest#pairingOnTheLocalNetworkNeedsNoCode` with
  `INSTRUMENT_ARGS="PAIR_LOCAL=1"` beside a `lum pair` on the same network. The keyboard
  commands and the wide layout are tested in the emulator, not yet with TalkBack and a real
  keyboard.
- **GTK**: "expanded" said twice on first focus of a row with subtasks; the shortcuts' X11 grab
  unchecked with Orca under a real X11 window manager.
- **Braille apart from speech on Windows**, only if braille users there ask: only through JAWS
  scripts or an NVDA add-on, since UI Automation gives a control one name.
- **Screen readers not yet tried by a person**: JAWS, Narrator, ChromeVox, Emacspeak and
  speechd-el. NVDA (Windows and the web), Orca, VoiceOver on the Mac and iPhone, TalkBack,
  Emacsvox and the BTSpeak have been.

## 5. The watches

- **watchOS**, in this order:
  - **On a real watch**: TN3135 is not enforced in the simulator, nor is memory, and
    Automerge loads whole documents.
  - **Later, if it matters**: syncing far from the phone through a relay of our own that the
    watch polls over plain HTTPS, the one networking watchOS allows.
- **Wear OS**, on a real watch: rotary scrolling with TalkBack, and pairing over the network.
- **Toolchain**: unpinned.

## 6. Integrations

Each is a thin adapter over the command surface.

- **Control Center controls** (iOS 18 and macOS 26 `ControlWidget`s, the watch's too): start and
  stop a sitting's timer, and New Task, as buttons over the App Intents already built (every
  assistant operation is one, `apps/apple/CLAUDE.md`), their names the core's.
- **A URL scheme**, `lumenna://`, on every platform, for Drafts, Scriptable, Keyboard Maestro,
  Alfred and any app's x-callback-url:
  - **Where:** Android's intent filter, Windows' protocol handler (the installer's and the
    MSIX's), the desktop entry's `x-scheme-handler/lumenna`, and the web's `web+lumenna`
    (browsers take only `web+` schemes). What a URL means and does is the core's, a records
    module beside `launch`, so every platform only hands the URL over; `run/` takes its values as
    text the way Tasker's actions do (`records::automation`). The Apple apps already
    answer `lumenna://launch/<id>`, which their widgets open; `open/` takes its place.
  - **Without a token**, what is harmless and undoable: `open/<launch id>` (the core's launch
    targets: `today`, `task:<id>`, …) and `add?text=…` (one task per line, in the quick-add
    grammar), answering `x-success` with no data.
  - **With a token**, every operation of the assistant table, as `lum mcp` has them
    (`run/<operation>?<parameter>=…`), its record answered to `x-success`. The token is this
    device's own, separate from `lum mcp`'s, made, shown and renewed in Settings, and this half
    is off until it is turned on. An operation that asks first (erasing permanently) is never
    run by a URL, token or not.
  - **Why a token:** x-callback-url names no caller (iOS does not say which other developer's app
    opened one), so a token is the only way to tell one apart, as Things and Bear do, and
    `x-success` sends results wherever the caller says.
- **Gemini through App Functions**: every assistant operation is an App Function
  (`apps/android/CLAUDE.md`), but Gemini calls only the apps in Google's early access programme
  (a private preview with trusted testers as of May 2026). Register interest; until then a
  function is checked by `adb shell cmd app_function`, not through Gemini.
- **Widgets, tried as a person gets them**: every platform's are built from the core's glance and
  `glance_widgets()` (Now and Today), and what CI cannot check is left by hand: Apple's on a
  signed build (the group's glance read, Mark Done in the widget on the iPhone and the App Store
  edition, and for the direct download handed to the app through the group's `requests/`,
  taken within a second while it runs and at its next start otherwise, a complication on a face
  and in the Smart Stack), the Windows board drawing the cards and pressing Mark Done (`apps/windows/CLAUDE.md`),
  and VoiceOver, TalkBack and Narrator on each.
- **A PowerShell module**: a script module (`.psm1`) in PowerShell over `lum --json`, for
  Windows PowerShell 5.1 and PowerShell 7 on every platform, and the same language as Windows'
  hooks. Unlike `lum`, it gives:
  - objects rather than text, so the pipeline works: `Get-LumennaTask -Filter '@calls' | Complete-LumennaTask`
  - verb-noun cmdlets with help
  - `-WhatIf` and `-Confirm` on what removes, erasing permanently confirming by default
  - errors as error records, and completion through `lum`'s own.

  It offers the assistant table's operations, checked by a Pester test as App Intents and App
  Functions are. Enough for scripting Windows: WSH and COM stay decided against (VBScript is on
  its way out of Windows), and what cannot run PowerShell runs `lum` or opens a `lumenna://` URL.
- **`lum mcp` without a command line.** Built: stdio and Streamable HTTP (`--listen`), tools,
  resources and their subscriptions, `--writes`. Still to come, so a person who never opens a
  terminal can add it: for Claude Desktop, a bundle (under Packaging); and for Claude on the web
  and phone and Meta's Muse, which reach a server from their own cloud, an HTTPS address and
  OAuth (a bearer token cannot be typed in there), each client approved by the person. Two
  ways, decided, neither letting whoever runs them read the store:
  - **Forwarded to the person's own device**, for someone with one always on: a relay at
    `lumenna.app` gives each person an address and forwards the connection by its name, still
    encrypted, to `lum mcp --listen` on their device, which holds the certificate and is its own
    authorization server. The relay sees that connections happen and how much passes, never what.
    A tunnel of their own (Tailscale Funnel) works too.
  - **Confidential computing**, for someone with none: `lum mcp` in an attested confidential
    VM (AMD SEV-SNP or Intel TDX, or AWS Nitro Enclaves), its memory unreadable to the operator
    and the host, its build reproducible and its measurement published. The person's own app
    checks the attestation before it hands the enclave the key to their store, so only that
    published code ever reads it. It is a device that syncs like any other, and keeps nothing
    once stopped but what syncs. Claude's or Meta's cloud cannot check the attestation itself, so
    what protects the person is that their app did. The assistant sees what it asks for, as with
    any cloud assistant.

    Call it the *enclave peer*. Being a device that is always awake, it is more than an MCP
    server: it is the always-on peer for someone without one (section 7), the sender of
    the web client's reminder wake-ups, at the moment each is due (section 8), and a peer every
    device can reach to sync without two of the person's own being awake at once. It keeps its
    copy of the store in the encrypted store, so a stopped or moved enclave loses nothing and
    the operator still holds only ciphertext.

    Alexa+ reaches either the same way, as an MCP client from Amazon's cloud, once its MCP
    Toolkit is open to more than chosen partners (in preview for partners alone, in the United
    States, as of July 2026). It asks a round trip under 500 ms, which the enclave meets more
    surely than a forwarded home connection. No classic Alexa skill: Amazon's new tools are all for
    Alexa+, where a skill is invoked directly only if Amazon picks it after an evaluation form.
- **MCP Apps** (the extension, official since January 2026; Claude and ChatGPT draw them): the
  whole app inside the assistant's conversation, so a person who asked for today sees the day
  and acts on it there. Decided:
  - **The Crux app runs in `lum mcp`**, as it runs in `lum rpc` for Emacs: each MCP App on
    screen is one of its screens (`Event::On`). The page is the web client's drawing layer,
    the same React Aria components over the same `ViewModel`, fed each turn as a tool's result
    instead of by the WASM core; a key or a click goes back as one tool (an `app_event(screen,
    event)`, offered to the page and not the model), which answers with that screen's turn.
    What is announced goes to the page's live region, as on the web. The page carries no WASM
    and no store, so the store never leaves the device and the snapshot never enters the
    assistant's page.
  - **One page for every tool, opened at a place** by `records::launch` (`today`, `task:<id>`,
    a proposal): every view and form is there, because the whole app is, and the person moves
    anywhere from where it opened.
  - **The model is told what the person did** (`updateModelContext`, the core's announcement),
    so "move it to tomorrow" after a change in the page means the row just changed.
  - **Proposals are the first use**: the model asks for Plan the Day or Replan from Now, the
    page shows the whole proposal, and the person accepts it there, as the planner's rule asks.

  Before building past a first page: whether Claude asks the person to approve each tool call
  the page makes (if it does, a key press each is unusable); whether Claude's page gives a
  screen reader the iframe's focus and live region (check VoiceOver and NVDA first); and the
  sandbox's limits on a bundled page. A page is not pushed what changed: it asks for its
  screen's turn when shown and on a slow timer. Through cloud MCP every event crosses the
  assistant's cloud, so moving through a list stays in the page and only a write or a new
  place makes a round trip.
- **Hooks**, `lum hook add|list|test|rm`:
  - **Reactive, never interceptive**: a change can arrive already committed from an
    unreachable device, and convergence cannot be vetoed. Hooks fire after a change is
    durable, see before and after, may make further changes, and cannot report failure to
    the person who made the change (log it; show it in sync status).
  - Change events: task created, completed, uncompleted, updated, deleted; assignment
    created, removed; timer started, stopped; block edited. Time events, from the scheduler:
    block started, ended, upcoming(offset); day start, end; task due. A time event past its
    hook's maximum lateness (a few minutes by default) is dropped and logged: Do Not Disturb
    40 minutes late is wrong.
  - `Hook { event, filter, action, max_lateness_secs }`, the filter in the filter language.
  - **Local only, never synced**: a hook running `ssh` belongs to one machine, and synced
    hooks would be remote code execution for anyone who can write the document. On one
    machine only the sync-lock holder runs them.
  - The event is JSON on stdin in `lum --json`'s shape, common fields in the environment.
    argv is run directly, without a shell unless asked. Windows: `pwsh -NoProfile
    -ExecutionPolicy Bypass -File` (without Bypass, unsigned scripts fail silently).
    Android: a broadcast, `com.lumenna.EVENT`, for Tasker and the like.
  - With nothing resident, hooks run late over the backlog; there is no ordering across
    devices; an always-on peer is the best host.
- **Calendar import**, read-only first (so the planner knows about real meetings), through
  `ExternalRef` (EventKit, CalDAV, ICS URL), which exists in the model only; `lum calendar
  add|list|rm`. Read-only must be enforced on edit. Two-way sync is a separate project.
  Importing an `.ics` file first, which Google Calendar exports and which needs no API.
  Google Calendar's API is open to any project, but its scopes are sensitive: an app used by
  the public needs Google's OAuth verification (no paid audit), and an unverified one is
  limited to 100 people and warns them. PKCE serves a desktop app that cannot keep a secret.
- **Imports from other systems**, each from what that system can actually export:
  - **Todoist** first, through its public API and a personal API token the person copies from
    Todoist's settings, so nothing needs registering (check the current API version when
    building; its priority 4 is P1); its CSV backups (a ZIP of a CSV per project, completed
    tasks left out) for someone without a token.
  - **Taskwarrior**, from `task export`'s JSON: `wait` and `scheduled` to the start date above,
    annotations into notes, its user-defined attributes kept in the notes as `name: value`
    rather than dropped.
  - **OmniFocus**, from its CSV or its plain-text (TaskPaper-style) export: defer dates to the
    start date, contexts and tags to labels, projects with their folders.
  - **todo.txt**, both ways: import a `todo.txt` and `done.txt` (priority `(A)`-`(D)` to P1-P4
    and lower ones to P4, `+project`, `@context` to labels, `due:`, other `key:value` kept in
    the notes), and export one, so Lumenna's tasks reach any plain-text tool and back.
  - Then Jira, Linear and GitHub, which `ExternalRef` was shaped for.
  - **Not Things 3**: it has no export of its own, only AppleScript on a Mac, so an import
    would read another app's private data; revisit if Cultured Code ships one.
- **Capture from other apps**: an iOS and Mac share extension and an Android share target
  that take a page's or a selection's text as a quick-add line (read back before it is added,
  as quick add always is), and Windows' share target for the MSIX. Email to a task stays under
  "Only when needed".

## 7. Before others use it

- **Packaging**, platform by platform. `release.yml` builds every app unsigned, each checked
  as a person would get it; nothing is signed or published yet. A command-line user has no app
  to prompt for updates, so packages matter most for `lum`.
  - **For all of them**: a version tag making a GitHub Release that carries the builds; the
    domain, `lumenna.app` (its DNS on Cloudflare), with the site and the web client on
    Cloudflare Pages; one appcast there (`lumenna.app/updates/…`), each entry signed with an
    EdDSA key, read by Sparkle, WinSparkle and the AppImage's updater; a `lumenna` alias for
    `lum`.
  - **Windows.** Built, for x64 and arm64: the Inno Setup installer (per user, `lum` on the
    path, an uninstaller that asks whether to delete the data), an MSIX (a profile of its own,
    opening at sign-in through a startup task, `lum` through an alias) installed and tested in
    CI, and `lum` alone as a zip. Still to come: winget manifests, one for the installer and one
    for `lum` alone; WinSparkle in the installed app; the Store, with Partner Center's identity
    in place of CI's stand-in (`MSIX_*` variables) and a submission; signing the installer and
    `lum` (Artifact Signing), without which SmartScreen warns; once signing exists, a package
    with an external location (a sparse package, signed, registered by the installer) giving the
    installed app the identity the Widgets board needs, which only the MSIX has now; and trying by
    hand what CI cannot: the packaged app's startup task, toasts and widget on the board (drawn
    in both sizes, kept up to date, Mark Done), the Jump List in both installs, the packaged and
    installed apps side by side, and the uninstaller's question.
  - **macOS.** Built: the app for both architectures on a disk image, `lum` inside it, and `lum`
    universal alone as a tarball. Still to come: Developer ID signing and notarization, with the
    paid account, without which the app opens only once allowed in System Settings; Sparkle; a
    Homebrew tap, a cask for the app linking the `lum` inside it, and a formula for `lum` alone
    (`brew services start lum`); and trying the login item by hand.

    Two editions, decided: the **direct download** (Developer ID, Sparkle) as it is now,
    unsandboxed, its store in `~/Library/Application Support/lumenna` shared with the `lum`
    inside it and with Emacs over its socket; and a **Mac App Store** edition, sandboxed, its
    store in its App Group container, with no `lum` inside (a sandboxed `lum` could not open the
    files named to it, nor be put on the path by the app). Someone using the App Store edition
    installs `lum` apart (Homebrew, the tarball) and pairs it with the app like another device,
    so the two sync on the Mac rather than share a file; Emacs goes through that `lum`. The
    App Store edition updates through the store, needs the sandbox's network entitlements for
    Iroh and discovery, and its global shortcuts and login item checked under the sandbox.
    Widgets read the glance file every Apple app writes to its App Group, in either edition
    (Integrations). Built: the App Store edition as a configuration of the Mac target (scheme
    `LumennaMacAppStore`, `apps/apple/CLAUDE.md`), its entitlements, store, socket and backup
    folder bookmark. Still to come: signing it with a provisioning profile for its group,
    trying it signed under the sandbox (pairing, discovery, the socket, the login item, the
    shortcuts, a chosen backup folder after a restart), App Store Connect and the review.
  - **iPhone and Apple Watch.** Built: the iPhone app with its watch app, for devices,
    unsigned. Still to come, with the paid account: signing, App Store Connect, TestFlight, the
    privacy manifest and the export-compliance answer.
  - **Android and Wear OS.** Built: App Bundles and APKs, unsigned. Still to come: an upload
    key and Play App Signing; the Play Console account and the closed test a new personal
    account runs before production; the Wear app in the same listing.
  - **Linux.** Built, for x86_64 and arm64: the GTK app as an AppImage, GTK and libadwaita
    inside it (glibc 2.39 or newer), and as a Flatpak bundle (GNOME 51, opening at sign-in
    through the Background portal), the AppImage offering once to add itself to the applications
    menu, its entry following it when it moves; and `lum` alone as a tarball, built on Debian 12. Still to come: the Flatpak from a repository of our own, signed with the
    project's GPG key (Flatter, on GitHub Pages) and offered by `.flatpakrepo` and
    `.flatpakref` files, so updates come through `flatpak update` and the software centres;
    not Flathub, which refuses manifests a language model wrote and asks that generated code be
    disclosed; the metainfo's release entries and screenshots; the AppImage's own updater; the AUR; and trying on a real desktop what CI
    cannot: the portal's question, the tray, the shortcuts, Orca, and finding devices from
    inside the sandbox.
  - **BTSpeak.** Built: the app and its core as a tarball, on Debian 12 for arm64. Still to
    come: the App Store's package, below.
  - **The web client.** Built: the static site. Still to come: serving it, from Cloudflare Pages.
- **BTSpeak's App Store**, Blazie's, once it ships (nothing of it was in `btspeak-dev` on
  2026-10-23): an ordinary Debian package for arm64 (`Section: btspeak-apps`), named
  `lumenna`. `/opt/lumenna` holds the app, its core and `lum`, the background service's unit
  among them; the manifest is `/usr/share/btspeak/apps/lumenna.app` (`Self-Voice: yes`, its
  `Help` the app's own topics); the profile stays in `~/.local/share/lumenna`, and nothing goes
  under `/BTSpeak`. Built in CI on the oldest Debian release the devices run, since the core
  is an extension for that release's Python and links its glibc. Submitted to Blazie with the
  catalogue's note (Title, Category, `Uses: network`); updates then reach every device that
  has it. Not BT Code's Code Store: that is for source anyone with BT Code can change and send
  back, and a core built for one Python breaks when a release moves to another.
- **A relay of Lumenna's own**, or n0's paid ones (shared or dedicated relays through Iroh
  Services). n0's public relays are for development and hobby use, rate-limited by limits it
  does not publish, and every device away from its own network syncs through a relay, every
  browser always. `node.rs` uses n0's: a relay of our own (`iroh-relay`, which sees only
  encrypted traffic) needs a name of its own and TLS, and a device on an older build, which
  knows only n0's relays, must still reach one on a newer build.
- **A Claude Desktop extension** (`.mcpb`), so `lum mcp` is added by opening a file. It carries
  `lum` itself, run by a path inside the bundle, the format's way: a macOS universal binary and
  a Windows one in one file (its overrides choose by platform, not processor, so Windows on ARM
  runs the x64 one). It opens the default profile, the apps' own. Its "Allow changes" check box
  reaches `lum mcp` as an environment variable (`LUMENNA_MCP_WRITES`), since the format passes a
  setting's value but cannot add a flag only when one is set. The Mac and Windows apps' "Add to
  Claude Desktop" makes one that runs the `lum` they ship, so it updates with the app; the
  bundle carrying its own is for someone without the app, and updates only when reinstalled.
- **An always-on peer**, documented: `lum daemon` on hardware the person controls. A VPS
  works, but say the provider can read the data, and make no encryption claim: unattended
  boot and provider-proof encryption conflict. What resolves that conflict is attestation, not
  encryption at rest: the enclave peer (under Integrations) is the always-on peer for someone
  with no hardware of their own.
- **Background Sync for the web client**, once the encrypted store exists. A service worker
  cannot open the store (OPFS's synchronous handles, which `sahpool` needs, are a dedicated
  worker's alone, and a service worker can start none), and Iroh needs a peer awake at the same
  moment. The encrypted store needs neither: syncing with it is plain HTTPS to something always
  there. So the store's worker writes each change, encrypted as it commits, to an outbox in
  IndexedDB; on a `sync` event the service worker sends the outbox with `fetch` and fetches what
  is new into an inbox, which the store takes in the next time a tab opens. The account key is a
  non-extractable WebCrypto key in IndexedDB, which a service worker can use. Working out a
  reminder in the service worker (the push wake-up below) further needs the store readable
  there: OPFS's asynchronous API, which a service worker has, into an in-memory copy. The
  enclave peer changes none of this: it is always awake, but the service worker still cannot
  hold an Iroh session, so it reaches what the enclave wrote through the encrypted store.
- **An IndexedDB fallback for the web client**, only for a browser without sync access handles
  someone needs: Chromium, Firefox and WebKit all run the suite in CI.

## 8. The encrypted store and accounts

Storage for change chunks that are encrypted before they leave a device, addressed by hash
and served on request. It never runs Automerge and never holds a key, so it is encrypted at
rest by construction. Clients find what they lack by exchanging hash lists. It must store,
not just forward: forwarding helps only when two devices are already awake. It is what a web
client alone syncs through, what optional hosting the operator cannot read would be, and
perhaps where attachments live. Call it the *encrypted store* (`lum store`), never a "blind
store" or a "relay" (Iroh's word). It must be cheap to run for many people. Meanwhile, the
sync layer must gain no assumption that makes a hash-addressed, opaque-chunk transport hard
to add.

**Keys.** One `account_key`, 32 random bytes made once, encrypts chunks. The password is
never the key: changing it would mean re-encrypting every append-only chunk and breaking
other devices until done. The key is stored in wrappers, each `{kind, salt, kdf params,
wrapped key, label}` with its own salt and KDF, so each credential rotates alone and only the
password is stretched:

- **Password**: Argon2id, about 64 MiB, t=3. Required: joining with no other device
  reachable needs something memorable.
- **Recovery phrase**: 12 BIP39 words (128 bits) through HKDF, made at setup and never
  offered again, for when every device is gone. A phrase, not a QR code: it can be read
  aloud, copied or saved to a file (offer all three), and its checksum catches
  transcription errors. Never shown or accepted in a browser.
- **Key file**: a random wrapping key through HKDF, for unattended setups. Not the account
  key on disk, so a leaked file can be revoked. `label` tells them apart.

**Authenticating.** `auth_value = HKDF(account_key, "store-auth")`; the store keeps only its
hash. Derived from the key, not the password, so it never changes and a password change is
invisible to the store. `account_key` and `auth_value` live in `local_state` and never sync;
the password is never kept. Joining with a password: fetch the wrappers by account name
without authenticating, run Argon2id, unwrap the key, derive `auth_value`, authenticate,
fetch chunks.

**The key never goes in a document**: Automerge history is permanent, so it would survive any
rotation. It reaches a new device over the pairing channel after the words are confirmed, or
by unwrapping from the store.

**Joining by account.** `lum store add <url>`. A device that signs in can read and write
through the store and learns every device from the device list, then dials the first one
awake for the ordinary word comparison. Until then it is pending: not in the device list,
no direct connections. An unexpected prompt elsewhere is the alarm that someone enrolled with
the password. A store is never paired with; it has no device identity.

**Rotation.** A credential (new password, key file or phrase) is re-wrapped with a fresh salt
and its wrapper replaced: cheap, local, needing no old password, from any enrolled device;
the normal recovery path, offered before the phrase. Rotating `account_key` itself is what
truly revokes a stolen device, and means re-encrypting everything and moving every other
device to the new key: **unsolved**. Keep saying which is which: unpair a device you
replaced, rotate the key for one that was stolen. A lost `auth_value` makes the store
disposable: a device with the key makes a new account and uploads again.

**Accounts.** A plain username the person chooses. Wrappers are fetchable without
authenticating, so offline guessing is possible regardless; a random identifier would only
get written down beside the password. No email: the store never sends mail, a reset is
impossible by construction, and an address is personal data for nothing. Billing, if any,
stays with the payment processor.

**Passwords.** At about 10⁶ guesses a second offline: 40 bits takes days, four words decades,
five words about 10⁵ years. Offer a generated five-word passphrase, and make accepting it the
easy path. A chosen password has a floor, estimated zxcvbn-style and stated in words ("about
six days to guess"), never a coloured bar. Rate-limit wrapper fetches per account.

**Say plainly**, once, at setup: against a hostile operator, the password is the only
protection. "We cannot read your tasks" is true; "we learn nothing" is not (identity, chunk
counts and sizes, timing, which devices connect when).

**Beside the enclave peer**, it stays: the two do different work. The encrypted store holds;
it trusts no hardware, runs no code over the data, and costs little per person, so it is what
everyone syncs, backs up and joins through. The enclave peer computes (MCP, reminders, being
awake), costs a running VM per person, and asks the person to trust the attestation chain of
AMD, Intel or AWS. It keeps its state in the encrypted store and gets the account key only
after its attestation checks. Someone who wants nothing in the cloud that can read their tasks
uses the store and no enclave.

What the web client gains from it:

- **Reminders**: only as an installed PWA, through a classic service-worker push (not
  Declarative Web Push, which needs plaintext at the sender). The wake-up carries no
  content: the service worker syncs, works out what is due, and shows it, within Chrome's
  budget for pushes that show nothing. The sender is an awake peer that can decrypt (the
  person's own, or the enclave peer, which works out from the store when each reminder is due
  and wakes the browser then), or, for someone with neither, the encrypted store sending a
  content-free wake-up on a schedule the person chose to give it (off by default, the
  trade-off stated: the schedule tells the operator when reminders fall).
- **Keys**: the account key as a non-extractable WebCrypto key.
- **Verifiability**: subresource integrity and a published bundle hash, and a one-time plain
  notice at sign-in that a web client is harder to verify than a native one.

## 9. Only when needed

- **Watching the WAL** (inotify, FSEvents, `ReadDirectoryChangesW`) instead of polling
  `data_version` every second; keep the poll where watching is unreliable, such as network
  filesystems.
- **Closed years**: a finished year's document is history. Leave it out of the default sync
  set and fetch it on demand; the always-on peer keeps every year. Nothing is ever
  discarded: pruning history would break offline replicas.
- **Sync status**: live reachability per device. `Device.last_seen` is written at pairing
  and never again; status comes from this device's own record, so leave the synced field.
- **Pairing across networks by a spoken code**: the cross-network code is the pairing key,
  fine to paste and too long to read aloud. An optional rendezvous service could hold it
  under a short code; the words still confirm both sides, and pasting must keep working.
- **Unattended provisioning**: a long pre-shared token in a headless device's config file in
  place of the words. Everything a person does still goes through the words.
- **Wake-up push** (optional, can be turned off): a silent APNs push when a device changes
  something, on top of background refresh, never instead; Web Push with VAPID for an
  installed web client. The push server learns timing, so say so, and batch with jitter. A
  sync wake-up needs no data; a reminder does, which is why the encrypted store can never
  send reminders.
- **The SQLite read model**, only if queries become slow or full-text search is wanted: a
  rebuildable projection (flattened tasks with depth and path, date indexes, FTS5, block
  occurrences for a window), recording the Automerge heads it reflects, filled in the same
  transaction as the changes, producing the same core structs. Filters would then compile
  partly to SQL, with the rest evaluated in memory over what remains.

- **An Outlook add-in** for turning an email into a task, only if email import is wanted.
  Office add-ins are web pages Office loads from a URL and cannot reach the local socket, so
  it would need a localhost endpoint in the desktop apps.

## Open questions

- **Rotating `account_key`** after a device is stolen (above).
- **A visual, proportional timeline**, Tiimo or Structured style, beside the list every day
  view is now: perhaps the most valuable addition for people with ADHD.
- **Location-based reminders**: mobile only (iOS caps regions at 20, so a rolling window;
  Android's Play review makes it a later submission; impossible on watchOS). Places entered
  by "use my current location", not address search.
- **Attachments**, which would rest on the encrypted store as a hash-addressed blob store.
- **Templates** for tasks and projects; try "duplicate this subtree" first.
- **Paid hosting** of an encrypted store, and whether any app is ever paid (decide before an
  App Store launch).
- **The licence and app stores**, to decide before accepting a contribution from anyone else.
  Lumenna is AGPL-3.0, and Apple's App Store terms conflict with the GPL family; a sole
  copyright holder can publish there regardless. With other contributors there are two ways:
  a contributor agreement, as Signal asks of its contributors to publish its AGPL iOS app, or
  an added permission for app stores (the AGPL's section 7), which, added later, needs every
  contributor's consent, as VLC's move to a dual licence did.

## Decided against

- **Teams** (decided 2026-10-25): shared projects, assignees and comments, as Todoist has.
  Lumenna is one person's tasks on that person's devices; the trust boundary (the device list),
  sync, erasure and the planner all assume it, and teams would multiply every one. Imports
  bring someone's tasks over from a team tool; they do not make Lumenna one.
- **Calendar export** (decided 2026-10-25): every block mirrored onto a calendar would bury the
  meetings there, and reminders already do the alerting. Calendar import, for the planner to
  know real meetings, stays (Integrations).
- **Contexts** (decided 2026-10-25), Taskwarrior's or OmniFocus' focus: one filter applied
  everywhere until switched off. Saved filters are views a person chooses, and a block's task
  filter is a context bound to the time it applies; a hidden filter left on would make every
  list quietly show less than it says.
- **A timeline of every field's history** (decided 2026-10-21): a task's history counts the one
  fact the records forget, how often its due date was moved later; the rest says nothing a
  person acts on.
- **Reordering tasks among siblings** (decided 2026-10-20): task lists are most urgent
  first, or in the order added (`task-order`, `sort:`), so a task has no place of its own to
  move to. Projects, labels, saved filters and a block's sittings are reordered.
- **AppleScript, WSH/COM**: Shortcuts, PowerShell and `lum --json` cover scripting.
- **A hosted MCP endpoint that can read the store**: cloud MCP is forwarded to the person's own
  device or run in the enclave peer (under Integrations), never on a server its operator reads.
- **Interceptive or synced hooks**, and hooks on iOS, watchOS, Wear OS and the web.
- **A language model in the planner, optimal packing, a strategy menu, automatic
  carry-forward.**
- **A daemon that delivers reminders, or that coordination requires.**
- **Pruning Automerge history**; saved per-peer sync state.
- **A second pairing mechanism** (a PAKE, short typed codes): one mechanism for everything
  a person does.
- **An email or random identifier for accounts; a QR code for recovery.**
- **Comparison operators in filters.**
- **A TUI** for `lum`.
- **A projected view for display glasses**: Gemini, through App Functions, serves glasses
  as it serves the phone; what glasses add to an app like this is speech.
- **Scripts for braille display keys**: the apps' keyboard commands already reach them
  through each screen reader's keystroke emulation, and a key that is hard to send from a
  braille keyboard is fixed in the app, for everyone.
