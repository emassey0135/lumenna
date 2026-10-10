import Foundation

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

/// Putting a completion the core offered in place of what it completes.
enum CompletionText {
    /// `text` with `candidate` in place of the bytes from `start` to `end` (the core counts
    /// UTF-8 bytes), and a space after it so the next word can follow.
    static func insert(_ candidate: Candidate, into text: String, start: UInt32, end: UInt32) -> (text: String, cursor: Int) {
        let bytes = Array(text.utf8)
        let from = min(Int(start), bytes.count)
        let to = min(max(Int(end), from), bytes.count)
        let before = String(decoding: bytes[..<from], as: UTF8.self)
        let after = String(decoding: bytes[to...], as: UTF8.self)
        let inserted = candidate.text + (after.hasPrefix(" ") ? "" : " ")
        return (before + inserted + after, (before + inserted).utf16.count)
    }
}

extension Error {
    /// The sentence the core wrote, which is already phrased to be read aloud.
    var sentence: String {
        if let error = self as? LumennaError {
            let message: String
            switch error {
            case let .Failed(text), let .SyncElsewhere(text): message = text
            }
            return message.prefix(1).uppercased() + message.dropFirst()
        }
        return localizedDescription
    }
}
