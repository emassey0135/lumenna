import Foundation
import SwiftUI

/// The shared forms' name for the open store (`Shared/Forms`): on the watch, this one.
typealias Core = WatchCore

/// The watch's open store, the app over it, and its link to the iPhone.
///
/// The watch keeps a whole store of its own and every view works on it with the phone away.
/// It runs no Iroh endpoint — watchOS allows no sockets outside an audio session — so it
/// syncs only with its iPhone, over WatchConnectivity (`PhoneSync`), and the phone passes
/// what it wrote on to every other device. Every screen draws the app's view (`AppModel`) and
/// sends it what the person did.
final class WatchCore: ObservableObject {
    /// Posted after anything changes the store from outside the app, as on the phone and the
    /// Mac: the app takes it in.
    static let changed = Notification.Name("LumennaCoreChanged")
    /// In a change's `userInfo` when it came from the phone, which need not hear it back.
    static let fromPhone = "fromPhone"

    /// The Crux app over the store, which every screen draws.
    let app: AppModel
    /// Moves with every turn of the app, so every SwiftUI view showing it draws it again.
    @Published private(set) var generation = 0
    /// Something that could not be done, in the core's words, for an alert.
    @Published var failure: String?
    private(set) var phone: PhoneSync!
    private var observer: NSObjectProtocol?
    // Foundation's, named in full: the core has a record called `Timer` too.
    private var watcher: Foundation.Timer?
    /// The store's heads at the last look, or as the last exchange with the phone left them.
    private var seenHeads: String?

    init() throws {
        // jiff finds the zone through TZ or /etc/localtime, and the sandbox is no place to
        // rely on the second.
        setenv("TZ", TimeZone.current.identifier, 1)
        let directory = try WatchCore.profileDirectory()
        app = try AppModel(directory: directory, platform: "watchos")
        phone = PhoneSync(core: self)
        observer = NotificationCenter.default.addObserver(forName: AppModel.shown, object: app, queue: .main) { [weak self] _ in
            self?.generation += 1
        }
        // What the app writes goes to the phone: its heads move with every write, looked at
        // once a second, as cheap as reading them.
        seenHeads = try? app.device.heads()
        watcher = Foundation.Timer.scheduledTimer(withTimeInterval: 1, repeats: true) { [weak self] _ in
            guard let self, let heads = try? self.app.device.heads(), heads != self.seenHeads else { return }
            self.seenHeads = heads
            self.phone.syncNow()
        }
    }

    /// The store as an exchange with the phone left it: what it brought is not sent back.
    func synced() {
        seenHeads = try? app.device.heads()
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
}

/// Saying what happened: every change of state is announced, not left to be noticed. The
/// shared forms call it as the phone's.
enum Announcer {
    static func say(_ text: String) {
        guard !text.isEmpty else { return }
        let sentence = text.prefix(1).uppercased() + text.dropFirst()
        Said.post(sentence)
        AccessibilityNotification.Announcement(sentence).post()
    }

    /// Says a result's announcement, then each notice.
    static func say(_ announcement: String, notices: [String]) {
        say(([announcement] + notices).filter { !$0.isEmpty }.joined(separator: ". "))
    }
}
