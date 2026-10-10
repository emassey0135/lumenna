import Foundation

/// The open store, for the whole app — shared by the iOS and macOS apps.
///
/// Every operation is a method on `lumenna`, generated from the Rust surface: this class adds
/// only what is about running on an Apple platform — where the profile lives, the time zone,
/// syncing while the app runs, and backups. Operations are milliseconds against a local SQLite
/// file, so they run on the main thread and a view never shows a state the store has moved past.
final class Core {
    /// Posted after anything changes the store, so every view showing it reloads.
    static let changed = Notification.Name("LumennaCoreChanged")

    let lumenna: Lumenna
    private var sync: SyncService?
    /// The command surface this app serves while it holds the endpoint (macOS).
    private var commands: CommandServer?
    private let syncQueue = DispatchQueue(label: "lumenna.sync")
    // Foundation's, named in full: the core has a record called `Timer` too.
    private var watcher: Foundation.Timer?
    /// How far other processes' writes had got at the last look.
    private var seenOutside: Int64?

    init() throws {
        Core.useSystemTimeZone()
        let directory = try Core.profileDirectory()
        lumenna = try Lumenna.open(directory: directory.path)
    }

    /// Where the store lives. A UI test names a fresh one, so every run starts empty.
    ///
    /// - **iOS**: Application Support, the app's own and never synced by iCloud Drive.
    /// - **macOS**: `~/Library/Application Support/lumenna` — where `lum` keeps it too, so the
    ///   app and the command line on one Mac are one device with one store, not two that would
    ///   have to pair with each other. `LUMENNA_PROFILE` names another, as it does for `lum`.
    static func profileDirectory() throws -> URL {
        let manager = FileManager.default
        let environment = ProcessInfo.processInfo.environment
        if let test = environment["LUMENNA_TEST_PROFILE"] {
            return manager.temporaryDirectory.appendingPathComponent(test, isDirectory: true)
        }
        #if os(macOS)
        if let explicit = environment["LUMENNA_PROFILE"], !explicit.isEmpty {
            return URL(fileURLWithPath: explicit, isDirectory: true)
        }
        #endif
        let support = try manager.url(
            for: .applicationSupportDirectory,
            in: .userDomainMask,
            appropriateFor: nil,
            create: true
        )
        return support.appendingPathComponent("lumenna", isDirectory: true)
    }

    /// Tells the core which time zone "today" is in.
    ///
    /// The core asks the operating system through `TZ` or `/etc/localtime`, and inside the iOS
    /// sandbox the second is not something to rely on. The time zone in Settings is the one
    /// the person chose, so it is the one that counts.
    private static func useSystemTimeZone() {
        setenv("TZ", TimeZone.current.identifier, 1)
    }

    /// Called when the app comes back, since the person may have flown somewhere.
    func timeZoneMayHaveChanged() {
        NSTimeZone.resetSystemTimeZone()
        Core.useSystemTimeZone()
    }

    /// Starts keeping this device in sync. On iOS, for as long as the app is in front;
    /// on macOS, for as long as it runs, which is what makes the resident app the device's
    /// sync process with no daemon or service to set up.
    func startSyncing() {
        // The UI tests' copy: syncing would listen on the network, and on a fresh machine
        // macOS then asks about local networks and incoming connections, over the window
        // under test.
        if ProcessInfo.processInfo.environment["LUMENNA_NO_SYNC"] != nil { return }
        let lumenna = self.lumenna
        syncQueue.async { [weak self] in
            guard let self, self.sync == nil else { return }
            self.sync = try? lumenna.startSync(reach: .internet, listener: Arrivals())
            #if os(macOS)
            // While this app holds the device's endpoint it answers the command surface, as
            // the daemon would, so Emacs and the BTSpeak app reach it. A client's writes come
            // through a store connection of its own, which `outsideVersion` already notices.
            if let sync = self.sync {
                self.commands = lumenna.serveCommands(
                    service: sync, app: "Lumenna for Mac",
                    deviceName: Host.current().localizedName ?? "Mac", platform: "macos"
                )
            }
            #endif
        }
    }

    /// Stops syncing, letting go of the endpoint — before returning, when `waiting`, as the
    /// app must when it quits.
    func stopSyncing(waiting: Bool = false) {
        let stop = { [weak self] in
            self?.commands?.stop()
            self?.commands = nil
            self?.sync?.stop()
            self?.sync = nil
        }
        if waiting { syncQueue.sync(execute: stop) } else { syncQueue.async(execute: stop) }
    }

    /// Syncs with every paired device now, off the main thread: on this app's own endpoint
    /// while it syncs, or on one opened for the round.
    func syncNow(then finished: @escaping (Result<SyncReport, Error>) -> Void) {
        let lumenna = self.lumenna
        syncQueue.async { [weak self] in
            let result = Result { try self?.sync?.syncNow() ?? lumenna.syncNow(reach: .internet) }
            DispatchQueue.main.async { finished(result) }
        }
    }

    /// Notices what another process — `lum`, the daemon — wrote to the store, once a second,
    /// as `lum rpc` does: SQLite has no way to be told of another process's commit. Only a
    /// Mac has those other processes.
    ///
    /// By `outsideVersion`, not by asking `refresh` whether it took anything in: every
    /// operation and the sync loop refresh too, and whichever came first after `lum` wrote
    /// would have had the answer, leaving the change unseen. Not by `version` either, which
    /// moves for the app's own edits as well — they redraw as they are made, and a second
    /// redraw a moment later lands on whatever the person has opened next. A sync's arrivals
    /// are announced by `Arrivals`.
    func watchForOtherProcesses() {
        guard watcher == nil else { return }
        seenOutside = try? lumenna.outsideVersion()
        watcher = Foundation.Timer.scheduledTimer(withTimeInterval: 1, repeats: true) { [weak self] _ in
            guard let self, let outside = try? self.lumenna.outsideVersion(), outside != self.seenOutside else { return }
            self.seenOutside = outside
            NotificationCenter.default.post(name: Core.changed, object: nil)
        }
    }

    /// Takes a backup if one is due, off the main thread; `failed` hears why not.
    func backUpIfDue(failed: @escaping (String) -> Void) {
        let lumenna = self.lumenna
        DispatchQueue.global(qos: .utility).async {
            do {
                _ = try lumenna.backUpIfDue()
            } catch {
                DispatchQueue.main.async { failed("The automatic backup failed. \(error.sentence)") }
            }
        }
    }
}

/// What a sync brought in: every view showing the store reads it again.
private final class Arrivals: SyncListener {
    func changed() {
        DispatchQueue.main.async {
            NotificationCenter.default.post(name: Core.changed, object: nil)
        }
    }
}
