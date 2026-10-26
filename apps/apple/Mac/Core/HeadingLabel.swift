import AppKit

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
