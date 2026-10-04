import UIKit

/// The open store, for the whole app.
///
/// Every operation is a method on `lumenna`, generated from the Rust surface: this class adds
/// only what is about running on iOS — where the profile lives, the time zone, and when to back
/// up. Operations are milliseconds against a local SQLite file, so they run on the main thread
/// and a view never shows a state the store has moved past.
final class Core {
    /// Posted after anything changes the store, so every view showing it reloads.
    static let changed = Notification.Name("LumennaCoreChanged")

    let lumenna: Lumenna
    private var sync: SyncService?
    private let syncQueue = DispatchQueue(label: "lumenna.sync")

    init() throws {
        Core.useSystemTimeZone()
        let directory = try Core.profileDirectory()
        lumenna = try Lumenna.open(directory: directory.path)
    }

    /// Where the store lives: Application Support, which is the app's own and never synced
    /// by iCloud Drive (§9). A UI test names a fresh one, so every run starts empty.
    static func profileDirectory() throws -> URL {
        let manager = FileManager.default
        if let test = ProcessInfo.processInfo.environment["LUMENNA_TEST_PROFILE"] {
            return manager.temporaryDirectory.appendingPathComponent(test, isDirectory: true)
        }
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

    /// Starts keeping this device in sync, for as long as the app is in front (§8). iOS
    /// suspends an app in the background, so the endpoint closes then and opens again on the
    /// way back; other devices catch up with this one at the next round either side.
    func startSyncing() {
        let lumenna = self.lumenna
        syncQueue.async { [weak self] in
            guard let self, self.sync == nil else { return }
            // Opening the endpoint can take a moment, and with no paired device there is
            // still something to answer: a device pairing with this one runs its own.
            self.sync = try? lumenna.startSync(reach: .internet, listener: Arrivals())
        }
    }

    /// Stops syncing, letting go of the endpoint.
    func stopSyncing() {
        syncQueue.async { [weak self] in
            self?.sync?.stop()
            self?.sync = nil
        }
    }

    /// Syncs with every paired device now, off the main thread.
    func syncNow(then finished: @escaping (Result<SyncReport, Error>) -> Void) {
        let lumenna = self.lumenna
        syncQueue.async { [weak self] in
            let result = Result { try self?.sync?.syncNow() ?? lumenna.syncNow(reach: .internet) }
            DispatchQueue.main.async { finished(result) }
        }
    }

    /// Takes a backup if one is due (§9), off the main thread, and says so if it fails.
    func backUpIfDue(presentingFrom presenter: UIViewController?) {
        let lumenna = self.lumenna
        DispatchQueue.global(qos: .utility).async {
            do {
                _ = try lumenna.backUpIfDue()
            } catch {
                DispatchQueue.main.async {
                    presenter?.showFailure(
                        "The automatic backup failed. \(error.sentence)",
                        title: "Backup"
                    )
                }
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

extension UIViewController {
    /// Says that something could not be done, with the core's own sentence.
    func showFailure(_ message: String, title: String = "Could not do that") {
        let alert = UIAlertController(title: title, message: message, preferredStyle: .alert)
        alert.addAction(UIAlertAction(title: "OK", style: .default))
        (presentedViewController ?? self).present(alert, animated: true)
    }
}
