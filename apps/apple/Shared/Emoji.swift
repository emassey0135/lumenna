import LumennaApp
import SwiftUI

/// The one emoji a task, a block or a project wears, drawn before its name as other planners
/// draw an icon, and never said: no line of the core's holds it, so a row's VoiceOver label,
/// the core's line, never has it, and the mark is hidden from VoiceOver. It is text, so it grows
/// with the text beside it; nothing is drawn for none. The person picks it with the platform's
/// own picker (Control-Command-Space on a Mac, the keyboard's on a phone), never from a set of
/// ours.
struct EmojiMark: View {
    let emoji: String?

    var body: some View {
        if let emoji = Self.shown(emoji) {
            Text(verbatim: emoji)
                .accessibilityHidden(true)
        }
    }

    /// What is drawn: the emoji, or nothing for none (the core sends only a whole emoji, or none).
    static func shown(_ emoji: String?) -> String? {
        guard let emoji, !emoji.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else { return nil }
        return emoji
    }

    /// `text` with the emoji before it, for a place that draws one text alone (a complication's
    /// line, a widget's label), and only where VoiceOver is given the core's words instead.
    static func before(_ text: String, _ emoji: String?) -> String {
        shown(emoji).map { "\($0) \(text)" } ?? text
    }
}

extension LumennaApp.DayItem {
    /// The emoji drawn before a row of the day: a sitting's task's, a block's own; none for free
    /// time, now or a cancelled day. A sitting carries its block's record too, whose emoji is
    /// not the sitting's.
    var emoji: String? {
        switch kind {
        case .sitting: sitting?.emoji
        case .block: block?.emoji
        default: nil
        }
    }
}
