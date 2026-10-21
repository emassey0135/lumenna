#if canImport(WatchConnectivity)
import Foundation
import LumennaApp
import WatchConnectivity

/// Changes handed on between the watch and its phone when the other cannot answer: what came
/// after the two last agreed (`Device.linkParcel`), queued with the system, which delivers it
/// when the two are next in reach, the other app closed or not. A newer parcel holds all an
/// older one did, so one still waiting is cancelled. Small ones go as user info, larger as a
/// file, which is what the system takes them in.
enum LinkParcel {
    /// Larger than this goes as a file: user info is meant for a small dictionary.
    private static let small = 48 * 1024
    private static let key = "lumenna-parcel"

    /// Queues what the other side may lack, if anything.
    static func handOn(_ device: Device, over session: WCSession) {
        guard session.activationState == .activated, let parcel = try? device.linkParcel() else { return }
        for waiting in session.outstandingUserInfoTransfers where waiting.userInfo[key] != nil {
            waiting.cancel()
        }
        for waiting in session.outstandingFileTransfers where waiting.file.metadata?[key] != nil {
            waiting.cancel()
        }
        if parcel.count <= small {
            session.transferUserInfo([key: parcel])
            return
        }
        let file = FileManager.default.temporaryDirectory.appendingPathComponent("lumenna-\(UUID().uuidString).parcel")
        guard (try? parcel.write(to: file)) != nil else { return }
        session.transferFile(file, metadata: [key: true])
    }

    /// The parcel in delivered user info, if it is one.
    static func from(_ userInfo: [String: Any]) -> Data? {
        userInfo[key] as? Data
    }

    /// The parcel in a delivered file, if it is one: read at once, since the system removes the
    /// file when the delegate's method returns.
    static func from(_ file: WCSessionFile) -> Data? {
        guard file.metadata?[key] != nil else { return nil }
        return try? Data(contentsOf: file.fileURL)
    }
}
#endif
