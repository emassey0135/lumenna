import Foundation
import SwiftUI

/// The watch's open store and its link to the iPhone.
///
/// The watch keeps a whole store of its own and every view works on it with the phone away.
/// It runs no Iroh endpoint — watchOS allows no sockets outside an audio session — so it
/// syncs only with its iPhone, over WatchConnectivity (`PhoneSync`), and the phone passes
/// what it wrote on to every other device. Operations are milliseconds against local SQLite,
/// so they run on the main thread, as on the phone.
@MainActor
final class WatchCore: ObservableObject {
    let lumenna: Lumenna
    let link: PhoneLink
    /// Moves after anything changes the store, so every view showing it reads it again.
    @Published private(set) var generation = 0
    /// Something that could not be done, in the core's words, for an alert.
    @Published var failure: String?
    private(set) var phone: PhoneSync!

    init() throws {
        // jiff finds the zone through TZ or /etc/localtime, and the sandbox is no place to
        // rely on the second.
        setenv("TZ", TimeZone.current.identifier, 1)
        lumenna = try Lumenna.open(directory: try WatchCore.profileDirectory().path)
        link = PhoneLink(lumenna: lumenna)
        phone = PhoneSync(core: self)
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

    /// Every view reads the store again.
    func changed() {
        generation += 1
    }

    /// Runs an operation a person asked for: says the core's announcement, redraws, and
    /// sends the change to the phone. Returns whether it ran.
    @discardableResult
    func act(_ operation: () throws -> some Announced) -> Bool {
        do {
            let result = try operation()
            changed()
            say(([result.announcement] + result.notices).filter { !$0.isEmpty }.joined(separator: ". "))
            phone.syncNow()
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

    /// Hands VoiceOver a sentence the core wrote.
    func say(_ sentence: String) {
        guard !sentence.isEmpty else { return }
        AccessibilityNotification.Announcement(sentence.prefix(1).uppercased() + sentence.dropFirst()).post()
    }
}

/// The records an operation returns, each with the core's sentence about what it did.
protocol Announced {
    var announcement: String { get }
    var notices: [String] { get }
}

extension Change: Announced {}
extension Timer: Announced {}

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
