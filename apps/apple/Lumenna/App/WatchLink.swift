import Foundation
import WatchConnectivity

/// The phone's side of the Apple Watch's sync: it answers the watch, message by message.
///
/// The watch has no network of its own (watchOS allows no sockets outside an audio session),
/// so it reconciles its store with this one over WatchConnectivity (`Device.linkAnswer`), and this
/// app's own sync carries what it wrote on to every other device: it shares the operations'
/// connection, so it sees the watch's changes as local edits. A message from the watch wakes
/// this app in the background, which is why the session is activated at launch. The phone
/// cannot start an exchange with a watch app that is not open, so when its store changes it
/// nudges an open one to start, and hands on what changed (`LinkParcel`) to one it cannot reach,
/// for the system to deliver when the two are next in reach.
final class WatchLink: NSObject, WCSessionDelegate {
    private let core: Core
    private var observer: NSObjectProtocol?

    init(core: Core) {
        self.core = core
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
        guard session.activationState == .activated, session.isPaired, session.isWatchAppInstalled else { return }
        guard session.isReachable else {
            LinkParcel.handOn(core.app.device, over: session)
            return
        }
        session.sendMessage(["changed": true], replyHandler: nil, errorHandler: nil)
    }

    /// What the watch handed on while it was out of reach, delivered now.
    func session(_ session: WCSession, didReceiveUserInfo userInfo: [String: Any]) {
        guard let parcel = LinkParcel.from(userInfo) else { return }
        DispatchQueue.main.async { self.take(parcel: parcel) }
    }

    func session(_ session: WCSession, didReceive file: WCSessionFile) {
        guard let parcel = LinkParcel.from(file) else { return }
        DispatchQueue.main.async { self.take(parcel: parcel) }
    }

    private func take(parcel: Data) {
        guard (try? core.app.device.linkUnparcel(parcel: parcel)) == true else { return }
        NotificationCenter.default.post(name: Core.changed, object: nil, userInfo: ["arrived": true])
    }

    func session(_ session: WCSession, didReceiveMessageData message: Data, replyHandler: @escaping (Data) -> Void) {
        // On the main thread, as every operation is: a view never shows a state the store
        // has moved past.
        DispatchQueue.main.async { [self] in
            do {
                let device = core.app.device
                replyHandler(try device.linkAnswer(message: message))
                if device.linkTookIn() {
                    NotificationCenter.default.post(name: Core.changed, object: nil, userInfo: ["arrived": true])
                }
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
