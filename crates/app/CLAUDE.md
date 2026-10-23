# The app's inner workings

Read when working in `crates/app/` or `crates/bridge/`. The root `CLAUDE.md` holds the app's
contract with every shell; this is how the app keeps it.

- **The store's effects are Crux middleware** (`crates/bridge`, Crux pinned at 0.20.0, whose
  middleware is marked experimental): `Watching` (the `Watch` stream) and `Writing`
  (`HostRequest`) hand both to one `StoreWorker`, which answers them in order off the caller's
  thread, a thread of its own natively and a task on the event loop in a browser, and answers
  every watch again when a `Store::tick` finds the heads or the minute moved. What reaches other
  devices (`NetworkRequest`: syncing, pairing) is a third middleware, the platform's, since each
  syncs on its own runtime (`bridge::stack` takes it); a pairing answers its request more than
  once. A resolver must never be called inside `try_process_effect` (Crux asserts it): every
  middleware hands its work on through a channel. `Busy` counts the store's work under way, so a
  test can wait until the app is still: the flow tests' own `Session` waits on every call.
- **The app is screens** (`Model::screens`, each a `Screen`): a window, an Emacs buffer, a tab,
  each with its own place, row in hand, folding, filter, question open, forms and landing. An
  event goes to one (`Event::On(screen, event)`; screen 0 without one), and what it asks for
  comes back to it (`map_event`), as what it says is said there (`Announce::screen`,
  `bridge::shell_on`). What the device holds alike — the store, the clock and capitals, what the
  shell keeps running in the background, the moment proposals are made against, reminders due,
  syncing — every screen holds a copy of, given to the others after each turn
  (`Screen::share`); the store, the clock and the capitals arriving are taken in by every
  screen. A screen opens with the person's first event on it, over the store as another holds
  it, and `CloseScreen` closes it: an answer that comes back for it afterwards goes nowhere. The
  view is every screen's (`Views`); a shell draws its own (`Views::screen`). Through
  `lumenna-ffi` a shell addresses a screen with `App::send_on`, `view_on`, `lines_on` and
  `close_screen`, and each `Turn` names its screen: one per screen with a view it has not drawn
  or something to say or save, so a batch that left a screen as it was draws nothing there.
- **The person can move on before a write is answered**: a write's answer that comes after an
  `Event::Focus` the person made since they last acted leaves the row they moved to in hand
  (`Screen::moves`, `acted_at`), rather than pulling them to the row the write would have put
  there.
- **A new effect or shell event reaches every shell's code at once**: `bridge::Shell`, the
  ffi's `Turn` and `deliver`, the RPC turn notification (`crates/ffi/src/rpc.rs`), the web's
  core (`apps/web/core/src/app.rs`) and the flow tests all match on them. BoltFFI cannot leave a
  variant out, so the shell's events are `ShellEvent` (`shell.rs`), the app's `Event` less what
  its own effects answer, converted each way by exhaustive matches.
- **In a view no row carries its actions whole**: `view::offer` takes them, and the app finds
  them again in `view::whole` when a shell's `ActOn` names the row. The exceptions are the
  reminder list's own Add Reminder, whose question's words a shell draws as its field, and
  `asking.action`. `Run` works on a side list's row while the person acts from it, so a key a
  row does not offer is refused in the core's words on every list.
- **What the app decides, in full**: besides the root's list, a new saved filter's two
  questions, Go to Day, a reminder's block shown on its day and put in hand (`ShowBlock`),
  a pick's length asked once one is picked, which occurrences of a repeating block a change is
  for (asked first), a proposal's checks, partial applies and other options, a code typed while
  pairing waits, and an overrun's Replan dismissing it once something is replanned. A place
  gone back to opens as `Screen::left` keeps it. Applying a proposal is a `HostRequest`, since
  the host checks the day has not moved.
- **The web's and Android's turns carry the lines**, and a day newly shown is said as
  "<day>. <summary>" before it reaches either.
