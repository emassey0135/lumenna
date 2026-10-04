import Foundation

/// How a name is written after a sigil in the filter and quick-add languages: quoted when it
/// has a space in it (§6.2).
func sigil(_ mark: Character, _ name: String) -> String {
    name.contains(" ") ? "\(mark)\"\(name)\"" : "\(mark)\(name)"
}

/// A sitting's status with its planned length beside it: "planned for 45 minutes" before it
/// starts, "45 minutes planned" after, so the two never read as "planned, planned" — as the
/// command line and the BTSpeak app say it.
func sittingStatus(_ sitting: PlanAssignment) -> [String] {
    guard let planned = sitting.plannedMins else { return [sitting.status] }
    if sitting.status == "planned" {
        return ["planned for \(Clock.length(planned))"]
    }
    return [sitting.status, "\(Clock.length(planned)) planned"]
}

/// The core counts text in UTF-8 bytes, as Rust strings do; Apple's text APIs count UTF-16.
enum TextOffsets {
    /// A UTF-16 offset as the byte offset the core wants.
    static func bytes(_ utf16: Int, in text: String) -> UInt32 {
        let index = String.Index(utf16Offset: max(0, min(utf16, text.utf16.count)), in: text)
        return UInt32(text[..<index].utf8.count)
    }

    /// A byte offset from the core as a UTF-16 offset, never splitting a character.
    static func utf16(_ bytes: UInt32, in text: String) -> Int {
        let prefix = Array(text.utf8.prefix(Int(bytes)))
        return String(decoding: prefix, as: UTF8.self).utf16.count
    }
}
