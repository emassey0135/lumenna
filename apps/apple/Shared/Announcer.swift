import Foundation
#if os(macOS)
import AppKit
#else
import SwiftUI
#endif

/// Saying what happened: every change of state is announced, not left to be noticed. One for
/// every Apple app, so none says an empty part or recases what the core worded.
enum Announcer {
    /// Speaks a sentence after whatever VoiceOver is saying now, rather than cutting it off —
    /// so it follows the element focus has just moved to, instead of being lost under it — and
    /// shows it (`Said`) for whoever reads rather than listens.
    static func say(_ text: String) {
        guard !text.isEmpty else { return }
        Said.post(text)
        #if os(macOS)
        let element = NSApp.keyWindow ?? NSApp.mainWindow ?? NSApp as Any
        NSAccessibility.post(
            element: element,
            notification: .announcementRequested,
            userInfo: [
                .announcement: text,
                .priority: NSAccessibilityPriorityLevel.medium.rawValue,
            ]
        )
        #else
        let queued = NSAttributedString(string: text, attributes: [.accessibilitySpeechQueueAnnouncement: true])
        AccessibilityNotification.Announcement(queued).post()
        #endif
    }

    /// Says a result's announcement, then each notice, leaving out what is empty.
    static func say(_ announcement: String, notices: [String]) {
        say(([announcement] + notices).filter { !$0.isEmpty }.joined(separator: ". "))
    }
}
