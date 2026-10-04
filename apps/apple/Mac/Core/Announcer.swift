import AppKit

/// Saying what happened (§13: announce state changes explicitly).
enum Announcer {
    /// Speaks a sentence after whatever VoiceOver is saying now, rather than cutting it off —
    /// so it follows the element focus has just moved to, instead of being lost under it.
    static func say(_ text: String) {
        guard !text.isEmpty else { return }
        let element = NSApp.keyWindow ?? NSApp.mainWindow ?? NSApp as Any
        NSAccessibility.post(
            element: element,
            notification: .announcementRequested,
            userInfo: [
                .announcement: text,
                .priority: NSAccessibilityPriorityLevel.medium.rawValue,
            ]
        )
    }

    /// Says a result's announcement, then each notice.
    static func say(_ announcement: String, notices: [String]) {
        say(([announcement] + notices).filter { !$0.isEmpty }.joined(separator: ". "))
    }
}

/// A line of text that VoiceOver's heading commands land on, read by its words.
///
/// A label exposes its text as its value, and VoiceOver reads a heading by its title, so a
/// label given the heading role was announced as "heading" and nothing more. This names
/// itself with its own text.
final class HeadingLabel: NSTextField {
    override func accessibilityRole() -> NSAccessibility.Role? {
        NSAccessibility.Role(rawValue: "AXHeading")
    }

    override func accessibilityLabel() -> String? {
        stringValue
    }

    /// No value as well: the words are the name, said once.
    override func accessibilityValue() -> String? {
        nil
    }
}
