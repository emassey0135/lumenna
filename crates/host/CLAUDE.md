# The host

Read when working in `crates/host/`: the command surface's operations, the RPC server, and
driving sync (`sync.rs`; the sync crate's own rules are in `crates/sync/CLAUDE.md`). The root
`CLAUDE.md` holds what every part of Lumenna shares; this is what is easy to get wrong here.

## The RPC server

The surface over JSON-RPC (`crates/host/src/rpc`, the `rpc` feature), for Emacs, and for the
BTSpeak tests to seed and check a store. `lum rpc` serves it on stdio to one client. Whichever
process holds the device's sync endpoint — `lum`'s daemon, the Mac, GTK or Windows app, or a
process with no screens through `Device::keep_in_sync` (BTSpeak's background service) — serves
it at the profile's address (`endpoint::address`: `<profile>/lumenna.sock`, or on Windows a
named pipe named from the profile's path) to any number of clients, for as long as it holds the
endpoint (`Endpoint::serve`, `Lumenna::serve_commands`).

- **A method calls the surface's operation and returns its record in the `rpc::api`
  envelope**, the one `lum --json` writes, so no transport drifts from another.
- **Each client gets a store connection of its own**, so what it writes is another
  connection's write to the process serving it, which already redraws for those by
  `outside_version`.
- **`sync` runs on the holder's own service**; `Lumenna::sync_now` anywhere else on the
  device asks the holder over this address. `initialize` says what is answering
  (`process`: `daemon`, an app's name, `lum rpc`); a second daemon is refused by it, while a
  daemon finding an app waits its turn.
- **`lum rpc` relays to a running holder** (`rpc::relay`) before serving on its own. Emacs on
  Windows cannot open a local socket or a named pipe, so it starts `lum rpc`, which reaches the
  running app this way.
- **Three ways to ask "did something change"**, for three questions. `refresh` says whether
  *that call* took anything in, so any other call can use the answer up. `version` moves for
  every write, this connection's included: for a sync loop. `outside_version` (`PRAGMA
  data_version`) moves only when another connection commits: for redrawing after `lum` or
  the daemon wrote. `lumenna/changed` is pushed from `outside_version`, polled once a
  second; built on `refresh`, a request just after another process wrote would take the
  change in and the push would never go.
- **No row numbers**: the server addresses records by identifier only. The last listing is
  one file per profile, so a resident server and a shell would overwrite each other's
  numbering.
- **`task.erase`, `task.erase_permanently` and `erase_everything_deleted` require
  `"confirm": true`**: nothing here can prompt.
- **A method refuses a parameter it does not take** (`TAKES`, checked before it runs), as
  `assist` does: a typo must not pass for a parameter left out. `METHODS` and `TAKES` list
  every method `answer` has an arm for, which a test reads from the source to hold them equal.
- **A path is refused unless whole** (`backup.to`, `restore.file`, `export.output`,
  `import.file`, `config.set backup-dir`): a relative one would be the server's working
  directory's, not the client's.
- **`reminder.give` gives this device's reminders** (`deliver_reminders`, taking the reminder
  lock for the connection); `reminder.deliver {id, on}` is `lum reminder deliver`'s, which
  devices one fires on. `reminder.deliver` without an `id` still gives them, its name before
  `reminder.give`, for clients built before it (Emacs's).
- **`assist {operation, args}` runs an operation of the assistant table** through
  `Lumenna::assist`, its arguments read by `rpc::assist` (`arguments_of`, which `lum mcp` and
  `lum assist` read theirs with too) and a name the operation does not take refused. One that
  `confirms` needs `"confirm": true` among its `args`. `assist.list` is the table, the
  `Operations` record `lum assist --list --json` writes (`api::Operations::all`, one for both,
  and a test in `apps/cli/tests/rpc.rs` holds them equal). `complete {kind, text}` is
  `Lumenna::complete_typed`: what could come next at the end of a line, a filter, a project's
  or a label's name. The PowerShell module speaks only these three, over one `lum rpc` for its
  session; a holder from before them (an app or daemon `lum rpc` relays to) answers none.
- **A real erasure is followed by every process** (`follow_erasure`, around every operation in
  `Lumenna::with`): when the store's generation is not the one its automatic backups were last
  taken of (`backups-of`), they are deleted and one is taken afresh. It is refused while a
  paired device runs a version from before generations (`too_old_to_follow`). It never fails
  the operation it runs beside: a backup folder it cannot clear is said as a notice, not
  tried again by this process, and `backups-of` is left for the next process to try.
- **Two framings, chosen per message by what arrived**: newline-delimited JSON (MCP's stdio
  transport) and `Content-Length` headers (`jsonrpc.el`). A reply is framed as its request. A
  byte order mark where a message starts is passed over: .NET Framework writes one into a
  child's input when the console's code page is UTF-8, before its client sends anything, so
  Windows PowerShell 5.1's `lum rpc` would otherwise refuse every request on such a console.
  A message, or a line, longer than 64 MiB (`LONGEST`) is passed over and answered as a parse
  error, never read into memory: one bad header used to abort whatever served it.
- **The socket is made where only its user can reach it, then moved to the address**, so it
  is never another user's to open, even for a moment; and a holder giving the address up
  removes it only if it is still its own socket (by device and inode), never the next
  holder's.
- **Spans are UTF-8 byte offsets.** A client that indexes by code point converts both ways.
- **`pair` replies when the pairing ends**, talking in between through `lumenna/pairing`
  notifications (`{code, name}`, then `{words}`); the client answers `pair.confirm {match}`
  or `pair.cancel`, and the server keeps answering everything else. One pairing at a time.
- **The app, for a client that cannot link it** (`app.*`, `Apps`, `AppSession`): at a client's
  first `app.*` call the server opens an `App` over the client's own connection
  (`lumenna_ffi::rpc`, given by whoever serves: `lum rpc`, the daemon, a desktop app,
  `Device::keep_in_sync`), each of the client's buffers a screen. `app.send {screen, event}`
  takes a `ShellEvent` in its serde shape; every turn comes back as a `lumenna/turn`
  notification (`screen`, `seq`, `view`, `lines`, `said`, `files` with base64 `data`,
  `notices`, `background`), worded in English in the clock `app.clock {twenty_four}` gave.
  `app.view`, `app.close`, `app.still`, `app.tick`, `app.in_front` and `app.complete` (at
  keystroke rate, straight from the store) answer the rest. A client keeps no copy of any rule
  and asks for no words: there are no form helpers over RPC, since the app's own forms,
  questions and words are what a client draws. In a `lum rpc` of the client's own (nothing
  else held the endpoint, or it would have relayed) the app keeps the device in sync once
  paired, as every app does, and serves the profile's address while it holds the endpoint
  (`Apps::keeps_in_sync`); wherever the process's own service syncs, it never does.
- **`lum rpc` takes backups at start and hourly** (`Host::backups`), as the CLI takes them
  before a command; a daemon or app serving the address takes its own.
