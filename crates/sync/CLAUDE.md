# Sync

Read when working in `crates/sync/`, or on driving sync in `crates/host/src/sync.rs`. The root
`CLAUDE.md` holds what every part of Lumenna shares; this is what is easy to get wrong here.

`crates/sync`: `session` reconciles every document with Automerge's sync protocol over any
byte stream; `pairing` is the word comparison; `node` is the device's Iroh endpoint;
`invite` is the short-lived endpoint a pairing runs on; `exchange` is the Apple Watch's link
to its iPhone, and `introduce` a Wear OS watch's to its phone. Driving sync (pairing through a
`PairingPrompt`, `sync_now`, status, devices, the `SyncService` loop) is the host's `sync.rs`;
the CLI adds only the terminal prompt, the daemon's socket and signals.

- **Membership in `devices` is the trust boundary.** `Node` refuses, in both directions, any
  key not listed there. The ways in are a pairing whose words were confirmed on both sides
  (`edit::enroll_devices`), and the first sync over a Wear OS watch's link to its phone, which
  only Google's pairing of the two can open (`introduce`).
- **Pairing never uses the device key.** It runs on a key minted per pairing, so it needs no
  daemon and no IPC; device keys are exchanged only after the words are confirmed.
- **The words** come from the connection's TLS exporter secret plus a commit-then-reveal
  nonce exchange: three PGP words. Without the commitment, 24 bits could be ground offline.
- **One endpoint per device**, decided by an advisory lock on `<profile>/sync.lock`. A
  `SyncService` holds it, or, started while another process does, waits its turn (trying
  every five seconds) and takes over when that process stops. The holder serves the profile's
  address with the whole RPC surface (`crates/host/CLAUDE.md`), so `sync_now` anywhere else on
  the device runs its round there; only when nothing answers is it
  `LumennaError::SyncElsewhere`.
- **A loop merging makes is written down once and said once** (`edit::repair`, through
  `Lumenna::told`): looked for whenever the documents' heads moved from outside, or right
  after an operation that called `merged()` (restore, import).
- **The service shares the operations' connection**, so it notices local edits by
  `Store::version()`; `refresh` cannot see a connection's own writes. The device key lives
  in the store's `local_state` table and never syncs.
- **Async underneath, blocking on top.** `pair_async` and `keep_in_sync` are the
  implementation; `pair`, `sync_now` and `SyncService` run them on a runtime of their own and
  are native only. The browser runs them on its event loop, so the sync crate uses
  n0-future for time and spawning, never tokio's runtime.
- **A round that misses a device is retried**, after 5 seconds and doubling to the full
  round's five minutes; otherwise a change sent while a device is briefly unreachable waits
  for the next full round.
- **The session is lockstep per document**: both sides send one frame (a message, or empty)
  then read one; both empty ends the document, so both stop together. Sync state is per
  session, not saved.
- **Local discovery is standard DNS-SD** (`discovery/`): `_lumenna._udp` (devices) and
  `_lumenna-pair._udp` (pairing), TXT `id` plus `a0`… addresses. Apple uses the system
  responder (on iOS, Bonjour, needing only `NSBonjourServices`, no multicast entitlement).
  Linux uses Avahi over D-Bus when it runs: beside Avahi, `mdns-sd` hears nothing, not even
  itself. `mdns-sd` covers Windows and Linux without Avahi. Iroh's own mDNS is not used: no
  standard browser can see it (no PTR record, every TTL zero).
- **A local lookup ends** (`LISTEN`, three seconds): iroh reports a device's lookup only
  once every service has finished, and starts no other while one runs. One waiting for good,
  as it does in a browser, which hears no network, keeps a failed relay lookup from ever being
  reported, and no dial follows a later look: two browsers pair and never sync.
- **Bonjour reports an instance once per interface**, and a gone device's record can
  linger. So an instance is resolved once, on its own thread, and lost only when gone from
  every interface; dials to heard pairing sessions give up after three seconds.
- **A session joining by code does not advertise.** Otherwise the waiting session dials it
  back and two connections each wait for the other to speak.
- **Tests that pair take turns** (`ONE_PAIRING_AT_A_TIME`, `crates/host/tests/sync.rs`):
  waiting sessions on one network find each other. Tests use `Network::LocalOnly` with explicit
  addresses.
- Device records are inline (`Record::INLINE`): both sides of a pairing write them.
- Iroh is built without its default features, metrics among them, which nothing reads.
- `lum daemon install` (`service.rs`) writes a systemd user unit plus linger on Linux (a
  BTSpeak the same without linger, since it logs its person in at every start), a LaunchAgent
  on macOS, a logon task on Windows, always with `--profile`. macOS and Linux have been run
  for real; Windows' is unit-tested only.
