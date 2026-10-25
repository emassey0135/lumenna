import Foundation

/// The open store, for the whole app — shared by the iOS and macOS apps: the Crux app over it
/// (`AppModel`), which every screen draws, and what is about running on an Apple platform —
/// where the profile lives, the time zone, noticing the store change, and backups.
final class Core {
    /// Posted on the main thread after the store changes, for what follows it outside the
    /// screens: reminders re-armed, the watch told. Its `userInfo` holds `arrived` when it came
    /// from elsewhere — a sync, another process — rather than from what the person just did.
    static let changed = Notification.Name("LumennaCoreChanged")

    /// The Crux app over the store, which every screen draws.
    let app: AppModel
    /// The glance written for the widgets (`GlanceFile`), where the build has an App Group.
    private(set) lazy var glance: GlanceFile? = GlanceFile(app: app)
    // Foundation's, named in full: the core has a record called `Timer` too.
    private var watcher: Foundation.Timer?
    /// The store's heads at the last look.
    private var seenHeads: String?
    #if os(macOS) && !APP_STORE
    /// Today's Mark Done, handed on by the direct download's widget, which cannot reach this
    /// store (`WidgetRequests`), where the build has an App Group.
    private lazy var requests: WidgetRequests? = AppGroup.container.map(WidgetRequests.init(container:))
    #endif

    init() throws {
        Core.useSystemTimeZone()
        let directory = try Core.profileDirectory()
        #if os(macOS)
        app = try AppModel(directory: directory, platform: "macos")
        #else
        app = try AppModel(directory: directory, platform: "ios")
        #endif
    }

    /// Where the store lives. A UI test names a fresh one, so every run starts empty.
    ///
    /// - **iOS**: the App Group's container (`AppGroup`), where the app's extensions reach it,
    ///   moved there from Application Support, the app's own, where builds before kept it. Never
    ///   synced by iCloud Drive either way.
    /// - **macOS, the direct download**: `~/Library/Application Support/lumenna` — where `lum`
    ///   keeps it too, so the app and the command line on one Mac are one device with one store,
    ///   not two that would have to pair with each other. `LUMENNA_PROFILE` names another, as it
    ///   does for `lum`.
    /// - **macOS, the App Store edition**: the App Group's container, the one place the sandbox
    ///   shares with the app's extensions. It has no `lum` to share a store with, and nothing to
    ///   move: it has only ever kept it there.
    static func profileDirectory() throws -> URL {
        let manager = FileManager.default
        let environment = ProcessInfo.processInfo.environment
        if let test = environment["LUMENNA_TEST_PROFILE"] {
            return manager.temporaryDirectory.appendingPathComponent(test, isDirectory: true)
        }
        #if os(macOS) && APP_STORE
        if let store = AppGroup.store {
            return store
        }
        #elseif os(macOS)
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
        let own = support.appendingPathComponent("lumenna", isDirectory: true)
        #if os(iOS)
        return AppGroup.profile(movingFrom: own)
        #else
        return own
        #endif
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

    /// Notices the store change, once a second, by its heads, which move for every write:
    /// the app's own, a sync's, another process's (`lum`, Emacs over the Mac's socket). The
    /// screens redraw from the app's own turns; this is for what follows a change outside them.
    /// A change soon after the person did something is theirs; any other arrived.
    func watchForChanges() {
        guard watcher == nil else { return }
        #if os(macOS) && !APP_STORE
        // Taken before the first look, so what a widget asked for while the app was not running
        // is done, and its change heard, as it starts.
        takeRequests()
        requests?.watch { [weak self] in self?.takeRequests() }
        #endif
        seenHeads = try? app.device.heads()
        watcher = Foundation.Timer.scheduledTimer(withTimeInterval: 1, repeats: true) { [weak self] _ in
            #if os(macOS) && !APP_STORE
            self?.takeRequests()
            self?.glance?.tick()
            #endif
            guard let self, let heads = try? self.app.device.heads(), heads != self.seenHeads else { return }
            self.seenHeads = heads
            let own = Date().timeIntervalSince(self.app.lastSent) < 3
            NotificationCenter.default.post(name: Core.changed, object: nil, userInfo: own ? nil : ["arrived": true])
        }
        // The widgets' glance, as the store is at launch and after every change.
        glance?.follow()
        glance?.write()
    }

    #if os(macOS) && !APP_STORE
    /// Runs every request the widgets handed on, then writes their glance again, as after any
    /// change.
    private func takeRequests() {
        let device = app.device
        guard requests?.take(running: { _ = try device.assist(operation: $0, arguments: $1) }) == true else { return }
        glance?.soon()
    }
    #endif

    /// Takes a backup if one is due, off the main thread; `failed` hears why not.
    func backUpIfDue(failed: @escaping (String) -> Void) {
        let device = app.device
        DispatchQueue.global(qos: .utility).async {
            do {
                _ = try device.backUpIfDue()
            } catch {
                DispatchQueue.main.async { failed("The automatic backup failed. \(error.sentence)") }
            }
        }
    }
}
