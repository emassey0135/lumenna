import Foundation

/// What the app says, for a person who reads it: VoiceOver speaks each sentence (`Announcer`),
/// and each platform also shows it, as Windows and GTK show it on their status line — a status
/// line at the foot of the Mac's window, a passing banner on iOS and the watch.
enum Said {
    /// Posted on the main thread with each sentence said, under `text`.
    static let posted = Notification.Name("LumennaSaid")

    /// Shows `text` wherever this platform shows what is said.
    static func post(_ text: String) {
        guard !text.isEmpty else { return }
        let post = { NotificationCenter.default.post(name: posted, object: nil, userInfo: ["text": text]) }
        if Thread.isMainThread { post() } else { DispatchQueue.main.async(execute: post) }
    }

    /// The sentence a notification carries.
    static func text(of note: Notification) -> String? {
        note.userInfo?["text"] as? String
    }

    /// How long a passing banner stays: long enough to read the sentence, never more than ten
    /// seconds.
    static func seconds(for text: String) -> TimeInterval {
        min(10, 3 + Double(text.count) / 15)
    }
}
