# The PowerShell module

Read when working in `apps/powershell/`. The root `CLAUDE.md` holds what every part of Lumenna
shares; this is what is easy to get wrong here.

A script module for Windows PowerShell 5.1 and PowerShell 7 on every platform: a cmdlet per
operation of the core's assistant table (`records::assistant`), each an RPC `assist` request to
one `lum rpc` the module keeps for its session. `Lumenna/` is the module as it ships;
`Cmdlets.psd1` and `Write-Operations.ps1` make it; `tests/` checks it.

- **One `lum rpc` per profile, for the module's session** (`Lumenna.Rpc.ps1`), never a process
  per call: started at the first request, keyed by the profile it serves (`-Profile`, else
  `$env:LUMENNA_PROFILE` as it is at that request, else the platform's own), started again if it
  has stopped, and stopped with the module (`OnRemove`). Newline-delimited JSON-RPC, as UTF-8
  bytes whatever the console's code page; what it pushes unasked (`lumenna/changed`) is passed
  over. A request whose server stops mid-way is not sent again, since it may have been done; one
  whose pipe breaks as it is written is. Every request holds one lock, so two threads never
  interleave on a pipe. When the app or `lum`'s daemon runs, `lum rpc` relays to it, so a write
  goes through whatever holds the store, and that holder must be new enough to answer `assist`.
- **It speaks three methods**: `assist {operation, args}`, `assist.list` and `complete {kind,
  text}` (`crates/host/CLAUDE.md`). Anything else a cmdlet wants belongs in the assistant table
  first.
- **The cmdlets are written, not kept by hand.** `Write-Operations.ps1` reads `assist.list` (over
  `lum rpc` against a throwaway profile, through `Lumenna.Rpc.ps1`) and `Cmdlets.psd1` and writes `Lumenna/Lumenna.Operations.ps1`: each cmdlet's help (the table's
  words, another operation named by its cmdlet), its parameters' types (by `ParameterKind`), which
  ones are mandatory and positional, which binds from the pipeline, and `-WhatIf`/`-Confirm`.
  When the table changes, add the operation's name to `Cmdlets.psd1` and to the manifest's
  `FunctionsToExport`, then run it with the `lum` this tree builds on `PATH`. The tests run it
  with `-Check` and fail on any difference.
- **The manifest's version, author, copyright and links are the workspace's**
  (`[workspace.metadata.lumenna]` in Cargo.toml): `Write-Operations.ps1` writes them into
  `Lumenna.psd1`, which is data PowerShell reads before anything of the module runs, and `-Check`
  fails while they differ. A version is changed in Cargo.toml, then the generator run.
- **`Cmdlets.psd1` is the only naming that is the module's**, as App Intents' type names are
  Swift's: an approved verb and a noun per operation, a parameter renamed where the table's name
  reads badly here (`query` is `-Filter`, `assignment` `-Sitting`, the table's name kept as an
  alias), and a `Switch` where one cmdlet runs two operations (`Remove-LumennaTask` trashes;
  `-Permanently` erases). Nothing else about a cmdlet is decided here.
- **The record a cmdlet acts on comes down the pipeline by its id**: an operation's first
  required identifier (a task, sitting, choice or block), or with nothing required its first
  parameter when that is one (`Stop-LumennaTimer`'s sitting, the timer running when left out),
  binds `ValueFromPipelineByPropertyName`, aliased `Id`, so a listing's rows, the day's sittings and What Should I Work On?'s choices pipe
  into what acts on them. No other parameter binds from the pipeline: a row's `title` piped into
  `Set-LumennaTask -Title` would send an unchanged field, which reverts a concurrent edit.
- **Whatever writes has `-WhatIf` and `-Confirm`**: `ConfirmImpact` Low for a change, Medium for
  one that removes (`destructive`), High for one that `confirms`, which then gets `confirm` once
  ShouldProcess has answered. A cmdlet has one `ConfirmImpact`, the first operation's, so
  `-Permanently` raises `$ConfirmPreference` in the cmdlet's own scope to ask as High would; a
  test checks it asks where no one can answer, and that trashing does not.
- **A listing's items are written one by one** (`rows`, `filters`, `choices`, `operations`), so
  the pipeline binds each; anything else is the record whole, without `version` and `result`.
  Each is named `Lumenna.<Kind>` for `Lumenna.Format.ps1xml`. Notices are warnings; a listing's
  announcement and a filter's readback are verbose. Nothing here composes a sentence of its own.
- **A refusal is an error record, never a thrown string**: non-terminating, the core's sentence
  (the RPC error's `message`) as its message and `ErrorDetails`, `Lumenna.<id>` its id, the
  identifier given its target.
- **`lum rpc` is started through `System.Diagnostics.Process`**, never `& lum`: Windows
  PowerShell 5.1 decodes a native command's output in the console's code page and turns its
  standard error into error records. Its standard error is drained as it comes
  (`ReadToEndAsync`), so it never blocks on it, and said if it stops.
- **Finding `lum`**: on `PATH`, else beside the module, else the Windows installer's
  `%LOCALAPPDATA%\Programs\Lumenna`. A relative `-Profile`, or `$env:LUMENNA_PROFILE`, is
  resolved against PowerShell's location, not the process's (`Resolve-LumennaProfile`), and the
  server is told the whole path.
- **Completers are registered by the module** (`Register-ArgumentCompleter`, from
  `$script:Completed`), never in a parameter's `[ArgumentCompleter]`, whose scriptblock cannot see
  the module's own functions. Each asks `complete` (a line, a filter, a project, a label), whose
  span is in UTF-8 bytes, and offers the whole argument again with the completion in place,
  quoted for PowerShell.
- **Windows PowerShell 5.1 shapes what is written here**: no classes, no `??` or `?.`, no
  `Select-Object -First` inside a cmdlet (5.1 leaves its stopping the pipeline in the caller's
  `-ErrorVariable`), `-ErrorAction Ignore` rather than `SilentlyContinue` for what may not be
  there, arrays unwrapped before `ConvertTo-Json` (5.1 writes a wrapped one as `value` and
  `Count`; `Set-LumennaArgument`, for a cmdlet's parameters and `Invoke-LumennaOperation`'s
  arguments alike, assigning rather than returning, since the pipeline may wrap it again), and any `.ps1` with a character outside ASCII saved with a byte order mark, as the
  generator saves its own.
- **Tests**: `tests/run.ps1` runs the Pester tests in whichever edition starts it, with Pester 5
  or later from `-Modules` (5.1 ships Pester 3), against `target/debug/lum`
  (`cargo build -p lumenna-cli` first). CI's `powershell` job runs them on Windows in both
  editions and on Linux in PowerShell 7, for any change under `apps/powershell` or `apps/cli`.
  Each round trip has a temporary profile of its own, whose server is stopped before the profile
  is deleted (Windows will not delete a database a process holds open). A server that never
  answers, or stops with a request in hand, is a stand-in `lum` (a batch file, a shell script)
  that `Find-Lum` is mocked to find.
