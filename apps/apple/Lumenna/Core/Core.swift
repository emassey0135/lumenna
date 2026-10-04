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

extension Error {
    /// The sentence the core wrote, which is already phrased to be read aloud.
    var sentence: String {
        if let error = self as? LumennaError, case let .Failed(message) = error {
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
