import Foundation

/// How a row is spoken, assembled from the components the core sends.
///
/// The core sends components rather than a sentence because speech and braille compose them
/// differently; this is the speech half. VoiceOver puts the label first, then the value, so the
/// title — the thing a person is scanning for — always comes first and is never abbreviated.
enum RowSpeech {
    /// The title, verbatim.
    static func label(_ row: RowView) -> String {
        row.title
    }

    /// Everything after the title: done or not, the due date, notable states, and the level.
    ///
    /// `previousDepth` is the depth of the row spoken before this one. Depth is said only where
    /// it changes: saying "level 2" on every subtask is noise, and indentation, which is
    /// how a sighted reader gets it, says nothing at all.
    static func value(_ row: RowView, previousDepth: UInt32?) -> String {
        var parts: [String] = []
        if row.checked == true {
            parts.append("done")
        }
        if let value = row.value {
            parts.append(value)
        }
        // `ready` is true of almost every task; saying it everywhere buries the states that
        // mean something.
        parts += row.state.filter { $0 != "ready" }
        if row.expanded != nil {
            parts.append("has subtasks")
        }
        if row.depth != (previousDepth ?? 0) {
            parts.append("level \(row.depth + 1)")
        }
        return parts.joined(separator: ", ")
    }
}
