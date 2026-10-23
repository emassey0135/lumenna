# The web client

Read when working in `apps/web/`. The root `CLAUDE.md` holds what every part of Lumenna shares; this is what is easy to get wrong here.

`apps/web/`: TypeScript, React and React Aria Components over the core compiled to
`wasm32-unknown-unknown` (`apps/web/core`, crate `lumenna-web`), exported through BoltFFI as
Android's is. `npm run core:dev` (`scripts/build-core.mjs`) has BoltFFI build it, with the
web's own Cargo profile through `cargo_args`, a `cdylib` alone (beside an `rlib` it was not
whole-program optimized: 10.6 MB became 9.2 MB; built by `cargo rustc` outside BoltFFI, the
crate's own exports were left out), and pack it into `src/core/` (not committed): the
module, its loader, the wasm-bindgen half its dependencies need (BoltFFI runs a wasm-bindgen CLI
matching `Cargo.lock`'s, building one if none is on `PATH`), and the TypeScript types, which
need `@boltffi/runtime` at the crates' version. The `web` profile is `opt-level = "s"`: `"z"` was
8% less to download and half again as slow, and wasm-opt is off, since its smaller module was
larger once compressed, which is what a visit fetches. Tests are Playwright over the accessibility tree with axe:
`npm test`; `LUMENNA_NETWORK=1` adds pairing over n0's relays, `LUMENNA_LUM=<lum>` pairing
with `lum`.

- **The same store, in OPFS.** rusqlite builds for the browser on `sqlite-wasm-rs`; the
  `sahpool` VFS needs sync access handles, which only a dedicated worker may use, so the
  core runs in `src/worker.ts` behind Comlink. No WAL there.
- **One tab owns a store** (a Web Lock named for the profile). Opening is idempotent per
  worker, because React's development mode runs effects twice.
- **Records are typed as BoltFFI writes them**, from the crates' own: fields in camelCase,
  data enums as tagged unions (`{ tag: "Open", value0: place }`), absent values `null`, and
  enums with no data as numbers (`ActionKind.MarkDone`), compared by their constants, never
  by value; `core.ts` re-exports the constants. A form field keeps the core's name for itself
  (`accepts_tasks`), which its edit sends back, so a form reads its value by BoltFFI's spelling
  (`fieldOf`). An event the page sends is `ShellEvent` (`lumenna_app::shell`), Android's too.
- **BoltFFI asks every export to be `Send`**, and what only a browser holds (the store's `Rc`
  state, its promises) is not: `Local` (`core/src/app.rs`) says it is, which a browser's one
  thread, built without atomics, makes true. The page's bundle takes only the enumerations'
  constants from the bindings (`treeshake` in `vite.config.ts`); the worker runs them whole.
- **Spans cross as UTF-8 bytes**; the worker converts to the UTF-16 offsets an input counts.
- **A React Aria tree builds its collection in a pass of its own**, so rows can lag the
  data by a render: focus after a change waits for current rows (`TaskList`).
- **Quick add is a combobox completing mid-line**, its filter off (the core decides what
  fits), and Down waits for the offer for where the cursor is *now*. Every other line in the filter
  language completes the same way (`Completing.tsx`): the list's filter, a saved filter's query
  (`Asking::syntax`) and a block's Tasks from (`FormField::syntax`), on Down or Ctrl+Space, never
  by itself, asking the core at the keystroke (`App::complete`), the list opened once its
  candidates have rendered (opened as they were set, it opened on none).
- **What cannot be undone is an `alertdialog` with Cancel focused first**, never `confirm()`.
  A refused answer stays in its dialog.
- **A row's action runs after its menu has closed** and focus is back on the row, so a
  dialog it opens returns focus there.
- **Every menu, key and button sends a record's own `actions` to the app**, and nothing waits
  on the answer: the question the app opens is drawn from the screen (`Questions.tsx`: a
  confirmation or a choice of answers as an `alertdialog`, a line of text with the app's
  refusal at the field, a pick from the lines the worker words), and answering it is another
  event. Space and Delete run the row's action of the matching kind. The forms are drawn from
  the app's: the details, the block form, quick add, Settings, a proposal, pairing's words.
- **A dialog that is the app's is open while the screen says so**, and closes when the app lets
  it go. Quick add opens at once, before the app's turn, so what is typed straight away goes in.
- **The sidebar's row in hand is the one focus was last on**, not the selection: its
  headings (Projects, Labels, Saved Filters) are never selected.
- **A React Aria check box's input is visually hidden**, so Playwright's `check()` waits for
  it forever: tests focus it and press Space.
- **A browser is an ordinary Iroh peer, always through a relay** (no UDP from a sandbox),
  and pairs by code. `.cargo/config.toml` sets getrandom's `wasm_js` backend.
- **The page's listener reaches the core through the worker**: the core calls a `Listener`
  object (`turn`), which the worker makes around the page's Comlink proxy, never the proxy
  itself, which answers any property.
- **Replan from Now and Plan the Day are the app's screen, drawn** (`Proposal.tsx`): each change
  its line with a check box named by the core's `item` and described by the line, as a
  reminder's buttons are by its text; a new proposal is a new list (`shown`), whose first box
  takes focus. The browser's clock (`Intl`'s `hour12`) is told the app at start.
