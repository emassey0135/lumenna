import Foundation
import SwiftUI

/// The shared forms' name for the open store (`Shared/Forms`): on the watch, this one.
typealias Core = WatchCore

/// The watch's open store and its link to the iPhone.
///
/// The watch keeps a whole store of its own and every view works on it with the phone away.
/// It runs no Iroh endpoint — watchOS allows no sockets outside an audio session — so it
/// syncs only with its iPhone, over WatchConnectivity (`PhoneSync`), and the phone passes
/// what it wrote on to every other device. Operations are milliseconds against local SQLite,
/// so they run on the main thread, as on the phone.
final class WatchCore: ObservableObject {
    /// Posted after anything changes the store, as on the phone and the Mac: every view
    /// showing it reads it again, and what the watch wrote goes to the phone.
    static let changed = Notification.Name("LumennaCoreChanged")
    /// In a change's `userInfo` when it came from the phone, which need not hear it back.
    static let fromPhone = "fromPhone"

    let lumenna: Lumenna
    let link: PhoneLink
    /// Moves after anything changes the store, so every SwiftUI view showing it reads it again.
    @Published private(set) var generation = 0
    /// Something that could not be done, in the core's words, for an alert.
    @Published var failure: String?
    private(set) var phone: PhoneSync!
    private var observer: NSObjectProtocol?

    init() throws {
        // jiff finds the zone through TZ or /etc/localtime, and the sandbox is no place to
        // rely on the second.
        setenv("TZ", TimeZone.current.identifier, 1)
        lumenna = try Lumenna.open(directory: try WatchCore.profileDirectory().path)
        link = PhoneLink(lumenna: lumenna)
        phone = PhoneSync(core: self)
        // Every change, whoever made it — this file, or a shared form, which posts the
        // notification itself — redraws and goes to the phone.
        observer = NotificationCenter.default.addObserver(forName: Self.changed, object: nil, queue: .main) { [weak self] note in
            guard let self else { return }
            generation += 1
            if note.userInfo?[Self.fromPhone] == nil { phone.syncNow() }
        }
    }

    /// Where the store lives: Application Support, or a fresh one a UI test names.
    static func profileDirectory() throws -> URL {
        let manager = FileManager.default
        if let test = ProcessInfo.processInfo.environment["LUMENNA_TEST_PROFILE"] {
            return manager.temporaryDirectory.appendingPathComponent(test, isDirectory: true)
        }
        let support = try manager.url(
            for: .applicationSupportDirectory, in: .userDomainMask, appropriateFor: nil, create: true
        )
        return support.appendingPathComponent("lumenna", isDirectory: true)
    }

    /// The store changed here.
    func changed() {
        NotificationCenter.default.post(name: Self.changed, object: nil)
    }

    /// Runs an operation a person asked for: says the core's announcement, redraws, and
    /// sends the change to the phone. Returns whether it ran.
    @discardableResult
    func act(_ operation: () throws -> some Announced) -> Bool {
        do {
            let result = try operation()
            changed()
            Announcer.say(result.announcement, notices: result.notices)
            return true
        } catch {
            failure = error.sentence
            return false
        }
    }

    /// Reads something, saying why not if it cannot be read.
    func read<T>(_ query: () throws -> T) -> T? {
        do {
            return try query()
        } catch {
            failure = error.sentence
            return nil
        }
    }
}

/// The records an operation returns, each with the core's sentence about what it did.
protocol Announced {
    var announcement: String { get }
    var notices: [String] { get }
}

extension Change: Announced {}
extension Timer: Announced {}

/// Saying what happened: every change of state is announced, not left to be noticed. The
/// shared forms call it as the phone's.
enum Announcer {
    static func say(_ text: String) {
        guard !text.isEmpty else { return }
        AccessibilityNotification.Announcement(text.prefix(1).uppercased() + text.dropFirst()).post()
    }

    /// Says a result's announcement, then each notice.
    static func say(_ announcement: String, notices: [String]) {
        say(([announcement] + notices).filter { !$0.isEmpty }.joined(separator: ". "))
    }
}
