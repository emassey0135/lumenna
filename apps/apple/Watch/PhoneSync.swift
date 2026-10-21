import Foundation
import WatchConnectivity
import WatchKit

/// Syncing with the iPhone over WatchConnectivity: the watch's only way to its other devices.
///
/// The watch starts every exchange (`Device.linkStart`), message by message with a reply to each,
/// until the two stores agree. A message from the watch wakes the iPhone app in the
/// background, so neither needs to be open; the reverse is not so, so the phone only nudges
/// the watch, while its app is open, to start one. It runs when the app opens, after each
/// change made here (the store's heads moving), when the phone comes into reach, when the
/// phone says it changed, and when asked.
///
/// Out of the phone's reach, what changed here is handed on (`LinkParcel`): queued with the
/// system, which delivers it when the two are next in reach, this app closed or not. The phone
/// hands on its own the same way while this app cannot answer.
final class PhoneSync: NSObject, ObservableObject {
    /// How the last attempt went, said in the settings row.
    @Published private(set) var status = "Not synced with the iPhone yet"
    private weak var core: WatchCore?
    private var running = false
    private var again = false
    /// Background tasks held while the phone's parcels are delivered.
    private var waiting: [WKWatchConnectivityRefreshBackgroundTask] = []

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
            LinkParcel.handOn(core.app.device, over: session)
            status = "The iPhone is not in reach; changes go to it when it is"
            return
        }
        do {
            running = true
            send(try core.app.device.linkStart(), over: session)
        } catch {
            finish("Could not sync with the iPhone. \(error.sentence)")
        }
    }

    private func send(_ message: Data, over session: WCSession) {
        session.sendMessageData(message, replyHandler: { reply in
            DispatchQueue.main.async { self.take(reply, over: session) }
        }, errorHandler: { error in
            DispatchQueue.main.async { self.finish("Could not sync with the iPhone. \(error.localizedDescription)") }
        })
    }

    private func take(_ reply: Data, over session: WCSession) {
        guard let core else { return }
        do {
            if let next = try core.app.device.linkTake(reply: reply) {
                send(next, over: session)
                return
            }
            // What the phone brought is not sent back; the app takes it in.
            core.synced()
            if core.app.device.linkTookIn() {
                NotificationCenter.default.post(name: Core.changed, object: nil, userInfo: [Core.fromPhone: true])
            }
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
    func session(
        _ session: WCSession, activationDidCompleteWith activationState: WCSessionActivationState, error: Error?
    ) {
        DispatchQueue.main.async { self.syncNow() }
    }

    func sessionReachabilityDidChange(_ session: WCSession) {
        guard session.isReachable else { return }
        DispatchQueue.main.async { self.syncNow() }
    }

    /// The phone's store changed: what it says is only that, and the watch asks for the rest.
    func session(_ session: WCSession, didReceiveMessage message: [String: Any]) {
        DispatchQueue.main.async { self.syncNow() }
    }

    /// What the phone handed on while this app could not answer, delivered now.
    func session(_ session: WCSession, didReceiveUserInfo userInfo: [String: Any]) {
        guard let parcel = LinkParcel.from(userInfo) else { return }
        DispatchQueue.main.async { self.take(parcel: parcel, over: session) }
    }

    func session(_ session: WCSession, didReceive file: WCSessionFile) {
        guard let parcel = LinkParcel.from(file) else { return }
        DispatchQueue.main.async { self.take(parcel: parcel, over: session) }
    }

    private func take(parcel: Data, over session: WCSession) {
        defer { finishDelivery(session) }
        guard let core, (try? core.app.device.linkUnparcel(parcel: parcel)) == true else { return }
        core.synced()
        NotificationCenter.default.post(name: Core.changed, object: nil, userInfo: [Core.fromPhone: true])
    }

    /// Holds a background task the system started to deliver what the phone handed on, until
    /// nothing more is waiting: completed sooner, the app is suspended mid-delivery.
    func deliver(during task: WKWatchConnectivityRefreshBackgroundTask) {
        waiting.append(task)
        finishDelivery(WCSession.default)
        // Never held for good: the system's budget is short.
        DispatchQueue.main.asyncAfter(deadline: .now() + 20) { self.finishDelivery(WCSession.default, anyway: true) }
    }

    private func finishDelivery(_ session: WCSession, anyway: Bool = false) {
        guard anyway || session.activationState == .activated && !session.hasContentPending else { return }
        let done = waiting
        waiting.removeAll()
        done.forEach { $0.setTaskCompletedWithSnapshot(false) }
    }
}
