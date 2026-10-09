import Foundation
import WatchConnectivity

/// Syncing with the iPhone over WatchConnectivity: the watch's only way to its other devices.
///
/// The watch starts every exchange (`PhoneLink`), message by message with a reply to each,
/// until the two stores agree. A message from the watch wakes the iPhone app in the
/// background, so neither needs to be open; the reverse is not so, so the phone only nudges
/// the watch, while its app is open, to start one. It runs when the app opens, after each
/// change made here, when the phone comes into reach, when the phone says it changed, and
/// when asked.
@MainActor
final class PhoneSync: NSObject, ObservableObject {
    /// How the last attempt went, said in the settings row.
    @Published private(set) var status = "Not synced with the iPhone yet"
    private weak var core: WatchCore?
    private var running = false
    private var again = false

    init(core: WatchCore) {
        self.core = core
        super.init()
        // The UI tests' copy: syncing would bring the paired phone's store into theirs.
        guard WCSession.isSupported(), ProcessInfo.processInfo.environment["LUMENNA_NO_SYNC"] == nil else { return }
        WCSession.default.delegate = self
        WCSession.default.activate()
    }

    /// Starts an exchange, or another straight after the one running, which may have missed
    /// what was just written.
    func syncNow() {
        guard let core else { return }
        if running {
            again = true
            return
        }
        let session = WCSession.default
        guard session.activationState == .activated else { return }
        guard session.isReachable else {
            status = "The iPhone is not in reach; changes wait here until it is"
            return
        }
        do {
            running = true
            send(try core.link.start(), over: session)
        } catch {
            finish("Could not sync with the iPhone. \(error.sentence)")
        }
    }

    private func send(_ message: Data, over session: WCSession) {
        session.sendMessageData(message, replyHandler: { reply in
            Task { @MainActor in self.take(reply, over: session) }
        }, errorHandler: { error in
            Task { @MainActor in self.finish("Could not sync with the iPhone. \(error.localizedDescription)") }
        })
    }

    private func take(_ reply: Data, over session: WCSession) {
        guard let core else { return }
        do {
            if let next = try core.link.take(reply: reply) {
                send(next, over: session)
                return
            }
            if core.link.tookIn() { core.changed() }
            finish("Synced with the iPhone at \(Date.now.formatted(date: .omitted, time: .shortened))")
        } catch {
            finish("Could not sync with the iPhone. \(error.sentence)")
        }
    }

    private func finish(_ said: String) {
        status = said
        running = false
        if again {
            again = false
            syncNow()
        }
    }
}

extension PhoneSync: WCSessionDelegate {
    nonisolated func session(
        _ session: WCSession, activationDidCompleteWith activationState: WCSessionActivationState, error: Error?
    ) {
        Task { @MainActor in self.syncNow() }
    }

    nonisolated func sessionReachabilityDidChange(_ session: WCSession) {
        guard session.isReachable else { return }
        Task { @MainActor in self.syncNow() }
    }

    /// The phone's store changed: what it says is only that, and the watch asks for the rest.
    nonisolated func session(_ session: WCSession, didReceiveMessage message: [String: Any]) {
        Task { @MainActor in self.syncNow() }
    }
}
