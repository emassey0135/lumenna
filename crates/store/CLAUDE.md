# The store

Read when working in `crates/store/`. The root `CLAUDE.md` holds what every part of Lumenna shares; this is what is easy to get wrong here.

## Store rules that are easy to break

**Writes take the version the user edited.** `put(record, before)` writes only the fields
that differ. Passing a freshly read `before`, or `None` on an update, writes every field,
and each write wins a last-write-wins race against a concurrent edit from another device,
silently reverting it.

**Containers must not be created concurrently.** Automerge object identity comes from the
creating operation, so two devices creating a map at the same key create two maps, and on
merge one loses everything written into it. Root collections come from a deterministic
genesis change (`Doc::new`); per-record sets are created with the record. Any *new*
container needs the same care.

**A record keyed by something two devices can produce on their own is inline.** Exceptions
(series and date) and reminder acks (reminder and date) store each field as its own key,
`<record>/<field>`, so nothing is created for merge to choose between. Set `Record::INLINE`
for any new record keyed that way.

**Things that must exist once per store come from deterministic changes.** The genesis
creates the root collections; `Doc::ensure_schema` creates the Inbox under the fixed
`ProjectId::INBOX` and the `series_years` map. Both are frozen bytes: never edit what they
write; add another change on top. Clients never create an Inbox; `edit::adopt_inbox` folds
a pre-existing one into the shared one.

**A series lives in its start year's document, and recurs into later ones.**
`series_years` records which years hold recurring series, and `Store::load_year` loads
those too. Without it a routine vanishes on 1 January.

**Change rowids are `AUTOINCREMENT` and compaction holds the write lock.** Reused rowids
make running processes skip changes. `Db::compact` folds everything on disk into the
document under `BEGIN IMMEDIATE` before deleting anything, and `refresh` reads in one
transaction and loads a snapshot whose heads it lacks.

**The snapshot is kept, per document, and followed record by record** (`Store::snapshot`,
`Hydrated`, `Doc::part`, `Doc::refresh_part`). Materializing was most of what every answer cost
(100 ms for 2,000 tasks, four times a command), so each loaded document's part is kept with its
heads; when they move, Automerge's diff from the kept heads names the collections and records
touched (`touched`: a patch's path begins at the collection, then the record's key, an inline
record's cut at its `/`), and only those are read again, else the whole document. Keyed by
heads, so no write can forget to invalidate it; `Snapshot` is still built by the store alone,
`Documents::snapshot` the uncached reading, which `tests/hydration.rs` compares it with after
every step of random histories (edits, purges, two years' blocks, exceptions, settings, merges,
another process writing, the store reopened). **Parts are kept on disk too** (the local
`hydrated` table, `doc::encode_part`, a rebuildable cache never synced or backed up), so a
process opening the store starts from the last one's parts and follows the changes since: used
only by the build that wrote them (`HYDRATION_BUILD`, `build.rs`'s hash of every source in
`core` and `store`, so a change to how records read invalidates them unasked) and only where
the document holds their heads; written after a whole reading or a following from disk, and at
most a minute apart while followed; never with a record unread. The postponements' tally
(`postponed`), which reads the core document's history from its start, is kept beside the core's
part. On 2,000 tasks a `lum` command went from 0.15 s to 0.05 s. A document's changes load in one
`load_incremental`, Automerge ordering them itself.

**The change cursor only advances in `refresh`.** `PRAGMA data_version` does not move for
your own connection's writes, and other processes' rows can interleave with yours. A cursor
that lags re-reads a change already held (a no-op); one that skips loses an edit.

## Versions and fields this build does not know

Builds of different versions sync with each other for good: nothing can make every device
update. So:

- **`SCHEMA_VERSION`** (core) is the stored format this build writes. Every document records
  the highest that has written it (`Doc::stamp_version`, on the way to disk), and every
  device its own in the device list (`edit::note_own_version`, on open; pairing says it
  too). A device on another version says so in its status. A change to the format waits for
  `Snapshot::all_devices_at_least`.
- **Never drop what you cannot read.** An edit writes only the fields it changed, so a field
  a newer build added survives. The store's own whole-record rewrites — a series moving to
  another year's document, a record becoming inline — copy the old record as it stands
  (`Raw`) and write only the changes over it, so an unknown field, or a value read as a
  default (an unknown block kind reads as work), survives those too. A new rewrite must do
  the same. So do the two places that build a record from the model alone: an undo that creates
  again what its edit removed writes the record back as it stood first (`Documents::removed_by`,
  kept in the undo entry, format 2), and the JSON export carries what each record holds beyond
  the model (`export_json_keeping`, its `unknown` list), which an import writes back into each
  record it creates (`Documents::revive`). Not kept: a record stored a key per field (an
  exception, an acknowledgement, a device), and a block's sittings in an export.


## Backup and export

Kept apart in code, commands and wording, because confusing them is how someone sends a
"task list" holding everything they ever deleted.

- **A backup** (`store::backup`) is every document's Automerge `save()` in one file: the
  whole history, trash included. Restoring **merges**, and can never take back a later
  change.
- **An export** (`store::export`) is current state only. JSON is complete and versioned
  (`export::VERSION`), keeps identifiers, and is what import reads; Markdown, org and ics
  are for reading only. **A field added to the model must be added to `export/json.rs`**,
  or `crates/store/tests/export.rs`, which round-trips every field, fails.
- Automatic backups go to `<profile>-backups` beside the profile, never inside it, `0600`,
  pruned beyond `backup-keep`. `backup-dir`, `backup-keep`, `backup-every` (and `clock`) are **device
  settings** (`<profile>/device-settings`) and never sync. `LUMENNA_BACKUP_DIR` overrides
  the directory; every test harness sets it.

