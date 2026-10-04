import UIKit

/// Saying what happened (§13: announce state changes explicitly).
enum Announcer {
    /// Speaks a sentence after whatever VoiceOver is saying now, rather than cutting it off —
    /// so it follows the element focus has just moved to, instead of being lost under it.
    static func say(_ text: String) {
        guard !text.isEmpty else { return }
        let queued = NSAttributedString(
            string: text,
            attributes: [.accessibilitySpeechQueueAnnouncement: true]
        )
        UIAccessibility.post(notification: .announcement, argument: queued)
    }

    /// Says a result's announcement, then each notice.
    static func say(_ announcement: String, notices: [String]) {
        say(([announcement] + notices).joined(separator: ". "))
    }
}