- **React Aria gives a component its default class only when no `className` is passed**, so
  a class of our own names the default too (`"react-aria-Dialog wide"`): passing `"wide"` alone
  left the Replan and block dialogs without a background, and destructive menu items unpadded.
- **A day or a time has the browser's own picker beside its line** (`Pickers.tsx`: `<input
  type="date">` and `type="time"`, named by `picker_words`), in the block form, Go to Day and a
  time setting, where it is grouped with its line under the setting's name, so each clock is
  told from the others; a time picked there is applied at once. Chrome makes each a textbox, so
  a test naming a field "Day" names it exactly.
- **Installed, it is a PWA** (`public/manifest.webmanifest`, `public/sw.js`, registered only in a
  production build): the page is fetched afresh while online and kept for when there is none, and
  every other file of the origin kept as first fetched, its name carrying its hash, so the app
  opens offline with its store; other origins (the relays) are never touched. Notifications go
  through the service worker where there is one (an installed app on Android shows them no other
  way), a click handed back to the page; what is due is the app icon's badge. `npm run test:pwa`
  builds and tests it (`playwright.pwa.config.ts`, against `vite preview`); CI runs it on its
  release build.
- **A row's braille is `aria-braillelabel`** (`braille.ts`), which JAWS and NVDA read: the app's
  `Line::braille` (states short, the day's lengths as hours and minutes and kinds short), set on
  the row from inside it, since React Aria passes on only the attributes it knows, and left off
  where braille and speech are the same.
- **Backups go into a folder chosen once** (`backups.ts`, the File System Access API, Chromium's),
  in a tab as much as installed: its handle kept in IndexedDB and in memory, a backup taken as the
  app opens and hourly when the core says one is due among the folder's names
  (`backup::due_among`, `surplus_among`: the core's naming, a day apart, ten kept), written
  through a writable stream so an interrupted one never looks whole. A folder the browser asks
  about again after a restart is written to only once allowed, a button's press. Playwright's
  Chromium crashes reading an OPFS folder's handle back from IndexedDB, so the test's stand-in
  folder is the one in memory.
- **A colour is a mark beside the name** (`Swatch.tsx`): a dot in the sidebar, the lists, the
  day, a pick's choices and the block form's colour, `aria-hidden`, its value
  `light-dark(light, dark)` by the page's scheme, left out under forced colours.
- **What the app says is shown too**, on a status line at the foot of the page (`useSaid`,
  `StatusLine`), kept in view however long the page: not a live region, since React Aria's
  announcer has said it already.
- **A task row's check box is for a pointer alone** (`.done-box`): `aria-hidden`, never focused,
  its pointer events stopped so the row is not selected or opened behind it. It runs the row's
  Mark Done or Mark Not Done, which Space and the row's menu already offer to everyone else.
- **`npm run screens`** takes a screenshot of every screen into `screens/out`, light and dark, at a
  desktop and a phone width, for a visual review: the tests read the accessibility tree and never
  look at the page. The layout stacks to two columns below 64rem and one below 40rem.
- **The app runs in the worker beside the store** (`core/src/app.rs`, `src/session.ts`). The
  page sends what the person did and draws the screen that comes back, each line as the turn's
  `lines` word it, the capitals sentence case (`Event::Capitals`). The store worker runs as a task, and the
  network middleware hands each request to a runner on the browser's event loop
  (`core/src/app.rs`). Whatever the app leaves, at once or later, goes through one outbox to
  the listener the page gave (`listen`), a turn per batch; a call's reply is only the screen
  once the app is still, with nothing to say, for a field that keeps its own value. Every turn
  is applied one way (`apply`): drawn, its files saved, its notifications shown, focus landed
  if `landing` went up, then said, after focus so the move does not cut it off. Turns are
  numbered (`seq`), and an older screen is never drawn over a newer one. The one-second poll
  ticks the store, for another tab's or process's writes and new minutes, one tick at a time:
  a clock that jumps fires a burst, and queued before a slow worker they held up what followed.
  Reminders' looks are never skipped by the page; the app takes those asked for while one is
  under way as one after it.
- **Landing is judged against where focus was when the turn arrived**, never later: a turn can
  arrive while the person has already moved on. It lands on the main list's row in hand while
  focus is still on the same row of that list, or lost, or wherever React Aria put it when that
  row went; or, for a turn that closed a dialog, given back to where the dialog was opened from
  (`opener`, noted as each dialog takes focus); or after a command that goes somewhere
  (`send(event, { land: true })`). Each frame it waits asks again, so a move to another row, the
  details or the sidebar meanwhile stays where the person took it. It lands on the row in hand
  in the latest screen, which a move reported since may have changed.
- **Quick add opens at once and closes at once**: kept closed after Escape until asked for again,
  so the app's late answer to opening it does not open it a second time.
- **A form keeps its own value while its typing is on its way**, and takes the last answer's
  values once none is: an answer for earlier typing would otherwise undo later typing, and a
  kind's flags would never arrive.
- **A tree's `selectedKeys` is memoized by the key it holds**: React Aria says "… selected"
  whenever it is handed a new selection while the tree has focus, and a fresh array each draw
  said it five times after adding a block.
- **A command key waits for what is on its way** (`settled`): going to a place is a round trip
  now, and a key pressed straight after Go to Trash acts in the trash. Keys run in the order
  pressed, each after what the one before it sent (`commands` in `App.tsx`): keys that all waited
  on one settling ran on the screen before the first had changed it. A deferred command keeps
  trying each frame for a moment while its screen cannot act yet (a day still loading).
- **Three engines**, each in CI: `npm test` is Chromium, `npm run test:webkit` Safari's engine and
  `npm run test:firefox` Firefox (whose ephemeral contexts open the store as they are; Playwright's
  Firefox does not start on the Mac this was built on, so it runs on Windows and CI). CI runs WebKit
on a Mac (`web-webkit`, given the Linux job's core): Linux's MiniBrowser aborted at every launch
with a persistent profile; there a test that fails once and then passes is reported flaky, since
WebKit's timing fails one or two tests a run, as it did before. A WebKit test gets a persistent context (`tests/fixtures.ts`): an
  ephemeral one is a private window, which refuses the private file system the store is in. WebKit's
  persistent contexts share one origin's storage, so each test empties it first and they run one at
  a time. What WebKit found: Safari's button grey under the destructive red (now #930000), and
  focus that React Aria did not give back as a dialog closed, which the page does
  (`session.ts`, once it has stayed lost); and that the core's error lost its reason crossing
  Comlink (a `throw` handler in the worker gives it back). What Firefox found: selected and
  highlighted text at 2.94:1 where the system's pair is a stand-in (Playwright's Firefox, and
  anyone resisting fingerprinting: Tor, LibreWolf, Mullvad), so `--selected-text` and
  `--highlight-text` are `contrast-color()` of the background where the browser has it, outside
  forced colours; and quick add, whose answer can come before the turn that draws its line,
  closing and reopening the dialog with focus off the field, so `opening` holds until the screen
  has a line.
- **Playwright's clock does not reach the worker**, where the core reads the time. Tests that
  need a fixed day serve `worker.ts` with a shifted `Date` in front (`at` in
  `tests/planner.spec.ts`).

