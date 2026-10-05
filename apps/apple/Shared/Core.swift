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
    /// - **iOS**: Application Support, the app's own and never synced by iCloud Drive (§9).
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
    /// the person chose, so it is the one that counts (§4).
    private static func useSystemTimeZone() {
        setenv("TZ", TimeZone.current.identifier, 1)
    }

    /// Called when the app comes back, since the person may have flown somewhere.
    func timeZoneMayHaveChanged() {
        NSTimeZone.resetSystemTimeZone()
        Core.useSystemTimeZone()
    }

    /// Starts keeping this device in sync (§8). On iOS, for as long as the app is in front;
    /// on macOS, for as long as it runs, which is what makes the resident app the device's
    /// sync process (§16.2) with no daemon or service to set up.
    func startSyncing() {
        let lumenna = self.lumenna
        syncQueue.async { [weak self] in
            guard let self, self.sync == nil else { return }
            self.sync = try? lumenna.startSync(reach: .internet, listener: Arrivals())
        }
    }

    /// Stops syncing, letting go of the endpoint — before returning, when `waiting`, as the
    /// app must when it quits.
    func stopSyncing(waiting: Bool = false) {
        let stop = { [weak self] in
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
    /// as `lum rpc` does (§8 sanctions the timer). Only a Mac has those other processes.
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

    /// Takes a backup if one is due (§9), off the main thread; `failed` hears why not.
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

extension Error {
    /// The sentence the core wrote, which is already phrased to be read aloud.
    var sentence: String {
        if let error = self as? LumennaError {
            let message: String
            switch error {
            case let .Failed(text), let .SyncElsewhere(text): message = text
            }
            return message.prefix(1).uppercased() + message.dropFirst()
        }
        return localizedDescription
    }
}
