import Foundation
import WatchConnectivity

/// The phone's side of the Apple Watch's sync: it answers the watch, message by message.
///
/// The watch has no network of its own (watchOS allows no sockets outside an audio session),
/// so it reconciles its store with this one over WatchConnectivity (`PhoneLink`), and this
/// app's own sync carries what it wrote on to every other device: it shares the operations'
/// connection, so it sees the watch's changes as local edits. A message from the watch wakes
/// this app in the background, which is why the session is activated at launch. The phone
/// cannot start an exchange with a watch app that is not open, so when its store changes it
/// only nudges an open one to start.
final class WatchLink: NSObject, WCSessionDelegate {
    private let core: Core
    private let link: PhoneLink
    private var observer: NSObjectProtocol?

    init(core: Core) {
        self.core = core
        link = PhoneLink(lumenna: core.lumenna)
        super.init()
        guard WCSession.isSupported() else { return }
        WCSession.default.delegate = self
        WCSession.default.activate()
        observer = NotificationCenter.default.addObserver(forName: Core.changed, object: nil, queue: .main) { [weak self] _ in
            self?.nudge()
        }
    }

    /// Tells an open watch app the store changed, so it asks for what is new. An exchange
    /// that brought nothing in posts no change, so this does not go back and forth.
    private func nudge() {
        let session = WCSession.default
        guard session.activationState == .activated, session.isPaired, session.isWatchAppInstalled,
              session.isReachable else { return }
        session.sendMessage(["changed": true], replyHandler: nil, errorHandler: nil)
    }

    func session(_ session: WCSession, didReceiveMessageData message: Data, replyHandler: @escaping (Data) -> Void) {
        // On the main thread, as every operation is: a view never shows a state the store
        // has moved past.
        DispatchQueue.main.async { [self] in
            do {
                replyHandler(try link.answer(message: message))
                if link.tookIn() { NotificationCenter.default.post(name: Core.changed, object: nil) }
            } catch {
                // Nothing a watch can read: it says it could not sync, and tries again later.
                replyHandler(Data())
            }
        }
    }

    func session(_ session: WCSession, activationDidCompleteWith activationState: WCSessionActivationState, error: Error?) {}

    func sessionDidBecomeInactive(_ session: WCSession) {}

    /// The person switched to another watch: start again with it.
    func sessionDidDeactivate(_ session: WCSession) {
        session.activate()
    }
}
